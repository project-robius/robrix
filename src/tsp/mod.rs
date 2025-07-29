use std::{ops::Deref, sync::Mutex};

use anyhow::anyhow;
use makepad_widgets::{makepad_micro_serde::*, *};
use quinn::rustls::crypto::{CryptoProvider, aws_lc_rs};
use tsp_sdk::{AskarSecureStorage, AsyncSecureStore, SecureStorage};

use crate::persistence;

pub mod tsp_settings_screen;

// pub mod create_wallet_modal;

pub fn live_design(cx: &mut Cx) {
    // create_wallet_modal::live_design(cx);
    tsp_settings_screen::live_design(cx);
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
            let metadata = entry.deref().clone();
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


/// A TSP wallet entry known to Robrix.
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
            .finish()
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


pub fn tsp_init(rt: &tokio::runtime::Runtime) -> anyhow::Result<()> {
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .map_err(|_| anyhow!("BUG: default CryptoProvider was already set."))?;

    rt.spawn(inner_tsp_init());
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
#[derive(Debug, Clone)]
pub enum TspWalletAction {
    /// A wallet was created or imported successfully.
    WalletAdded {
        metadata: TspWalletMetadata,
        is_default: bool,
    },
    /// A wallet was deleted successfully.
    WalletDeleted(TspWalletMetadata),
    /// The default wallet was set to the given wallet.
    DefaultWalletSet(TspWalletMetadata),
}
