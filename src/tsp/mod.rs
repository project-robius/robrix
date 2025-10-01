use std::{borrow::Cow, collections::BTreeMap, ops::Deref, path::Path, sync::{Arc, Mutex, OnceLock}};

use anyhow::anyhow;
use futures_util::StreamExt;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedUserId, UserId};
use quinn::rustls::crypto::{CryptoProvider, aws_lc_rs};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinHandle, runtime::Handle, sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender}};
use tsp_sdk::{definitions::{PublicKeyData, PublicVerificationKeyData, VidEncryptionKeyType, VidSignatureKeyType}, vid::{verify_vid, VidError}, AskarSecureStorage, AsyncSecureStore, OwnedVid, ReceivedTspMessage, SecureStorage, VerifiedVid, Vid};
use url::Url;

use crate::{persistence::{self, tsp_wallets_dir, SavedTspState}, shared::popup_list::{enqueue_popup_notification, PopupItem, PopupKind}, sliding_sync::current_user_id, tsp::tsp_verification_modal::TspVerificationModalAction, utils::DebugWrapper};


pub mod create_did_modal;
pub mod create_wallet_modal;
pub mod sign_anycast_checkbox;
pub mod tsp_settings_screen;
pub mod tsp_sign_indicator;
pub mod tsp_verification_modal;
pub mod wallet_entry;
pub mod verify_user;

pub fn live_design(cx: &mut Cx) {
    create_did_modal::live_design(cx);
    create_wallet_modal::live_design(cx);
    wallet_entry::live_design(cx);
    verify_user::live_design(cx);
    sign_anycast_checkbox::live_design(cx);
    tsp_sign_indicator::live_design(cx);
    tsp_verification_modal::live_design(cx);
    tsp_settings_screen::live_design(cx);
}

/// The sender used by [`submit_tsp_request()`] to send TSP requests to the async worker thread.
/// Currently there is only one, but it can be cloned if we need more concurrent senders.
static TSP_REQUEST_SENDER: OnceLock<UnboundedSender<TspRequest>> = OnceLock::new();

/// Submits a TSP request to the worker thread to be executed asynchronously
///
/// If an error occurs, a popup notification will be displayed to the user
/// informing them of the error with a recommendation to restart the app.
pub fn submit_tsp_request(req: TspRequest) {
    let Some(sender) = TSP_REQUEST_SENDER.get() else {
        enqueue_popup_notification(PopupItem {
            message: "Failed to submit TSP request: TSP request sender was not initialized.\n\n\
                Please restart Robrix to continue using TSP features.".into(),
            auto_dismissal_duration: None,
            kind: PopupKind::Error
        });
        return;
    };
    if sender.send(req).is_err() {
        enqueue_popup_notification(PopupItem {
            message: "Failed to submit TSP request: the background TSP worker task has died.\n\n\
                Please restart Robrix to continue using TSP features.".into(),
            auto_dismissal_duration: None,
            kind: PopupKind::Error
        });
    }
}

/// A background loop task that receives messages for a specific VID.
#[derive(Debug)]
struct ReceiveLoopTask {
    join_handle: JoinHandle<Result<(), anyhow::Error>>,
    sender: UnboundedSender<TspReceiveLoopRequest>,
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
    /// The current (default) VID for the our current logged-in user.
    pub current_local_vid: Option<String>,
    /// Maps a user's Matrix ID to their published DID.
    pub associations: BTreeMap<OwnedUserId, String>,
    /// Tasks that are currently running to receive messages for specific VIDs.
    ///
    /// This maps the private VID to a tuple of the running task and a sender
    /// that is used to send requests to the receive loop task.
    receive_loop_tasks: BTreeMap<String, ReceiveLoopTask>,
    /// Verification requests that we have sent out and are awaiting responses for.
    pending_verification_requests: SmallVec<[TspVerificationDetails; 1]>,
}
impl TspState {
    const fn new() -> Self {
        Self {
            current_wallet: None,
            other_wallets: Vec::new(),
            current_local_vid: None,
            associations: BTreeMap::new(),
            receive_loop_tasks: BTreeMap::new(),
            pending_verification_requests: SmallVec::new_const(),
        }
    }

    /// Returns true if this TSP state has any co
    /// ntent, i.e. at least one wallet.
    pub fn has_content(&self) -> bool {
        self.current_wallet.is_some()
            || !self.other_wallets.is_empty()
            || self.current_local_vid.is_some()
    }

    /// Opens all wallets in the given saved TSP state in order to populate a new `TspState`.
    pub async fn deserialize_from(saved_state: SavedTspState) -> Result<Self, tsp_sdk::Error> {
        let mut current_wallet = None;
        let mut other_wallets = Vec::with_capacity(saved_state.num_wallets());
        let mut current_local_vid = None;

        for (idx, wallet_metadata) in saved_state.wallets.into_iter().enumerate() {
            if let Ok(opened_wallet) = wallet_metadata.open_wallet().await {
                if saved_state.default_wallet == Some(idx) {
                    current_wallet = Some(opened_wallet);
                } else {
                    other_wallets.push(TspWalletEntry::Opened(opened_wallet));
                }
            } else {
                other_wallets.push(TspWalletEntry::NotFound(wallet_metadata));
            }
        }
        match current_wallet.is_some() as usize + other_wallets.len() {
            0 => log!("Restored no TSP wallets from saved TSP state."),
            1 => log!("Restored 1 TSP wallet from saved state."),
            n => log!("Restored {n} TSP wallets from saved state."),
        }

        if let Some(saved_local_vid) = saved_state.default_vid {
            if let Some(cw) = current_wallet.as_ref() {
                if cw.db.has_private_vid(&saved_local_vid)? {
                    log!("Restored current local VID {saved_local_vid} from in default wallet.");
                    current_local_vid = Some(saved_local_vid);
                } else {
                    warning!("Previously-saved local VID {saved_local_vid} was not found in default wallet.");
                    enqueue_popup_notification(PopupItem {
                        message: format!("Previously-saved local VID \"{saved_local_vid}\" \
                            was not found in default wallet.\n\n\
                            Please select a default wallet and then a new default VID."),
                        auto_dismissal_duration: None,
                        kind: PopupKind::Warning
                    });
                }
            } else {
                warning!("Found a previously-saved local VID {saved_local_vid}, but not the default wallet that contained it.");
                enqueue_popup_notification(PopupItem {
                    message: format!("Found a previously-saved local VID \"{saved_local_vid}\", \
                        but not the default wallet that contained it.\n\n\
                        Please select or create a default wallet and a new default VID."),
                    auto_dismissal_duration: None,
                    kind: PopupKind::Warning
                });
            }
        }

        if saved_state.default_wallet.is_some() && current_wallet.is_none() {
            warning!("Saved TSP state had a default wallet, but it wasn't opened successfully.");
        }

        Ok(Self {
            current_wallet,
            other_wallets,
            current_local_vid,
            associations: saved_state.associations,
            receive_loop_tasks: BTreeMap::new(),
            pending_verification_requests: SmallVec::new_const(),
        })
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
            default_vid: self.current_local_vid,
            associations: self.associations,
        })
    }

    /// Returns the associated TSP DID for a given Matrix user ID, if it exists.
    pub fn get_associated_did(&self, user_id: &UserId) -> Option<&String> {
        self.associations.get(user_id)
    }

    /// Returns the verified VID for a given Matrix user ID, if the association exists
    /// and the user's associated DID is in the current default wallet.
    pub fn get_verified_vid_for(
        &self,
        user_id: &UserId,
    ) -> Option<Arc<dyn VerifiedVid>> {
        let did = self.get_associated_did(user_id)?;
        self.current_wallet.as_ref()?
            .db
            .as_store()
            .get_verified_vid(did)
            .ok()
    }

    /// Gets or spawns a new receive loop that listens for messages for the given VID in the given wallet.
    ///
    /// Returns a sender that can be used to send requests to the receive loop.
    fn get_or_spawn_receive_loop(
        &mut self,
        rt_handle: Handle,
        wallet_db: &AsyncSecureStore,
        vid: &str,
    ) -> UnboundedSender<TspReceiveLoopRequest> {
        if let Some(task) = self.receive_loop_tasks.get(vid) {
            return task.sender.clone();
        }

        let (sender, receiver) = unbounded_channel::<TspReceiveLoopRequest>();
        let join_handle = rt_handle.spawn(
            receive_messages_for_vid(wallet_db.clone(), vid.to_string(), receiver)
        );
        let old = self.receive_loop_tasks.insert(
            vid.to_string(),
            ReceiveLoopTask { join_handle, sender: sender.clone() }
        );
        if let Some(old) = old {
            warning!("BUG: aborting previous receive loop for VID \"{}\".", vid);
            old.join_handle.abort();
        }
        sender
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
#[derive(Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug)]
#[allow(unused)] // This is a placeholder, will be used later.
enum TspReceiveLoopRequest {
    /// Stop receiving messages for the given private VID.
    Stop { vid: String },
}

pub fn tsp_init(rt_handle: tokio::runtime::Handle) -> anyhow::Result<()> {
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .map_err(|_| anyhow!("BUG: default CryptoProvider was already set."))?;

    // Create a channel to be used between UI thread(s) and the TSP async worker thread.
    // We do this early on in order to allow TSP init routines to submit requests.
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
                        "Failed to initialize TSP state. \
                        Your TSP wallets may not be fully available. Error: {e}",
                    ),
                    auto_dismissal_duration: None,
                    kind: PopupKind::Error,
                });
            }
        }

        // Spawn the actual async worker thread.
        let mut tsp_worker_join_handle = Handle::current().spawn(async_tsp_worker(receiver));

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
    let mut new_tsp_state = TspState::deserialize_from(saved_tsp_state).await?;
    if new_tsp_state.has_content() && new_tsp_state.current_wallet.is_none() {
        enqueue_popup_notification(PopupItem {
            message: String::from("TSP wallet(s) were loaded successfully, but no default wallet was set.\n\n\
                TSP features will not work properly until you set a default wallet."),
            auto_dismissal_duration: None,
            kind: PopupKind::Warning,
        });
    }
    // If there is a private VID and a current wallet, spawn a receive loop
    // to listen for incoming messages for that private VID.
    if let (Some(private_vid), Some(cw)) =
        (new_tsp_state.current_local_vid.clone(), new_tsp_state.current_wallet.as_ref())
    {
        log!("Starting receive loop for private VID \"{}\".", private_vid);
        new_tsp_state.get_or_spawn_receive_loop(
            Handle::current(),
            &cw.db.clone(),
            &private_vid,
        );
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
}

/// Actions related to TSP identities (DIDs and VIDs).
#[derive(Debug)]
pub enum TspIdentityAction {
    /// A new identity (DID) was successfully created or had an error.
    ///
    /// If successful, the result contains the created DID string.
    DidCreationResult(Result<String, anyhow::Error>),
    /// An existing identity (DID) was successfully republished or had an error.
    ///
    /// If successful, the result contains the republished DID string.
    DidRepublishResult(Result<String, anyhow::Error>),
    /// We successfully sent a request to associate another user's DID
    /// with their Matrix user ID.
    ///
    /// This does *NOT* mean that the response has been received yet.
    SentDidAssociationRequest {
        did: String,
        user_id: OwnedUserId,
    },
    /// An error occurred while sending the request to associate another
    /// user's DID with their Matrix user ID.
    ErrorSendingDidAssociationRequest {
        did: String,
        user_id: OwnedUserId,
        error: anyhow::Error,
    },
    /// We received a response to our above request to
    /// associate another user's DID with their Matrix user ID.
    ReceivedDidAssociationResponse {
        did: String,
        user_id: OwnedUserId,
        accepted: bool,
    },
    /// We received a request to associate another user's DID with their Matrix user ID.
    ReceivedDidAssociationRequest {
        /// The details of the request.
        details: TspVerificationDetails,
        /// The wallet that received the request, which will be used to send the response
        /// and store the sender's verified DID.
        wallet_db: DebugWrapper<AsyncSecureStore>,
    },
    /// An error occurred in the async task that is receiving TSP messages
    /// for the given VID.
    ReceiveLoopError {
        receiving_vid: String,
        error: anyhow::Error,
    },
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
    /// Request to create a new identity (DID) on the given server
    /// and store it in the default TSP wallet.
    CreateDid {
        username: String,
        alias: Option<String>,
        server: String,
        did_server: String,
    },
    /// Request to re-publish/re-upload our own DID back up to the DID server.
    ///
    /// The given `did` must already exist in the current default wallet.
    RepublishDid {
        did: String,
    },
    /// Request to associate another user's identity (DID) with their Matrix User ID.
    ///
    /// This will verify the DID and store it in the current default wallet
    /// (using their Matrix User ID as the alias for that new verified ID),
    /// and then send a verification/relationship request to that new verified ID.
    AssociateDidWithUserId {
        did: String,
        user_id: OwnedUserId,
    },
    /// Request to respond to a previously-received `DidAssociationRequest`.
    RespondToDidAssociationRequest {
        details: TspVerificationDetails,
        wallet_db: AsyncSecureStore,
        accepted: bool,
    },
    // TODO: support canceling a previously-initiated association/verification request.
    // /// Request to cancel a previously-sent `AssociateDidWithUserId` request.
    // CancelAssociateDidRequest(TspVerificationDetails),
}


fn create_reqwest_client() -> reqwest::Result<reqwest::Client> {
    reqwest::ClientBuilder::new()
        .user_agent(format!("Robrix v{}", env!("CARGO_PKG_VERSION")))
        .build()
}


/// The entry point for an async worker thread that processes TSP-related async tasks.
///
/// All this task does is wait for [`TspRequests`] from other threads
/// and then executes them within an async runtime context.
async fn async_tsp_worker(
    mut request_receiver: UnboundedReceiver<TspRequest>,
) -> anyhow::Result<()> {
    log!("Started async_tsp_worker task.");

    // Allow lazy initialization of the reqwest client.
    let mut __reqwest_client = None;
    let mut get_reqwest_client = || {
        __reqwest_client.get_or_insert_with(|| create_reqwest_client().unwrap()).clone()
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
                let result = create_did_and_add_to_wallet(
                    &client,
                    username,
                    alias,
                    server,
                    did_server,
                ).await;
                Cx::post_action(TspIdentityAction::DidCreationResult(result));
            });
        }

        TspRequest::RepublishDid { did } => {
            log!("Received TspRequest::RepublishDid(did: {did})");
            let client = get_reqwest_client();

            Handle::current().spawn(async move {
                let result = republish_did(&did, &client).await
                    .map(|_| did);
                Cx::post_action(TspIdentityAction::DidRepublishResult(result));
            });
        }

        TspRequest::AssociateDidWithUserId { did, user_id } => {
            log!("Received TspRequest::AssociateDidWithUserId(did: {did}, user_id: {user_id})");
            Handle::current().spawn(async move {
                let action = match associate_did_with_user_id(&did, &user_id).await {
                    Ok(_) => TspIdentityAction::SentDidAssociationRequest { did, user_id },
                    Err(error) => TspIdentityAction::ErrorSendingDidAssociationRequest { did, user_id, error },
                };
                Cx::post_action(action);
            });
        }

        TspRequest::RespondToDidAssociationRequest { details, wallet_db, accepted } => {
            log!("Received TspRequest::RespondToDidAssociationRequest(details: {details:?}, accepted: {accepted})");
            Handle::current().spawn(async move {
                let result = respond_to_did_association_request(&details, &wallet_db, accepted).await;
                // If all was successful, add this new association to the TSP state.
                if result.is_ok() {
                    tsp_state_ref().lock().unwrap().associations.insert(
                        details.initiating_user_id.clone(),
                        details.initiating_vid.clone(),
                    );
                }
                Cx::post_action(TspVerificationModalAction::SentDidAssociationResponse {
                    details,
                    result,
                });
            });
        }
    }
}

    error!("async_tsp_worker task ended unexpectedly");
    anyhow::bail!("async_tsp_worker task ended unexpectedly")
}


/// Creates & publishes a new DID, adds it to the default wallet,
/// and sets the new private VID to be default if none exists.
///
/// Returns the new DID that was published.
async fn create_did_and_add_to_wallet(
    client: &reqwest::Client,
    username: String,
    alias: Option<String>,
    server: String,
    did_server: String,
) -> Result<String, anyhow::Error> {
    let cw_db = tsp_state_ref().lock().unwrap()
        .current_wallet.as_ref()
        .map(|w| w.db.clone())
        .ok_or_else(|| anyhow!("Please choose a default TSP wallet to hold the DID."))?;
    let (did, private_vid, metadata) = create_did_web(&did_server, &server, &username, client).await?;
    let new_vid = private_vid.identifier().to_string();
    log!("Successfully created & published new DID: {did}.\n\
        Adding private VID {new_vid} to current wallet...",
    );
    let did = store_did_in_wallet(&cw_db, private_vid, metadata, alias, did)?;

    {
        // If there's no default VID, set this new one as the default,
        // associate our currently-logged-in Matrix User ID with it,
        // and start a receive loop to listen for incoming requests for it.
        let mut tsp_state = tsp_state_ref().lock().unwrap();
        if tsp_state.current_local_vid.is_none() {
            log!("Setting new VID \"{}\" (from DID \"{}\") as current local VID and starting receive loop...", new_vid, did);
            tsp_state.current_local_vid = Some(new_vid.clone());
            tsp_state.get_or_spawn_receive_loop(
                Handle::current(),
                &cw_db,
                &new_vid,
            );
            if let Some(user_id) = current_user_id() {
                tsp_state.associations
                    .entry(user_id.clone())
                    .or_insert_with(|| {
                        log!("Automatically associating DID \"{did}\" with the current Matrix User ID \"{user_id}\".");
                        did.clone()
                    });
            }
        }
    }
    Ok(did)
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
        "did:web:{}:endpoint:{}",
        did_server.replace(":", "%3A").replace("/", ":"),
        username,
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
                     or the server had another problem.\n\nResponse: \"{text}\"."
                ));
            } else {
                return Err(anyhow!(
                    "The DID server returned error code {}.\n\nResponse: \"{text}\".",
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
fn store_did_in_wallet(
    current_wallet_db: &AsyncSecureStore,
    private_vid: OwnedVid,
    metadata: Option<serde_json::Value>,
    alias: Option<String>,
    did: String,
) -> Result<String, anyhow::Error> {
    current_wallet_db.add_private_vid(private_vid, metadata)?;
    if let Some(alias) = alias {
        current_wallet_db.set_alias(alias.clone(), did.clone())?;
        log!("added alias {alias} -> {did}");
    }
    Ok(did)
}


/// Re-publishes/re-uploads our own DID to the DID server it was originally created on.
async fn republish_did(
    did: &str,
    client: &reqwest::Client,
) -> Result<(), anyhow::Error> {

    /// A copy of the Vid struct that we can actually instantiate
    /// from an existing VID in a local wallet.
    ///
    /// This is a hack because there is no way to create a `Vid` struct
    /// instance from an existing VID in a local wallet.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct VidDuplicate {
        id: String,
        transport: Url,
        #[serde(default)]
        sig_key_type: VidSignatureKeyType,
        public_sigkey: PublicVerificationKeyData,
        #[serde(default)]
        enc_key_type: VidEncryptionKeyType,
        public_enckey: PublicKeyData,
    }


    let our_vid = {
        let tsp_state = tsp_state_ref().lock().unwrap();
        tsp_state.current_wallet.as_ref()
            .ok_or_else(no_default_wallet_error)?
            .db
            .as_store()
            .get_verified_vid(did)
            .map_err(|_e| anyhow!("The DID to republish \"{did}\" was not found in the current default wallet."))?
    };

    let vid_dup = VidDuplicate {
        id: our_vid.identifier().to_owned(),
        transport: our_vid.endpoint().to_owned(),
        sig_key_type: our_vid.signature_key_type(),
        public_sigkey: our_vid.verifying_key().to_owned(),
        enc_key_type: our_vid.encryption_key_type(),
        public_enckey: our_vid.encryption_key().to_owned(),
    };

    let did_transport_url = tsp_sdk::vid::did::get_resolve_url(did)?;

    let response = client
        .post(format!("{}/add-vid", did_transport_url.origin().ascii_serialization()))
        .json(&vid_dup)
        .send()
        .await
        .map_err(|e| anyhow!("Could not republish VID. The DID server responded with error: {e}"))?;

    match response.status() {
        r if r.is_success() => {
            log!("Successfully republished DID {did}.");
            Ok(())
        }
        r => {
            let text = response.text().await.unwrap_or_else(|_| "[Unknown]".to_string());
            if r.as_u16() == 500 {
                Err(anyhow!(
                    "The DID server returned error code 500. The DID username may already exist, \
                     or the server had another problem.\n\nResponse: \"{text}\"."
                ))
            } else {
                Err(anyhow!(
                    "The DID server returned error code {}.\n\nResponse: \"{text}\".",
                    r.as_u16()
                ))
            }
        }
    }
}


async fn receive_messages_for_vid(
    wallet_db: AsyncSecureStore,
    private_vid_to_receive_on: String,
    mut request_rx: UnboundedReceiver<TspReceiveLoopRequest>,
) -> Result<(), anyhow::Error> {
    // Ensure that our receiving VID is currently published to the DID server.
    if republish_did(&private_vid_to_receive_on, &create_reqwest_client()?).await.is_ok() {
        log!("Auto-republished DID \"{private_vid_to_receive_on}\" to its DID server.");
    }

    let mut message_stream = wallet_db.receive(&private_vid_to_receive_on).await?;

    loop {
        tokio::select! {
            // we should handle new TspReceiveLoopRequests before incoming messages,
            // in case the new request affects the message handling logic.
            biased;

            Some(request) = request_rx.recv() => {
                log!("Received TSP receive loop request: {:?}", request);
                match request {
                    TspReceiveLoopRequest::Stop { vid } if vid == private_vid_to_receive_on => {
                        log!("Stopping receive loop for VID: {}", vid);
                        break;
                    }
                    // Handle other request types as needed
                    _ => {}
                }
            }

            Some(msg_result) = message_stream.next() => { match msg_result {
                Ok(message) => match message {
                    ReceivedTspMessage::GenericMessage { sender, receiver, nonconfidential_data, message, .. } => {
                        log!("Received generic TSP message from {sender} to {receiver:?}: {:?}, nonconfidential_data: {:?}", message, nonconfidential_data);
                        let Ok(tsp_message) = serde_json::from_slice::<TspMessage>(&message) else {
                            log!("Received a message that couldn't be deserialized into a TspMessage.");
                            continue;
                        };
                        match tsp_message {
                            TspMessage::VerificationRequest(details) => {
                                log!("Received TSP verification request: {:?}", details);
                                Cx::post_action(TspIdentityAction::ReceivedDidAssociationRequest {
                                    details,
                                    wallet_db: wallet_db.clone().into(),
                                });
                            }
                            TspMessage::VerificationResponse {details, accepted} => {
                                log!("Received {} verification response: {:?}", accepted, details);
                                let mut tsp_state = tsp_state_ref().lock().unwrap();
                                if let Some(matching_request) = tsp_state.pending_verification_requests.iter()
                                    .position(|vreq| vreq == &details)
                                    .map(|idx| tsp_state.pending_verification_requests.swap_remove(idx))
                                {
                                    log!("Found matching verification request: {:?}", matching_request);
                                    tsp_state.associations.insert(
                                        details.responding_user_id.clone(),
                                        details.responding_vid.clone(),
                                    );
                                    Cx::post_action(TspIdentityAction::ReceivedDidAssociationResponse {
                                        did: details.responding_vid,
                                        user_id: details.responding_user_id,
                                        accepted,
                                    });
                                }
                                else {
                                    log!("Verification response was unexpected: no matching verification request found");
                                }
                            }
                        }
                    }
                    _other => {
                        log!("Received other TSP message: {:?}", _other);
                    }
                }
                Err(e) => {
                    log!("Error receiving TSP message: {:?}", e);
                }
            } }
        }
    }

    Ok(())
}


fn no_default_wallet_error() -> anyhow::Error {
    anyhow!("Please choose a default TSP wallet.")
}

fn no_default_vid_error() -> anyhow::Error {
    anyhow!("Please choose a default VID from your default \
        TSP wallet to represent your own Matrix account.")
}


/// Associates the given DID with a Matrix User ID.
///
/// This function only performs the local verification of the given DID into
/// the local default wallet, and then sends a verification request to the user.
/// It does not wait to receive a verification response.
async fn associate_did_with_user_id(
    did: &str,
    user_id: &OwnedUserId,
) -> Result<(), anyhow::Error> {
    let our_user_id = crate::sliding_sync::current_user_id()
        .ok_or_else(|| anyhow!("Must be logged into Matrix in order to associate a DID with a Matrix User ID."))?;
    let (wallet_db, our_vid) = {
        let tsp_state = tsp_state_ref().lock().unwrap();
        let wallet = tsp_state.current_wallet.as_ref().ok_or_else(no_default_wallet_error)?;
        let our_vid = tsp_state.current_local_vid.clone().ok_or_else(no_default_vid_error)?;
        (wallet.db.clone(), our_vid)
    };
    if !wallet_db.has_verified_vid(did)? {
        wallet_db.verify_vid(did, Some(user_id.to_string())).await?;
        log!("DID {did} was verified and added to the default wallet.");
    }

    let verification_details = TspVerificationDetails {
        initiating_vid: our_vid.clone(),
        initiating_user_id: our_user_id.clone(),
        responding_vid: did.to_string(),
        responding_user_id: user_id.clone(),
        random_str: {
            use rand::{Rng, thread_rng};
            thread_rng()
                .sample_iter(rand::distributions::Alphanumeric)
                .take(32)
                .map(char::from)
                .collect()
        },
    };
    tsp_state_ref().lock().unwrap().pending_verification_requests.push(verification_details.clone());
    let request_msg = TspMessage::VerificationRequest(verification_details);
    wallet_db.send(
        &our_vid,
        did,
        // This is just for debugging and should be removed before production.
        Some(format!("Verification from {our_user_id} to {user_id}").as_bytes()),
        serde_json::to_string(&request_msg)?.as_bytes(),
    ).await?;

    // Note: the receive loop will wait to receive the verification response,
    //       upon which the verification procedure will be completed
    //       and the UI layer will be informed and updated to reflect this.
    Ok(())
}


/// Sends a positive/negative response to a previous incoming DID association request.
async fn respond_to_did_association_request(
    details: &TspVerificationDetails,
    wallet_db: &AsyncSecureStore,
    accepted: bool,
) -> Result<(), anyhow::Error> {
    wallet_db.verify_vid(&details.initiating_vid, Some(details.initiating_user_id.to_string())).await?;
    log!("Verification requester's initiating DID {} was verified and added to your wallet.", details.initiating_vid);

    let response_msg = TspMessage::VerificationResponse {
        details: details.clone(),
        accepted,
    };
    wallet_db.send(
        &details.responding_vid,
        &details.initiating_vid,
        // This is just for debugging and should be removed before production.
        Some(format!("Verification Response ({accepted}) from {} to {}", details.responding_user_id, details.initiating_user_id).as_bytes()),
        serde_json::to_string(&response_msg)?.as_bytes(),
    ).await?;

    Ok(())
}

/// Signs the given message using the default VID from the default wallet.
pub fn sign_anycast_with_default_vid(message: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let (wallet_db, signing_vid) = {
        let tsp_state = tsp_state_ref().lock().unwrap();
        (
            tsp_state.current_wallet.as_ref().ok_or_else(no_default_wallet_error)?.db.clone(),
            tsp_state.current_local_vid.clone().ok_or_else(no_default_vid_error)?,
        )
    };
    let signed = wallet_db.as_store().sign_anycast(&signing_vid, message)?;
    Ok(signed)
}


/// The types/schema of messages that we send over the TSP protocol.
#[derive(Debug, Serialize, Deserialize)]
enum TspMessage {
    /// A request to verify another Matrix user's TSP DID.
    VerificationRequest(TspVerificationDetails),
    /// A response to a verification request.
    VerificationResponse {
        details: TspVerificationDetails,
        accepted: bool,
    },
    // TODO: support initiator-side cancelation of a request.
    // /// A request to cancel a previously-initiated verification request.
    // VerificationCancel(TspVerificationDetails),
}

/// The payload for a new verification request / response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TspVerificationDetails {
    /// The VID of the user who initiated the verification request.
    initiating_vid: String,
    /// The Matrix User ID of the user who initiated the verification request.
    initiating_user_id: OwnedUserId,
    /// The VID of the user who is receiving this verification request.
    responding_vid: String,
    /// The Matrix User ID of the user who is receiving this verification request.
    responding_user_id: OwnedUserId,
    /// A string to be manually matched/verified on both sides of the request.
    random_str: String,
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
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq)]
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
    /// ## Usage notes
    /// The returned URL is suitable for use in `AskarSecureStorage` methods
    /// like `new()` and `open()`.
    ///
    /// ## Implementation notes
    /// We cannot use the `Path`/`PathBuf` type because the sqlite backend
    /// always expects URLs with filename paths encoded using Unix-style `/` path separators,
    /// even on Windows. Therefore, we manually percent-encode each part of the path
    /// and push them in between manually-added `/` separators, instead of using 
    /// the Rust `std::path` functions like `Path::join()` or `PathBuf::push()`.
    pub fn to_url_encoded(&self) -> Cow<'_, str> {
        const DELIMITER_ABS: &str = ":///";
        const DELIMITER_REG: &str = "://";
        /// SQLite requires Unix-style path separators.
        const SEPARATOR: &str = "/";
        /// The percent-encoded version of the separator,
        /// which can be used everywhere except for prefixes.
        const SEPARATOR_PE: &str = "%2F";

        let try_encode = |delim: &str| -> Option<String> {
            if let Some(idx) = self.0.find(delim) {
                let before = self.0.get(.. (idx + delim.len())).unwrap_or("");
                let after  = self.0.get((idx + delim.len()) ..).unwrap_or("");
                let mut after_encoded = String::new();
                for component in Path::new(after).components() {
                    match component {
                        std::path::Component::Prefix(prefix) => {
                            // Windows drive prefixes must not be percent-encoded.
                            after_encoded = format!("{}{}{}", after_encoded, SEPARATOR, prefix.as_os_str().to_string_lossy());
                        }
                        std::path::Component::RootDir => {
                            // ignore, since we already manually add '/' between components.
                        }
                        std::path::Component::Normal(p) => {
                            let percent_encoded = percent_encoding::percent_encode(
                                p.as_encoded_bytes(),
                                percent_encoding::NON_ALPHANUMERIC
                            );
                            after_encoded = format!("{}{}{}", after_encoded, SEPARATOR_PE, percent_encoded);
                        }
                        other => {
                            after_encoded = format!("{}{}{}", after_encoded, SEPARATOR_PE, other.as_os_str().to_string_lossy());
                        }
                    }
                }
                Some(format!("{}{}", before, after_encoded))
            } else {
                None
            }
        };

        try_encode(DELIMITER_ABS)
            .or_else(|| try_encode(DELIMITER_REG))
            .map(Cow::from)
            .unwrap_or_else(|| {
                percent_encoding::utf8_percent_encode(
                    &self.0,
                    percent_encoding::NON_ALPHANUMERIC,
                ).into()
            })
    }
}
