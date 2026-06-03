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
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientSessionPersisted {
    /// The URL of the homeserver of the user.
    pub homeserver: String,

    /// The database path. New sessions store this as a relative subfolder
    /// (joined with `app_data_dir()` at restore time); legacy sessions
    /// may have an absolute path. `restore_session` handles both.
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

/// Resolves the path that `restore_session()` would actually open.
fn resolve_db_path(stored: PathBuf) -> PathBuf {
    if !stored.is_absolute() {
        return app_data_dir().join(stored);
    }
    if stored.exists() {
        return stored;
    }
    let Some(name) = stored.file_name() else {
        return stored;
    };
    // iOS sandbox UUID changes across reinstalls; the absolute path
    // baked into the session is now stale. Use the basename instead.
    app_data_dir().join(name)
}

/// Returns the set of `db` paths referenced by any saved session file.
///
/// This basically scans every saved user session dir, not just the most recent one,
/// to help ensure that db dirs don't get orphaned on the filesystem forever.
async fn collect_referenced_db_paths() -> std::collections::HashSet<PathBuf> {
    use std::collections::HashSet;
    let mut paths = HashSet::new();
    let data_dir = app_data_dir();

    let Ok(mut entries) = tokio::fs::read_dir(data_dir).await else {
        return paths;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("db_") {
            continue;
        }
        let session_file = path.join("persistent_state").join("session");
        let Ok(bytes) = tokio::fs::read(&session_file).await else {
            continue;
        };
        let session: FullSessionPersisted = match serde_json::from_slice(&bytes) {
            Ok(s) => s,
            Err(e) => {
                log!("collect_referenced_db_paths: skipping unparsable session file {}: {e}",
                    session_file.display(),
                );
                continue;
            }
        };
        paths.insert(resolve_db_path(session.client_session.db_path));
    }

    paths
}

/// Deletes `db_*` subdirs not referenced by any saved session. Only touches
/// entries that match the `db_*` prefix and that came from
/// `read_dir(app_data_dir())`, so it can't escape the data dir even with a
/// malicious session file.
pub async fn cleanup_orphan_db_dirs() {
    let data_dir = app_data_dir();
    let active = collect_referenced_db_paths().await;

    let mut entries = match tokio::fs::read_dir(data_dir).await {
        Ok(e) => e,
        Err(e) => {
            log!("cleanup_orphan_db_dirs: could not read data dir {}: {e}", data_dir.display());
            return;
        }
    };

    let mut deleted = 0usize;
    let mut bytes_freed = 0u64;
    let mut kept = 0usize;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("db_") {
            continue;
        }
        if active.contains(&path) {
            kept += 1;
            // log!("cleanup_orphan_db_dirs: preserving referenced db dir: {}", path.display());
            continue;
        }
        let size = dir_size_bytes(&path).await.unwrap_or(0);
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => {
                deleted += 1;
                bytes_freed += size;
                log!(
                    "cleanup_orphan_db_dirs: deleted orphaned db dir ({} bytes): {}",
                    size,
                    path.display(),
                );
            }
            Err(e) => {
                log!(
                    "cleanup_orphan_db_dirs: failed to delete {}: {e}",
                    path.display(),
                );
            }
        }
    }

    if deleted > 0 || kept > 0 {
        log!(
            "cleanup_orphan_db_dirs: deleted {deleted} orphan(s), freed {bytes_freed} bytes; kept {kept} active referenced",
        );
    }
}

/// Recursive size sum, best-effort. Just for the cleanup log line.
async fn dir_size_bytes(path: &std::path::Path) -> Option<u64> {
    let mut total = 0u64;
    let mut entries = tokio::fs::read_dir(path).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(md) = entry.metadata().await else {
            continue;
        };
        if md.is_file() {
            total = total.saturating_add(md.len());
        } else if md.is_dir() {
            // matrix-sdk-sqlite doesn't nest subdirectories, but be safe.
            if let Some(sub) = Box::pin(dir_size_bytes(&entry.path())).await {
                total = total.saturating_add(sub);
            }
        }
    }
    Some(total)
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
    let user_id = if let Some(user_id) = user_id {
        Some(user_id)
    } else {
        most_recent_user_id().await
    };

    let Some(user_id) = user_id else {
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
        "Loaded session file for:\n{user_id}\n\nTrying to connect to homeserver...\n{}",
        client_session.homeserver,
    );
    log!("{status_str}");
    Cx::post_action(LoginAction::Status {
        title: "Connecting to homeserver".into(),
        status: status_str,
    });
    let original_stored = client_session.db_path.clone();
    let db_path = resolve_db_path(client_session.db_path);
    if db_path != original_stored {
        log!(
            "Stored db_path '{}' relocated to '{}'",
            original_stored.display(),
            db_path.display(),
        );
    }
    log!(
        "Restoring session for {user_id} with db at: {} (stored as: {})",
        db_path.display(),
        original_stored.display(),
    );
    let store_config = crate::sliding_sync::build_sqlite_store_config(&db_path, &client_session.passphrase);
    // Build the client with the previous settings from the session.
    let client = Client::builder()
        .homeserver_url(client_session.homeserver)
        .sqlite_store_with_config_and_cache_path(store_config, None::<&std::path::Path>)
        .with_threading_support(matrix_sdk::ThreadingSupport::Enabled {
            with_subscriptions: true,
        })
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
