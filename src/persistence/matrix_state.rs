//! Handles app persistence by saving and restoring client session data to/from the filesystem.

use std::{
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use anyhow::{anyhow, bail};
use makepad_widgets::{log, warning, Cx};
use matrix_sdk::{
    authentication::{AuthSession, matrix::MatrixSession, oauth::{ClientId, OAuthSession, UserSession}},
    ruma::{OwnedUserId, UserId, api::client::error::ErrorKind},
    sliding_sync,
    Client,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_data_dir,
    login::login_screen::LoginAction,
};

/// The data needed to re-build a client.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientSessionPersisted {
    /// The URL of the homeserver of the user.
    pub homeserver: String,

    /// The path of the database.
    pub db_path: PathBuf,

    /// The passphrase of the database.
    pub passphrase: String,
}

impl std::fmt::Debug for ClientSessionPersisted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientSessionPersisted")
            .field("homeserver", &self.homeserver)
            .field("db_path", &self.db_path)
            .field("passphrase", &"<REDACTED>")
            .finish()
    }
}

/// The full session to persist.
#[derive(Debug, Serialize, Deserialize)]
pub struct FullSessionPersisted {
    /// The data to re-build the client.
    pub client_session: ClientSessionPersisted,

    /// The persisted auth session.
    pub user_session: PersistedAuthSession,

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

/// Persisted OAuth session payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedOAuthSession {
    pub client_id: String,
    pub user_session: UserSession,
}

/// Persisted auth session, backward-compatible with old matrix-only files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PersistedAuthSession {
    Matrix(MatrixSession),
    OAuth(PersistedOAuthSession),
}

impl PersistedAuthSession {
    fn user_id(&self) -> &UserId {
        match self {
            PersistedAuthSession::Matrix(session) => session.meta.user_id.as_ref(),
            PersistedAuthSession::OAuth(session) => session.user_session.meta.user_id.as_ref(),
        }
    }

    fn into_auth_session(self) -> AuthSession {
        match self {
            PersistedAuthSession::Matrix(session) => AuthSession::Matrix(session),
            PersistedAuthSession::OAuth(session) => AuthSession::OAuth(Box::new(OAuthSession {
                client_id: ClientId::new(session.client_id),
                user: session.user_session,
            })),
        }
    }
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

#[derive(Debug)]
pub enum RestoreSessionError {
    NoLatestUserId,
    MissingSessionFile {
        user_id: OwnedUserId,
    },
    ReadSessionFile {
        user_id: OwnedUserId,
        path: PathBuf,
        message: String,
    },
    CorruptSessionFile {
        user_id: OwnedUserId,
        path: PathBuf,
        message: String,
    },
    ClientBuild {
        user_id: OwnedUserId,
        message: String,
    },
    RestoreAuth {
        user_id: OwnedUserId,
        message: String,
    },
    InvalidToken {
        user_id: OwnedUserId,
        message: String,
    },
    SaveLatestUserId {
        user_id: OwnedUserId,
        message: String,
    },
}

impl RestoreSessionError {
    pub fn user_id(&self) -> Option<&UserId> {
        match self {
            RestoreSessionError::NoLatestUserId => None,
            RestoreSessionError::MissingSessionFile { user_id }
            | RestoreSessionError::ReadSessionFile { user_id, .. }
            | RestoreSessionError::CorruptSessionFile { user_id, .. }
            | RestoreSessionError::ClientBuild { user_id, .. }
            | RestoreSessionError::RestoreAuth { user_id, .. }
            | RestoreSessionError::InvalidToken { user_id, .. }
            | RestoreSessionError::SaveLatestUserId { user_id, .. } => Some(user_id.as_ref()),
        }
    }
}

impl fmt::Display for RestoreSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RestoreSessionError::NoLatestUserId => write!(f, "could not find previous latest User ID"),
            RestoreSessionError::MissingSessionFile { user_id } => {
                write!(f, "could not find previous session file for {user_id}")
            }
            RestoreSessionError::ReadSessionFile { path, message, .. } => {
                write!(f, "failed to read session file {}: {message}", path.display())
            }
            RestoreSessionError::CorruptSessionFile { path, message, .. } => {
                write!(f, "failed to parse session file {}: {message}", path.display())
            }
            RestoreSessionError::ClientBuild { user_id, message } => {
                write!(f, "failed to build Matrix client for {user_id}: {message}")
            }
            RestoreSessionError::RestoreAuth { user_id, message } => {
                write!(f, "failed to restore Matrix auth session for {user_id}: {message}")
            }
            RestoreSessionError::InvalidToken { user_id, message } => {
                write!(f, "saved Matrix token for {user_id} is invalid: {message}")
            }
            RestoreSessionError::SaveLatestUserId { user_id, message } => {
                write!(f, "failed to save latest user id for {user_id}: {message}")
            }
        }
    }
}

impl std::error::Error for RestoreSessionError {}

/// Returns the user ID of the most recently-logged in user session.
pub async fn most_recent_user_id() -> Option<OwnedUserId> {
    tokio::fs::read_to_string(
        app_data_dir().join(LATEST_USER_ID_FILE_NAME)
    )
    .await
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

pub async fn delete_latest_user_id_if_matches(user_id: &UserId) -> anyhow::Result<bool> {
    delete_latest_user_id_if_matches_path(
        &app_data_dir().join(LATEST_USER_ID_FILE_NAME),
        user_id,
    )
    .await
}

pub async fn delete_latest_user_id_if_matches_path(
    latest_user_id_path: &Path,
    user_id: &UserId,
) -> anyhow::Result<bool> {
    let latest_user_id = match tokio::fs::read_to_string(latest_user_id_path).await {
        Ok(latest_user_id) => latest_user_id,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(anyhow!("Failed to read latest user file: {e}")),
    };

    let latest_user_id: Result<OwnedUserId, _> = latest_user_id.trim().try_into();
    let Ok(latest_user_id) = latest_user_id else {
        return Ok(false);
    };
    if latest_user_id.as_str() != user_id.as_str() {
        return Ok(false);
    }

    tokio::fs::remove_file(latest_user_id_path).await
        .map_err(|e| anyhow!("Failed to remove latest user file: {e}"))?;
    Ok(true)
}

pub async fn archive_bad_session_file(session_file: &Path) -> anyhow::Result<PathBuf> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let file_name = session_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session");
    let archive_path = session_file.with_file_name(format!("{file_name}.bad.{suffix}"));
    tokio::fs::rename(session_file, &archive_path).await
        .map_err(|e| anyhow!("Failed to archive corrupt session file {}: {e}", session_file.display()))?;
    Ok(archive_path)
}


/// Restores the given user's previous session from the filesystem.
///
/// If no User ID is specified, the ID of the most recently-logged in user
/// is retrieved from the filesystem.
pub async fn restore_session(
    user_id: Option<OwnedUserId>
) -> Result<(Client, Option<String>, ClientSessionPersisted), RestoreSessionError> {
    let user_id = if let Some(user_id) = user_id {
        Some(user_id)
    } else {
        most_recent_user_id().await
    };

    let Some(user_id) = user_id else {
        log!("Could not find previous latest User ID");
        return Err(RestoreSessionError::NoLatestUserId);
    };
    let session_file = session_file_path(&user_id);
    if let Err(e) = tokio::fs::metadata(&session_file).await {
        if e.kind() == std::io::ErrorKind::NotFound {
            log!("Could not find previous session file for user {user_id}");
            return Err(RestoreSessionError::MissingSessionFile { user_id });
        }
        return Err(RestoreSessionError::ReadSessionFile {
            user_id,
            path: session_file,
            message: e.to_string(),
        });
    }
    let status_str = format!("Loading previous session file for {user_id}...");
    log!("{status_str}: '{}'", session_file.display());
    Cx::post_action(LoginAction::Status {
        title: "Restoring session".into(),
        status: status_str,
    });

    // The session was serialized as JSON in a file.
    let serialized_session = match tokio::fs::read_to_string(&session_file).await {
        Ok(serialized_session) => serialized_session,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log!("Could not find previous session file for user {user_id}");
            return Err(RestoreSessionError::MissingSessionFile { user_id });
        }
        Err(e) => {
            return Err(RestoreSessionError::ReadSessionFile {
                user_id,
                path: session_file,
                message: e.to_string(),
            });
        }
    };
    let FullSessionPersisted { client_session, user_session, sync_token, sliding_sync_version } =
        serde_json::from_str(&serialized_session)
            .map_err(|e| RestoreSessionError::CorruptSessionFile {
                user_id: user_id.clone(),
                path: session_file.clone(),
                message: e.to_string(),
            })?;

    let status_str = format!(
        "Loaded session file for:\n{user_id}\n\nTrying to connect to homeserver...\n{}",
        client_session.homeserver,
    );
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Connecting to homeserver".into(),
        status: status_str,
    });
    // Build the client with the previous settings from the session.
    let mut client_builder = Client::builder()
        .homeserver_url(client_session.homeserver.clone())
        .sqlite_store(client_session.db_path.clone(), Some(&client_session.passphrase))
        .with_threading_support(matrix_sdk::ThreadingSupport::Enabled {
            with_subscriptions: true,
        })
        .handle_refresh_tokens();
    let saved_proxy = crate::proxy_config::load_saved_proxy_url();
    if let Some(proxy) = saved_proxy.as_deref() {
        if let Err(e) = crate::proxy_config::apply_proxy_to_process_env(Some(proxy)) {
            warning!("Failed to apply proxy env before restoring Matrix session: {e}");
        }
    }
    if let Some(proxy) = saved_proxy {
        client_builder = client_builder.proxy(proxy);
    }
    let client = client_builder.build().await
        .map_err(|e| RestoreSessionError::ClientBuild {
            user_id: user_id.clone(),
            message: e.to_string(),
        })?;
    let sliding_sync_version = sliding_sync_version.into();
    client.set_sliding_sync_version(sliding_sync_version);
    let restored_user_id = user_session.user_id().to_owned();
    let status_str = format!("Authenticating previous login session for {}...", restored_user_id);
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Authenticating session".into(),
        status: status_str,
    });

    client.restore_session(user_session.into_auth_session()).await
        .map_err(|e| {
            let message = e.to_string();
            if matches!(
                e.client_api_error_kind(),
                Some(ErrorKind::UnknownToken { .. } | ErrorKind::MissingToken)
            ) {
                RestoreSessionError::InvalidToken {
                    user_id: restored_user_id.clone(),
                    message,
                }
            } else {
                RestoreSessionError::RestoreAuth {
                    user_id: restored_user_id.clone(),
                    message,
                }
            }
        })?;
    save_latest_user_id(&restored_user_id).await
        .map_err(|e| RestoreSessionError::SaveLatestUserId {
            user_id: restored_user_id.clone(),
            message: e.to_string(),
        })?;

    Ok((client, sync_token, client_session))
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
        .session()
        .ok_or_else(|| anyhow!("A logged-in client should have a session"))?;

    let user_session = match user_session {
        AuthSession::Matrix(session) => PersistedAuthSession::Matrix(session),
        AuthSession::OAuth(session) => PersistedAuthSession::OAuth(PersistedOAuthSession {
            client_id: session.client_id.to_string(),
            user_session: session.user,
        }),
        other => bail!("Unsupported auth session variant for persistence: {other:?}"),
    };

    let persisted_user_id = user_session.user_id().to_owned();
    save_latest_user_id(&persisted_user_id).await?;
    let sliding_sync_version = client.sliding_sync_version().into();
    // Save that user's session.
    let session_file = session_file_path(&persisted_user_id);
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

async fn delete_path_if_exists(path: &Path) -> anyhow::Result<bool> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(anyhow!("Failed to inspect path {}: {e}", path.display())),
    };

    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(|e| anyhow!("Failed to remove directory {}: {e}", path.display()))?;
    } else {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| anyhow!("Failed to remove file {}: {e}", path.display()))?;
    }

    Ok(true)
}

/// Remove the persisted Matrix session file for the given user if it exists.
///
/// Returns:
/// - Ok(true) if the session file was found and deleted
/// - Ok(false) if the session file didn't exist
/// - Err if deletion failed
pub async fn delete_session(user_id: &UserId) -> anyhow::Result<bool> {
    let session_file = session_file_path(user_id);

    if session_file.exists() {
        let persisted_db_path = match tokio::fs::read_to_string(&session_file).await {
            Ok(serialized_session) => {
                match serde_json::from_str::<FullSessionPersisted>(&serialized_session) {
                    Ok(session) => Some(session.client_session.db_path),
                    Err(e) => {
                        warning!(
                            "Failed to parse session file {} before cleanup: {e}",
                            session_file.display()
                        );
                        None
                    }
                }
            }
            Err(e) => {
                warning!(
                    "Failed to read session file {} before cleanup: {e}",
                    session_file.display()
                );
                None
            }
        };

        if let Some(db_path) = persisted_db_path {
            if let Err(e) = delete_path_if_exists(&db_path).await {
                warning!(
                    "Failed to remove persisted Matrix store {} for {user_id}: {e}",
                    db_path.display()
                );
            }
        }

        tokio::fs::remove_file(&session_file)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove session file {session_file:?}: {e}"))
            .map(|_| true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use matrix_sdk::{
        SessionMeta, SessionTokens,
        authentication::oauth::UserSession,
        ruma::{device_id, user_id},
    };

    use super::{
        PersistedAuthSession, PersistedOAuthSession, archive_bad_session_file,
        delete_latest_user_id_if_matches_path,
    };

    #[test]
    fn persisted_auth_session_round_trips_oauth_variant() {
        let persisted = PersistedAuthSession::OAuth(PersistedOAuthSession {
            client_id: "client-id".into(),
            user_session: UserSession {
                meta: SessionMeta {
                    user_id: user_id!("@alice:example.org").to_owned(),
                    device_id: device_id!("DEVICEID").to_owned(),
                },
                tokens: SessionTokens {
                    access_token: "access".into(),
                    refresh_token: Some("refresh".into()),
                },
            },
        });

        let json = serde_json::to_string(&persisted).unwrap();
        let restored: PersistedAuthSession = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.user_id().as_str(), "@alice:example.org");
    }

    fn temp_restore_policy_dir(test_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("robrix_restore_policy_{test_name}_{}_{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn restore_session_policy_clears_latest_user_for_missing_session_file() {
        let dir = temp_restore_policy_dir("missing_session");
        let latest_user_id_path = dir.join("latest_user_id.txt");
        tokio::fs::write(&latest_user_id_path, "@alice:example.org").await.unwrap();

        let deleted = delete_latest_user_id_if_matches_path(
            &latest_user_id_path,
            user_id!("@alice:example.org"),
        )
        .await
        .unwrap();

        assert!(deleted);
        assert!(!latest_user_id_path.exists());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn corrupt_session_file_is_archived_and_latest_user_is_cleared() {
        let dir = temp_restore_policy_dir("corrupt_session");
        let latest_user_id_path = dir.join("latest_user_id.txt");
        let session_file = dir.join("session");
        tokio::fs::write(&latest_user_id_path, "@alice:example.org").await.unwrap();
        tokio::fs::write(&session_file, "{not json").await.unwrap();

        let archive_path = archive_bad_session_file(&session_file).await.unwrap();
        let deleted = delete_latest_user_id_if_matches_path(
            &latest_user_id_path,
            user_id!("@alice:example.org"),
        )
        .await
        .unwrap();

        assert!(deleted);
        assert!(!session_file.exists());
        assert!(archive_path.exists());
        assert!(archive_path.file_name().unwrap().to_string_lossy().contains(".bad"));
        assert!(!latest_user_id_path.exists());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
