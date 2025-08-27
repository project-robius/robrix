//! Handles app persistence by saving and restoring client session data to/from the filesystem.

use std::path::PathBuf;
use anyhow::{anyhow, bail};
use makepad_widgets::{log, Cx};
use matrix_sdk::{
    authentication::matrix::MatrixSession,
    ruma::{OwnedUserId, UserId},
    sliding_sync,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
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

    /// The sliding sync version to use for this client session.
    /// 
    /// This determines the sync protocol used by the Matrix client:
    /// - `Native`: Uses the server's native sliding sync implementation for efficient syncing
    /// - `None`: Falls back to standard Matrix sync (without sliding sync optimizations)
    /// 
    /// The value is restored and applied to the client via `client.set_sliding_sync_version()`
    /// when rebuilding the session from persistent storage.
    #[serde(default)]
    pub sliding_sync_version: SlidingSyncVersion,
}

/// A serializable duplicate of [`sliding_sync::Version`].
#[derive(Debug, Default, Serialize, Deserialize)]
pub enum SlidingSyncVersion {
    #[default]
    Native,
    None,
}
impl From<SlidingSyncVersion> for sliding_sync::Version {
    fn from(version: SlidingSyncVersion) -> Self {
        match version {
            SlidingSyncVersion::None => sliding_sync::Version::None,
            SlidingSyncVersion::Native => sliding_sync::Version::Native,
        }
    }
}
impl From<sliding_sync::Version> for SlidingSyncVersion {
    fn from(version: sliding_sync::Version) -> Self {
        match version {
            sliding_sync::Version::None => SlidingSyncVersion::None,
            sliding_sync::Version::Native => SlidingSyncVersion::Native,
        }
    }
}

fn user_id_to_file_name(user_id: &UserId) -> String {
    user_id.as_str()
        .replace(":", "_")
        .replace("@", "")
}

/// Returns the path to the persistent state directory for the given user.
pub fn persistent_state_dir(user_id: &UserId) -> PathBuf {
    app_data_dir()
        .join(user_id_to_file_name(user_id))
        .join("persistent_state")
}

/// Returns the path to the session file for the given user.
pub fn session_file_path(user_id: &UserId) -> PathBuf {
    persistent_state_dir(user_id).join("session")
}

const LATEST_USER_ID_FILE_NAME: &str = "latest_user_id.txt";

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
    tokio::fs::write(
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
    let serialized_session = tokio::fs::read_to_string(session_file).await?;
    let FullSessionPersisted { client_session, user_session, sync_token, sliding_sync_version } =
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
        .homeserver_url(client_session.homeserver)
        .sqlite_store(client_session.db_path, Some(&client_session.passphrase))
        .handle_refresh_tokens()
        .build()
        .await?;
    let sliding_sync_version = sliding_sync_version.into();
    client.set_sliding_sync_version(sliding_sync_version);
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
    let sliding_sync_version = client.sliding_sync_version().into();
    // Save that user's session.
    let session_file = session_file_path(&user_session.meta.user_id);
    let serialized_session = serde_json::to_string(&FullSessionPersisted {
        client_session,
        user_session,
        sync_token: None,
        sliding_sync_version
    })?;
    if let Some(parent) = session_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&session_file, serialized_session).await?;

    log!("Session persisted to: {}", session_file.display());
    Ok(())
}

/// Remove the LATEST_USER_ID_FILE_NAME file if it exists
/// 
/// Returns:
/// - Ok(true) if file was found and deleted
/// - Ok(false) if file didn't exist
/// - Err if deletion failed
pub async fn delete_latest_user_id() -> anyhow::Result<bool> {
    let last_login_path = app_data_dir().join(LATEST_USER_ID_FILE_NAME);
    
    if last_login_path.exists() {
        tokio::fs::remove_file(&last_login_path).await
            .map_err(|e| anyhow::anyhow!("Failed to remove latest user file: {e}"))
            .map(|_| true)
    } else {
        Ok(false)
    }
}
