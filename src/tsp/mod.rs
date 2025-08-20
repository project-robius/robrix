use std::{borrow::Cow, ops::Deref, path::Path, sync::{Mutex, OnceLock}};

use anyhow::anyhow;
use makepad_widgets::{makepad_micro_serde::*, *};
use quinn::rustls::crypto::{CryptoProvider, aws_lc_rs};
use tokio::{runtime::Handle, sync::mpsc::{UnboundedReceiver, UnboundedSender}};
use tsp_sdk::{vid::{verify_vid, VidError}, AskarSecureStorage, AsyncSecureStore, OwnedVid, SecureStorage, VerifiedVid, Vid};
use url::Url;

use crate::{persistence::{self, tsp_wallets_dir, SavedTspState}, shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind}};


pub mod create_did_modal;
pub mod create_wallet_modal;
pub mod tsp_settings_screen;
pub mod wallet_entry;

pub fn live_design(cx: &mut Cx) {
    create_did_modal::live_design(cx);
    create_wallet_modal::live_design(cx);
    wallet_entry::live_design(cx);
    tsp_settings_screen::live_design(cx);
}

/// The sender used by [`submit_tsp_request()`] to send TSP requests to the async worker thread.
/// Currently there is only one, but it can be cloned if we need more concurrent senders.
static TSP_REQUEST_SENDER: OnceLock<UnboundedSender<TspRequest>> = OnceLock::new();

/// Submits a TSP request to the worker thread to be executed asynchronously.
pub fn submit_tsp_request(req: TspRequest) -> anyhow::Result<()> {
    TSP_REQUEST_SENDER.get()
        .ok_or(anyhow!("TSP request sender was not initialized."))?
        .send(req)
        .map_err(|_| anyhow!("TSP async worker task receiver has died!"))
}


/// The global singleton TSP state, storing all known TSP wallets.
static TSP_STATE: Mutex<TspState> = Mutex::new(TspState::new());
pub fn tsp_state_ref() -> &'static Mutex<TspState> {
    &TSP_STATE
}

/// The current actively-used (singleton) state of TSP wallets known to Robrix.
#[derive(Default, Debug)]
pub struct TspState {
    /// The current active (default) TSP wallet, if any.
    pub current_wallet: Option<OpenedTspWallet>,
    /// All other TSP wallets that have been created or imported.
    pub other_wallets: Vec<TspWalletEntry>,
}
impl TspState {
    const fn new() -> Self {
        Self {
            current_wallet: None,
            other_wallets: Vec::new(),
        }
    }

    /// Returns true if this TSP state has any content, i.e. at least one wallet.
    pub fn has_content(&self) -> bool {
        self.current_wallet.is_some()
            || !self.other_wallets.is_empty()
    }

    /// Opens all wallets in the given saved TSP state in order to populate this `TspState`.
    pub async fn deserialize_and_open_default_wallet_from(
        &mut self,
        saved_state: SavedTspState,
    ) -> Result<(), tsp_sdk::Error> {
        for (idx, wallet_metadata) in saved_state.wallets.into_iter().enumerate() {
            if let Ok(opened_wallet) = wallet_metadata.open_wallet().await {
                if saved_state.default_wallet == Some(idx) {
                    self.current_wallet = Some(opened_wallet);
                } else {
                    self.other_wallets.push(TspWalletEntry::Opened(opened_wallet));
                }
            } else {
                self.other_wallets.push(TspWalletEntry::NotFound(wallet_metadata));
            }
        }
        match self.current_wallet.is_some() as usize + self.other_wallets.len() {
            0 => log!("Restored no TSP wallets from saved TSP state."),
            1 => log!("Restored 1 TSP wallet from saved state."),
            n => log!("Restored {n} TSP wallets from saved state."),
        }
        if saved_state.default_wallet.is_some() && self.current_wallet.is_none() {
            warning!("Saved TSP state had a default wallet, but it wasn't opened successfully.");
        }
        Ok(())
    }

    /// Closes all opened wallets, serializing their state to persistent storage.
    ///
    /// Returns the serialized metadata that represents all persisted wallets,
    /// which should be saved to persistent app data storage.
    pub async fn close_and_serialize(self) -> Result<SavedTspState, tsp_sdk::Error> {
        let mut default_wallet = None;
        let mut wallets = Vec::<TspWalletMetadata>::with_capacity(
            self.current_wallet.is_some() as usize + self.other_wallets.len()
        );

        if let Some(current_wallet) = self.current_wallet {
            let metadata = current_wallet.metadata.clone();
            default_wallet = Some(0);
            current_wallet.persist_and_close().await?;
            wallets.push(metadata);
        }
        for entry in self.other_wallets {
            let metadata = entry.metadata().clone();
            if let TspWalletEntry::Opened(opened) = entry {
                opened.persist_and_close().await?;
            }
            wallets.push(metadata);
        }
        Ok(SavedTspState {
            wallets,
            default_wallet,
        })
    }
}


/// A TSP wallet entry known to Robrix. Derefs to `TspWalletMetadata`.
#[derive(Debug)]
pub enum TspWalletEntry {
    /// A wallet that currently exists and has been opened successfully.
    Opened(OpenedTspWallet),
    /// A wallet that previously existed but wasn't found.
    NotFound(TspWalletMetadata),
}
impl Deref for TspWalletEntry {
    type Target = TspWalletMetadata;
    fn deref(&self) -> &Self::Target {
        match self {
            TspWalletEntry::Opened(opened) => &opened.metadata,
            TspWalletEntry::NotFound(metadata) => metadata,
        }
    }
}
impl TspWalletEntry {
    pub fn metadata(&self) -> &TspWalletMetadata {
        match self {
            TspWalletEntry::Opened(opened) => &opened.metadata,
            TspWalletEntry::NotFound(metadata) => metadata,
        }
    }
}


/// A TSP wallet that exists and is currently opened / ready to use.
pub struct OpenedTspWallet {
    pub vault: AskarSecureStorage,
    pub db: AsyncSecureStore,
    pub metadata: TspWalletMetadata,
}
impl Deref for OpenedTspWallet {
    type Target = TspWalletMetadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}
impl std::fmt::Debug for OpenedTspWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenedTspWallet")
            .field("wallet_name", &self.wallet_name)
            .field("url", &self.url)
            .finish()
    }
}
impl OpenedTspWallet {
    /// Saves this opened wallet to persistent storage without closing it.
    pub async fn persist(&self) -> Result<(), tsp_sdk::Error> {
        self.vault.persist(self.db.export()?).await
    }

    /// Saves this opened wallet to persistent storage and closes it.
    pub async fn persist_and_close(self) -> Result<(), tsp_sdk::Error> {
        self.persist().await?;
        self.vault.close().await
    }
}

/// Metadata about a TSP wallet that has been created or imported.
///
/// When comparing two `TspWalletMetadata` instances,
/// they are considered equal if their `url` is the same
/// regardless of their `wallet_name` or `password`.
///
/// The URL is *NOT* percent-encoded yet, but it should be before
/// being passed to `AskarSecureStorage` methods like `new()` and `open()`.
#[derive(Clone, Default, DeRon, SerRon)]
pub struct TspWalletMetadata {
    /// The human-readable, user-defined name of the wallet.
    pub wallet_name: String,
    /// The URL of the wallet, which is NOT percent-encoded yet.
    pub url: TspWalletSqliteUrl,
    pub password: String,
}
impl std::fmt::Debug for TspWalletMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TspWalletMetadata")
            .field("wallet_name", &self.wallet_name)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}
impl PartialEq for TspWalletMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}
impl TspWalletMetadata {
    /// Attempts to open the wallet described by this metadata.
    pub async fn open_wallet(&self) -> Result<OpenedTspWallet, tsp_sdk::Error> {
        let encoded_url = self.url.to_url_encoded();
        log!("Opening TSP wallet at URL: {encoded_url}");
        let vault = AskarSecureStorage::open(&encoded_url, self.password.as_bytes()).await?;
        let (vids, aliases, keys) = vault.read().await?;
        let db = AsyncSecureStore::new();
        db.import(vids, aliases, keys)?;
        Ok(OpenedTspWallet {
            vault,
            db,
            metadata: self.clone(),
        })
    }
}


pub fn tsp_init(rt_handle: tokio::runtime::Handle) -> anyhow::Result<()> {
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .map_err(|_| anyhow!("BUG: default CryptoProvider was already set."))?;

    // Create a channel to be used between UI thread(s) and the TSP async worker thread.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<TspRequest>();
    TSP_REQUEST_SENDER.set(sender).expect("BUG: TSP_REQUEST_SENDER already set!");

    // Start a high-level async task that will start and monitor all other tasks.
    let _monitor = rt_handle.spawn(async move {
        // First, run the inner TSP initialization logic to load prior TSP state.
        match inner_tsp_init().await {
            Ok(()) => log!("TSP state initialized successfully."),
            Err(e) => {
                error!("Failed to initialize TSP state: {e:?}");
                enqueue_popup_notification(PopupItem {
                    message: format!(
                        "Failed to initialize TSP state.\
                        Your TSP wallets may not be fully available. Error: {e}",
                    ),
                    auto_dismissal_duration: None,
                    kind: PopupKind::Error,
                });
            }
        }

        // Spawn the actual async worker thread.
        let mut tsp_worker_join_handle = Handle::current().spawn(async_tsp_worker(receiver));

        // TODO: start the main loop that drives the TSP SDK's receiver handler,
        //       e.g., to process incoming requests from other TSP instances.
        // let mut main_loop_join_handle = rt2.spawn(async_main_loop(...));

        #[allow(clippy::never_loop)] // unsure if needed, just following tokio's examples.
        loop {
            tokio::select! {
                // result = &mut main_loop_join_handle => {
                //     match result {
                //         Ok(Ok(())) => {
                //             error!("BUG: main async loop task ended unexpectedly!");
                //         }
                //         Ok(Err(e)) => {
                //             error!("Error: main async loop task ended:\n\t{e:?}");
                //             enqueue_popup_notification(PopupItem { message: format!("Rooms list update error: {e}"), auto_dismissal_duration: None });
                //         },
                //         Err(e) => {
                //             error!("BUG: failed to join main async loop task: {e:?}");
                //         }
                //     }
                //     break;
                // }
                result = &mut tsp_worker_join_handle => {
                    match result {
                        Ok(Ok(())) => {
                            error!("BUG: async_tsp_worker task ended unexpectedly!");
                        }
                        Ok(Err(e)) => {
                            error!("Error: async TSP worker task ended:\n\t{e:?}");
                            enqueue_popup_notification(PopupItem {
                                message: format!("TSP background worker error: {e}"),
                                auto_dismissal_duration: None,
                                kind: PopupKind::Error
                            });
                        },
                        Err(e) => {
                            error!("BUG: failed to join async_tsp_worker task: {e:?}");
                        }
                    }
                    break;
                }
            }
        }
    });

    Ok(())
}


async fn inner_tsp_init() -> anyhow::Result<()> {
    // Load the TSP state from persistent storage.
    let saved_tsp_state = persistence::load_tsp_state().await?;
    let mut new_tsp_state = TspState::new();
    new_tsp_state.deserialize_and_open_default_wallet_from(saved_tsp_state).await?;
    if new_tsp_state.has_content() && new_tsp_state.current_wallet.is_none() {
        enqueue_popup_notification(PopupItem {
            message: String::from("TSP wallet(s) were loaded successfully, but no default wallet was set.\n\n\
                TSP wallet-related features will not work properly until you set a default wallet."),
            auto_dismissal_duration: None,
            kind: PopupKind::Warning,
        });
    }
    *TSP_STATE.lock().unwrap() = new_tsp_state;
    Ok(())
}


/// Actions related to TSP wallets.
#[derive(Debug)]
pub enum TspWalletAction {
    /// A wallet was created successfully.
    CreateWalletSuccess {
        metadata: TspWalletMetadata,
        is_default: bool,
    },
    /// Failed to create a wallet.
    CreateWalletError {
        metadata: TspWalletMetadata,
        error: anyhow::Error,
    },
    /// A wallet was removed from the list.
    WalletRemoved {
        metadata: TspWalletMetadata,
        was_default: bool,
    },
    /// The default wallet was successfully or unsuccessfully changed.
    DefaultWalletChanged(Result<TspWalletMetadata, ()>),
    /// The given wallet was successfully or unsuccessfully opened.
    WalletOpened(Result<TspWalletMetadata, tsp_sdk::Error>),
    /// A new identity (DID) was successfully created or had an error.
    ///
    /// If successful, the result contains the created DID string.
    DidCreationResult(Result<String, anyhow::Error>),
}

/// Requests that can be sent to the TSP async worker thread.
pub enum TspRequest {
    /// Request to create a new TSP wallet.
    CreateWallet {
        metadata: TspWalletMetadata,
    },
    /// Request to open an existing TSP wallet.
    ///
    /// This does not modify the current active/default wallet.
    /// If the wallet exists in the list of other wallets, it will be opened in-place,
    /// otherwise it will be opened and added to the end of the other wallets list.
    OpenWallet {
        metadata: TspWalletMetadata,
    },
    /// Request to set an existing open wallet as the default.
    SetDefaultWallet(TspWalletMetadata),
    /// Request to remove a TSP wallet from the list without deleting it.
    RemoveWallet(TspWalletMetadata),
    /// Request to permanently delete a TSP wallet.
    DeleteWallet(TspWalletMetadata),
    /// Requeqst to create a new identity (DID) on the given server
    /// and store it in the default TSP wallet.
    CreateDid {
        username: String,
        alias: Option<String>,
        server: String,
        did_server: String,
    },
}

/// The entry point for an async worker thread that processes TSP-related async tasks.
///
/// All this task does is wait for [`TspRequests`] from other threads
/// and then executes them within an async runtime context.
async fn async_tsp_worker(
    mut request_receiver: UnboundedReceiver<TspRequest>,
) -> anyhow::Result<()> {
    log!("Started async_tsp_worker task.");

    // Lazily initialize the reqwest client.
    let mut __reqwest_client = None;
    let mut get_reqwest_client = || {
        __reqwest_client.get_or_insert_with(|| {
            reqwest::ClientBuilder::new()
                .user_agent(format!("Robrix v{}", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap()
        }).clone()
    };

    while let Some(req) = request_receiver.recv().await { match req {
        TspRequest::CreateWallet { metadata } => {
            log!("Received TspRequest::CreateWallet({metadata:?})");
            Handle::current().spawn(async move {
                if let Some(sqlite_path) = metadata.url.get_path() {
                    if let Ok(true) = tokio::fs::try_exists(sqlite_path).await {
                        error!("Wallet already exists at path: {}", sqlite_path.display());
                        Cx::post_action(TspWalletAction::CreateWalletError {
                            metadata: metadata.clone(),
                            error: anyhow!("Wallet already exists at path: {}", sqlite_path.display()),
                        });
                        return;
                    }
                    if let Some(parent_dir) = sqlite_path.parent() {
                        log!("Ensuring that new wallet's parent dir exists: {}", parent_dir.display());
                        if let Err(e) = tokio::fs::create_dir_all(parent_dir).await {
                            error!("Failed to create directory to hold new wallet: {e:?}");
                            Cx::post_action(TspWalletAction::CreateWalletError {
                                metadata: metadata.clone(),
                                error: anyhow!("Failed to create directory for new wallet: {}, error: {}", parent_dir.display(), e),
                            });
                            return;
                        }
                    }
                }
                let encoded_url = metadata.url.to_url_encoded();
                log!("Attempting to create new wallet at:\n   Reg: {}\n   Enc: {}", metadata.url, encoded_url);
                match AskarSecureStorage::new(&encoded_url, metadata.password.as_bytes()).await {
                    Ok(vault) => {
                        log!("Successfully created new wallet: {metadata:?}");
                        let db = AsyncSecureStore::new();
                        let mut tsp_state = tsp_state_ref().lock().unwrap();
                        let opened_wallet = OpenedTspWallet {
                            vault,
                            db,
                            metadata: metadata.clone(),
                        };
                        let is_default: bool;
                        if tsp_state.current_wallet.is_none() {
                            tsp_state.current_wallet = Some(opened_wallet);
                            is_default = true;
                        } else {
                            tsp_state.other_wallets.push(TspWalletEntry::Opened(opened_wallet));
                            is_default = false;
                        }
                        Cx::post_action(
                            TspWalletAction::CreateWalletSuccess {
                                metadata,
                                is_default,
                            }
                        );
                    }
                    Err(error) => {
                        error!("Failed to create new wallet: {error:?}");
                        Cx::post_action(
                            TspWalletAction::CreateWalletError {
                                metadata: metadata.clone(),
                                error: error.into(),
                            }
                        );
                    }
                }
            });
        }

        TspRequest::SetDefaultWallet(metadata) => {
            log!("Received TspRequest::SetDefaultWallet({metadata:?})");
            match tsp_state_ref().lock().unwrap().current_wallet.as_ref() {
                Some(cw) if cw.metadata == metadata => {
                    log!("Wallet was already set as default: {metadata:?}");
                    continue;
                }
                _ => {}
            }

            // If the new default wallet exists and is already opened, set it as default.
            Handle::current().spawn(async move {
                let mut result = Err(());
                let mut tsp_state = tsp_state_ref().lock().unwrap();
                if let Some(TspWalletEntry::Opened(opened)) = tsp_state.other_wallets.iter()
                    .position(|w| match w {
                        TspWalletEntry::Opened(opened) => opened.metadata == metadata,
                        _ => false,
                    })
                    .map(|idx| tsp_state.other_wallets.remove(idx))
                {
                    let prev_opt = tsp_state.current_wallet.replace(opened);
                    if let Some(previous_active) = prev_opt {
                        tsp_state.other_wallets.insert(0, TspWalletEntry::Opened(previous_active));
                    }
                    result = Ok(metadata);
                }
                Cx::post_action(TspWalletAction::DefaultWalletChanged(result));
            });
        }

        TspRequest::OpenWallet { metadata } => {
            log!("Received TspRequest::OpenWallet({metadata:?})");
            Handle::current().spawn(async move {
                let result = match metadata.open_wallet().await {
                    Ok(opened_wallet) => {
                        log!("Successfully opened wallet: {metadata:?}");
                        let mut tsp_state = tsp_state_ref().lock().unwrap();
                        // If the newly-opened wallet exists in the other wallets list,
                        // convert it into an opened wallet in-place.
                        // Otherwise, add it to the end of the other wallet list
                        if let Some(w) = tsp_state.other_wallets.iter_mut().find(|w| w.metadata() == &metadata) {
                            *w = TspWalletEntry::Opened(opened_wallet);
                        } else {
                            tsp_state.other_wallets.push(TspWalletEntry::Opened(opened_wallet));
                        }
                        Ok(metadata)
                    }
                    Err(error) => {
                        error!("Error opening wallet {metadata:?}: {error:?}");
                        Err(error)
                    }
                };
                Cx::post_action(TspWalletAction::WalletOpened(result));
            });
        }

        TspRequest::RemoveWallet(metadata) => {
            log!("Received TspRequest::RemoveWallet({metadata:?})");
            Handle::current().spawn(async move {
                let mut tsp_state = tsp_state_ref().lock().unwrap();
                let was_default = if tsp_state.current_wallet.as_ref().is_some_and(|cw| cw.metadata == metadata) {
                    tsp_state.current_wallet = None;
                    true
                }
                else if let Some(i) = tsp_state.other_wallets.iter().position(|w| w.metadata() == &metadata) {
                    tsp_state.other_wallets.remove(i);
                    false
                } else {
                    error!("BUG: failed to remove wallet not found in TSP state: {metadata:?}");
                    return;
                };
                Cx::post_action(TspWalletAction::WalletRemoved { metadata, was_default });
            });
        }

        TspRequest::DeleteWallet(metadata) => {
            log!("Received TspRequest::DeleteWallet({metadata:?})");
            todo!("handle deleting a wallet");
        }

        TspRequest::CreateDid { username, alias, server, did_server } => {
            log!("Received TspRequest::CreateDid(username: {username}, alias: {alias:?}, server: {server}, did_server: {did_server})");
            let client = get_reqwest_client();

            Handle::current().spawn(async move {
                let result = if tsp_state_ref().lock().unwrap().current_wallet.is_none() {
                    Err(anyhow!("Please choose a default TSP wallet to hold the DID."))
                } else {
                    match create_did_web(&did_server, &server, &username, &client).await {
                        Ok((did, private_vid, metadata)) => {
                            log!("Successfully created & published new DID: {did}.\n\
                                Adding private VID to current wallet: {private_vid:?}",
                            );
                            store_did_in_wallet(private_vid, metadata, alias, did).await
                        }
                        Err(e) => {
                            error!("Failed to create new DID: {e}");
                            Err(e)
                        }
                    }
                };
                Cx::post_action(TspWalletAction::DidCreationResult(result));
            });
        }
    } }

    error!("async_tsp_worker task ended unexpectedly");
    anyhow::bail!("async_tsp_worker task ended unexpectedly")
}


/// Creates a new DID on the given `did_server` and publishes it to the TSP `server`.
///
/// This function does not modify or add anything to the current TSP wallet.
/// The caller must do that separately.
///
/// Returns a tuple of the DID string, the private VID, and optional metadata.
async fn create_did_web(
    did_server: &str,
    server: &str,
    username: &str,
    client: &reqwest::Client,
) -> Result<(String, OwnedVid, Option<serde_json::Value>), anyhow::Error> {
    // The following code is based on the TSP SDK's CLI example for creating a DID.
    let did = format!(
        "did:web:{}:endpoint:{username}",
        did_server.replace(":", "%3A").replace("/", ":")
    );

    let transport = Url::parse(
        &format!("https://{}/endpoint/{}",
            server,
            &did.replace("%", "%25")
        )
    ).map_err(|e| anyhow!("Invalid transport URL: {e}"))?;

    let private_vid = OwnedVid::bind(&did, transport);
    log!("created identity {}", private_vid.identifier());

    let response = client
        .post(format!("https://{did_server}/add-vid"))
        .json(&private_vid.vid())
        .send()
        .await
        .inspect(|r| log!("DID server responded with status code {}", r.status()))
        .map_err(|e| anyhow!("Could not publish VID. The DID server responded with error: {e}"))?;

    let vid_result: Result<Vid, anyhow::Error> = match response.status() {
        r if r.is_success() => {
            response.json().await
                .map_err(|e| anyhow!("Could not decode response from DID server as a valid VID: {e}"))
        }
        r => {
            let text = response.text().await.unwrap_or_else(|_| "[Unknown]".to_string());
            if r.as_u16() == 500 {
                return Err(anyhow!(
                    "The DID server returned error code 500. The DID username may already exist, \
                     or the server had another problem.\n\nResponse: \"{text}\""
                ));
            } else {
                return Err(anyhow!(
                    "The DID server returned error code {}.\n\nResponse: \"{text}\"",
                    r.as_u16()
                ));
            }
        }
    };

    let _vid = vid_result?;

    log!("published DID document at {}",
        tsp_sdk::vid::did::get_resolve_url(&did)?.to_string()
    );

    let (_vid, metadata) = verify_vid(private_vid.identifier())
        .await
        .map_err(|err| tsp_sdk::Error::Vid(VidError::InvalidVid(err.to_string())))?;

    Ok((did, private_vid, metadata))
}


/// Stores the given private VID in the current default TSP wallet,
/// and optionally establishes an alias for the given `did`.
///
/// Returns the DID string if successful, otherwise an error.
async fn store_did_in_wallet(
    private_vid: OwnedVid,
    metadata: Option<serde_json::Value>,
    alias: Option<String>,
    did: String,
) -> Result<String, anyhow::Error> {
    let tsp_state = tsp_state_ref().lock().unwrap();
    let Some(current_wallet) = tsp_state.current_wallet.as_ref() else {
        anyhow::bail!("Please select a default TSP wallet to hold the DID.");
    };
    current_wallet.db.add_private_vid(private_vid, metadata)?;
    if let Some(alias) = alias {
        current_wallet.db.set_alias(alias.clone(), did.clone())?;
        log!("added alias {alias} -> {did}");
    }
    Ok(did)
}


/// Sanitizes a wallet name to ensure it is safe to use in file paths.
pub fn sanitize_wallet_name(name: &str) -> String {
    sanitize_filename::sanitize(name)
        .replace(char::is_whitespace, "_")
}


/// Represents a SQLite URL for a TSP wallet, which is *NOT* percent-encoded yet.
///
/// Currently the scheme is always "sqlite://" (or "sqlite:///" for absolute paths),
/// and the path is the full file path to the local SQLite database file for the wallet.
/// We haven't tested it with remote URLs yet.
#[derive(Clone, Debug, Default, DeRon, SerRon, Eq)]
pub struct TspWalletSqliteUrl(String);
impl std::fmt::Display for TspWalletSqliteUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl PartialEq for TspWalletSqliteUrl {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl TspWalletSqliteUrl {
    /// Returns a SQLite scheme and full file path for a wallet with the given `name`.
    pub fn from_wallet_file_name(name: &str) -> Self {
        let parent = tsp_wallets_dir();
        let path = parent.join(sanitize_wallet_name(name));
        let scheme = if path.is_absolute() {
            "sqlite:///"
        } else {
            "sqlite://"
        };
        Self(format!("{}{}.sqlite", scheme, path.display()))
    }

    /// Returns the part of the URL after the "scheme://", which may be a local file path.
    pub fn get_path(&self) -> Option<&Path> {
        let url = &self.0;
        // Handle URLs with a scheme for absolute paths, e.g., "sqlite:///"
        if let Some(p) = url.find(":///").and_then(|pos| url.get(pos + 4 ..)) {
            Some(Path::new(p))
        }
        // Handle URLs with a scheme for relative paths, e.g., "sqlite://"
        else if let Some(p) = url.find("://").and_then(|pos| url.get(pos + 3 ..)) {
            Some(Path::new(p))
        }
        else { None }
    }

    /// Returns the URL as a string that is not percent-encoded.
    ///
    /// This is suitable for display purposes, but should not be passed into
    /// `AskarSecureStorage` methods like `new()` and `open()`.
    /// For those, use [`Self::to_url_encoded()`] instead.
    pub fn as_url_unencoded(&self) -> &str {
        &self.0
    }


    /// Converts this wallet URL to a percent-encoded URL.
    ///
    /// Note: this URL is suitable for use in `AskarSecureStorage` methods
    /// like `new()` and `open()`.
    pub fn to_url_encoded(&self) -> Cow<'_, str> {
        const DELIMITER_ABS: &str = ":///";
        const DELIMITER_REG: &str = "://";
        let try_encode = |delim: &str| -> Option<Cow<str>> {
            if let Some(idx) = self.0.find(delim) {
                let before = self.0.get(.. (idx + delim.len())).unwrap_or("");
                let after  = self.0.get((idx + delim.len()) ..).unwrap_or("");
                Some(format!("{}{}",
                    before,
                    percent_encoding::utf8_percent_encode(
                        after,
                        percent_encoding::NON_ALPHANUMERIC,
                    )
                ).into())
            } else {
                None
            }
        };

        try_encode(DELIMITER_ABS)
            .or_else(|| try_encode(DELIMITER_REG))
            .unwrap_or_else(|| {
                percent_encoding::utf8_percent_encode(
                    &self.0,
                    percent_encoding::NON_ALPHANUMERIC,
                ).into()
            })
    }
}
