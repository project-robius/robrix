use makepad_widgets::{makepad_micro_serde::{DeRon, SerRon}, *};
use serde::{self, Deserialize, Serialize};
use matrix_sdk::ruma::{OwnedUserId, UserId};
use crate::{app::{AppState, SelectedRoom}, app_data_dir, persistence::persistent_state_dir};


const LATEST_APP_STATE_FILE_NAME: &str = "latest_app_state.ron";

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
    std::fs::write(
        persistent_state_dir(&user_id).join(LATEST_APP_STATE_FILE_NAME),
        app_state.serialize_ron(),
    )?;
    for (tab_id, room) in &app_state.saved_dock_state.open_rooms {
        match room {
            SelectedRoom::JoinedRoom { room_id, .. }
            | SelectedRoom::InvitedRoom { room_id, .. }
            | SelectedRoom::PreviewedRoom { room_id, .. } => {
                if !app_state.saved_dock_state.dock_items.contains_key(tab_id) {
                    error!("Room id: {} already in dock state", room_id);
                }
            }
        }
    }
    log!("Successfully saved app state to persistent storage.");
    Ok(())
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
pub async fn load_app_state(user_id: &UserId) -> anyhow::Result<AppState> {
    let content = match tokio::fs::read_to_string(persistent_state_dir(user_id).join(LATEST_APP_STATE_FILE_NAME)).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(AppState::default()),
        Err(e) => return Err(e.into())
    };
    AppState::deserialize_ron(&content)
        .map_err(|er| anyhow::Error::msg(er.msg))
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
