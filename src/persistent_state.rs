//! Handles app persistence by saving and restoring client session data to/from the filesystem.
use std::path::PathBuf;
use anyhow::{anyhow, bail};
use makepad_widgets::{
    dvec2, error, log, makepad_micro_serde::{DeRon, SerRon}, Cx, WindowRef
};
use matrix_sdk::{
    authentication::matrix::MatrixSession,
    ruma::{OwnedUserId, UserId},
    sliding_sync::VersionBuilder,
    Client,
};
use serde::{Deserialize, Serialize};
use tokio::{fs, io};

use crate::{
    app::{SavedDockState, SelectedRoom, WindowGeomState},
    app_data_dir,
    login::login_screen::LoginAction,
};

/// The data needed to re-build a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSessionPersisted {
    /// The URL of the homeserver of the user.
    pub homeserver: String,

    /// The path of the database.
    pub db_path: PathBuf,

    /// The passphrase of the database.
    pub passphrase: String,
}

/// The full session to persist.
#[derive(Debug, Serialize, Deserialize)]
pub struct FullSessionPersisted {
    /// The data to re-build the client.
    pub client_session: ClientSessionPersisted,

    /// The Matrix user session.
    pub user_session: MatrixSession,

    /// The latest sync token.
    ///
    /// It is only needed to persist it when using `Client::sync_once()` and we
    /// want to make our syncs faster by not receiving all the initial sync
    /// again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,
}

fn user_id_to_file_name(user_id: &UserId) -> String {
    user_id.as_str()
        .replace(":", "_")
        .replace("@", "")
}

pub fn persistent_state_dir(user_id: &UserId) -> PathBuf {
    app_data_dir()
        .join(user_id_to_file_name(user_id))
        .join("persistent_state")
}

pub fn session_file_path(user_id: &UserId) -> PathBuf {
    persistent_state_dir(user_id).join("session")
}

const LATEST_USER_ID_FILE_NAME: &str = "latest_user_id.txt";

const LATEST_DOCK_STATE_FILE_NAME: &str = "latest_dock_state.ron";

const WINDOW_GEOM_STATE_FILE_NAME: &str = "window_geom_state.json";

/// Returns the user ID of the most recently-logged in user session.
pub fn most_recent_user_id() -> Option<OwnedUserId> {
    std::fs::read_to_string(
        app_data_dir().join(LATEST_USER_ID_FILE_NAME)
    )
    .ok()?
    .trim()
    .try_into()
    .ok()
}

/// Save which user was the most recently logged in.
async fn save_latest_user_id(user_id: &UserId) -> anyhow::Result<()> {
    fs::write(
        app_data_dir().join(LATEST_USER_ID_FILE_NAME),
        user_id.as_str(),
    ).await?;
    Ok(())
}


/// Restores the given user's previous session from the filesystem.
///
/// If no User ID is specified, the ID of the most recently-logged in user
/// is retrieved from the filesystem.
pub async fn restore_session(
    user_id: Option<OwnedUserId>
) -> anyhow::Result<(Client, Option<String>)> {
    let Some(user_id) = user_id.or_else(most_recent_user_id) else {
        log!("Could not find previous latest User ID");
        bail!("Could not find previous latest User ID");
    };
    let session_file = session_file_path(&user_id);
    if !session_file.exists() {
        log!("Could not find previous session file for user {user_id}");
        bail!("Could not find previous session file");
    }
    let status_str = format!("Loading previous session file for {user_id}...");
    log!("{status_str}: '{}'", session_file.display());
    Cx::post_action(LoginAction::Status {
        title: "Restoring session".into(),
        status: status_str,
    });

    // The session was serialized as JSON in a file.
    let serialized_session = fs::read_to_string(session_file).await?;
    let FullSessionPersisted { client_session, user_session, sync_token } =
        serde_json::from_str(&serialized_session)?;

    let status_str = format!(
        "Loaded session file for {user_id}. Trying to connect to homeserver ({})...",
        client_session.homeserver,
    );
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Connecting to homeserver".into(),
        status: status_str,
    });

    // Build the client with the previous settings from the session.
    let client = Client::builder()
        .server_name_or_homeserver_url(client_session.homeserver)
        .sqlite_store(client_session.db_path, Some(&client_session.passphrase))
        .sliding_sync_version_builder(VersionBuilder::DiscoverNative)
        .handle_refresh_tokens()
        .build()
        .await?;

    let status_str = format!("Authenticating previous login session for {}...", user_session.meta.user_id);
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Authenticating session".into(),
        status: status_str,
    });

    // Restore the Matrix user session.
    client.restore_session(user_session).await?;
    save_latest_user_id(&user_id).await?;

    Ok((client, sync_token))
}

/// Persist a logged-in client session to the filesystem for later use.
///
/// TODO: This is not very secure, for simplicity. We should use robius-keychain
///       or `keyring-rs` to storing secrets securely.
///
/// Note that we could also build the user session from the login response.
pub async fn save_session(
    client: &Client,
    client_session: ClientSessionPersisted,
) -> anyhow::Result<()> {
    let user_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| anyhow!("A logged-in client should have a session"))?;

    save_latest_user_id(&user_session.meta.user_id).await?;

    // Save that user's session.
    let session_file = session_file_path(&user_session.meta.user_id);
    let serialized_session = serde_json::to_string(&FullSessionPersisted {
        client_session,
        user_session,
        sync_token: None,
    })?;
    if let Some(parent) = session_file.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(&session_file, serialized_session).await?;

    log!("Session persisted to: {}", session_file.display());

    // After logging in, you might want to verify this session with another one (see
    // the `emoji_verification` example), or bootstrap cross-signing if this is your
    // first session with encryption, or if you need to reset cross-signing because
    // you don't have access to your old sessions (see the
    // `cross_signing_bootstrap` example).

    Ok(())
}

/// Save the current display state of the room panel to persistent storage.
pub fn save_room_panel(
    rooms_panel_state: SavedDockState,
    user_id: OwnedUserId,
) -> anyhow::Result<()> {
    std::fs::write(
        persistent_state_dir(&user_id).join(LATEST_DOCK_STATE_FILE_NAME),
        rooms_panel_state.serialize_ron(),
    )?;
    for (tab_id, room) in &rooms_panel_state.open_rooms {
        match room {
            SelectedRoom::JoinedRoom { room_id, .. }
            | SelectedRoom::InvitedRoom { room_id, .. } => {
                if !rooms_panel_state.dock_items.contains_key(tab_id) {
                    error!("Room id: {} already in dock state", room_id);
                }
            }
        }
    }
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

/// Loads the rooms panel's state from persistent storage.
pub async fn load_rooms_panel_state(user_id: &UserId) -> anyhow::Result<SavedDockState> {
    let content = match tokio::fs::read_to_string(persistent_state_dir(user_id).join(LATEST_DOCK_STATE_FILE_NAME)).await {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SavedDockState::default()),
        Err(e) => return Err(e.into())
    };
    SavedDockState::deserialize_ron(&content)
        .map_err(|er| anyhow::Error::msg(er.msg))
}

/// Loads the window geometry's state from persistent storage.
pub fn load_window_state(window_ref: WindowRef, cx: &mut Cx) -> anyhow::Result<()> {
    let file = match std::fs::File::open(app_data_dir().join(WINDOW_GEOM_STATE_FILE_NAME)) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let window_geom = serde_json::from_reader(file).map_err(|e| anyhow!(e))?;
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
