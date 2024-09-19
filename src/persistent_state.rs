//! Handles app persistence by saving and restoring client session data to/from the filesystem.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};
use anyhow::{anyhow, bail};
use makepad_widgets::{error, log};
use matrix_sdk::{
    config::SyncSettings,
    matrix_auth::MatrixSession,
    ruma::api::client::filter::FilterDefinition,
    Client, Error, LoopCtrl,
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::app_data_dir;

/// The data needed to re-build a client.
#[derive(Debug, Serialize, Deserialize)]
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


pub fn persistent_state_dir() -> PathBuf {
    app_data_dir().join("persistent_state")
}

pub fn session_file_path() -> PathBuf {
    persistent_state_dir().join("session")
}


/// Restores a previous session from the filesystem at the optional path provided.
///
/// If no path is provided (the default recommendation), the path given by [`session_file_path()`] is used.
pub async fn restore_session<P: AsRef<Path>>(
    session_file_path_ref: Option<P>
) -> anyhow::Result<(Client, Option<String>)> {
    let session_file = session_file_path_ref
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or_else(|| session_file_path());

    if !session_file.exists() {
        log!("Could not find previous session file at {}", session_file.display());
        bail!("Could not find previous session file");
    }
    log!("Found existing session at '{}'", session_file.to_string_lossy());

    // The session was serialized as JSON in a file.
    let serialized_session = fs::read_to_string(session_file).await?;
    let FullSessionPersisted { client_session, user_session, sync_token } =
        serde_json::from_str(&serialized_session)?;

    // Build the client with the previous settings from the session.
    let client = Client::builder()
        .homeserver_url(client_session.homeserver)
        .sqlite_store(client_session.db_path, Some(&client_session.passphrase))
        .simplified_sliding_sync(false)
        .build()
        .await?;

    log!("Restoring previous session for {}", user_session.meta.user_id);

    // Restore the Matrix user session.
    client.restore_session(user_session).await?;

    Ok((client, sync_token))
}



/// Persist a logged-in client session to the filesystem for later use.
///
/// TODO: This is not very secure, for simplicity. We should use robius-keychain
///       or `keyring-rs` to storing secrets securely.
///
/// Note that we could also build the user session from the login response.
pub async fn save_session<P: AsRef<Path>>(
    client: &Client,
    client_session: ClientSessionPersisted,
    session_file_path_ref: Option<P>,
) -> anyhow::Result<()> {
    let user_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| anyhow!("A logged-in client should have a session"))?;

    let serialized_session = serde_json::to_string(&FullSessionPersisted {
        client_session,
        user_session,
        sync_token: None,
    })?;
    let session_file = session_file_path_ref
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or_else(|| session_file_path());
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


/// Setup the client to listen to new messages.
async fn sync(
    client: Client,
    initial_sync_token: Option<String>,
    session_file: &Path,
) -> anyhow::Result<()> {
    log!("Launching a first sync to ignore past messages...");

    // Enable room members lazy-loading, it will speed up the initial sync a lot
    // with accounts in lots of rooms.
    // See <https://spec.matrix.org/v1.6/client-server-api/#lazy-loading-room-members>.
    let filter = FilterDefinition::with_lazy_loading();

    let mut sync_settings = SyncSettings::default().filter(filter.into());

    // We restore the sync where we left.
    // This is not necessary when not using `sync_once`. The other sync methods get
    // the sync token from the store.
    if let Some(sync_token) = initial_sync_token {
        sync_settings = sync_settings.token(sync_token);
    }

    // Let's ignore messages before the program was launched.
    // This is a loop in case the initial sync is longer than our timeout. The
    // server should cache the response and it will ultimately take less time to
    // receive.
    loop {
        match client.sync_once(sync_settings.clone()).await {
            Ok(response) => {
                // This is the last time we need to provide this token, the sync method after
                // will handle it on its own.
                sync_settings = sync_settings.token(response.next_batch.clone());
                persist_sync_token(session_file, response.next_batch).await?;
                break;
            }
            Err(error) => {
                error!("An error occurred during initial sync: {error}");
                error!("Trying again...");
            }
        }
    }

    log!("The client is ready! Listening to new messages...");

    // This loops until we kill the program or an error happens.
    client
        .sync_with_result_callback(sync_settings, |sync_result| async move {
            let response = sync_result?;

            // We persist the token each time to be able to restore our session
            persist_sync_token(session_file, response.next_batch)
                .await
                .map_err(|err| Error::UnknownError(err.into()))?;

            Ok(LoopCtrl::Continue)
        })
        .await?;

    Ok(())
}


/// Persist the sync token for a future session.
/// Note that this is needed only when using `sync_once`. Other sync methods get
/// the sync token from the store.
async fn persist_sync_token(session_file: &Path, sync_token: String) -> anyhow::Result<()> {
    let serialized_session = fs::read_to_string(session_file).await?;
    let mut full_session: FullSessionPersisted = serde_json::from_str(&serialized_session)?;

    full_session.sync_token = Some(sync_token);
    let serialized_session = serde_json::to_string(&full_session)?;
    fs::write(session_file, serialized_session).await?;

    Ok(())
}
