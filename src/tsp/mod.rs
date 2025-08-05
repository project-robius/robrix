use std::{ops::Deref, sync::{Arc, Mutex, OnceLock}};

use anyhow::anyhow;
use makepad_widgets::{makepad_micro_serde::*, *};
use quinn::rustls::crypto::{CryptoProvider, aws_lc_rs};
use tokio::{runtime::Handle, sync::mpsc::{UnboundedReceiver, UnboundedSender}};
use tsp_sdk::{AskarSecureStorage, AsyncSecureStore, SecureStorage};

use crate::{persistence, shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind}};


pub mod create_wallet_modal;
pub mod tsp_settings_screen;
pub mod wallet_entry;

pub fn live_design(cx: &mut Cx) {
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
        if saved_state.default_wallet.is_some() && self.current_wallet.is_none() {
            error!("BUG: saved TSP state had a default wallet, but it wasn't opened successfully.");
        }
        match self.current_wallet.is_some() as usize + self.other_wallets.len() {
            0 => log!("Restored no TSP wallets from saved TSP state."),
            1 => log!("Restored 1 TSP wallet from saved state."),
            n => log!("Restored {n} TSP wallets from saved state."),
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
            .field("path", &self.path)
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
/// they are considered equal if their `path` is the same
/// regardless of their `wallet_name` or `password`.
#[derive(Clone, Default, DeRon, SerRon)]
pub struct TspWalletMetadata {
    pub wallet_name: String,
    pub path: String,
    pub password: String,
}
impl std::fmt::Debug for TspWalletMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TspWalletMetadata")
            .field("wallet_name", &self.wallet_name)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
impl PartialEq for TspWalletMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}
impl TspWalletMetadata {
    /// Attempts to open the wallet described by this metadata.
    pub async fn open_wallet(&self) -> Result<OpenedTspWallet, tsp_sdk::Error> {
        let vault = AskarSecureStorage::open(&self.path, self.password.as_bytes()).await?;
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


/// The TSP state that is saved to persistent storage.
///
/// It contains metadata about all wallets that have been created or imported.
#[derive(Clone, Default, Debug, DeRon, SerRon)]
pub struct SavedTspState {
    /// All wallets that have been created or imported into Robrix.
    ///
    /// This is a list of metadata, not the actual wallet objects.
    pub wallets: Vec<TspWalletMetadata>,
    /// The index of the default wallet in `wallets`, if any.
    pub default_wallet: Option<usize>,
}


pub fn tsp_init(rt: Arc<tokio::runtime::Runtime>) -> anyhow::Result<()> {
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .map_err(|_| anyhow!("BUG: default CryptoProvider was already set."))?;

    // Create a channel to be used between UI thread(s) and the TSP async worker thread.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<TspRequest>();
    TSP_REQUEST_SENDER.set(sender).expect("BUG: TSP_REQUEST_SENDER already set!");

    let rt2 = rt.clone();
    // Start a high-level async task that will start and monitor all other tasks.
    let _monitor = rt.spawn(async move {
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
                return;
            }
        }

        // Spawn the actual async worker thread.
        let mut tsp_worker_join_handle = rt2.spawn(async_tsp_worker(receiver));

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
        error: tsp_sdk::Error,
    },
    /// A wallet was removed from the list.
    WalletRemoved(TspWalletMetadata),
    /// The default wallet was successfully or unsuccessfully changed.
    DefaultWalletChanged(Result<TspWalletMetadata, ()>),
    /// The given wallet was successfully or unsuccessfully opened.
    WalletOpened(Result<TspWalletMetadata, tsp_sdk::Error>),
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
    /// Request to delete a TSP wallet.
    DeleteWallet(TspWalletMetadata),
}

/// The entry point for an async worker thread that processes TSP-related async tasks.
///
/// All this task does is wait for [`TspRequests`] from other threads
/// and then executes them within an async runtime context.
async fn async_tsp_worker(
    mut request_receiver: UnboundedReceiver<TspRequest>,
) -> anyhow::Result<()> {
    log!("Started async_tsp_worker task.");

    while let Some(req) = request_receiver.recv().await { match req {
        TspRequest::CreateWallet { metadata } => {
            log!("Received TspRequest::CreateWallet({metadata:?})");
            Handle::current().spawn(async move {
                match AskarSecureStorage::new(&metadata.path, metadata.password.as_bytes()).await {
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
                                error,
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
                if let Some(existing_opened) = tsp_state.other_wallets.iter()
                    .position(|w| match w {
                        TspWalletEntry::Opened(opened) => opened.path == metadata.path,
                        _ => false,
                    })
                    .map(|idx| tsp_state.other_wallets.remove(idx))
                {
                    if let TspWalletEntry::Opened(opened) = existing_opened {
                        if let Some(previous_active) = tsp_state.current_wallet.replace(opened) {
                            tsp_state.other_wallets.insert(0, TspWalletEntry::Opened(previous_active));
                        }
                        result = Ok(metadata);
                    }
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

        TspRequest::DeleteWallet(metadata) => {
            log!("Received TspRequest::DeleteWallet({metadata:?})");
            todo!("handle deleting a wallet");
        }
    } }
    error!("async_tsp_worker task ended unexpectedly");
    anyhow::bail!("async_tsp_worker task ended unexpectedly")
}


/// Generate the default wallet SQLite path based on the wallet name.
pub fn wallet_path_from_name(name: &str) -> String {
    format!(
        "sqlite://{}.sqlite",
        sanitize_filename::sanitize(name)
            .replace(char::is_whitespace, "_"),
    )
}
