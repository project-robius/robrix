//! Functions for loading and saving the TSP wallet state to persistent storage.

use std::collections::BTreeMap;

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedUserId;
use serde::{Deserialize, Serialize};
use crate::{app_data_dir, tsp::TspWalletMetadata};

const TSP_STATE_FILE_NAME: &str = "tsp_state.json";

const WALLETS_DIR_NAME: &str = "tsp_wallets";

/// Returns the path to the persistent app data directory for TSP wallets.
/// When Robrix creates a new TSP wallet, it will be saved in this directory.
pub fn tsp_wallets_dir() -> std::path::PathBuf {
    app_data_dir().join(WALLETS_DIR_NAME)
}

/// The TSP state that is saved to persistent storage.
///
/// It contains metadata about all wallets that have been created or imported.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SavedTspState {
    /// All wallets that have been created or imported into Robrix.
    ///
    /// This is a list of metadata, not the actual wallet objects.
    pub wallets: Vec<TspWalletMetadata>,
    /// The index of the default wallet in `wallets`, if any.
    pub default_wallet: Option<usize>,
    /// The default VID selected by the current user as the
    /// public-facing identity for sending and receiving TSP messages.
    /// This must be stored/present in the default wallet.
    pub default_vid: Option<String>,
    /// Known associations between other Matrix user IDs and their TSP DIDs.
    pub associations: BTreeMap<OwnedUserId, String>,
}
impl SavedTspState {
    /// Returns true if this TSP state has any content.
    pub fn has_content(&self) -> bool {
        !self.wallets.is_empty() || self.default_wallet.is_some() || self.default_vid.is_some()
    }

    pub fn num_wallets(&self) -> usize {
        self.default_wallet.is_some() as usize + self.wallets.len()
    }
}

/// Loads the TSP state from persistent storage.
pub async fn load_tsp_state() -> anyhow::Result<SavedTspState> {
    let content = match tokio::fs::read_to_string(app_data_dir().join(TSP_STATE_FILE_NAME)).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SavedTspState::default()),
        Err(e) => return Err(e.into()),
    };
    serde_json::from_str(&content).map_err(anyhow::Error::msg)
}

/// Asynchronously save the current TSP state to persistent storage.
pub async fn save_tsp_state_async(tsp_state: SavedTspState) -> anyhow::Result<()> {
    let path = app_data_dir().join(TSP_STATE_FILE_NAME);
    if tsp_state.has_content() {
        tokio::fs::write(path, serde_json::to_string(&tsp_state)?).await?;
    } else {
        // If the TSP state is empty, we must delete the existing file
        // such that we don't leave behind stale data.
        // Ignore errors: if it doesn't exist or can't be removed, nothing we can do.
        let _ = tokio::fs::remove_file(path).await;
    }
    log!("Successfully saved TSP state to persistent storage.");
    Ok(())
}
