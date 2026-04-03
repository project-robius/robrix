use std::io::Write;

use makepad_widgets::*;
use serde::{self, Deserialize, Serialize};
use matrix_sdk::ruma::{OwnedUserId, UserId};
use crate::{app::AppState, app_data_dir, persistence::persistent_state_dir};


const LATEST_APP_STATE_FILE_NAME: &str = "latest_app_state.json";
const SKIP_APP_STATE_RESTORE_ONCE_FILE_NAME: &str = "skip_app_state_restore_once";

const WINDOW_GEOM_STATE_FILE_NAME: &str = "window_geom_state.json";


/// Persistable state of the window's size, position, and fullscreen status.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeomState {
    /// A tuple containing the window's width and height.
    pub inner_size: (f64, f64),
    /// A tuple containing the window's x and y position.
    pub position: (f64, f64),
    /// Maximise fullscreen if true.
    pub is_fullscreen: bool,
}


/// Save the current app state to persistent storage.
pub fn save_app_state(
    app_state: AppState,
    user_id: OwnedUserId,
) -> anyhow::Result<()> {
    let file = std::fs::File::create(
        persistent_state_dir(&user_id).join(LATEST_APP_STATE_FILE_NAME)
    )?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, &app_state)?;
    writer.flush()?;
    log!("Successfully saved app state to persistent storage.");
    Ok(())
}

/// Marks that the next login for this user should skip automatic app-state restore once.
pub async fn skip_app_state_restore_once(user_id: &UserId) -> anyhow::Result<()> {
    let marker_path = persistent_state_dir(user_id).join(SKIP_APP_STATE_RESTORE_ONCE_FILE_NAME);
    if let Some(parent) = marker_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(marker_path, b"1").await?;
    Ok(())
}

/// Consumes the one-shot "skip automatic restore" marker for the given user, if present.
pub async fn take_skip_app_state_restore_once(user_id: &UserId) -> anyhow::Result<bool> {
    let marker_path = persistent_state_dir(user_id).join(SKIP_APP_STATE_RESTORE_ONCE_FILE_NAME);
    match tokio::fs::remove_file(marker_path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Save the current state of the given window's geometry to persistent storage.
pub fn save_window_state(window_ref: WindowRef, cx: &Cx) -> anyhow::Result<()> {
    let inner_size = window_ref.get_inner_size(cx);
    let position = window_ref.get_position(cx);
    let window_geom = WindowGeomState {
        inner_size: (inner_size.x, inner_size.y),
        position: (position.x, position.y),
        is_fullscreen: window_ref.is_fullscreen(cx),
    };
    std::fs::write(
        app_data_dir().join(WINDOW_GEOM_STATE_FILE_NAME),
        serde_json::to_string(&window_geom)?,
    )?;
    log!("Successfully saved window geometry: {window_geom:?}");
    Ok(())
}

/// Loads the App state from persistent storage.
///
/// If the file doesn't exist or deserialization fails (e.g., due to incompatible format changes),
/// this function returns a default `AppState` and backs up the old file if it exists.
pub async fn load_app_state(user_id: &UserId) -> anyhow::Result<AppState> {
    let state_path = persistent_state_dir(user_id).join(LATEST_APP_STATE_FILE_NAME);
    let file_bytes = match tokio::fs::read(&state_path).await {
        Ok(fb) => fb,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log!("No saved app state found, using default.");
            return Ok(AppState::default());
        }
        Err(e) => return Err(e.into())
    };
    match serde_json::from_slice(&file_bytes) {
        Ok(app_state) => {
            log!("Successfully loaded app state from persistent storage.");
            Ok(app_state)
        }
        Err(e) => {
            error!("Failed to deserialize app state: {e}. This may be due to an incompatible format from a previous version.");

            // Backup the old file to preserve user's data
            let backup_path = state_path.with_extension("json.bak");
            if let Err(backup_err) = tokio::fs::rename(&state_path, &backup_path).await {
                error!("Failed to backup old app state file: {}", backup_err);
            } else {
                log!("Old app state backed up to: {:?}", backup_path);
            }

            log!("Using default app state. Your previous tabs and selections will be reset.");
            Ok(AppState::default())
        }
    }
}

/// Loads the window geometry's state from persistent storage.
pub fn load_window_state(window_ref: WindowRef, cx: &mut Cx) -> anyhow::Result<()> {
    let file = match std::fs::File::open(app_data_dir().join(WINDOW_GEOM_STATE_FILE_NAME)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let window_geom = serde_json::from_reader(file).map_err(|e| anyhow::anyhow!(e))?;
    log!("Restoring window geometry: {window_geom:?}");
    let WindowGeomState {
        inner_size,
        position,
        is_fullscreen,
    } = window_geom;
    window_ref.configure_window(
        cx,
        dvec2(inner_size.0, inner_size.1),
        dvec2(position.0, position.1),
        is_fullscreen,
        "Robrix".to_string(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skip_restore_marker_is_consumed_once() {
        let user_id = UserId::parse("@robrix-test-skip-restore:example.invalid")
            .unwrap()
            .to_owned();

        let _ = tokio::fs::remove_dir_all(persistent_state_dir(&user_id)).await;

        skip_app_state_restore_once(&user_id).await.unwrap();

        assert!(take_skip_app_state_restore_once(&user_id).await.unwrap());
        assert!(!take_skip_app_state_restore_once(&user_id).await.unwrap());

        let _ = tokio::fs::remove_dir_all(persistent_state_dir(&user_id)).await;
    }
}
