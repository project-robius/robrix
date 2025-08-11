//! Functions for loading and saving the TSP wallet state to persistent storage.

use makepad_widgets::{*, makepad_micro_serde::*};

use crate::{app_data_dir, tsp::SavedTspState};


const TSP_STATE_FILE_NAME: &str = "tsp_state.ron";

const WALLETS_DIR_NAME: &str = "tsp_wallets";

/// Returns the path to the persistent app data directory for TSP wallets.
/// When Robrix creates a new TSP wallet, it will be saved in this directory.
pub fn tsp_wallets_dir() -> std::path::PathBuf {
    app_data_dir().join(WALLETS_DIR_NAME)
}

/// Loads the TSP state from persistent storage.
pub async fn load_tsp_state() -> anyhow::Result<SavedTspState> {
    let content = match tokio::fs::read_to_string(
        app_data_dir().join(TSP_STATE_FILE_NAME)
    ).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SavedTspState::default()),
        Err(e) => return Err(e.into())
    };
    SavedTspState::deserialize_ron(&content)
        .map_err(|e| anyhow::Error::msg(e.msg))
}


/// Save the current TSP state to persistent storage.
pub fn save_tsp_state(tsp_state: SavedTspState) -> anyhow::Result<()> {
    std::fs::write(
        app_data_dir().join(TSP_STATE_FILE_NAME),
        tsp_state.serialize_ron(),
    )?;
    log!("Successfully saved TSP state to persistent storage.");
    Ok(())
}

/// Asynchronously save the current TSP state to persistent storage.
pub async fn save_tsp_state_async(tsp_state: SavedTspState) -> anyhow::Result<()> {
    tokio::fs::write(
        app_data_dir().join(TSP_STATE_FILE_NAME),
        tsp_state.serialize_ron(),
    ).await?;
    log!("Successfully saved TSP state to persistent storage.");
    Ok(())
}
