use anyhow::{anyhow, bail, Result};
use bitflags::bitflags;
use clap::Parser;
use eyeball::Subscriber;
use eyeball_im::VectorDiff;
use futures_util::{future::join_all, pin_mut, StreamExt};
use imbl::Vector;
use makepad_widgets::{error, log, warning, Cx, SignalToUI};
use mime::{IMAGE_JPEG, IMAGE_PNG};
use matrix_sdk_base::crypto::{DecryptionSettings, TrustRequirement};
use matrix_sdk::{
    config::RequestConfig, encryption::EncryptionSettings, event_handler::EventHandlerDropGuard, media::MediaRequestParameters, room::{edit::EditedContent, reply::Reply, IncludeRelations, ListThreadsOptions, RelationsOptions, RoomMember, RoomMemberRole}, ruma::{
        api::{Direction, client::{
            account::register::v3::Request as RegistrationRequest,
            room::{Visibility, create_room::v3::{Request as CreateRoomRequest, RoomPreset}},
            directory::get_public_rooms_filtered,
            error::ErrorKind,
            profile::{AvatarUrl, DisplayName, set_avatar_url},
            receipt::create_receipt::v3::ReceiptType,
            uiaa::{AuthData, AuthType, Dummy},
        }}, directory::{Filter as PublicRoomsFilter, RoomTypeFilter}, events::{
            direct::DirectUserIdentifier,
            relation::RelationType,
            room::{
                encryption::RoomEncryptionEventContent, member::MembershipState, message::RoomMessageEventContent, power_levels::RoomPowerLevels, MediaSource
            },
            space::{child::SpaceChildEventContent, parent::SpaceParentEventContent},
            sticker::StickerEventContent,
            room::ImageInfo,
            AnyMessageLikeEventContent, InitialStateEvent, MessageLikeEventType, StateEventType
        }, EventId, MatrixToUri, MatrixUri, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedMxcUri, OwnedRoomId, OwnedUserId, RoomOrAliasId, UserId, int, uint
    }, sliding_sync::VersionBuilder, Client, ClientBuildError, Error, OwnedServerName, Room, RoomDisplayName, RoomMemberships, RoomState, SessionChange, SuccessorRoom
};
use matrix_sdk_ui::{
    RoomListService, Timeline, encryption_sync_service, room_list_service::{RoomListItem, RoomListLoadingState, SyncIndicator, filters}, sync_service::{self, SyncService}, timeline::{LatestEventValue, RoomExt, TimelineEventItemId, TimelineFocus, TimelineItem, TimelineReadReceiptTracking, TimelineDetails}
};
use robius_open::Uri;
use ruma::{OwnedRoomOrAliasId, RoomId, events::tag::Tags};
use tokio::{
    runtime::Handle,
    sync::{broadcast, mpsc::{Sender, UnboundedReceiver, UnboundedSender}, oneshot, watch, Notify}, task::JoinHandle, time::error::Elapsed,
};
use url::Url;
use std::{borrow::Cow, cmp::{max, min}, future::Future, hash::{BuildHasherDefault, DefaultHasher}, iter::Peekable, ops::{Deref, DerefMut, Not}, path::{ Path, PathBuf }, sync::{Arc, LazyLock, Mutex, atomic::{AtomicBool, Ordering}}, time::Duration};
use std::io;
use hashbrown::{HashMap, HashSet};
use crate::{
    account_manager::{self, Account},
    app::{AppStateAction, RoomFilterRemoteSearchAction}, app_data_dir, avatar_cache::AvatarUpdate, event_preview::{BeforeText, TextPreview, text_preview_of_raw_timeline_event, text_preview_of_timeline_item}, home::{
        add_room::{CreatableSpacesAction, CreateRoomAction, CreateRoomContext, KnockResultAction}, invite_screen::{JoinRoomResultAction, LeaveRoomResultAction}, link_preview::{LinkPreviewData, LinkPreviewDataNonNumeric, LinkPreviewRateLimitResponse}, room_screen::{ActionResponseResultAction, InviteResultAction, ReportRoomResultAction, TimelineUpdate}, rooms_list::{self, InvitedRoomInfo, InviterInfo, JoinedRoomInfo, RoomsListUpdate, build_room_search_text, enqueue_rooms_list_update}, rooms_list_header::RoomsListHeaderAction, tombstone_footer::SuccessorRoomDetails
    }, homeserver::{CapabilityProbeAction, HsCapabilities, IdentityProviderSummary}, login::login_screen::LoginAction, logout::{logout_confirm_modal::LogoutAction, logout_state_machine::{LogoutConfig, is_logout_in_progress, logout_with_state_machine}}, room_preview_cache::{enqueue_room_preview_update, RoomPreviewUpdate}, media_cache::{MediaCacheEntry, MediaCacheEntryRef}, persistence::{self, ClientSessionPersisted, load_app_state, take_skip_app_state_restore_once}, profile::{
        user_profile::UserProfile,
        user_profile_cache::{UserProfileUpdate, enqueue_user_profile_update},
    }, room::{FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction}, shared::{
        avatar::AvatarState, jump_to_bottom_button::UnreadMessageCount, popup_list::{PopupKind, enqueue_popup_notification}
    }, space_service_sync::space_service_loop, utils::{self, AVATAR_THUMBNAIL_FORMAT, RoomNameId, VecDiff, avatar_from_room_name}, verification::add_verification_event_handlers_and_sync_client
};

#[derive(Parser, Default)]
struct Cli {
    /// The user ID to login with.
    #[clap(value_parser)]
    user_id: String,

    /// The password that should be used for the login.
    #[clap(value_parser)]
    password: String,

    /// The homeserver to connect to.
    #[clap(value_parser)]
    homeserver: Option<String>,

    /// Set the proxy that should be used for the connection.
    #[clap(short, long)]
    proxy: Option<String>,

    /// Force login screen.
    #[clap(short, long, action)]
    login_screen: bool,

    /// Enable verbose logging output.
    #[clap(short, long, action)]
    verbose: bool,
}

impl std::fmt::Debug for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cli")
            .field("user_id", &self.user_id)
            .field("password", &"<REDACTED>")
            .field("homeserver", &self.homeserver)
            .field("proxy", &self.proxy)
            .field("login_screen", &self.login_screen)
            .field("verbose", &self.verbose)
            .finish()
    }
}

impl From<LoginByPassword> for Cli {
    fn from(login: LoginByPassword) -> Self {
        Self {
            user_id: login.user_id.trim().to_owned(),
            password: login.password,
            homeserver: login.homeserver
                .map(|homeserver| homeserver.trim().to_owned())
                .filter(|homeserver| !homeserver.is_empty()),
            proxy: login.proxy
                .map(|proxy| proxy.trim().to_owned())
                .filter(|proxy| !proxy.is_empty()),
            login_screen: false,
            verbose: false,
        }
    }
}

impl From<RegisterAccount> for Cli {
    fn from(registration: RegisterAccount) -> Self {
        Self {
            user_id: registration.user_id.trim().to_owned(),
            password: registration.password,
            homeserver: registration.homeserver
                .map(|homeserver| homeserver.trim().to_owned())
                .filter(|homeserver| !homeserver.is_empty()),
            proxy: registration.proxy
                .map(|proxy| proxy.trim().to_owned())
                .filter(|proxy| !proxy.is_empty()),
            login_screen: false,
            verbose: false,
        }
    }
}

fn infer_homeserver_from_user_id(user_id: &str) -> Option<String> {
    let user_id: OwnedUserId = user_id.trim().try_into().ok()?;
    Some(user_id.server_name().to_string())
}

async fn finalize_authenticated_client(
    client: Client,
    client_session: ClientSessionPersisted,
    fallback_user_id: &str,
    is_add_account: bool,
) -> Result<(Client, Option<String>, bool, ClientSessionPersisted)> {
    if client.matrix_auth().logged_in() {
        let logged_in_user_id = client.user_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| fallback_user_id.to_owned());
        log!("Logged in successfully.");
        let status = format!("Logged in as {}.\n → Loading rooms...", logged_in_user_id);
        enqueue_rooms_list_update(RoomsListUpdate::Status { status });
        if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
            let err_msg = format!("Failed to save session state to storage: {e}");
            error!("{err_msg}");
            enqueue_popup_notification(err_msg, PopupKind::Error, None);
        }
        Ok((client, None, is_add_account, client_session))
    } else {
        let err_msg = format!(
            "Authentication succeeded for {fallback_user_id}, but the homeserver did not return a login session."
        );
        enqueue_popup_notification(err_msg.clone(), PopupKind::Error, None);
        enqueue_rooms_list_update(RoomsListUpdate::Status { status: err_msg.clone() });
        bail!(err_msg);
    }
}

fn registration_localpart(user_id: &str) -> Result<String> {
    let trimmed = user_id.trim();
    if trimmed.is_empty() {
        bail!("Please enter a valid username or Matrix user ID.");
    }

    if let Ok(full_user_id) = <OwnedUserId as TryFrom<&str>>::try_from(trimmed) {
        return Ok(full_user_id.localpart().to_owned());
    }

    let localpart = trimmed.trim_start_matches('@');
    if localpart.is_empty() || localpart.contains(':') || localpart.chars().any(char::is_whitespace) {
        bail!("Please enter a valid username or full Matrix user ID.");
    }

    Ok(localpart.to_owned())
}

fn registration_request(
    username: &str,
    password: &str,
    session: Option<String>,
) -> RegistrationRequest {
    let mut request = RegistrationRequest::new();
    request.username = Some(username.to_owned());
    request.password = Some(password.to_owned());
    request.initial_device_display_name = Some("robrix-un-pw".to_owned());
    request.refresh_token = true;
    if let Some(session) = session {
        let mut dummy = Dummy::new();
        dummy.session = Some(session);
        request.auth = Some(AuthData::Dummy(dummy));
    }
    request
}

fn registration_uiaa_error_message(error: &matrix_sdk::Error) -> String {
    if let matrix_sdk::Error::Http(http_error) = error {
        match http_error.client_api_error_kind() {
            Some(ErrorKind::UserInUse) => {
                return "That user ID is already taken. Please choose another one.".to_owned();
            }
            Some(ErrorKind::InvalidUsername) => {
                return "That user ID is invalid. Use a username like `alice` or a full Matrix ID like `@alice:matrix.org`.".to_owned();
            }
            Some(ErrorKind::WeakPassword) => {
                return "That password is too weak. Please choose a stronger password.".to_owned();
            }
            Some(ErrorKind::Forbidden) => {
                return "This homeserver does not allow open registration.".to_owned();
            }
            Some(ErrorKind::LimitExceeded { .. }) => {
                return "The homeserver is rate limiting account creation right now. Please try again shortly.".to_owned();
            }
            _ => {}
        }
    }

    format!("Could not create account: {error}")
}

fn unsupported_registration_flow_message(
    flows: &[matrix_sdk::ruma::api::client::uiaa::AuthFlow],
) -> String {
    let supports_registration_token = flows.iter().any(|flow| {
        flow.stages
            .iter()
            .any(|stage| matches!(stage, AuthType::RegistrationToken))
    });
    if supports_registration_token {
        return "This homeserver requires a registration token. Robrix does not support token-based registration yet.".to_owned();
    }

    let supports_terms = flows.iter().any(|flow| {
        flow.stages
            .iter()
            .any(|stage| matches!(stage, AuthType::Terms))
    });
    if supports_terms {
        return "This homeserver requires an interactive terms-of-service step. Robrix does not support that registration flow yet.".to_owned();
    }

    "This homeserver requires an unsupported registration flow. Please try another homeserver or register with a different client.".to_owned()
}

async fn clear_persisted_session(user_id: Option<&UserId>) {
    let Some(user_id) = user_id else {
        return;
    };

    if let Err(e) = persistence::delete_session(user_id).await {
        warning!("Failed to delete persisted session for {user_id}: {e}");
    }

    let latest_user_id = persistence::most_recent_user_id().await;
    if latest_user_id.as_deref() == Some(user_id) {
        if let Err(e) = persistence::delete_latest_user_id().await {
            warning!("Failed to delete latest user id for {user_id}: {e}");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreSessionFailureAction {
    Preserve,
    DeleteLatestUserId,
    ArchiveBadSessionAndDeleteLatestUserId,
    ClearPersistedSession,
}

fn restore_session_failure_action(error: &persistence::RestoreSessionError) -> RestoreSessionFailureAction {
    match error {
        persistence::RestoreSessionError::MissingSessionFile { .. } => {
            RestoreSessionFailureAction::DeleteLatestUserId
        }
        persistence::RestoreSessionError::CorruptSessionFile { .. } => {
            RestoreSessionFailureAction::ArchiveBadSessionAndDeleteLatestUserId
        }
        persistence::RestoreSessionError::InvalidToken { .. } => {
            RestoreSessionFailureAction::ClearPersistedSession
        }
        persistence::RestoreSessionError::NoLatestUserId
        | persistence::RestoreSessionError::ReadSessionFile { .. }
        | persistence::RestoreSessionError::ClientBuild { .. }
        | persistence::RestoreSessionError::RestoreAuth { .. }
        | persistence::RestoreSessionError::SaveLatestUserId { .. } => {
            RestoreSessionFailureAction::Preserve
        }
    }
}

fn session_validation_failure_action(is_invalid_token: bool) -> RestoreSessionFailureAction {
    if is_invalid_token {
        RestoreSessionFailureAction::ClearPersistedSession
    } else {
        RestoreSessionFailureAction::Preserve
    }
}

fn restore_session_failure_message(error: &persistence::RestoreSessionError) -> String {
    match restore_session_failure_action(error) {
        RestoreSessionFailureAction::ClearPersistedSession => {
            "Your login token is no longer valid.\n\nPlease log in again.".to_owned()
        }
        RestoreSessionFailureAction::DeleteLatestUserId => {
            "Could not find the saved session file.\n\nPlease log in again.".to_owned()
        }
        RestoreSessionFailureAction::ArchiveBadSessionAndDeleteLatestUserId => {
            "The saved session file is corrupted and was archived.\n\nPlease log in again.".to_owned()
        }
        RestoreSessionFailureAction::Preserve => {
            let detail = if matches!(error, persistence::RestoreSessionError::SaveLatestUserId { .. }) {
                "Robrix restored the session but could not update the latest user pointer."
            } else {
                "Robrix kept your saved session so it can try again after the server or network issue is fixed."
            };
            format!("Could not restore previous user session.\n\n{detail}\n\nError: {error}")
        }
    }
}

async fn apply_restore_session_failure_policy(error: &persistence::RestoreSessionError) {
    match restore_session_failure_action(error) {
        RestoreSessionFailureAction::Preserve => {}
        RestoreSessionFailureAction::DeleteLatestUserId => {
            if let Some(user_id) = error.user_id() {
                if let Err(e) = persistence::delete_latest_user_id_if_matches(user_id).await {
                    warning!("Failed to delete stale latest user id for {user_id}: {e}");
                }
            }
        }
        RestoreSessionFailureAction::ArchiveBadSessionAndDeleteLatestUserId => {
            if let persistence::RestoreSessionError::CorruptSessionFile { user_id, path, .. } = error {
                if let Err(e) = persistence::archive_bad_session_file(path).await {
                    warning!("Failed to archive corrupt session file for {user_id}: {e}");
                }
                if let Err(e) = persistence::delete_latest_user_id_if_matches(user_id).await {
                    warning!("Failed to delete latest user id for corrupt session {user_id}: {e}");
                }
            }
        }
        RestoreSessionFailureAction::ClearPersistedSession => {
            clear_persisted_session(error.user_id()).await;
        }
    }
}

enum SessionResetAction {
    Reauthenticate { message: String },
}

async fn reset_runtime_state_for_relogin() {
    let sync_service = { SYNC_SERVICE.lock().unwrap().take() };
    if let Some(sync_service) = sync_service {
        sync_service.stop().await;
    }

    CLIENT.lock().unwrap().take();
    DEFAULT_SSO_CLIENT.lock().unwrap().take();
    IGNORED_USERS.lock().unwrap().clear();
    ALL_JOINED_ROOMS.lock().unwrap().clear();

    let on_clear_appstate = Arc::new(Notify::new());
    Cx::post_action(LogoutAction::ClearAppState { on_clear_appstate: on_clear_appstate.clone() });

    if tokio::time::timeout(Duration::from_secs(5), on_clear_appstate.notified()).await.is_err() {
        warning!("Timed out waiting for UI-side app state cleanup during re-login reset");
    }
}

fn is_invalid_token_http_error(error: &matrix_sdk::HttpError) -> bool {
    matches!(
        error.client_api_error_kind(),
        Some(ErrorKind::UnknownToken { .. } | ErrorKind::MissingToken)
    )
}

fn is_invalid_batch_token_timeline_error(error: &matrix_sdk_ui::timeline::Error) -> bool {
    let error_text = error.to_string().to_ascii_lowercase();
    error_text.contains("invalid batch token")
        || error_text.contains("must start with 's' or 't'")
}

fn is_thread_unknown_parent_timeline_error(error: &matrix_sdk_ui::timeline::Error) -> bool {
    let error_text = error.to_string().to_ascii_lowercase();
    error_text.contains("unknown parent event")
}


/// Build a new client.
async fn build_client(
    cli: &Cli,
    data_dir: &Path,
) -> Result<(Client, ClientSessionPersisted), ClientBuildError> {
    // Generate a unique subfolder name for the client database,
    // which allows multiple clients to run simultaneously.
    let now = chrono::Local::now();
    let db_subfolder_name: String = format!("db_{}", now.format("%F_%H_%M_%S_%f"));
    let db_path = data_dir.join(db_subfolder_name);

    // Generate a random passphrase.
    let passphrase: String = {
        use rand::{Rng, thread_rng};
        thread_rng()
            .sample_iter(rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    };

    let inferred_homeserver = infer_homeserver_from_user_id(&cli.user_id);
    let homeserver_url = cli.homeserver.as_deref()
        .filter(|homeserver| !homeserver.trim().is_empty())
        .or(inferred_homeserver.as_deref())
        .unwrap_or("https://matrix-client.matrix.org/");
        // .unwrap_or("https://matrix.org/");

    let mut builder = Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
        // Use a sqlite database to persist the client's encryption setup.
        .sqlite_store(&db_path, Some(&passphrase))
        .with_threading_support(matrix_sdk::ThreadingSupport::Enabled {
            with_subscriptions: true,
        })
        // The sliding sync proxy has now been deprecated in favor of native sliding sync.
        .sliding_sync_version_builder(VersionBuilder::DiscoverNative)
        .with_decryption_settings(DecryptionSettings {
            sender_device_trust_requirement: TrustRequirement::Untrusted,
        })
        .with_encryption_settings(EncryptionSettings {
            auto_enable_cross_signing: true,
            backup_download_strategy: matrix_sdk::encryption::BackupDownloadStrategy::OneShot,
            auto_enable_backups: true,
        })
        .with_enable_share_history_on_invite(true)
        .handle_refresh_tokens();

    let effective_proxy = crate::proxy_config::resolve_effective_proxy_url(cli.proxy.as_deref());
    if let Some(proxy) = effective_proxy.as_deref() {
        if let Err(e) = crate::proxy_config::apply_proxy_to_process_env(Some(proxy)) {
            warning!("Failed to apply proxy env before building Matrix client: {e}");
        }
    }
    if let Some(proxy) = effective_proxy {
        builder = builder.proxy(proxy);
    }

    // Use a 60 second timeout for all requests to the homeserver.
    // Yes, this is a long timeout, but the standard matrix homeserver is often very slow.
    builder = builder.request_config(
        RequestConfig::new()
            .timeout(std::time::Duration::from_secs(60))
    );

    let client = builder.build().await?;
    let homeserver_url =  client.homeserver().to_string();
    Ok((
        client,
        ClientSessionPersisted {
            homeserver: homeserver_url,
            db_path,
            passphrase,
        },
    ))
}

/// Logs in to the given Matrix homeserver using the given username and password.
///
/// This function is used by the login screen to log in to the Matrix server.
///
/// Upon success, this function returns the logged-in client, an optional sync token,
/// a boolean indicating if this is an add-account operation (multi-account mode),
/// and the client session for storing in the account manager.
async fn login(
    cli: &Cli,
    login_request: LoginRequest,
) -> Result<(Client, Option<String>, bool, ClientSessionPersisted)> {
    match login_request {
        LoginRequest::LoginByCli | LoginRequest::LoginByPassword(_) => {
            let (cli, is_add_account) = if let LoginRequest::LoginByPassword(login_by_password) = login_request {
                let is_add_account = login_by_password.is_add_account;
                (&Cli::from(login_by_password), is_add_account)
            } else {
                (cli, false)
            };
            let (client, client_session) = build_client(cli, app_data_dir()).await?;
            Cx::post_action(LoginAction::Status {
                title: "Authenticating".into(),
                status: format!("Logging in as {}...", cli.user_id),
            });
            // Attempt to login using the CLI-provided username & password.
            let login_result = client
                .matrix_auth()
                .login_username(&cli.user_id, &cli.password)
                .initial_device_display_name("robrix-un-pw")
                .send()
                .await?;
            if client.matrix_auth().logged_in() {
                log!("Logged in successfully.");
                let status = format!("Logged in as {}.\n → Loading rooms...", cli.user_id);
                // enqueue_popup_notification(status.clone());
                enqueue_rooms_list_update(RoomsListUpdate::Status { status });
                if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
                    let err_msg = format!("Failed to save session state to storage: {e}");
                    error!("{err_msg}");
                    enqueue_popup_notification(err_msg, PopupKind::Error, None);
                }
            } else {
                let err_msg = format!("Failed to login as {}: {:?}", cli.user_id, login_result);
                enqueue_popup_notification(err_msg.clone(), PopupKind::Error, None);
                enqueue_rooms_list_update(RoomsListUpdate::Status { status: err_msg.clone() });
                bail!(err_msg);
            }
            finalize_authenticated_client(client, client_session, &cli.user_id, is_add_account).await
        }

        LoginRequest::Register(registration) => {
            // This arm drives BOTH signals intentionally:
            //   - LoginAction::Status — a no-op when the login screen isn't visible
            //     (the normal register flow); retained so the login-screen-based
            //     LoginByCli path can still surface progress if ever re-wired.
            //   - RegisterAction::* (dispatched at the failure sites and at the
            //     finalize-success site below) — drives RegisterScreen state.
            let cli = Cli::from(RegisterAccount {
                user_id: registration.user_id.clone(),
                password: registration.password.clone(),
                homeserver: registration.homeserver.clone(),
                proxy: registration.proxy.clone(),
            });
            let localpart = registration_localpart(&registration.user_id)?;
            let (client, client_session) = build_client(&cli, app_data_dir()).await?;
            Cx::post_action(LoginAction::Status {
                title: "Creating account".into(),
                status: format!("Creating account {localpart}..."),
            });

            let auth = client.matrix_auth();
            let initial_request = registration_request(&localpart, &registration.password, None);
            let register_result = match auth.register(initial_request).await {
                Ok(response) => Ok(response),
                Err(error) => {
                    if let Some(uiaa_info) = error.as_uiaa_response() {
                        let supports_dummy = uiaa_info.flows.iter().any(|flow| {
                            flow.stages
                                .iter()
                                .any(|stage| matches!(stage, AuthType::Dummy))
                        });
                        if supports_dummy {
                            Cx::post_action(LoginAction::Status {
                                title: "Completing sign up".into(),
                                status: "Confirming registration with the homeserver...".into(),
                            });
                            auth.register(registration_request(
                                &localpart,
                                &registration.password,
                                uiaa_info.session.clone(),
                            ))
                            .await
                        } else {
                            let msg = unsupported_registration_flow_message(&uiaa_info.flows);
                            Cx::post_action(crate::register::RegisterAction::RegistrationFailed(msg.clone()));
                            bail!(msg);
                        }
                    } else {
                        let msg = registration_uiaa_error_message(&error);
                        Cx::post_action(crate::register::RegisterAction::RegistrationFailed(msg.clone()));
                        bail!(msg);
                    }
                }
            }?;

            if !client.matrix_auth().logged_in() {
                let err_msg = format!(
                    "Account {} was created, but the homeserver did not return a login session. Please log in manually.",
                    register_result.user_id,
                );
                enqueue_popup_notification(err_msg.clone(), PopupKind::Error, None);
                enqueue_rooms_list_update(RoomsListUpdate::Status { status: err_msg.clone() });
                Cx::post_action(crate::register::RegisterAction::RegistrationFailed(err_msg.clone()));
                bail!(err_msg);
            }

            let finalized = finalize_authenticated_client(client, client_session, register_result.user_id.as_str(), false)
                .await;
            if finalized.is_ok() {
                Cx::post_action(crate::register::RegisterAction::RegistrationSuccess);
            }
            finalized
        }

        LoginRequest::LoginBySSOSuccess(client, client_session, is_add_account) => {
            if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
                error!("Failed to save session state to storage: {e:?}");
            }
            Ok((client, None, is_add_account, client_session))
        }
        LoginRequest::LoginByOidcSuccess(client, client_session, is_add_account) => {
            // Mirrors the SSO arm: the OIDC worker already performed
            // finish_login, so the client is fully authenticated. We only
            // need to persist and return — finalize_authenticated_client in
            // the outer loop handles account-manager + rooms-list status.
            if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
                error!("Failed to save session state to storage: {e:?}");
            }
            Ok((client, None, is_add_account, client_session))
        }
        LoginRequest::HomeserverLoginTypesQuery(_) => {
            bail!("LoginRequest::HomeserverLoginTypesQuery not handled earlier");
        }
    }
}

/// Thin wrapper around `build_client` that exposes just what the OIDC worker
/// needs, without leaking the private `Cli` type across module boundaries.
pub(crate) async fn build_client_for_oidc(
    homeserver: Option<String>,
    proxy: Option<String>,
) -> std::result::Result<(Client, ClientSessionPersisted), ClientBuildError> {
    let cli = Cli { homeserver, proxy, ..Default::default() };
    build_client(&cli, app_data_dir()).await
}


/// Which direction to paginate in.
///
/// * `Forwards` will retrieve later events (towards the end of the timeline),
///   which only works if the timeline is *focused* on a specific event.
/// * `Backwards`: the more typical choice, in which earlier events are retrieved
///   (towards the start of the timeline), which works in  both live mode and focused mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaginationDirection {
    Forwards,
    Backwards,
}
impl std::fmt::Display for PaginationDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Forwards => write!(f, "forwards"),
            Self::Backwards => write!(f, "backwards"),
        }
    }
}

/// The function signature for the callback that gets invoked when media is fetched.
pub type OnMediaFetchedFn = fn(
    &Mutex<MediaCacheEntry>,
    MediaRequestParameters,
    matrix_sdk::Result<Vec<u8>>,
    Option<crossbeam_channel::Sender<TimelineUpdate>>,
);

/// Error types for URL preview operations.
#[derive(Debug)]
pub enum UrlPreviewError {
    /// HTTP request failed.
    Request(matrix_sdk::reqwest::Error),
    /// JSON parsing failed.
    Json(serde_json::Error),
    /// Client not available.
    ClientNotAvailable,
    /// Access token not available.
    AccessTokenNotAvailable,
    /// HTTP error status.
    HttpStatus(u16),
    /// URL parsing error.
    UrlParse(url::ParseError),
}

impl std::fmt::Display for UrlPreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrlPreviewError::Request(e) => write!(f, "HTTP request failed: {e}"),
            UrlPreviewError::Json(e) => write!(f, "JSON parsing failed: {}", e),
            UrlPreviewError::ClientNotAvailable => write!(f, "Matrix client not available"),
            UrlPreviewError::AccessTokenNotAvailable => write!(f, "Access token not available"),
            UrlPreviewError::HttpStatus(status) => write!(f, "HTTP {} error", status),
            UrlPreviewError::UrlParse(e) => write!(f, "URL parsing failed: {}", e),
        }
    }
}

impl std::error::Error for UrlPreviewError {}

/// The function signature for the callback that gets invoked when link preview data is fetched.
pub type OnLinkPreviewFetchedFn = fn(
    String,
    Arc<Mutex<crate::home::link_preview::TimestampedCacheEntry>>,
    Result<LinkPreviewData, UrlPreviewError>,
    Option<crossbeam_channel::Sender<TimelineUpdate>>,
);


/// Actions emitted in response to a [`MatrixRequest::GenerateMatrixLink`].
#[derive(Clone, Debug)]
pub enum MatrixLinkAction {
    MatrixToUri(MatrixToUri),
    MatrixUri(MatrixUri),
    Error(String),
}

/// Actions emitted when account data (e.g., avatar, display name) changes.
#[derive(Clone, Debug)]
pub enum AccountDataAction {
    /// The user's avatar was successfully updated or removed.
    AvatarChanged(Option<OwnedMxcUri>),
    /// Failed to update or remove the user's avatar.
    AvatarChangeFailed(String),
    /// The user's display name was successfully updated or removed.
    DisplayNameChanged(Option<String>),
    /// Failed to update the user's display name.
    DisplayNameChangeFailed(String),
    /// Result of [`MatrixRequest::GetOwnDevice`].
    /// * `None` if not logged in or the crypto store isn't ready yet.
    OwnDeviceFetched(Option<OwnDeviceInfo>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnDeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
}

/// Actions emitted in response to account switching.
#[derive(Debug, Clone)]
pub enum AccountSwitchAction {
    /// Account switch is starting - UI should show loading state.
    Starting(OwnedUserId),
    /// Successfully switched to a different account.
    Switched(OwnedUserId),
    /// Failed to switch accounts.
    Failed(String),
}

/// Actions emitted in response to a [`MatrixRequest::OpenOrCreateDirectMessage`].
#[derive(Debug)]
pub enum DirectMessageRoomAction {
    /// A direct message room already existed with the given user.
    FoundExisting {
        user_id: OwnedUserId,
        room_name_id: RoomNameId,
    },
    /// A direct message room didn't exist, and we didn't attempt to create a new one.
    DidNotExist {
        user_profile: UserProfile,
    },
    /// A direct message room didn't exist, but we successfully created a new one.
    NewlyCreated {
        user_profile: UserProfile,
        room_name_id: RoomNameId,
    },
    /// A direct message room didn't exist, and we failed to create a new one.
    FailedToCreate {
        user_profile: UserProfile,
        error: matrix_sdk::Error,
    },
}

#[derive(Clone, Debug)]
pub struct FetchedRoomThread {
    pub thread_root_event_id: OwnedEventId,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    pub title: String,
    pub reply_count: u32,
    pub latest_reply_preview: Option<String>,
}

#[derive(Clone, Debug)]
pub enum RoomThreadsAction {
    Loaded {
        room_id: OwnedRoomId,
        from: Option<String>,
        threads: Vec<FetchedRoomThread>,
        prev_batch_token: Option<String>,
    },
    Failed {
        room_id: OwnedRoomId,
        from: Option<String>,
        error: String,
    },
}

/// Either a main room timeline or a thread-focused timeline.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TimelineKind {
    MainRoom {
        room_id: OwnedRoomId,
    },
    Thread {
        room_id: OwnedRoomId,
        thread_root_event_id: OwnedEventId,
    },
}
impl TimelineKind {
    pub fn room_id(&self) -> &OwnedRoomId {
        match self {
            TimelineKind::MainRoom { room_id } => room_id,
            TimelineKind::Thread { room_id, .. } => room_id,
        }
    }

    pub fn thread_root_event_id(&self) -> Option<&OwnedEventId> {
        match self {
            TimelineKind::MainRoom { .. } => None,
            TimelineKind::Thread { thread_root_event_id, .. } => Some(thread_root_event_id),
        }
    }
}
impl std::fmt::Display for TimelineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimelineKind::MainRoom { room_id } => write!(f, "MainRoom({})", room_id),
            TimelineKind::Thread { room_id, thread_root_event_id } => {
                write!(f, "Thread({}, {})", room_id, thread_root_event_id)
            }
        }
    }
}

/// How the worker should deliver the result of a [`MatrixRequest::GetRoomPreview`].
#[derive(Clone, Debug)]
pub enum RoomPreviewResponseMode {
    /// Posts a [`RoomPreviewAction::Fetched`] action with the result, success
    /// or error. Used by interactive flows like the "join room" UI.
    Action,
    /// Stores the result in the [`crate::room_preview_cache`] on success;
    /// logs and drops on error (the cache entry stays `Requested` until
    /// `clear_all_pending_requests()` is called on offline→online recovery).
    /// Used by `RobrixHtmlLink` pills.
    RoomPreviewCache,
}

/// The set of requests for async work that can be made to the worker thread.
#[allow(clippy::large_enum_variant)]
pub enum MatrixRequest {
    /// Request from the login screen to log in with the given credentials.
    Login(LoginRequest),
    /// Request the currently-authenticated user's access token for copying to
    /// external Matrix client integrations such as Hermes/OpenClaw.
    GetAccessTokenForCopy,
    /// Load the user's sticker pack catalog by talking to the public scalar
    /// widgets API (`integrations.element.io`). On success posts a
    /// [`crate::home::sticker_modal::StickerCatalogAction::Ready`] action with
    /// the parsed pack list; on failure posts the `Failed` variant.
    LoadStickerCatalog,
    /// Enable or disable a single sticker pack on the scalar widgets API.
    /// Fired by the per-row `ToggleFlat` in the sticker modal.
    SetStickerPackState {
        /// The pack's wire identifier (`Asset::asset_type` on the wire,
        /// e.g. `"isabella"`).
        asset_type: String,
        /// `true` → enable, `false` → disable.
        enable: bool,
    },
    /// Fetch image bytes for the individual stickers in one pack.
    /// Fired when the user taps the "▸" drill-down button for an active pack.
    /// On success posts [`crate::home::sticker_modal::StickerGridAction::Ready`];
    /// on failure posts the `Failed` variant.
    LoadPackStickers {
        pack_id: String,
        pack_name: String,
        /// `(mxc_url, https_url, body)` tuples for each sticker in the pack.
        sticker_infos: Vec<(String, String, String)>,
    },
    /// Probe a homeserver's registration capabilities.
    /// Sent from RegisterScreen's Next button; result arrives via
    /// `CapabilityProbeAction::Discovered` / `Failed`.
    DiscoverHomeserverCapabilities {
        /// Already-normalized homeserver URL (has scheme, no trailing slash).
        url: String,
        /// Optional proxy override from the login screen. Falls back to the
        /// saved global proxy when omitted.
        proxy: Option<String>,
    },
    /// Begin the OIDC (MAS) login flow for an already-existing account on a
    /// MAS-delegated homeserver. `homeserver_url` is the normalized URL from
    /// capability discovery; `proxy` mirrors the password login's optional
    /// per-request proxy override.
    ///
    /// Outcome dispatch:
    ///   - `LoginAction::OidcLoginStarted` fires once the loopback server is
    ///     live and the system browser has been opened.
    ///   - On success, `LoginAction::LoginSuccess` fires after
    ///     `finalize_authenticated_client()` persists the session.
    ///   - Cancellation (in-app Cancel, browser `error=access_denied`, or
    ///     3-min timeout) dispatches `LoginAction::OidcLoginCancelled`.
    ///   - Any other failure dispatches `LoginAction::OidcLoginFailed(msg)`.
    StartOidcLogin {
        homeserver_url: String,
        proxy: Option<String>,
        is_add_account: bool,
    },
    /// Abort the in-flight OIDC login. Posted by LoginScreen's Cancel button.
    /// No-op if no OIDC login is currently in flight.
    CancelOidcLogin,
    /// Register a new account on a UIAA server using the single-stage
    /// `m.login.dummy` flow. `homeserver_url` is the already-normalized URL
    /// from capability discovery.
    ///
    /// Success dispatches two actions in sequence:
    ///   1. `RegisterAction::RegistrationSuccess` — fires immediately after
    ///      `finalize_authenticated_client` persists the session. The
    ///      RegisterScreen uses this to clear form state and stop showing
    ///      the submission spinner.
    ///   2. `LoginAction::LoginSuccess` — fires ~100-200ms later after the
    ///      sync service finishes building. App.rs uses this to navigate
    ///      from the register screen to the main UI, mirroring the login
    ///      path exactly.
    ///
    /// Any failure dispatches a single `RegisterAction::RegistrationFailed(msg)`
    /// with a user-displayable message.
    ///
    /// Proxy support is Phase 5 scope; this variant always uses the process
    /// default proxy (if any) rather than a per-request override.
    RegisterViaUiaa {
        username: String,
        password: String,
        homeserver_url: String,
    },
    /// Request to switch to a different logged-in account.
    SwitchAccount {
        user_id: OwnedUserId,
    },
    /// Request to logout.
    Logout {
        is_desktop: bool,
    },
    /// Request to paginate the older (or newer) events of a room or thread timeline.
    PaginateTimeline {
        timeline_kind: TimelineKind,
        /// The maximum number of timeline events to fetch in each pagination batch.
        num_events: u16,
        direction: PaginationDirection,
    },
    /// Request to edit the content of an event in the given room's timeline.
    EditMessage {
        timeline_kind: TimelineKind,
        timeline_event_item_id: TimelineEventItemId,
        edited_content: EditedContent,
    },
    /// Request to fetch the full details of the given event in the given room's timeline.
    FetchDetailsForEvent {
        timeline_kind: TimelineKind,
        event_id: OwnedEventId,
    },
    /// Request to fetch the latest thread-reply preview and latest reply count
    /// for the given thread root.
    FetchThreadSummaryDetails {
        timeline_kind: TimelineKind,
        thread_root_event_id: OwnedEventId,
        timeline_item_index: usize,
    },
    /// Request to fetch a page of thread roots for the given room.
    ListRoomThreads {
        room_id: OwnedRoomId,
        from: Option<String>,
    },
    /// Request to fetch profile information for all members of a room.
    ///
    /// This can be *very* slow depending on the number of members in the room.
    ///
    /// Even though it operates on a room itself, this accepts a `TimelineKind`
    /// in order to be able to send the fetched room member list to a specific timeline UI.
    SyncRoomMemberList {
        timeline_kind: TimelineKind,
    },
    /// Request to create a thread timeline focused on the given thread root event in the given room.
    CreateThreadTimeline {
        room_id: OwnedRoomId,
        thread_root_event_id: OwnedEventId,
    },
    /// Request to knock on (request an invite to) the given room.
    Knock {
        room_or_alias_id: OwnedRoomOrAliasId,
        reason: Option<String>,
        #[doc(alias("via"))]
        server_names: Vec<OwnedServerName>,
    },
    /// Request to invite the given user to the given room.
    InviteUser {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
    },
    /// Request to bind or unbind the configured botfather for the given room.
    SetRoomBotBinding {
        room_id: OwnedRoomId,
        bound: bool,
        bot_user_id: OwnedUserId,
    },
    /// Request to join the given room.
    JoinRoom {
        room_id: OwnedRoomId,
    },
    /// Request to leave the given room.
    LeaveRoom {
        room_id: OwnedRoomId,
    },
    /// Request to report the given room.
    ReportRoom {
        room_id: OwnedRoomId,
        reason: String,
    },
    /// Request to get the actual list of members in a room.
    ///
    /// This returns the list of members that can be displayed in the UI.
    ///
    /// Even though it operates on a room itself, this accepts a `TimelineKind`
    /// in order to be able to send the fetched room member list to a specific timeline UI.
    GetRoomMembers {
        timeline_kind: TimelineKind,
        memberships: RoomMemberships,
        /// * If `true` (not recommended), only the local cache will be accessed.
        /// * If `false` (recommended), details will be fetched from the server.
        local_only: bool,
    },
    /// Request to fetch the preview (basic info) for the given room,
    /// either one that is joined locally or one that is unknown.
    ///
    /// On completion, the result is dispatched according to `response_mode`:
    /// either as a [`RoomPreviewAction::Fetched`] action, or by enqueueing
    /// a cache update into the [`crate::room_preview_cache`].
    GetRoomPreview {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
        response_mode: RoomPreviewResponseMode,
    },
    /// Request to search server-side directory for users, rooms, or spaces.
    SearchDirectory {
        query: String,
        kind: RemoteDirectorySearchKind,
        limit: u64,
    },
    /// Request to fetch the full details (the room preview) of a tombstoned room.
    GetSuccessorRoomDetails {
        tombstoned_room_id: OwnedRoomId,
    },
    /// Request to create or open a direct message room with the given user.
    ///
    /// If there is no existing DM room with the given user, this will create a new DM room
    /// if `allow_create` is `true`; otherwise it will emit an action indicating that
    /// no DM room existed, upon which the UI will prompt the user to confirm that they want
    /// to proceed with creating a new DM room.
    #[doc(alias("dm"))]
    OpenOrCreateDirectMessage {
        user_profile: UserProfile,
        allow_create: bool,
        create_encrypted: bool,
    },
    /// Request to create a new room, optionally underneath a selected parent space.
    CreateRoom {
        room_name: String,
        topic: Option<String>,
        is_public: bool,
        is_encrypted: bool,
        parent_space_id: Option<OwnedRoomId>,
        context: CreateRoomContext,
    },
    /// Request the list of joined spaces where the current user may create child rooms.
    GetCreatableSpaces,
    /// Request to fetch profile information for the given user ID.
    GetUserProfile {
        user_id: OwnedUserId,
        /// * If `Some`, the user is known to be a member of a room, so this will
        ///   fetch the user's profile from that room's membership info.
        /// * If `None`, the user's profile info will be fetched from the server
        ///   in a room-agnostic manner, and no room membership info will be returned.
        room_id: Option<OwnedRoomId>,
        /// * If `true` (not recommended), only the local cache will be accessed.
        /// * If `false` (recommended), details will be fetched from the server.
        local_only: bool,
    },
    /// Request to fetch the number of unread messages in the given room.
    GetNumberUnreadMessages {
        timeline_kind: TimelineKind,
    },
    /// Request to set the unread flag for the given room.
    SetUnreadFlag {
        room_id: OwnedRoomId,
        /// If `true`, marks the room as unread.
        /// If `false`, marks the room as read.
        mark_as_unread: bool,
    },
    /// Request to set the favorite flag for the given room.
    SetIsFavorite {
        room_id: OwnedRoomId,
        is_favorite: bool,
    },
    /// Request to set the low priority flag for the given room.
    SetIsLowPriority {
        room_id: OwnedRoomId,
        is_low_priority: bool,
    },
    /// Request to generate a Matrix link (permalink) for a room or event.
    GenerateMatrixLink {
        /// The ID of the room to generate a link for.
        room_id: OwnedRoomId,
        /// * If `Some`, the link will point to this specific event within the room.
        /// * If `None`, the link will point to the room itself.
        event_id: Option<OwnedEventId>,
        /// * If `true`, the `matrix:` URI scheme will be used to create a [`MatrixUri`].
        /// * If `false` (default), the `https://matrix.to` scheme will be used to create a [`MatrixToUri`].
        use_matrix_scheme: bool,
        /// * If `true` (default is false), the link will include an action hint to join the room.
        join_on_click: bool,
    },
    /// Request to ignore/block or unignore/unblock a user.
    IgnoreUser {
        /// Whether to ignore (`true`) or unignore (`false`) the user.
        ignore: bool,
        /// The room membership info of the user to (un)ignore.
        room_member: RoomMember,
        /// The room ID of the room where the user is a member,
        /// which is only needed because it isn't present in the `RoomMember` object.
        room_id: OwnedRoomId,
    },
    /// Request to change the room-member power level for a user.
    SetRoomMemberPowerLevel {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
        /// * `None` means reset to the room's default user power level.
        /// * `Some` means set a role preset.
        room_member_role: Option<RoomMemberRole>,
    },
    /// Request to upload and set the avatar of the current user's account.
    UploadAvatar {
        /// The path to a local PNG or JPEG image file.
        avatar_path: PathBuf,
    },
    /// Request to set or remove the avatar of the current user's account.
    SetAvatar {
        /// * If `Some`, the avatar will be set to the given MXC URI.
        /// * If `None`, the avatar will be removed.
        avatar_url: Option<OwnedMxcUri>,
    },
    /// Request to set or remove the display name of the current user's account.
    SetDisplayName {
        /// * If `Some`, the display name will be set to the given value.
        /// * If `None`, the display name will be removed.
        new_display_name: Option<String>,
    },
    /// Request to fetch our own [`Device`].
    /// The response is delivered via [`AccountDataAction::OwnDeviceFetched`].
    GetOwnDevice,
    /// Request to fetch an Avatar image from the server.
    /// Upon completion of the async media request, the `on_fetched` function
    /// will be invoked with the content of an `AvatarUpdate`.
    FetchAvatar {
        mxc_uri: OwnedMxcUri,
        on_fetched: fn(AvatarUpdate),
    },
    /// Request to fetch media from the server.
    /// Upon completion of the async media request, the `on_fetched` function
    /// will be invoked with four arguments: the `destination`, the `media_request`,
    /// the result of the media fetch, and the `update_sender`.
    FetchMedia {
        media_request: MediaRequestParameters,
        on_fetched: OnMediaFetchedFn,
        destination: MediaCacheEntryRef,
        update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    },
    /// Request to download a file from an mxc:// URI and save it to disk.
    /// This bypasses MediaCache to avoid header parsing issues with non-ASCII filenames.
    DownloadAndSaveFile {
        mxc_uri: OwnedMxcUri,
        app_language: crate::i18n::AppLanguage,
    },
    /// Request to send a message to the given room.
    SendMessage {
        timeline_kind: TimelineKind,
        message: RoomMessageEventContent,
        replied_to: Option<Reply>,
        target_user_id: Option<OwnedUserId>,
        explicit_room: bool,
        #[cfg(feature = "tsp")]
        sign_with_tsp: bool,
    },
    /// Request to forward an existing message's effective content to another room.
    ForwardMessage {
        source_room_id: OwnedRoomId,
        source_event_id: OwnedEventId,
        destination_room_id: OwnedRoomId,
        message: RoomMessageEventContent,
    },
    /// Request to send a bot action response below a timeline message.
    SendActionResponse {
        timeline_kind: TimelineKind,
        content: serde_json::Value,
        target_user_id: OwnedUserId,
        explicit_room: bool,
        source_event_id: OwnedEventId,
    },
    /// Send an `m.sticker` event to the given room.
    SendSticker {
        timeline_kind: TimelineKind,
        /// Human-readable description (the `body` field).
        body: String,
        /// Original `mxc://` URL of the sticker image.
        mxc_url: String,
        /// Image dimensions in pixels (0 when unknown).
        width: u32,
        height: u32,
        /// File size in bytes (0 when unknown).
        size: u64,
    },
    /// Request to send a file attachment to the given room.
    SendAttachment {
        timeline_kind: TimelineKind,
        file_data: crate::shared::file_upload_modal::FileData,
        replied_to: Option<Reply>,
        #[cfg(feature = "tsp")]
        sign_with_tsp: bool,
    },
    /// Sends a notice to the given room that the current user is or is not typing.
    ///
    /// This request does not return a response or notify the UI thread, and
    /// furthermore, there is no need to send a follow-up request to stop typing
    /// (though you certainly can do so).
    SendTypingNotice {
        room_id: OwnedRoomId,
        typing: bool,
    },
    /// Spawn an async task to login to the given Matrix homeserver using the given SSO identity provider ID.
    ///
    /// While an SSO request is in flight, the login screen will temporarily prevent the user
    /// from submitting another redundant request, until this request has succeeded or failed.
    SpawnSSOServer{
        brand: String,
        homeserver_url: String,
        identity_provider_id: String,
        proxy: Option<String>,
    },
    /// Subscribe to typing notices for the given room.
    ///
    /// This is only valid for the main room timeline, not for thread-focused timelines.
    ///
    /// This request does not immediately return a response or notify the UI thread,
    /// but it will send updates to the UI via the timeline's update sender.
    SubscribeToTypingNotices {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Subscribe to changes in the read receipts of our own user.
    ///
    /// This request does not immediately return a response or notify the UI thread,
    /// but it will send updates to the UI via the timeline's update sender.
    SubscribeToOwnUserReadReceiptsChanged {
        timeline_kind: TimelineKind,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Subscribe to changes in the set of pinned events for the given room.
    ///
    /// This is only valid for the main room timeline, not for thread-focused timelines.
    SubscribeToPinnedEvents {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Sends a read receipt for the given event to the given room or thread timeline.
    ReadReceipt {
        timeline_kind: TimelineKind,
        event_id: OwnedEventId,
        receipt_type: ReceiptType,
    },
    /// Sends a request to obtain the power levels for this room.
    ///
    /// The response is delivered back to the main UI thread via [`TimelineUpdate::UserPowerLevels`].
    ///
    /// Even though it operates on a room itself, this accepts a `TimelineKind`
    /// in order to be able to send the fetched room member list to a specific timeline UI.
    GetRoomPowerLevels {
        timeline_kind: TimelineKind,
    },
    /// Toggles the given reaction to the given event in the given room.
    ToggleReaction {
        timeline_kind: TimelineKind,
        timeline_event_id: TimelineEventItemId,
        reaction: String,
    },
    /// Redacts (deletes) the given event in the given room.
    #[doc(alias("delete"))]
    RedactMessage {
        timeline_kind: TimelineKind,
        timeline_event_id: TimelineEventItemId,
        reason: Option<String>,
    },
    /// Pin or unpin the given event in the given room.
    #[doc(alias("unpin"))]
    PinEvent {
        timeline_kind: TimelineKind,
        event_id: OwnedEventId,
        pin: bool,
    },
    /// Request to fetch URL preview from the Matrix homeserver.
    GetUrlPreview {
        url: String,
        on_fetched: OnLinkPreviewFetchedFn,
        destination: Arc<Mutex<crate::home::link_preview::TimestampedCacheEntry>>,
        update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    },
}

fn add_octos_target_user_id(
    mut content: serde_json::Value,
    target_user_id: &UserId,
) -> serde_json::Value {
    if let Some(content_obj) = content.as_object_mut() {
        content_obj.insert(
            "org.octos.target_user_id".to_string(),
            serde_json::Value::String(target_user_id.to_string()),
        );
    }
    content
}

fn add_octos_explicit_room_marker(
    mut content: serde_json::Value,
    explicit_room: bool,
) -> serde_json::Value {
    if explicit_room
        && let Some(content_obj) = content.as_object_mut()
    {
        content_obj.insert(
            "org.octos.explicit_room".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    content
}

fn add_octos_routing_metadata(
    content: serde_json::Value,
    target_user_id: Option<&UserId>,
    explicit_room: bool,
) -> serde_json::Value {
    let content = add_octos_explicit_room_marker(content, explicit_room);
    if let Some(target_user_id) = target_user_id {
        add_octos_target_user_id(content, target_user_id)
    } else {
        content
    }
}

async fn ensure_target_user_joined_room(
    room: &Room,
    target_user_id: &UserId,
) -> Result<()> {
    let already_present = room
        .get_member_no_sync(target_user_id)
        .await
        .ok()
        .flatten()
        .is_some();
    if already_present {
        return Ok(());
    }

    room.invite_user_by_id(target_user_id).await?;

    for _attempt in 0..20 {
        let joined = room
            .get_member_no_sync(target_user_id)
            .await
            .ok()
            .flatten()
            .is_some();
        if joined {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Ok(())
}

/// Returns whether a DM room in the given state is reusable without rejoining.
fn is_active_dm_room_state(state: RoomState) -> bool {
    state == RoomState::Joined
}

fn is_empty_direct_room_display_name(display_name: Option<&RoomDisplayName>) -> bool {
    matches!(
        display_name,
        Some(RoomDisplayName::Empty | RoomDisplayName::EmptyWas(_))
    )
}

fn should_display_joined_room_entry(
    room_state: RoomState,
    is_direct: bool,
    display_name: Option<&RoomDisplayName>,
) -> bool {
    !(room_state == RoomState::Joined
        && is_direct
        && is_empty_direct_room_display_name(display_name))
}

/// Semantic result of comparing a Joined room's display eligibility between two
/// successive sliding-sync snapshots, while the room stays `RoomState::Joined`.
///
/// Used by [`update_room`] to decide whether the visibility flip should hide or
/// restore the room in the sidebar *without* destroying its
/// [`JoinedRoomDetails`]. Tearing `JoinedRoomDetails` down mid-session orphans
/// the open `RoomScreen`'s singleton timeline receiver and leaves the pane
/// blank forever — see `specs/task-dm-joined-room-details-churn.spec.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinedRoomDisplayFlip {
    /// The room became eligible for display (e.g., an Empty direct DM finally
    /// got a calculated name after the bot joined).
    BecameDisplayable,
    /// The room lost display eligibility (e.g., `is_direct` flipped true while
    /// `display_name` was still `Empty`).
    BecameHidden,
    /// No change in display eligibility; the caller should perform no
    /// visibility-only side effect.
    NoDisplayChange,
}

fn classify_joined_room_display_flip(
    old_should_display: bool,
    new_should_display: bool,
) -> JoinedRoomDisplayFlip {
    match (old_should_display, new_should_display) {
        (true, false) => JoinedRoomDisplayFlip::BecameHidden,
        (false, true) => JoinedRoomDisplayFlip::BecameDisplayable,
        _ => JoinedRoomDisplayFlip::NoDisplayChange,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DmRoomReuseCandidate {
    room_state: RoomState,
    display_name: Option<RoomDisplayName>,
    target_membership: Option<MembershipState>,
    latest_event_timestamp: Option<u64>,
}

fn is_reusable_dm_room_candidate(candidate: &DmRoomReuseCandidate) -> bool {
    is_active_dm_room_state(candidate.room_state)
        && should_display_joined_room_entry(
            candidate.room_state,
            true,
            candidate.display_name.as_ref(),
        )
        && matches!(
            candidate.target_membership,
            Some(MembershipState::Join | MembershipState::Invite)
        )
}

fn choose_reusable_dm_candidate(candidates: &[DmRoomReuseCandidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| is_reusable_dm_room_candidate(candidate))
        .max_by_key(|(_, candidate)| candidate.latest_event_timestamp.unwrap_or(0))
        .map(|(idx, _)| idx)
}

async fn find_reusable_direct_message_room(client: &Client, target_user_id: &UserId) -> Option<Room> {
    let mut candidate_rooms = Vec::new();
    let mut candidate_metas = Vec::new();

    for room in client.joined_rooms() {
        let direct_targets = room.direct_targets();
        if direct_targets.len() != 1
            || !direct_targets.contains(<&DirectUserIdentifier>::from(target_user_id))
        {
            continue;
        }

        let target_membership = room
            .get_member_no_sync(target_user_id)
            .await
            .ok()
            .flatten()
            .map(|member| member.membership().clone());
        let display_name = room.display_name().await.ok();

        candidate_metas.push(DmRoomReuseCandidate {
            room_state: room.state(),
            display_name,
            target_membership,
            latest_event_timestamp: room.latest_event_timestamp().map(|ts| u64::from(ts.get())),
        });
        candidate_rooms.push(room);
    }

    choose_reusable_dm_candidate(&candidate_metas)
        .and_then(|idx| candidate_rooms.into_iter().nth(idx))
}

#[cfg(test)]
mod matrix_request_tests {
    use super::*;

    #[test]
    fn test_forward_success_feedback() {
        let room_id = RoomId::parse("!dest:example.org").unwrap();

        assert_eq!(
            forward_success_feedback_text(room_id.as_ref()),
            "Forwarded message to !dest:example.org.",
        );
    }

    #[test]
    fn test_forward_failure_feedback() {
        assert_eq!(
            forward_failure_feedback_text("network error"),
            "Failed to forward message: network error",
        );
    }

    #[test]
    fn is_active_dm_room_state_only_joined_is_reusable() {
        assert!(is_active_dm_room_state(RoomState::Joined));
        assert!(!is_active_dm_room_state(RoomState::Invited));
        assert!(!is_active_dm_room_state(RoomState::Left));
        assert!(!is_active_dm_room_state(RoomState::Banned));
        assert!(!is_active_dm_room_state(RoomState::Knocked));
    }

    #[test]
    fn should_display_joined_room_entry_hides_empty_direct_dm() {
        assert!(!should_display_joined_room_entry(
            RoomState::Joined,
            true,
            Some(&RoomDisplayName::EmptyWas("octosbot".into())),
        ));
        assert!(!should_display_joined_room_entry(
            RoomState::Joined,
            true,
            Some(&RoomDisplayName::Empty),
        ));
    }

    #[test]
    fn should_display_joined_room_entry_keeps_non_empty_or_non_direct_rooms() {
        assert!(should_display_joined_room_entry(
            RoomState::Joined,
            true,
            Some(&RoomDisplayName::Named("octosbot".into())),
        ));
        assert!(should_display_joined_room_entry(
            RoomState::Joined,
            false,
            Some(&RoomDisplayName::EmptyWas("room".into())),
        ));
        assert!(should_display_joined_room_entry(
            RoomState::Invited,
            true,
            Some(&RoomDisplayName::EmptyWas("octosbot".into())),
        ));
    }

    #[test]
    fn classify_joined_room_display_flip_becomes_hidden() {
        assert_eq!(
            classify_joined_room_display_flip(true, false),
            JoinedRoomDisplayFlip::BecameHidden,
        );
    }

    #[test]
    fn classify_joined_room_display_flip_becomes_displayable() {
        assert_eq!(
            classify_joined_room_display_flip(false, true),
            JoinedRoomDisplayFlip::BecameDisplayable,
        );
    }

    #[test]
    fn classify_joined_room_display_flip_no_change_when_stable() {
        assert_eq!(
            classify_joined_room_display_flip(true, true),
            JoinedRoomDisplayFlip::NoDisplayChange,
        );
        assert_eq!(
            classify_joined_room_display_flip(false, false),
            JoinedRoomDisplayFlip::NoDisplayChange,
        );
    }

    #[test]
    fn choose_reusable_dm_candidate_prefers_room_where_target_is_still_active() {
        let candidates = vec![
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("old".into())),
                target_membership: Some(MembershipState::Leave),
                latest_event_timestamp: Some(20),
            },
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("active".into())),
                target_membership: Some(MembershipState::Join),
                latest_event_timestamp: Some(10),
            },
        ];

        assert_eq!(choose_reusable_dm_candidate(&candidates), Some(1));
    }

    #[test]
    fn choose_reusable_dm_candidate_returns_none_when_target_left_every_candidate() {
        let candidates = vec![
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("old".into())),
                target_membership: Some(MembershipState::Leave),
                latest_event_timestamp: Some(20),
            },
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("missing".into())),
                target_membership: None,
                latest_event_timestamp: Some(30),
            },
        ];

        assert_eq!(choose_reusable_dm_candidate(&candidates), None);
    }

    #[test]
    fn choose_reusable_dm_candidate_prefers_latest_active_candidate() {
        let candidates = vec![
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("older".into())),
                target_membership: Some(MembershipState::Invite),
                latest_event_timestamp: Some(10),
            },
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::Named("latest".into())),
                target_membership: Some(MembershipState::Join),
                latest_event_timestamp: Some(50),
            },
        ];

        assert_eq!(choose_reusable_dm_candidate(&candidates), Some(1));
    }

    #[test]
    fn choose_reusable_dm_candidate_rejects_empty_direct_room() {
        let candidates = vec![
            DmRoomReuseCandidate {
                room_state: RoomState::Joined,
                display_name: Some(RoomDisplayName::EmptyWas("octosbot".into())),
                target_membership: Some(MembershipState::Join),
                latest_event_timestamp: Some(50),
            },
        ];

        assert_eq!(choose_reusable_dm_candidate(&candidates), None);
    }

    #[test]
    fn should_add_octos_target_user_id_to_message_content() {
        let target_user_id = OwnedUserId::try_from("@bot_weather:example.com").unwrap();
        let content = serde_json::json!({
            "msgtype": "m.text",
            "body": "hello",
        });

        let content = add_octos_target_user_id(content, target_user_id.as_ref());

        assert_eq!(
            content
                .get("org.octos.target_user_id")
                .and_then(|value| value.as_str()),
            Some("@bot_weather:example.com")
        );
    }

    #[test]
    fn test_send_message_explicit_room_sets_octos_explicit_room_marker() {
        let content = serde_json::json!({
            "msgtype": "m.text",
            "body": "hello room",
        });

        let content = add_octos_explicit_room_marker(content, true);

        assert_eq!(
            content
                .get("org.octos.explicit_room")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(
            content.get("org.octos.target_user_id").is_none(),
            "ExplicitRoom should not also set a targeted bot MXID",
        );
    }

    #[test]
    fn test_send_reply_explicit_room_sets_octos_explicit_room_marker() {
        let content = serde_json::json!({
            "msgtype": "m.text",
            "body": "reply body",
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": "$reply"
                }
            }
        });

        let content = add_octos_explicit_room_marker(content, true);

        assert_eq!(
            content
                .get("org.octos.explicit_room")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(
            content.get("org.octos.target_user_id").is_none(),
            "ExplicitRoom replies should suppress room fallback without setting target_user_id",
        );
    }

    #[test]
    fn test_send_message_room_default_does_not_set_octos_explicit_room_marker() {
        let content = serde_json::json!({
            "msgtype": "m.text",
            "body": "hello bot",
        });

        let content = add_octos_explicit_room_marker(content, false);

        assert!(
            content.get("org.octos.explicit_room").is_none(),
            "RoomDefault should not suppress Octos room fallback",
        );
    }

    #[test]
    fn test_should_restore_loaded_app_state_with_bot_settings_and_empty_dock() {
        let mut app_state = crate::app::AppState::default();
        app_state.bot_settings.enabled = true;
        app_state.bot_settings.botfather_user_id = "@octosbot:example.com".to_string();
        app_state.bot_settings.octos_service_url = "http://192.168.5.12:8010".to_string();

        assert!(
            should_restore_loaded_app_state(&app_state),
            "non-default bot settings must restore even when dock state is empty",
        );
    }

    #[test]
    fn test_should_restore_loaded_app_state_with_selected_room_and_empty_dock() {
        let app_state = crate::app::AppState {
            selected_room: Some(crate::app::SelectedRoom::JoinedRoom {
                room_name_id: crate::utils::RoomNameId::new(
                    matrix_sdk::RoomDisplayName::Named("octosbot".into()),
                    "!room:example.org".parse().unwrap(),
                ),
            }),
            ..Default::default()
        };

        assert!(
            should_restore_loaded_app_state(&app_state),
            "selected_room is persisted state and must restore even when dock state is empty",
        );
    }

    #[test]
    fn test_should_not_restore_loaded_default_app_state() {
        assert!(
            !should_restore_loaded_app_state(&crate::app::AppState::default()),
            "fresh installs should keep in-memory defaults instead of dispatching a no-op restore",
        );
    }

    #[test]
    fn test_access_token_copy_result_returns_token_when_available() {
        assert_eq!(
            access_token_copy_result(Some("secret-token".to_owned())),
            AccessTokenCopyAction::Ready {
                access_token: "secret-token".to_owned(),
            },
        );
    }

    #[test]
    fn test_access_token_copy_action_debug_redacts_token() {
        let debug_text = format!(
            "{:?}",
            AccessTokenCopyAction::Ready {
                access_token: "secret-token".to_owned(),
            },
        );

        assert!(debug_text.contains("<redacted>"));
        assert!(!debug_text.contains("secret-token"));
    }

    #[test]
    fn test_access_token_copy_result_fails_without_client() {
        assert_eq!(
            access_token_copy_result_for_client(None),
            AccessTokenCopyAction::Failed {
                reason: AccessTokenCopyError::NoSession,
            },
        );
    }

    #[test]
    fn test_access_token_copy_result_fails_without_access_token() {
        assert_eq!(
            access_token_copy_result(None),
            AccessTokenCopyAction::Failed {
                reason: AccessTokenCopyError::Unavailable,
            },
        );
    }

}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteDirectorySearchKind {
    People,
    Rooms,
    Spaces,
}

#[derive(Clone, Debug)]
pub enum RemoteDirectorySearchResult {
    User(UserProfile),
    Room {
        room_name_id: RoomNameId,
        avatar_uri: Option<OwnedMxcUri>,
    },
    Space {
        space_name_id: RoomNameId,
        avatar_uri: Option<OwnedMxcUri>,
    },
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    if let Some(sender) = REQUEST_SENDER.lock().unwrap().as_ref() {
        sender.send(req)
            .expect("BUG: matrix worker task receiver has died!");
    }
}

fn forward_success_feedback_text(destination_room_id: &RoomId) -> String {
    format!("Forwarded message to {destination_room_id}.")
}

fn forward_failure_feedback_text(error: impl std::fmt::Display) -> String {
    format!("Failed to forward message: {error}")
}

/// Details of a login request that get submitted within [`MatrixRequest::Login`].
pub enum LoginRequest{
    LoginByPassword(LoginByPassword),
    Register(RegisterAccount),
    LoginBySSOSuccess(Client, ClientSessionPersisted, bool),
    /// Sent by the OIDC worker task after `OAuth::finish_login()` returns
    /// successfully. The payload mirrors `LoginBySSOSuccess` — already-built
    /// client + its session bundle + `is_add_account`. The main login
    /// handler just persists the session and returns it to the outer loop,
    /// so sync-service startup is shared with password/SSO flows.
    LoginByOidcSuccess(Client, ClientSessionPersisted, bool),
    LoginByCli,
    HomeserverLoginTypesQuery(String),

}

/// Why a [`MatrixRequest::GetAccessTokenForCopy`] request produced no token.
///
/// Variants are locale-independent: the worker thread has no `AppLanguage`, so
/// it reports *what* went wrong and leaves the user-facing wording to the UI
/// thread, which owns the active language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessTokenCopyError {
    /// No Matrix client is currently logged in.
    NoSession,
    /// A client is logged in but its session carries no access token.
    Unavailable,
}

#[derive(Clone, PartialEq, Eq)]
pub enum AccessTokenCopyAction {
    Ready {
        access_token: String,
    },
    Failed {
        reason: AccessTokenCopyError,
    },
}

impl std::fmt::Debug for AccessTokenCopyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessTokenCopyAction::Ready { .. } => f
                .debug_struct("AccessTokenCopyAction::Ready")
                .field("access_token", &"<redacted>")
                .finish(),
            AccessTokenCopyAction::Failed { reason } => f
                .debug_struct("AccessTokenCopyAction::Failed")
                .field("reason", reason)
                .finish(),
        }
    }
}

fn access_token_copy_result(access_token: Option<String>) -> AccessTokenCopyAction {
    match access_token {
        Some(access_token) => AccessTokenCopyAction::Ready { access_token },
        None => AccessTokenCopyAction::Failed {
            reason: AccessTokenCopyError::Unavailable,
        },
    }
}

fn access_token_copy_result_for_client(client: Option<Client>) -> AccessTokenCopyAction {
    let Some(client) = client else {
        return AccessTokenCopyAction::Failed {
            reason: AccessTokenCopyError::NoSession,
        };
    };
    access_token_copy_result(client.access_token())
}

/// Information needed to log in to a Matrix homeserver.
pub struct LoginByPassword {
    pub user_id: String,
    pub password: String,
    pub homeserver: Option<String>,
    pub proxy: Option<String>,
    /// Whether this login is for adding another account (multi-account mode).
    pub is_add_account: bool,
}

/// Information needed to register a new account on a Matrix homeserver.
#[derive(Clone)]
pub struct RegisterAccount {
    pub user_id: String,
    pub password: String,
    pub homeserver: Option<String>,
    pub proxy: Option<String>,
}


/// The entry point for the worker task that runs Matrix-related operations.
///
/// All this task does is wait for [`MatrixRequests`] from the main UI thread
/// and then executes them within an async runtime context.
async fn matrix_worker_task(
    mut request_receiver: UnboundedReceiver<MatrixRequest>,
    login_sender: Sender<LoginRequest>,
) -> Result<()> {
    log!("Started matrix_worker_task.");
    // The async tasks that are spawned to subscribe to changes in our own user's read receipts for each timeline.
    let mut subscribers_own_user_read_receipts: HashMap<TimelineKind, JoinHandle<()>> = HashMap::new();
    // The async tasks that are spawned to subscribe to changes in the pinned events for each room.
    let mut subscribers_pinned_events: HashMap<OwnedRoomId, JoinHandle<()>> = HashMap::new();

    while let Some(request) = request_receiver.recv().await {
        match request {
            MatrixRequest::Login(login_request) => {
                // Check if this is an add-account login (when already logged in)
                let is_add_account = match &login_request {
                    LoginRequest::LoginByPassword(lpw) => lpw.is_add_account,
                    LoginRequest::LoginBySSOSuccess(_, _, is_add) => *is_add,
                    LoginRequest::LoginByOidcSuccess(_, _, is_add) => *is_add,
                    _ => false,
                };

                if is_add_account {
                    // Handle add-account login directly in the worker task
                    log!("Processing add-account login directly in worker task");
                    let cli = Cli::default();
                    match login(&cli, login_request).await {
                        Ok((client, _sync_token, _is_add, session)) => {
                            let user_id = client.user_id()
                                .expect("BUG: client.user_id() returned None after login!");

                            // Add to account manager
                            let account = Account {
                                client: client.clone(),
                                user_id: user_id.to_owned(),
                                session,
                                display_name: None,
                                avatar_url: None,
                            };
                            let is_new = account_manager::add_account(account);
                            log!("Add-account login successful for {}. New account: {}", user_id, is_new);

                            // Post success action
                            Cx::post_action(LoginAction::AddAccountSuccess);
                            enqueue_popup_notification(
                                format!("Added account: {}", user_id),
                                PopupKind::Success,
                                Some(3.0),
                            );
                        }
                        Err(e) => {
                            error!("Add-account login failed: {e:?}");
                            Cx::post_action(LoginAction::LoginFailure(format!("{e}")));
                        }
                    }
                } else {
                    // Forward to login_sender for initial login flow
                    if let Err(e) = login_sender.send(login_request).await {
                        error!("Error sending login request to login_sender: {e:?}");
                        Cx::post_action(LoginAction::LoginFailure(String::from(
                            "BUG: failed to send login request to login worker task."
                        )));
                    }
                }
            }

            MatrixRequest::GetAccessTokenForCopy => {
                Cx::post_action(access_token_copy_result_for_client(get_client()));
            }

            MatrixRequest::LoadStickerCatalog => {
                use crate::home::sticker_modal::{
                    StickerCatalogAction, load_sticker_catalog, report_failure,
                };
                let Some(client) = get_client() else {
                    report_failure("not logged in");
                    continue;
                };
                let _task = Handle::current().spawn(async move {
                    match load_sticker_catalog(client).await {
                        Ok(packs) => Cx::post_action(StickerCatalogAction::Ready { packs }),
                        Err(e) => report_failure(e),
                    }
                });
            }

            MatrixRequest::SetStickerPackState { asset_type, enable } => {
                use crate::home::sticker_modal::set_pack_state;
                let Some(client) = get_client() else { continue };
                let _task = Handle::current().spawn(async move {
                    if let Err(e) = set_pack_state(client, asset_type, enable).await {
                        log!("[sticker] set_pack_state failed: {e}");
                    }
                });
            }

            MatrixRequest::LoadPackStickers { pack_id, pack_name, sticker_infos } => {
                use crate::home::sticker_modal::load_pack_stickers_streaming;
                // `load_pack_stickers_streaming` posts StickerGridAction::Ready
                // from disk-cache data immediately, then posts
                // StickerImagePatchAction batches for any cache-miss images —
                // no additional action posting needed here.
                let _task = Handle::current().spawn(async move {
                    load_pack_stickers_streaming(pack_id, pack_name, sticker_infos).await;
                });
            }

            MatrixRequest::DiscoverHomeserverCapabilities { url, proxy } => {
                tokio::spawn(async move {
                    let requested_url = url.clone();
                    match discover_homeserver_capabilities(&url, proxy.as_deref()).await {
                        Ok(caps) => {
                            Cx::post_action(CapabilityProbeAction::Discovered {
                                requested_url,
                                caps: Box::new(caps),
                            });
                        }
                        Err(e) => {
                            Cx::post_action(CapabilityProbeAction::Failed {
                                requested_url,
                                error: e.to_string(),
                            });
                        }
                    }
                });
            }

            MatrixRequest::StartOidcLogin { homeserver_url, proxy, is_add_account } => {
                let (flow_id, cancel_rx) = match try_start_oidc_flow() {
                    Ok(flow) => flow,
                    Err(msg) => {
                        warning!("{msg}");
                        continue;
                    }
                };

                let login_sender = login_sender.clone();
                tokio::spawn(async move {
                    let outcome = crate::login::oidc_login::start_oidc_login(
                        homeserver_url,
                        proxy,
                        cancel_rx,
                    ).await;

                    match outcome {
                        Ok((client, client_session, user_id)) => {
                            log!("OIDC login succeeded for {user_id}; forwarding to login pipeline.");
                            if let Err(e) = login_sender.send(
                                LoginRequest::LoginByOidcSuccess(client, client_session, is_add_account)
                            ).await {
                                error!("Failed to forward OIDC login result: {e:?}");
                                Cx::post_action(LoginAction::OidcLoginFailed(
                                    "BUG: couldn't hand OIDC login result to the login pipeline.".to_string(),
                                ));
                            }
                        }
                        Err(crate::login::oidc_login::OidcLoginError::Cancelled) => {
                            Cx::post_action(LoginAction::OidcLoginCancelled);
                        }
                        Err(e) => {
                            error!("OIDC login failed: {e:?}");
                            let msg = crate::login::oidc_login::map_oidc_error(&e);
                            Cx::post_action(LoginAction::OidcLoginFailed(msg));
                        }
                    }

                    finish_oidc_flow(flow_id);
                });
            }

            MatrixRequest::CancelOidcLogin => {
                cancel_active_oidc_flow();
            }

            MatrixRequest::RegisterViaUiaa { username, password, homeserver_url } => {
                Cx::post_action(crate::register::RegisterAction::RegistrationSubmitted);
                let register_request = LoginRequest::Register(RegisterAccount {
                    user_id: username,
                    password,
                    homeserver: Some(homeserver_url),
                    proxy: None,
                });
                if let Err(e) = login_sender.send(register_request).await {
                    error!("Error sending register request to login_sender: {e:?}");
                    Cx::post_action(crate::register::RegisterAction::RegistrationFailed(
                        "Internal error: registration worker is unavailable. Please restart Robrix.".to_owned(),
                    ));
                }
            }

            MatrixRequest::SwitchAccount { user_id } => {
                // Check if the account exists in AccountManager
                if account_manager::get_client_for_user(&user_id).is_some() {
                    // Set the target account for switch
                    set_account_switch_target(user_id.clone());

                    // Notify UI that switch is starting (app.rs handles the popup notification)
                    Cx::post_action(AccountSwitchAction::Starting(user_id.clone()));

                    // Stop the sync service - this will cause the main loop to restart
                    if let Some(sync_service) = get_sync_service() {
                        sync_service.stop().await;
                    }

                    // The main loop will detect the account switch target and restart with the new account
                    // We return Ok(()) to signal the worker should end gracefully
                    return Ok(());
                } else {
                    error!("Account {} not found in AccountManager", user_id);
                    Cx::post_action(AccountSwitchAction::Failed(
                        format!("Account {} not found", user_id)
                    ));
                    enqueue_popup_notification(
                        format!("Account not found: {}", user_id),
                        PopupKind::Error,
                        Some(3.0),
                    );
                }
            }

            MatrixRequest::Logout { is_desktop } => {
                log!("Received MatrixRequest::Logout, is_desktop: {}", is_desktop);
                let _logout_task = Handle::current().spawn(async move {
                    log!("Starting logout task");
                    // Use the state machine implementation
                    match logout_with_state_machine(is_desktop).await {
                        Ok(()) => {
                            log!("Logout completed successfully via state machine");
                        },
                        Err(e) => {
                            error!("Logout failed: {e:?}");
                        }
                    }
                });
            }

            MatrixRequest::PaginateTimeline {timeline_kind, num_events, direction} => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("Skipping pagination request for unknown {timeline_kind}");
                    continue;
                };
                let client = get_client();

                // Spawn a new async task that will make the actual pagination request.
                let _paginate_task = Handle::current().spawn(async move {
                    log!("Starting {direction} pagination request for {timeline_kind}...");
                    if sender.send(TimelineUpdate::PaginationRunning(direction)).is_err() {
                        warning!("Skipping {direction} pagination request for {timeline_kind}: timeline receiver was dropped before start.");
                        return;
                    }
                    SignalToUI::set_ui_signal();

                    let mut attempted_invalid_batch_token_recovery = false;
                    let mut res = if direction == PaginationDirection::Forwards {
                        timeline.paginate_forwards(num_events).await
                    } else {
                        timeline.paginate_backwards(num_events).await
                    };

                    if direction == PaginationDirection::Backwards
                        && res
                            .as_ref()
                            .err()
                            .is_some_and(is_invalid_batch_token_timeline_error)
                    {
                        attempted_invalid_batch_token_recovery = true;
                        warning!(
                            "Detected an invalid cached batch token for {timeline_kind}; clearing the room event cache and retrying once."
                        );
                        let room_id = timeline_kind.room_id().clone();
                        if let Some(room) = client.and_then(|client| client.get_room(&room_id)) {
                            match room.event_cache().await {
                                Ok((room_event_cache, _drop_handles)) => {
                                    match room_event_cache.clear().await {
                                        Ok(()) => {
                                            res = timeline.paginate_backwards(num_events).await;
                                        }
                                        Err(clear_error) => {
                                            warning!(
                                                "Failed to clear event cache for room {room_id} after invalid batch token: {clear_error}"
                                            );
                                        }
                                    }
                                }
                                Err(event_cache_error) => {
                                    warning!(
                                        "Failed to access room event cache for room {room_id} after invalid batch token: {event_cache_error}"
                                    );
                                }
                            }
                        }
                    }

                    match res {
                        Ok(fully_paginated) => {
                            log!("Completed {direction} pagination request for {timeline_kind}, hit {} of timeline? {}",
                                if direction == PaginationDirection::Forwards { "end" } else { "start" },
                                if fully_paginated { "yes" } else { "no" },
                            );
                            if sender.send(TimelineUpdate::PaginationIdle {
                                fully_paginated,
                                direction,
                            }).is_ok() {
                                SignalToUI::set_ui_signal();
                            } else {
                                warning!("Dropping completed {direction} pagination update for {timeline_kind}: timeline receiver was dropped.");
                            }
                        }
                        Err(error) => {
                            if direction == PaginationDirection::Backwards
                                && attempted_invalid_batch_token_recovery
                                && is_invalid_batch_token_timeline_error(&error)
                            {
                                warning!(
                                    "Still got invalid batch token for {timeline_kind} after one recovery attempt; treating as fully paginated."
                                );
                                if sender.send(TimelineUpdate::PaginationIdle {
                                    fully_paginated: true,
                                    direction,
                                }).is_ok() {
                                    SignalToUI::set_ui_signal();
                                } else {
                                    warning!(
                                        "Dropping recovered {direction} pagination update for {timeline_kind}: timeline receiver was dropped."
                                    );
                                }
                                return;
                            }
                            if direction == PaginationDirection::Backwards
                                && matches!(timeline_kind, TimelineKind::Thread { .. })
                                && is_thread_unknown_parent_timeline_error(&error)
                            {
                                warning!(
                                    "Treating unknown parent event as end-of-thread for {timeline_kind}."
                                );
                                sender.send(TimelineUpdate::PaginationIdle {
                                    fully_paginated: true,
                                    direction,
                                }).unwrap();
                                SignalToUI::set_ui_signal();
                                return;
                            }
                            error!("Error sending {direction} pagination request for {timeline_kind}: {error:?}");
                            if sender.send(TimelineUpdate::PaginationError {
                                error,
                                direction,
                            }).is_ok() {
                                SignalToUI::set_ui_signal();
                            } else {
                                warning!("Dropping failed {direction} pagination update for {timeline_kind}: timeline receiver was dropped.");
                            }
                        }
                    }
                });
            }

            MatrixRequest::EditMessage { timeline_kind, timeline_event_item_id, edited_content } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for edit request");
                    continue;
                };

                // Spawn a new async task that will make the actual edit request.
                let _edit_task = Handle::current().spawn(async move {
                    log!("Sending request to edit message {timeline_event_item_id:?} in {timeline_kind}...");
                    let result = timeline.edit(&timeline_event_item_id, edited_content).await;
                    match result {
                        Ok(_) => log!("Successfully edited message {timeline_event_item_id:?} in {timeline_kind}."),
                        Err(ref e) => error!("Error editing message {timeline_event_item_id:?} in {timeline_kind}: {e:?}"),
                    }
                    if sender.send(TimelineUpdate::MessageEdited {
                        timeline_event_item_id,
                        result,
                    }).is_ok() {
                        SignalToUI::set_ui_signal();
                    } else {
                        warning!("Dropping message edited update for {timeline_kind}: timeline receiver was dropped.");
                    }
                });
            }

            MatrixRequest::FetchDetailsForEvent { timeline_kind, event_id } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for fetch details for event request");
                    continue;
                };

                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending request to fetch details for event {event_id} in {timeline_kind}...");
                    let result = timeline.fetch_details_for_event(&event_id).await;
                    match &result {
                        Ok(_) => {
                            // log!("Successfully fetched details for event {event_id} in {timeline_kind}.");
                        }
                        Err(_e) => {
                            // error!("Error fetching details for event {event_id} in {timeline_kind}: {_e:?}");
                        }
                    }
                    if sender.send(TimelineUpdate::EventDetailsFetched { event_id, result }).is_err() {
                        error!("Failed to send fetched event details to UI for {timeline_kind}");
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::FetchThreadSummaryDetails {
                timeline_kind,
                thread_root_event_id,
                timeline_item_index,
            } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for fetch thread summary details request");
                    continue;
                };

                let _fetch_task = Handle::current().spawn(async move {
                    let (num_replies, latest_reply_event) = fetch_thread_summary_details(
                        timeline.room(),
                        &thread_root_event_id,
                    ).await;
                    let latest_reply_preview_text = match latest_reply_event.as_ref() {
                        Some(event) => text_preview_of_latest_thread_reply(timeline.room(), event).await,
                        None => None,
                    };

                    if sender.send(TimelineUpdate::ThreadSummaryDetailsFetched {
                        thread_root_event_id,
                        timeline_item_index,
                        num_replies,
                        latest_reply_preview_text,
                    }).is_err() {
                        error!("Failed to send fetched thread summary details to UI for {timeline_kind}");
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::ListRoomThreads { room_id, from } => {
                let Some(room) = get_client().and_then(|client| client.get_room(&room_id)) else {
                    Cx::post_action(RoomThreadsAction::Failed {
                        room_id,
                        from,
                        error: String::from("Room not found."),
                    });
                    continue;
                };

                let _list_threads_task = Handle::current().spawn(async move {
                    match fetch_room_threads_page(&room, from.clone()).await {
                        Ok((threads, prev_batch_token)) => {
                            Cx::post_action(RoomThreadsAction::Loaded {
                                room_id,
                                from,
                                threads,
                                prev_batch_token,
                            });
                        }
                        Err(error) => {
                            Cx::post_action(RoomThreadsAction::Failed {
                                room_id,
                                from,
                                error: error.to_string(),
                            });
                        }
                    }
                });
            }

            MatrixRequest::SyncRoomMemberList { timeline_kind } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for sync members list request");
                    continue;
                };

                let _fetch_task = Handle::current().spawn(async move {
                    log!("Sending sync room members request for {timeline_kind}...");
                    timeline.fetch_members().await;
                    log!("Completed sync room members request for {timeline_kind}.");
                    if sender.send(TimelineUpdate::RoomMembersSynced).is_ok() {
                        SignalToUI::set_ui_signal();
                    } else {
                        warning!("Dropping room members synced update for {timeline_kind}: timeline receiver was dropped.");
                    }
                });
            }

            MatrixRequest::CreateThreadTimeline { room_id, thread_root_event_id } => {
                let main_room_timeline = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        error!("BUG: room info not found for create thread timeline request, room {room_id}");
                        continue;
                    };
                    if room_info.thread_timelines.contains_key(&thread_root_event_id) {
                        continue;
                    }
                    let newly_pending = room_info.pending_thread_timelines.insert(thread_root_event_id.clone());
                    if !newly_pending {
                        continue;
                    }
                    room_info.main_timeline.timeline.clone()
                };

                let _create_thread_timeline_task = Handle::current().spawn(async move {
                    log!("Creating thread-focused timeline for room {room_id}, thread {thread_root_event_id}...");
                    let build_result = main_room_timeline.room()
                        .timeline_builder()
                        .with_focus(TimelineFocus::Thread {
                            root_event_id: thread_root_event_id.clone(),
                        })
                        .track_read_marker_and_receipts(TimelineReadReceiptTracking::AllEvents)
                        .build()
                        .await;

                    match build_result {
                        Ok(thread_timeline) => {
                            let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                            let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                                return;
                            };
                            log!("Successfully created thread-focused timeline for room {room_id}, thread {thread_root_event_id}.");
                            let thread_timeline = Arc::new(thread_timeline);
                            let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();
                            let (request_sender, request_receiver) = watch::channel(Vec::new());
                            let timeline_subscriber_handler_task = Handle::current().spawn(
                                timeline_subscriber_handler(
                                    main_room_timeline.room().clone(),
                                    thread_timeline.clone(),
                                    timeline_update_sender.clone(),
                                    request_receiver,
                                    Some(thread_root_event_id.clone()),
                                )
                            );
                            room_info
                                .pending_thread_timelines
                                .remove(&thread_root_event_id);
                            room_info.thread_timelines.insert(
                                thread_root_event_id.clone(),
                                PerTimelineDetails {
                                    timeline: thread_timeline,
                                    timeline_update_sender,
                                    timeline_singleton_endpoints: Some((
                                        timeline_update_receiver,
                                        request_sender,
                                    )),
                                    timeline_subscriber_handler_task,
                                },
                            );
                            SignalToUI::set_ui_signal();
                        }
                        Err(error) => {
                            error!("Failed to create thread-focused timeline for room {room_id}, thread {thread_root_event_id}: {error}");
                            let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                            if let Some(room_info) = all_joined_rooms.get_mut(&room_id) {
                                room_info
                                    .pending_thread_timelines
                                    .remove(&thread_root_event_id);
                            }
                            enqueue_popup_notification(
                                format!("Failed to create thread-focused timeline. Please retry opening the thread again later.\n\nError: {error}"),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                });
            }

            MatrixRequest::Knock { room_or_alias_id, reason, server_names } => {
                let Some(client) = get_client() else { continue };
                let _knock_room_task = Handle::current().spawn(async move {
                    log!("Sending request to knock on room {room_or_alias_id}...");
                    match client.knock(room_or_alias_id.clone(), reason, server_names).await {
                        Ok(room) => {
                            let _ = room.display_name().await; // populate this room's display name cache
                            Cx::post_action(KnockResultAction::Knocked {
                                room_or_alias_id,
                                room,
                            });
                        }
                        Err(error) => Cx::post_action(KnockResultAction::Failed {
                            room_or_alias_id,
                            error,
                        }),
                    }
                });
            }

            MatrixRequest::InviteUser { room_id, user_id } => {
                let Some(client) = get_client() else { continue };
                let _invite_task = Handle::current().spawn(async move {
                    // We use `client.get_room()` here because the room might also be a space,
                    // not just a joined room.
                    if let Some(room) = client.get_room(&room_id) {
                        log!("Sending request to invite user {user_id} to room {room_id}...");
                        match room.invite_user_by_id(&user_id).await {
                            Ok(_) => Cx::post_action(InviteResultAction::Sent {
                                room_id,
                                user_id,
                            }),
                            Err(error) => Cx::post_action(InviteResultAction::Failed {
                                room_id,
                                user_id,
                                error,
                            }),
                        }
                    }
                    else {
                        error!("Room/Space not found for invite user request {room_id}, {user_id}");
                        Cx::post_action(InviteResultAction::Failed {
                            room_id,
                            user_id,
                            error: matrix_sdk::Error::UnknownError("Room/Space not found in client's known list.".into()),
                        })
                    }
                });
            }

            MatrixRequest::SetRoomBotBinding {
                room_id,
                bound,
                bot_user_id,
            } => {
                let Some(client) = get_client() else { continue };
                let _bot_binding_task = Handle::current().spawn(async move {
                    let Some(room) = client.get_room(&room_id) else {
                        let error_message =
                            format!("Room {room_id} was not found for the bot binding request.");
                        error!("{error_message}");
                        enqueue_popup_notification(error_message, PopupKind::Error, None);
                        return;
                    };

                    let membership_result = if bound {
                        room.invite_user_by_id(&bot_user_id).await
                    } else {
                        room.kick_user(&bot_user_id, Some("Robrix app service unbind")).await
                    };

                    match membership_result {
                        Ok(()) => {
                            Cx::post_action(AppStateAction::BotRoomBindingUpdated {
                                room_id,
                                bound,
                                bot_user_id: Some(bot_user_id),
                                warning: None,
                            });
                        }
                        Err(error) => {
                            let membership_exists = if bound {
                                room.get_member_no_sync(&bot_user_id).await.ok().flatten().is_some()
                                    || room
                                        .members_no_sync(RoomMemberships::ACTIVE)
                                        .await
                                        .ok()
                                        .is_some_and(|members| members.iter().any(|member| member.user_id().as_str() == bot_user_id.as_str()))
                                    || room
                                        .members(RoomMemberships::ACTIVE)
                                        .await
                                        .ok()
                                        .is_some_and(|members| members.iter().any(|member| member.user_id().as_str() == bot_user_id.as_str()))
                            } else {
                                false
                            };
                            let should_mark_bound = if bound { membership_exists } else { false };

                            if should_mark_bound != bound {
                                error!(
                                    "Failed to {} BotFather {bot_user_id} for room {room_id}: {error:?}",
                                    if bound { "invite" } else { "remove" }
                                );
                                enqueue_popup_notification(
                                    format!(
                                        "Failed to {} BotFather {bot_user_id}: {error}",
                                        if bound { "invite" } else { "remove" }
                                    ),
                                    PopupKind::Error,
                                    None,
                                );
                                return;
                            }

                            Cx::post_action(AppStateAction::BotRoomBindingUpdated {
                                room_id,
                                bound,
                                bot_user_id: Some(bot_user_id),
                                warning: Some(error.to_string()),
                            });
                        }
                    }
                });
            }

            MatrixRequest::JoinRoom { room_id } => {
                let Some(client) = get_client() else { continue };
                let _join_room_task = Handle::current().spawn(async move {
                    log!("Sending request to join room {room_id}...");
                    let result_action = if let Some(room) = client.get_room(&room_id) {
                        if room.state() == RoomState::Joined {
                            log!("Room {room_id} is already joined, skipping join request.");
                            JoinRoomResultAction::Joined { room_id }
                        } else {
                            match room.join().await {
                                Ok(()) => {
                                    log!("Successfully joined known room {room_id}.");
                                    JoinRoomResultAction::Joined { room_id }
                                }
                                Err(e) => {
                                    error!("Error joining known room {room_id}: {e:?}");
                                    JoinRoomResultAction::Failed { room_id, error: e }
                                }
                            }
                        }
                    }
                    else {
                        match client.join_room_by_id(&room_id).await {
                            Ok(_room) => {
                                log!("Successfully joined new unknown room {room_id}.");
                                JoinRoomResultAction::Joined { room_id }
                            }
                            Err(e) => {
                                error!("Error joining new unknown room {room_id}: {e:?}");
                                JoinRoomResultAction::Failed { room_id, error: e }
                            }
                        }
                    };
                    Cx::post_action(result_action);
                });
            }

            MatrixRequest::LeaveRoom { room_id } => {
                let Some(client) = get_client() else { continue };
                let _leave_room_task = Handle::current().spawn(async move {
                    log!("Sending request to leave room {room_id}...");
                    let result_action = if let Some(room) = client.get_room(&room_id) {
                        match room.leave().await {
                            Ok(()) => {
                                log!("Successfully left room {room_id}.");
                                LeaveRoomResultAction::Left { room_id }
                            }
                            Err(e) => {
                                error!("Error leaving room {room_id}: {e:?}");
                                LeaveRoomResultAction::Failed { room_id, error: e }
                            }
                        }
                    } else {
                        error!("BUG: client could not get room with ID {room_id}");
                        LeaveRoomResultAction::Failed {
                            room_id,
                            error: matrix_sdk::Error::UnknownError("Client couldn't locate room to leave it.".into()),
                        }
                    };
                    Cx::post_action(result_action);
                });
            }

            MatrixRequest::ReportRoom { room_id, reason } => {
                let Some(client) = get_client() else { continue };
                let _report_room_task = Handle::current().spawn(async move {
                    log!("Sending request to report room {room_id}...");
                    let result_action = if let Some(room) = client.get_room(&room_id) {
                        match room.report_room(reason).await {
                            Ok(_) => {
                                ReportRoomResultAction::Sent { room_id }
                            }
                            Err(e) => {
                                error!("Error reporting room {room_id}: {e:?}");
                                ReportRoomResultAction::Failed { room_id, error: e }
                            }
                        }
                    } else {
                        error!("BUG: client could not get room with ID {room_id}");
                        ReportRoomResultAction::Failed {
                            room_id,
                            error: matrix_sdk::Error::UnknownError("Client couldn't locate room to report it.".into()),
                        }
                    };
                    Cx::post_action(result_action);
                });
            }

            MatrixRequest::GetRoomMembers { timeline_kind, memberships, local_only } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for get room members request");
                    continue;
                };

                let _get_members_task = Handle::current().spawn(async move {
                    let send_update = |members: Vec<matrix_sdk::room::RoomMember>, source: &str| {
                        log!("{} {} members for {timeline_kind}", source, members.len());
                        if sender.send(TimelineUpdate::RoomMembersListFetched { members }).is_ok() {
                            SignalToUI::set_ui_signal();
                        } else {
                            warning!("Dropping room members list update for {timeline_kind}: timeline receiver was dropped.");
                        }
                    };

                    let room = timeline.room();
                    if local_only {
                        match room.members_no_sync(memberships).await {
                            Ok(members) => send_update(members, "Got"),
                            Err(e) => error!("Failed to get room members (local_only) for {timeline_kind}: {e:?}"),
                        }
                    } else {
                        match room.members(memberships).await {
                            Ok(members) => send_update(members, "Successfully fetched"),
                            Err(e) => error!("Failed to fetch room members for {timeline_kind}: {e:?}"),
                        }
                    }
                });
            }

            MatrixRequest::GetRoomPreview { room_or_alias_id, via, response_mode } => {
                let Some(client) = get_client() else { continue };
                let _fetch_task = Handle::current().spawn(async move {
                    let res = fetch_room_preview_with_avatar(&client, &room_or_alias_id, via).await;
                    match response_mode {
                        RoomPreviewResponseMode::Action => {
                            Cx::post_action(RoomPreviewAction::Fetched(res));
                        }
                        RoomPreviewResponseMode::RoomPreviewCache => match res {
                            Ok(fetched) => enqueue_room_preview_update(RoomPreviewUpdate {
                                room_or_alias_id,
                                fetched,
                            }),
                            Err(e) => log!("Failed to get room preview for {room_or_alias_id:?}: {e:?}"),
                        },
                    }
                });
            }

            MatrixRequest::SearchDirectory { query, kind, limit } => {
                let Some(client) = get_client() else { continue };
                let _search_task = Handle::current().spawn(async move {
                    let query = query.trim().to_owned();
                    let action_kind = kind.clone();
                    if query.is_empty() {
                        Cx::post_action(RoomFilterRemoteSearchAction::Results {
                            query,
                            kind: action_kind,
                            results: Vec::new(),
                        });
                        return;
                    }

                    let result = match &kind {
                        RemoteDirectorySearchKind::People => {
                            let mut users = Vec::new();
                            let mut seen_user_ids = HashSet::new();

                            if let Ok(user_id) = UserId::parse(&query).map(|u| u.to_owned()) {
                                if let Ok(response) = client.account().fetch_user_profile_of(&user_id).await {
                                    if seen_user_ids.insert(user_id.clone()) {
                                        users.push(RemoteDirectorySearchResult::User(UserProfile {
                                            username: response.get_static::<DisplayName>().ok().flatten(),
                                            user_id,
                                            avatar_state: response.get_static::<AvatarUrl>()
                                                .ok()
                                                .map_or(AvatarState::Unknown, AvatarState::Known),
                                        }));
                                    }
                                }
                            }

                            match client.search_users(&query, limit).await {
                                Ok(response) => {
                                    for user in response.results.into_iter() {
                                        if seen_user_ids.insert(user.user_id.clone()) {
                                            users.push(RemoteDirectorySearchResult::User(UserProfile {
                                                username: user.display_name,
                                                user_id: user.user_id,
                                                avatar_state: AvatarState::Known(user.avatar_url),
                                            }));
                                        }
                                        if users.len() >= limit as usize {
                                            break;
                                        }
                                    }
                                    Ok(users)
                                }
                                Err(_e) if !users.is_empty() => Ok(users),
                                Err(e) => Err(e.to_string()),
                            }
                        }
                        RemoteDirectorySearchKind::Rooms | RemoteDirectorySearchKind::Spaces => {
                            let mut filter = PublicRoomsFilter::new();
                            filter.generic_search_term = Some(query.clone());
                            filter.room_types = match &kind {
                                RemoteDirectorySearchKind::Rooms => vec![RoomTypeFilter::Default],
                                RemoteDirectorySearchKind::Spaces => vec![RoomTypeFilter::Space],
                                RemoteDirectorySearchKind::People => Vec::new(),
                            };
                            let mut request = get_public_rooms_filtered::v3::Request::new();
                            request.filter = filter;
                            client.public_rooms_filtered(request).await
                                .map(|response| {
                                    response.chunk.into_iter()
                                        .take(limit as usize)
                                        .map(|room| {
                                            let display_name = room.name
                                                .or_else(|| room.canonical_alias.as_ref().map(ToString::to_string))
                                                .unwrap_or_else(|| room.room_id.to_string());
                                            let room_name_id = RoomNameId::new(
                                                RoomDisplayName::Named(display_name),
                                                room.room_id.clone(),
                                            );
                                            match &kind {
                                                RemoteDirectorySearchKind::Spaces => {
                                                    RemoteDirectorySearchResult::Space {
                                                        space_name_id: room_name_id,
                                                        avatar_uri: room.avatar_url,
                                                    }
                                                }
                                                _ => {
                                                    RemoteDirectorySearchResult::Room {
                                                        room_name_id,
                                                        avatar_uri: room.avatar_url,
                                                    }
                                                }
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .map_err(|e| e.to_string())
                        }
                    };

                    match result {
                        Ok(results) => {
                            Cx::post_action(RoomFilterRemoteSearchAction::Results {
                                query,
                                kind: action_kind,
                                results,
                            });
                        }
                        Err(error) => {
                            Cx::post_action(RoomFilterRemoteSearchAction::Failed {
                                query,
                                kind: action_kind,
                                error,
                            });
                        }
                    }
                });
            }

            MatrixRequest::GetSuccessorRoomDetails { tombstoned_room_id } => {
                let Some(client) = get_client() else { continue };
                let (sender, successor_room) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&tombstoned_room_id) else {
                        error!("BUG: tombstoned room {tombstoned_room_id} info not found for get successor room details request");
                        continue;
                    };
                    (
                        room_info.main_timeline.timeline_update_sender.clone(),
                        room_info.main_timeline.timeline.room().successor_room(),
                    )
                };
                spawn_fetch_successor_room_preview(
                    client,
                    successor_room,
                    tombstoned_room_id,
                    sender,
                );
            }

            MatrixRequest::OpenOrCreateDirectMessage { user_profile, allow_create, create_encrypted } => {
                let Some(client) = get_client() else { continue };
                let _create_dm_task = Handle::current().spawn(async move {
                    let existing_dm = find_reusable_direct_message_room(&client, &user_profile.user_id).await;
                    if let Some(room) = existing_dm {
                        log!("Found existing DM room: {}", room.room_id());
                        Cx::post_action(DirectMessageRoomAction::FoundExisting {
                            user_id: user_profile.user_id,
                            room_name_id: RoomNameId::from_room(&room).await,
                        });
                        return;
                    }
                    if !allow_create {
                        Cx::post_action(DirectMessageRoomAction::DidNotExist { user_profile });
                        return;
                    }
                    log!("Creating new DM room with {user_profile:?}...");
                    let create_dm_result = if create_encrypted {
                        client.create_dm(&user_profile.user_id).await
                    } else {
                        let mut request = CreateRoomRequest::new();
                        request.invite = vec![user_profile.user_id.clone()];
                        request.is_direct = true;
                        request.preset = Some(RoomPreset::TrustedPrivateChat);
                        client.create_room(request).await
                    };
                    match create_dm_result {
                        Ok(room) => {
                            log!("Successfully created DM room: {}", room.room_id());
                            Cx::post_action(DirectMessageRoomAction::NewlyCreated {
                                user_profile,
                                room_name_id: RoomNameId::from_room(&room).await,
                            });
                        },
                        Err(error) => {
                            error!("Failed to create DM with {user_profile:?}: {error}");
                            Cx::post_action(DirectMessageRoomAction::FailedToCreate {
                                user_profile,
                                error,
                            });
                        }
                    }
                });
            }

            MatrixRequest::CreateRoom { room_name, topic, is_public, is_encrypted, parent_space_id, context } => {
                let Some(client) = get_client() else { continue };
                let _create_room_task = Handle::current().spawn(async move {
                    let mut request = CreateRoomRequest::new();
                    request.name = Some(room_name.clone());
                    request.topic = topic;
                    request.visibility = if is_public {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
                    request.preset = Some(if is_public {
                        RoomPreset::PublicChat
                    } else {
                        RoomPreset::PrivateChat
                    });
                    if is_encrypted {
                        request.initial_state.push(
                            InitialStateEvent::with_empty_state_key(
                                RoomEncryptionEventContent::with_recommended_defaults(),
                            ).to_raw_any()
                        );
                    }

                    log!("Creating new room \"{room_name}\"...");
                    match client.create_room(request).await {
                        Ok(room) => {
                            let mut space_link_error = None;
                            if let Some(space_id) = parent_space_id.as_ref()
                                && let Err(error) = attach_room_to_space(&client, &room, space_id).await
                            {
                                error!("Created room {} but failed to add it to space {space_id}: {error}", room.room_id());
                                space_link_error = Some(error.to_string());
                            }

                            let room_name_id = RoomNameId::from_room(&room).await;
                            Cx::post_action(CreateRoomAction::Created {
                                room_name_id,
                                parent_space_id,
                                space_link_error,
                                context,
                            });
                        }
                        Err(error) => {
                            error!("Failed to create room \"{room_name}\": {error}");
                            Cx::post_action(CreateRoomAction::Failed { room_name, error, context });
                        }
                    }
                });
            }

            MatrixRequest::GetCreatableSpaces => {
                let Some(client) = get_client() else { continue };
                let _creatable_spaces_task = Handle::current().spawn(async move {
                    let Some(user_id) = client.user_id().map(ToOwned::to_owned) else {
                        Cx::post_action(CreatableSpacesAction::Loaded { spaces: Vec::new() });
                        return;
                    };

                    let mut spaces = Vec::new();
                    for room in client.joined_rooms() {
                        if room.room_type() != Some(ruma::room::RoomType::Space) {
                            continue;
                        }

                        let Ok(power_levels) = room.power_levels().await else {
                            continue;
                        };
                        if !power_levels.user_can_send_state(&user_id, StateEventType::SpaceChild) {
                            continue;
                        }

                        spaces.push(RoomNameId::from_room(&room).await);
                    }

                    spaces.sort_by_cached_key(|space| space.to_string().to_lowercase());
                    Cx::post_action(CreatableSpacesAction::Loaded { spaces });
                });
            }

            MatrixRequest::GetUserProfile { user_id, room_id, local_only } => {
                let Some(client) = get_client() else { continue };
                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending get user profile request: user: {user_id}, \
                    //     room: {room_id:?}, local_only: {local_only}...",
                    // );

                    let mut update = None;

                    if let Some(room_id) = room_id.as_ref() {
                        if let Some(room) = client.get_room(room_id) {
                            let member = if local_only {
                                room.get_member_no_sync(&user_id).await
                            } else {
                                room.get_member(&user_id).await
                            };
                            if let Ok(Some(room_member)) = member {
                                update = Some(UserProfileUpdate::Full {
                                    new_profile: UserProfile {
                                        username: room_member.display_name().map(|u| u.to_owned()),
                                        user_id: user_id.clone(),
                                        avatar_state: AvatarState::Known(room_member.avatar_url().map(|u| u.to_owned())),
                                    },
                                    room_id: room_id.to_owned(),
                                    room_member,
                                });
                            } else {
                                // log!("User profile request: user {user_id} was not a member of room {room_id}");
                            }
                        } else {
                            log!("User profile request: client could not get room with ID {room_id}");
                        }
                    }

                    if !local_only {
                        if update.is_none() {
                            if let Ok(response) = client.account().fetch_user_profile_of(&user_id).await {
                                update = Some(UserProfileUpdate::UserProfileOnly(
                                    UserProfile {
                                        username: response.get_static::<DisplayName>().ok().flatten(),
                                        user_id: user_id.clone(),
                                        avatar_state: response.get_static::<AvatarUrl>()
                                            .ok()
                                            .map_or(AvatarState::Unknown, AvatarState::Known),
                                    }
                                ));
                            } else {
                                log!("User profile request: client could not get user with ID {user_id}");
                            }
                        }

                        match update.as_mut() {
                            Some(UserProfileUpdate::Full { new_profile: UserProfile { username, .. }, .. }) if username.is_none() => {
                                if let Ok(response) = client.account().fetch_user_profile_of(&user_id).await {
                                    *username = response.get_static::<DisplayName>().ok().flatten();
                                }
                            }
                            _ => { }
                        }
                    }

                    if let Some(upd) = update {
                        // log!("Successfully completed get user profile request: user: {user_id}, room: {room_id:?}, local_only: {local_only}.");
                        enqueue_user_profile_update(upd);
                    } else {
                        log!("Failed to get user profile: user: {user_id}, room: {room_id:?}, local_only: {local_only}.");
                    }
                });
            }

            MatrixRequest::GetNumberUnreadMessages { timeline_kind } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("Skipping get number of unread messages request for {timeline_kind}");
                    continue;
                };

                let _get_unreads_task = Handle::current().spawn(async move {
                    match sender.send(TimelineUpdate::NewUnreadMessagesCount(
                        UnreadMessageCount::Known(timeline.room().num_unread_messages())
                    )) {
                        Ok(_) => SignalToUI::set_ui_signal(),
                        Err(e) => log!("Failed to send timeline update: {e:?} for GetNumberUnreadMessages request for {timeline_kind}"),
                    }
                    if let TimelineKind::MainRoom { room_id } = timeline_kind {
                        enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                            room_id,
                            is_marked_unread: timeline.room().is_marked_unread(),
                            unread_messages: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                            unread_mentions: timeline.room().num_unread_mentions(),
                        });
                    }
                });
            }

            MatrixRequest::SetUnreadFlag { room_id, mark_as_unread } => {
                let Some(main_timeline) = get_room_timeline(&room_id) else {
                    log!("BUG: skipping set unread flag request for not-yet-known room {room_id}");
                    continue;
                };
                let _set_unread_task = Handle::current().spawn(async move {
                    let result = main_timeline.room().set_unread_flag(mark_as_unread).await;
                    match result {
                        Ok(_) => log!("Set unread flag to {} for room {}", mark_as_unread, room_id),
                        Err(e) => error!("Failed to set unread flag to {} for room {}: {:?}", mark_as_unread, room_id, e),
                    }
                });
            }

            MatrixRequest::SetIsFavorite { room_id, is_favorite } => {
                let Some(main_timeline) = get_room_timeline(&room_id) else {
                    log!("BUG: skipping set favorite flag request for not-yet-known room {room_id}");
                    continue;
                };
                let _set_favorite_task = Handle::current().spawn(async move {
                    let result = main_timeline.room().set_is_favourite(is_favorite, None).await;
                    match result {
                        Ok(_) => log!("Set favorite to {} for room {}", is_favorite, room_id),
                        Err(e) => error!("Failed to set favorite to {} for room {}: {:?}", is_favorite, room_id, e),
                    }
                });
            }

            MatrixRequest::SetIsLowPriority { room_id, is_low_priority } => {
                let Some(main_timeline) = get_room_timeline(&room_id) else {
                    log!("BUG: skipping set low priority flag request for not-yet-known room {room_id}");
                    continue;
                };
                let _set_lp_task = Handle::current().spawn(async move {
                    let result = main_timeline.room().set_is_low_priority(is_low_priority, None).await;
                    match result {
                        Ok(_) => log!("Set low priority to {} for room {}", is_low_priority, room_id),
                        Err(e) => error!("Failed to set low priority to {} for room {}: {:?}", is_low_priority, room_id, e),
                    }
                });
            }

            MatrixRequest::UploadAvatar { avatar_path } => {
                let Some(client) = get_client() else { continue };
                let _upload_avatar_task = Handle::current().spawn(async move {
                    let data = match std::fs::read(&avatar_path) {
                        Ok(data) => data,
                        Err(e) => {
                            Cx::post_action(AccountDataAction::AvatarChangeFailed(
                                format!("Failed to read selected avatar file {:?}: {e}", avatar_path)
                            ));
                            return;
                        }
                    };

                    let content_type = match imghdr::from_bytes(&data) {
                        Some(imghdr::Type::Png) => IMAGE_PNG,
                        Some(imghdr::Type::Jpeg) => IMAGE_JPEG,
                        _ => {
                            let ext = avatar_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.to_ascii_lowercase());
                            match ext.as_deref() {
                                Some("png") => IMAGE_PNG,
                                Some("jpg") | Some("jpeg") => IMAGE_JPEG,
                                _ => {
                                    Cx::post_action(AccountDataAction::AvatarChangeFailed(
                                        "Unsupported avatar format. Please choose a PNG or JPEG image.".to_string()
                                    ));
                                    return;
                                }
                            }
                        }
                    };

                    log!("Uploading avatar from file: {:?}", avatar_path);
                    match client.account().upload_avatar(&content_type, data).await {
                        Ok(new_avatar_uri) => {
                            log!("Successfully uploaded avatar.");
                            Cx::post_action(AccountDataAction::AvatarChanged(Some(new_avatar_uri)));
                        }
                        Err(e) => {
                            Cx::post_action(AccountDataAction::AvatarChangeFailed(
                                format!("Failed to upload avatar: {e}")
                            ));
                        }
                    }
                });
            }

            MatrixRequest::SetAvatar { avatar_url } => {
                let Some(client) = get_client() else { continue };
                let _set_avatar_task = Handle::current().spawn(async move {
                    let is_removing = avatar_url.is_none();
                    log!("Sending request to {} avatar...", if is_removing { "remove" } else { "set" });
                    let result = client.account().set_avatar_url(avatar_url.as_deref()).await;
                    match result {
                        Ok(_) => {
                            log!("Successfully {} avatar.", if is_removing { "removed" } else { "set" });
                            Cx::post_action(AccountDataAction::AvatarChanged(avatar_url));
                        }
                        Err(e) => {
                            if is_removing && e.client_api_error_kind() == Some(&ErrorKind::Unrecognized) {
                                log!("Avatar delete endpoint not recognized by homeserver, retrying fallback request...");
                                let Some(user_id) = client.user_id() else {
                                    Cx::post_action(AccountDataAction::AvatarChangeFailed(
                                        "Failed to remove avatar: not authenticated.".to_string()
                                    ));
                                    return;
                                };
                                #[allow(deprecated)]
                                let fallback_result = client.send(
                                    set_avatar_url::v3::Request::new(user_id.to_owned(), None)
                                ).await;
                                match fallback_result {
                                    Ok(_) => {
                                        log!("Successfully removed avatar via fallback endpoint.");
                                        Cx::post_action(AccountDataAction::AvatarChanged(None));
                                    }
                                    Err(fallback_err) => {
                                        let err_msg = format!("Failed to remove avatar: {fallback_err}");
                                        Cx::post_action(AccountDataAction::AvatarChangeFailed(err_msg));
                                    }
                                }
                                return;
                            }
                            let err_msg = format!("Failed to {} avatar: {e}", if is_removing { "remove" } else { "set" });
                            Cx::post_action(AccountDataAction::AvatarChangeFailed(err_msg));
                        }
                    }
                });
            }

            MatrixRequest::SetDisplayName { new_display_name } => {
                let Some(client) = get_client() else { continue };
                let _set_display_name_task = Handle::current().spawn(async move {
                    let is_removing = new_display_name.is_none();
                    log!("Sending request to {} display name{}...",
                        if is_removing { "remove" } else { "set" },
                        new_display_name.as_ref().map(|n| format!(" to '{n}'")).unwrap_or_default()
                    );
                    let result = client.account().set_display_name(new_display_name.as_deref()).await;
                    match result {
                        Ok(_) => {
                            log!("Successfully {} display name.", if is_removing { "removed" } else { "set" });
                            Cx::post_action(AccountDataAction::DisplayNameChanged(new_display_name));
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to {} display name: {e}", if is_removing { "remove" } else { "set" });
                            Cx::post_action(AccountDataAction::DisplayNameChangeFailed(err_msg));
                        }
                    }
                });
            }

            MatrixRequest::GetOwnDevice => {
                let Some(client) = get_client() else { continue };
                let _get_own_device_task = Handle::current().spawn(async move {
                    let device = match client.encryption().get_own_device().await {
                        Ok(device) => device,
                        Err(e) => {
                            error!("Failed to get own device: {e:?}");
                            None
                        }
                    };
                    let device_info = device.map(|device| OwnDeviceInfo {
                        device_id: device.device_id().to_string(),
                        display_name: device.display_name().map(ToOwned::to_owned),
                    });
                    Cx::post_action(AccountDataAction::OwnDeviceFetched(device_info));
                });
            }

            MatrixRequest::GenerateMatrixLink { room_id, event_id, use_matrix_scheme, join_on_click } => {
                let Some(client) = get_client() else { continue };
                let _gen_link_task = Handle::current().spawn(async move {
                    if let Some(room) = client.get_room(&room_id) {
                        let result = if use_matrix_scheme {
                            if let Some(event_id) = event_id {
                                room.matrix_event_permalink(event_id).await
                                    .map(MatrixLinkAction::MatrixUri)
                            } else {
                                room.matrix_permalink(join_on_click).await
                                    .map(MatrixLinkAction::MatrixUri)
                            }
                        } else {
                            if let Some(event_id) = event_id {
                                room.matrix_to_event_permalink(event_id).await
                                    .map(MatrixLinkAction::MatrixToUri)
                            } else {
                                room.matrix_to_permalink().await
                                    .map(MatrixLinkAction::MatrixToUri)
                            }
                        };
    
                        match result {
                            Ok(action) => Cx::post_action(action),
                            Err(e) => Cx::post_action(MatrixLinkAction::Error(e.to_string())),
                        }
                    } else {
                         Cx::post_action(MatrixLinkAction::Error(format!("Room {room_id} not found")));
                    }
                });
            }

            MatrixRequest::IgnoreUser { ignore, room_member, room_id } => {
                let Some(client) = get_client() else { continue };
                let _ignore_task = Handle::current().spawn(async move {
                    let user_id = room_member.user_id();
                    log!("Sending request to {}ignore user: {user_id}...", if ignore { "" } else { "un" });
                    let ignore_result = if ignore {
                        room_member.ignore().await
                    } else {
                        room_member.unignore().await
                    };

                    log!("{} user {user_id} {}",
                        if ignore { "Ignoring" } else { "Unignoring" },
                        if ignore_result.is_ok() { "succeeded." } else { "failed." },
                    );

                    if ignore_result.is_err() {
                        return;
                    }

                    // We need to re-acquire the `RoomMember` object now that its state
                    // has changed, i.e., the user has been (un)ignored.
                    // We then need to send an update to replace the cached `RoomMember`
                    // with the now-stale ignored state.
                    if let Some(room) = client.get_room(&room_id) {
                        if let Ok(Some(new_room_member)) = room.get_member(user_id).await {
                            log!("Enqueueing user profile update for user {user_id}, who went from {}ignored to {}ignored.",
                                if room_member.is_ignored() { "" } else { "un" },
                                if new_room_member.is_ignored() { "" } else { "un" },
                            );
                            enqueue_user_profile_update(UserProfileUpdate::RoomMemberOnly {
                                room_id: room_id.clone(),
                                room_member: new_room_member,
                            });
                        }
                    }

                    // After successfully (un)ignoring a user, all timelines are fully cleared by the Matrix SDK.
                    // Therefore, we need to re-fetch all timelines for all rooms,
                    // and currently the only way to actually accomplish this is via pagination.
                    // See: <https://github.com/matrix-org/matrix-rust-sdk/issues/1703#issuecomment-2250297923>
                    //
                    // Note that here we only proactively re-paginate the *current* room
                    // (the one being viewed by the user when this ignore request was issued),
                    // and all other rooms will be re-paginated in `handle_ignore_user_list_subscriber()`.`
                    submit_async_request(MatrixRequest::PaginateTimeline {
                        timeline_kind: TimelineKind::MainRoom { room_id },
                        num_events: 50,
                        direction: PaginationDirection::Backwards,
                    });
                });
            }

            MatrixRequest::SetRoomMemberPowerLevel { room_id, user_id, room_member_role } => {
                let Some(client) = get_client() else { continue };
                let _set_room_member_power_level_task = Handle::current().spawn(async move {
                    let Some(room) = client.get_room(&room_id) else {
                        enqueue_popup_notification(
                            format!("Failed to update power level for {user_id}: room {room_id} not found."),
                            PopupKind::Error,
                            None,
                        );
                        return;
                    };
                    let Some(acting_user_id) = client.user_id() else {
                        enqueue_popup_notification(
                            "Failed to update power level: not logged in.",
                            PopupKind::Error,
                            None,
                        );
                        return;
                    };

                    let power_levels = match room.power_levels().await {
                        Ok(power_levels) => power_levels,
                        Err(e) => {
                            enqueue_popup_notification(
                                format!("Failed to load current power levels for room {room_id}: {e}"),
                                PopupKind::Error,
                                None,
                            );
                            return;
                        }
                    };

                    if !power_levels.user_can_change_user_power_level(acting_user_id, user_id.as_ref()) {
                        enqueue_popup_notification(
                            format!("You do not have permission to change power level for {user_id}."),
                            PopupKind::Error,
                            None,
                        );
                        return;
                    }

                    let new_level = match room_member_role {
                        Some(RoomMemberRole::Moderator) => int!(50),
                        Some(RoomMemberRole::Creator | RoomMemberRole::Administrator) => int!(100),
                        Some(RoomMemberRole::User) | None => power_levels.users_default,
                    };

                    match room.update_power_levels(vec![(user_id.as_ref(), new_level)]).await {
                        Ok(_) => {
                            enqueue_popup_notification(
                                format!("Updated power level for {user_id}."),
                                PopupKind::Success,
                                Some(3.0),
                            );
                            if let Ok(Some(new_room_member)) = room.get_member(user_id.as_ref()).await {
                                enqueue_user_profile_update(UserProfileUpdate::RoomMemberOnly {
                                    room_id: room_id.clone(),
                                    room_member: new_room_member,
                                });
                            }
                        }
                        Err(e) => {
                            enqueue_popup_notification(
                                format!("Failed to update power level for {user_id}: {e}"),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                });
            }

            MatrixRequest::SendTypingNotice { room_id, typing } => {
                let Some(main_room_timeline) = get_room_timeline(&room_id) else {
                    log!("BUG: skipping send typing notice request for not-yet-known room {room_id}");
                    continue;
                };
                let _typing_task = Handle::current().spawn(async move {
                    if let Err(e) = main_room_timeline.room().typing_notice(typing).await {
                        error!("Failed to send typing notice to room {room_id}: {e:?}");
                    }
                });
            }

            MatrixRequest::SubscribeToTypingNotices { room_id, subscribe } => {
                let (main_timeline, timeline_update_sender, mut typing_notice_receiver) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(jrd) = all_joined_rooms.get_mut(&room_id) else {
                        log!("BUG: room info not found for subscribe to typing notices request, room {room_id}");
                        continue;
                    };
                    let (main_timeline, receiver) = if subscribe {
                        if jrd.typing_notice_subscriber.is_some() {
                            warning!("Note: room {room_id} is already subscribed to typing notices.");
                            continue;
                        } else {
                            let main_timeline = jrd.main_timeline.timeline.clone();
                            let (drop_guard, receiver) = main_timeline.room().subscribe_to_typing_notifications();
                            jrd.typing_notice_subscriber = Some(drop_guard);
                            (main_timeline, receiver)
                        }
                    } else {
                        jrd.typing_notice_subscriber.take();
                        continue;
                    };
                    // Here: we don't have an existing subscriber running, so we fall through and start one.
                    (main_timeline, jrd.main_timeline.timeline_update_sender.clone(), receiver)
                };

                let _typing_notices_task = Handle::current().spawn(async move {
                    while let Ok(user_ids) = typing_notice_receiver.recv().await {
                        // log!("Received typing notifications for room {room_id}: {user_ids:?}");
                        let mut users = Vec::with_capacity(user_ids.len());
                        for user_id in user_ids {
                            let display_name = main_timeline.room()
                                .get_member_no_sync(&user_id)
                                .await
                                .ok()
                                .flatten()
                                .and_then(|m| m.display_name().map(|d| d.to_owned()))
                                .unwrap_or_else(|| user_id.to_string());
                            users.push(display_name);
                        }
                        if let Err(e) = timeline_update_sender.send(TimelineUpdate::TypingUsers { users }) {
                            error!("Error: timeline update sender couldn't send the list of typing users: {e:?}");
                        }
                        SignalToUI::set_ui_signal();
                    }
                    // log!("Note: typing notifications recv loop has ended for room {}", room_id);
                });
            }

            MatrixRequest::SubscribeToOwnUserReadReceiptsChanged { timeline_kind, subscribe } => {
                if !subscribe {
                    if let Some(task_handler) = subscribers_own_user_read_receipts.remove(&timeline_kind) {
                        task_handler.abort();
                    }
                    continue;
                }
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: skipping subscribe to own user read receipts changed request for {timeline_kind}");
                    continue;
                };

                let timeline_kind_clone = timeline_kind.clone();
                let subscribe_own_read_receipt_task = Handle::current().spawn(async move {
                    let update_receiver = timeline.subscribe_own_user_read_receipts_changed().await;
                    pin_mut!(update_receiver);
                    if let Some(client_user_id) = current_user_id() {
                        if let Some((event_id, receipt)) = timeline.latest_user_read_receipt(&client_user_id).await {
                            log!("Received own user read receipt for {timeline_kind}: {receipt:?}, event ID: {event_id:?}");
                            if sender.send(TimelineUpdate::OwnUserReadReceipt(receipt)).is_err() {
                                error!("Failed to send own user read receipt to UI.");
                            }
                        }

                        while update_receiver.next().await.is_some() {
                            if let Some((_, receipt)) = timeline.latest_user_read_receipt(&client_user_id).await {
                                if sender.send(TimelineUpdate::OwnUserReadReceipt(receipt)).is_err() {
                                    error!("Failed to send own user read receipt to UI.");
                                }
                                // When read receipts change (from other devices), update unread count
                                let unread_count = timeline.room().num_unread_messages();
                                let unread_mentions = timeline.room().num_unread_mentions();
                                if sender.send(TimelineUpdate::NewUnreadMessagesCount(
                                    UnreadMessageCount::Known(unread_count)
                                )).is_err() {
                                    error!("Failed to send unread message count update to UI.");
                                }
                                if let TimelineKind::MainRoom { room_id } = &timeline_kind {
                                    // Update the rooms list with new unread counts
                                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                                        room_id: room_id.clone(),
                                        is_marked_unread: timeline.room().is_marked_unread(),
                                        unread_messages: UnreadMessageCount::Known(unread_count),
                                        unread_mentions,
                                    });
                                }
                            }
                        }
                    }
                });
                subscribers_own_user_read_receipts.insert(timeline_kind_clone, subscribe_own_read_receipt_task);
            }

            MatrixRequest::SubscribeToPinnedEvents { room_id, subscribe } => {
                if !subscribe {
                    if let Some(task_handler) = subscribers_pinned_events.remove(&room_id) {
                        task_handler.abort();
                    }
                    continue;
                }
                let kind = TimelineKind::MainRoom { room_id: room_id.clone() };
                let Some((main_timeline, sender)) = get_timeline_and_sender(&kind) else {
                    log!("BUG: skipping subscribe to pinned events request for unknown room {room_id}");
                    continue;
                };
                let subscribe_pinned_events_task = Handle::current().spawn(async move {
                    // Send an initial update, as the stream may not update immediately.
                    let pinned_events = main_timeline.room().pinned_event_ids().unwrap_or_default();
                    match sender.send(TimelineUpdate::PinnedEvents(pinned_events)) {
                        Ok(()) => SignalToUI::set_ui_signal(),
                        Err(_) => log!("Failed to send initial pinned events update to UI."),
                    }
                    let update_receiver = main_timeline.room().pinned_event_ids_stream();
                    pin_mut!(update_receiver);
                    while let Some(pinned_events) = update_receiver.next().await {
                        match sender.send(TimelineUpdate::PinnedEvents(pinned_events)) {
                            Ok(()) => SignalToUI::set_ui_signal(),
                            Err(e) => log!("Failed to send pinned events update: {e:?}"),
                        }
                    }
                });
                subscribers_pinned_events.insert(room_id, subscribe_pinned_events_task);
            }

            MatrixRequest::SpawnSSOServer { brand, homeserver_url, identity_provider_id, proxy } => {
                spawn_sso_server(brand, homeserver_url, identity_provider_id, proxy, login_sender.clone()).await;
            }

            MatrixRequest::FetchAvatar { mxc_uri, on_fetched } => {
                let Some(client) = get_client() else { continue };
                Handle::current().spawn(async move {
                    // log!("Sending fetch avatar request for {mxc_uri:?}...");
                    let media_request = MediaRequestParameters {
                        source: MediaSource::Plain(mxc_uri.clone()),
                        format: AVATAR_THUMBNAIL_FORMAT.into(),
                    };
                    let res = client.media().get_media_content(&media_request, true).await;
                    // log!("Fetched avatar for {mxc_uri:?}, succeeded? {}", res.is_ok());
                    on_fetched(AvatarUpdate { mxc_uri, avatar_data: res.map(|v| v.into()) });
                });
            }

            MatrixRequest::DownloadAndSaveFile { mxc_uri, app_language } => {
                let Some(client) = get_client() else { continue };

                let _download_task = Handle::current().spawn(async move {
                    use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};
                    use crate::i18n::{tr_key, tr_fmt};

                    log!("DownloadAndSaveFile: downloading {mxc_uri}");

                    // Use the client's homeserver URL to construct a direct download URL,
                    // bypassing matrix-sdk's header parsing which fails on non-ASCII Content-Disposition.
                    let server_name = mxc_uri.server_name().map(|s| s.to_string()).unwrap_or_default();
                    let media_id = mxc_uri.media_id().map(|s| s.to_string()).unwrap_or_default();

                    let homeserver = client.homeserver().to_string();
                    let homeserver = homeserver.trim_end_matches('/');
                    let download_url = format!(
                        "{homeserver}/_matrix/media/v3/download/{server_name}/{media_id}",
                    );

                    let http_client = matrix_sdk::reqwest::Client::new();
                    match http_client.get(&download_url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            // Extract filename from Content-Disposition header or use media_id
                            let filename = resp.headers()
                                .get("content-disposition")
                                .and_then(|v: &matrix_sdk::reqwest::header::HeaderValue| {
                                    let val = String::from_utf8_lossy(v.as_bytes());
                                    // Parse filename="..." or filename*=UTF-8''...
                                    val.split("filename=").nth(1)
                                        .or_else(|| val.split("filename*=").nth(1))
                                        .map(|s| s.trim_matches(|c: char| c == '"' || c == '\'' || c == ';' || c == ' ').to_string())
                                })
                                .unwrap_or_else(|| format!("robrix_{media_id}"));

                            match resp.bytes().await {
                                Ok(data) => {
                                    let downloads_dir = crate::app_data_dir().join("downloads");
                                    if let Err(e) = std::fs::create_dir_all(&downloads_dir) {
                                        error!("Failed to create downloads dir: {e:?}");
                                        return;
                                    }
                                    let dest = downloads_dir.join(&filename);
                                    match std::fs::write(&dest, &data) {
                                        Ok(()) => {
                                            log!("DownloadAndSaveFile: saved to {}", dest.display());
                                            let dest_str = dest.display().to_string();
                                            enqueue_popup_notification(
                                                tr_fmt(app_language, "room_screen.file.saved_at", &[("path", &dest_str)]),
                                                PopupKind::Success,
                                                Some(8.0),
                                            );
                                            // Try to open with system handler
                                            if let Err(e) = robius_open::Uri::new(&format!("file://{dest_str}")).open() {
                                                log!("Could not open file: {e:?}");
                                            }
                                            SignalToUI::set_ui_signal();
                                        }
                                        Err(e) => {
                                            error!("DownloadAndSaveFile: write failed: {e:?}");
                                            enqueue_popup_notification(
                                                tr_key(app_language, "room_screen.file.save_failed").to_string(),
                                                PopupKind::Error,
                                                Some(6.0),
                                            );
                                            SignalToUI::set_ui_signal();
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("DownloadAndSaveFile: failed to read response body: {e:?}");
                                    enqueue_popup_notification(
                                        tr_key(app_language, "room_screen.file.download_failed").to_string(),
                                        PopupKind::Error,
                                        Some(6.0),
                                    );
                                    SignalToUI::set_ui_signal();
                                }
                            }
                        }
                        Ok(resp) => {
                            error!("DownloadAndSaveFile: server returned {}", resp.status());
                            enqueue_popup_notification(
                                tr_key(app_language, "room_screen.file.download_failed").to_string(),
                                PopupKind::Error,
                                Some(6.0),
                            );
                            SignalToUI::set_ui_signal();
                        }
                        Err(e) => {
                            error!("DownloadAndSaveFile: request failed: {e:?}");
                            enqueue_popup_notification(
                                tr_key(app_language, "room_screen.file.download_failed").to_string(),
                                PopupKind::Error,
                                Some(6.0),
                            );
                            SignalToUI::set_ui_signal();
                        }
                    }
                });
            }

            MatrixRequest::FetchMedia { media_request, on_fetched, destination, update_sender } => {
                let Some(client) = get_client() else { continue };
                
                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending fetch media request for {media_request:?}...");
                    let res = client.media().get_media_content(&media_request, true).await;
                    on_fetched(&destination, media_request, res, update_sender);
                });
            }

            MatrixRequest::SendMessage {
                timeline_kind,
                message,
                replied_to,
                target_user_id,
                explicit_room,
                #[cfg(feature = "tsp")]
                sign_with_tsp,
            } => {
                // TODO: use this timeline `_sender` once we support sending-message status/operations in the UI.
                let Some((timeline, _sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for send message request");
                    continue;
                };

                // Spawn a new async task that will send the actual message.
                let _send_message_task = Handle::current().spawn(async move {
                    log!("Sending message to {timeline_kind}: {message:?}...");
                    let message = {
                        #[cfg(not(feature = "tsp"))] {
                            message
                        }

                        #[cfg(feature = "tsp")] {
                            let mut message = message;
                            if sign_with_tsp {
                                log!("Signing message with TSP...");
                                match serde_json::to_vec(&message) {
                                    Ok(message_bytes) => {
                                        log!("Serialized message to bytes, length {}", message_bytes.len());
                                        match crate::tsp::sign_anycast_with_default_vid(&message_bytes) {
                                            Ok(signed_msg) => {
                                                log!("Successfully signed message with TSP, length {}", signed_msg.len());
                                                use matrix_sdk::ruma::serde::Base64;
                                                message.tsp_signature = Some(Base64::new(signed_msg));
                                            }
                                            Err(e) => {
                                                error!("Failed to sign message with TSP: {e:?}");
                                                enqueue_popup_notification(
                                                    format!("Failed to sign message with TSP: {e}"),
                                                    PopupKind::Error,
                                                    None,
                                                );
                                                return;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to serialize message to bytes for TSP signing: {e:?}");
                                        enqueue_popup_notification(
                                            format!("Failed to serialize message for TSP signing: {e}"),
                                            PopupKind::Error,
                                            None,
                                        );
                                        return;
                                    }
                                }
                            }
                            message
                        }
                    };

                    if let Some(replied_to_info) = replied_to {
                        let reply_content = match timeline
                            .room()
                            .make_reply_event(message.into(), replied_to_info)
                            .await
                        {
                            Ok(content) => content,
                            Err(_e) => {
                                error!("Failed to build reply content to send to {timeline_kind}: {_e:?}");
                                enqueue_popup_notification(
                                    format!("Failed to send reply: {_e}"),
                                    PopupKind::Error,
                                    None,
                                );
                                return;
                            }
                        };

                        if target_user_id.is_some() || explicit_room {
                            let target_user_id = target_user_id.as_ref();
                            if let Some(target_user_id) = target_user_id
                                && let Err(_e) = ensure_target_user_joined_room(
                                    timeline.room(),
                                    target_user_id.as_ref(),
                                )
                                .await
                            {
                                error!("Failed to ensure targeted bot {target_user_id} joined {timeline_kind}: {_e:?}");
                                enqueue_popup_notification(
                                    format!("Failed to invite {target_user_id} into this room: {_e}"),
                                    PopupKind::Error,
                                    None,
                                );
                                return;
                            }

                            let raw_content = match serde_json::to_value(&reply_content) {
                                Ok(content) => add_octos_routing_metadata(
                                    content,
                                    target_user_id.map(|user_id| user_id.as_ref()),
                                    explicit_room,
                                ),
                                Err(_e) => {
                                    error!("Failed to serialize reply content for {timeline_kind}: {_e:?}");
                                    enqueue_popup_notification(
                                        format!("Failed to send reply: {_e}"),
                                        PopupKind::Error,
                                        None,
                                    );
                                    return;
                                }
                            };
                            match timeline.room().send_raw("m.room.message", raw_content).await {
                                Ok(_response) => {
                                    if target_user_id.is_some() {
                                        log!("Sent targeted reply message to {timeline_kind}.");
                                    } else {
                                        log!("Sent explicit-room reply message to {timeline_kind}.");
                                    }
                                }
                                Err(_e) => {
                                    error!("Failed to send reply message to {timeline_kind}: {_e:?}");
                                    enqueue_popup_notification(format!("Failed to send reply: {_e}"), PopupKind::Error, None);
                                }
                            }
                        } else {
                            match timeline.send(reply_content.into()).await {
                                Ok(_send_handle) => log!("Sent reply message to {timeline_kind}."),
                                Err(_e) => {
                                    error!("Failed to send reply message to {timeline_kind}: {_e:?}");
                                    enqueue_popup_notification(format!("Failed to send reply: {_e}"), PopupKind::Error, None);
                                }
                            }
                        }
                    } else if target_user_id.is_some() || explicit_room {
                        let target_user_id = target_user_id.as_ref();
                        if let Some(target_user_id) = target_user_id
                            && let Err(_e) = ensure_target_user_joined_room(
                                timeline.room(),
                                target_user_id.as_ref(),
                            )
                            .await
                        {
                            error!("Failed to ensure targeted bot {target_user_id} joined {timeline_kind}: {_e:?}");
                            enqueue_popup_notification(
                                format!("Failed to invite {target_user_id} into this room: {_e}"),
                                PopupKind::Error,
                                None,
                            );
                            return;
                        }

                        let raw_content = match serde_json::to_value(&message) {
                            Ok(content) => add_octos_routing_metadata(
                                content,
                                target_user_id.map(|user_id| user_id.as_ref()),
                                explicit_room,
                            ),
                            Err(_e) => {
                                error!("Failed to serialize message content for {timeline_kind}: {_e:?}");
                                enqueue_popup_notification(
                                    format!("Failed to send message: {_e}"),
                                    PopupKind::Error,
                                    None,
                                );
                                return;
                            }
                        };
                        match timeline.room().send_raw("m.room.message", raw_content).await {
                            Ok(_response) => {
                                if target_user_id.is_some() {
                                    log!("Sent targeted message to {timeline_kind}.");
                                } else {
                                    log!("Sent explicit-room message to {timeline_kind}.");
                                }
                            }
                            Err(_e) => {
                                error!("Failed to send message to {timeline_kind}: {_e:?}");
                                enqueue_popup_notification(format!("Failed to send message: {_e}"), PopupKind::Error, None);
                            }
                        }
                    } else {
                        match timeline.send(message.into()).await {
                            Ok(_send_handle) => log!("Sent message to {timeline_kind}."),
                            Err(_e) => {
                                error!("Failed to send message to {timeline_kind}: {_e:?}");
                                enqueue_popup_notification(format!("Failed to send message: {_e}"), PopupKind::Error, None);
                            }
                        }
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::ForwardMessage {
                source_room_id,
                source_event_id,
                destination_room_id,
                message,
            } => {
                let Some(client) = get_client() else {
                    enqueue_popup_notification(
                        "Cannot forward message: Matrix client is not ready.",
                        PopupKind::Error,
                        None,
                    );
                    continue;
                };

                let _forward_message_task = Handle::current().spawn(async move {
                    let Some(destination_room) = client.get_room(&destination_room_id) else {
                        enqueue_popup_notification(
                            format!("Cannot forward message: room {destination_room_id} is not known locally."),
                            PopupKind::Error,
                            None,
                        );
                        SignalToUI::set_ui_signal();
                        return;
                    };
                    if destination_room.state() != RoomState::Joined {
                        enqueue_popup_notification(
                            format!("Cannot forward message: not joined to {destination_room_id}."),
                            PopupKind::Error,
                            None,
                        );
                        SignalToUI::set_ui_signal();
                        return;
                    }

                    match destination_room.send(message).await {
                        Ok(_response) => {
                            log!(
                                "Forwarded message {source_event_id} from {source_room_id} to {destination_room_id}."
                            );
                            enqueue_popup_notification(
                                forward_success_feedback_text(destination_room_id.as_ref()),
                                PopupKind::Info,
                                Some(4.0),
                            );
                        }
                        Err(error) => {
                            error!(
                                "Failed to forward message {source_event_id} from {source_room_id} to {destination_room_id}: {error:?}"
                            );
                            enqueue_popup_notification(
                                forward_failure_feedback_text(&error),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::SendActionResponse {
                timeline_kind,
                content,
                target_user_id,
                explicit_room,
                source_event_id,
            } => {
                let Some((timeline, _sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for send action response request");
                    continue;
                };
                let room_id = timeline_kind.room_id().to_owned();

                let _send_action_response_task = Handle::current().spawn(async move {
                    if let Err(error) = ensure_target_user_joined_room(
                        timeline.room(),
                        target_user_id.as_ref(),
                    )
                    .await
                    {
                        error!("Failed to ensure targeted bot {target_user_id} joined {timeline_kind}: {error:?}");
                        Cx::post_action(ActionResponseResultAction::Failed {
                            room_id,
                            source_event_id,
                            error: error.to_string(),
                        });
                        return;
                    }

                    let raw_content = add_octos_routing_metadata(
                        content,
                        Some(target_user_id.as_ref()),
                        explicit_room,
                    );
                    match timeline.room().send_raw("m.room.message", raw_content).await {
                        Ok(_response) => {
                            log!("Sent action response message to {timeline_kind}.");
                            Cx::post_action(ActionResponseResultAction::Sent {
                                room_id,
                                source_event_id,
                            });
                        }
                        Err(error) => {
                            error!("Failed to send action response to {timeline_kind}: {error:?}");
                            Cx::post_action(ActionResponseResultAction::Failed {
                                room_id,
                                source_event_id,
                                error: error.to_string(),
                            });
                        }
                    }
                });
            }

            MatrixRequest::SendSticker { timeline_kind, body, mxc_url, width, height, size } => {
                log!("[sticker-dbg] LAYER4: MatrixRequest::SendSticker received timeline={timeline_kind} body={body:?} mxc={mxc_url:?} w={width} h={height} size={size}");
                let Some((timeline, _sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for send sticker request");
                    continue;
                };
                if mxc_url.is_empty() {
                    log!("SendSticker: mxc_url is empty, skipping");
                    continue;
                }
                let _task = Handle::current().spawn(async move {
                    use ruma::events::room::{MediaSource, ThumbnailInfo};
                    use ruma::UInt;
                    let mxc_uri = OwnedMxcUri::from(mxc_url);
                    let mut info = ImageInfo::new();
                    let mimetype = "image/png".to_owned();
                    let uint_w = if width > 0 { Some(UInt::from(width)) } else { None };
                    let uint_h = if height > 0 { Some(UInt::from(height)) } else { None };
                    let uint_s = if size > 0 { UInt::try_from(size).ok() } else { None };
                    info.width = uint_w;
                    info.height = uint_h;
                    info.size = uint_s;
                    info.mimetype = Some(mimetype.clone());
                    info.thumbnail_source = Some(MediaSource::Plain(mxc_uri.clone()));
                    info.thumbnail_info = Some(Box::new({
                        let mut ti = ThumbnailInfo::new();
                        ti.width = uint_w;
                        ti.height = uint_h;
                        ti.size = uint_s;
                        ti.mimetype = Some(mimetype);
                        ti
                    }));
                    let content = AnyMessageLikeEventContent::Sticker(
                        StickerEventContent::new(body, info, mxc_uri),
                    );
                    log!("[sticker-dbg] LAYER4-async: calling timeline.send() for {timeline_kind}");
                    match timeline.send(content).await {
                        Ok(_) => log!("[sticker-dbg] LAYER4-async: OK sent sticker to {timeline_kind}."),
                        Err(e) => {
                            error!("[sticker-dbg] LAYER4-async: FAILED send sticker to {timeline_kind}: {e:?}");
                            enqueue_popup_notification(
                                format!("Failed to send sticker: {e}"),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::SendAttachment {
                timeline_kind,
                file_data,
                replied_to,
                #[cfg(feature = "tsp")]
                sign_with_tsp: _sign_with_tsp,
            } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for send attachment request");
                    continue;
                };

                // Spawn a new async task to send the attachment.
                let _send_attachment_task = Handle::current().spawn(async move {
                    use matrix_sdk::attachment::AttachmentConfig;
                    use eyeball::SharedObservable;

                    log!("Sending attachment to {timeline_kind}: {} ({} bytes)...",
                        file_data.name, file_data.size);

                    // For now, we'll just send the attachment without reply support
                    // TODO: Add proper reply support for attachments
                    let _ = replied_to; // Suppress unused warning for now

                    // Parse MIME type
                    let content_type: mime::Mime = file_data.mime_type.parse()
                        .unwrap_or_else(|_| "application/octet-stream".parse().unwrap());

                    // Create a progress observable to track upload progress
                    let send_progress: SharedObservable<matrix_sdk::TransmissionProgress> = Default::default();
                    let progress_subscriber = send_progress.subscribe();

                    // Spawn a task to handle progress updates
                    let sender_clone = sender.clone();
                    Handle::current().spawn(async move {
                        let mut subscriber = progress_subscriber;
                        loop {
                            let progress = subscriber.get();
                            let current: u64 = progress.current as u64;
                            let total: u64 = progress.total as u64;
                            if sender_clone.send(TimelineUpdate::FileUploadUpdate {
                                current,
                                total,
                            }).is_err() {
                                break;
                            }
                            SignalToUI::set_ui_signal();
                            // Wait for next update
                            if subscriber.next().await.is_none() {
                                break;
                            }
                        }
                    });

                    // Use the Room's send_attachment method directly
                    let room = timeline.room();
                    let config = AttachmentConfig::new();

                    let send_future = room.send_attachment(
                        &file_data.name,
                        &content_type,
                        file_data.data.clone(),
                        config,
                    ).with_send_progress_observable(send_progress);

                    match send_future.await {
                        Ok(_response) => {
                            log!("Successfully sent attachment to {timeline_kind}.");
                            let _ = sender.send(TimelineUpdate::FileUploadComplete);
                        }
                        Err(e) => {
                            error!("Failed to send attachment to {timeline_kind}: {e:?}");
                            let _ = sender.send(TimelineUpdate::FileUploadError {
                                error: format!("{e}"),
                                file_data: file_data.clone(),
                            });
                            enqueue_popup_notification(
                                format!("Failed to upload file: {e}"),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }

                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::ReadReceipt { timeline_kind, event_id, receipt_type } => {
                let Some(timeline) = get_timeline(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found when sending read receipt, {event_id}");
                    continue;
                };

                let _send_rr_task = Handle::current().spawn(async move {
                    match timeline.send_single_receipt(receipt_type.clone(), event_id.clone()).await {
                        Ok(sent) => log!("{} {receipt_type} read receipt to {timeline_kind} for event {event_id}", if sent { "Sent" } else { "Already sent" }),
                        Err(_e) => error!("Failed to send {receipt_type} read receipt to {timeline_kind} for event {event_id}; error: {_e:?}"),
                    }
                    if let TimelineKind::MainRoom { room_id } = timeline_kind {
                        // Also update the number of unread messages in the room.
                        enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                            room_id,
                            is_marked_unread: timeline.room().is_marked_unread(),
                            unread_messages: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                            unread_mentions: timeline.room().num_unread_mentions()
                        });
                    }
                });
            },

            MatrixRequest::GetRoomPowerLevels { timeline_kind } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for room power levels request");
                    continue;
                };

                let Some(user_id) = current_user_id() else { continue };

                let _power_levels_task = Handle::current().spawn(async move {
                    match timeline.room().power_levels().await {
                        Ok(power_levels) => {
                            log!("Successfully fetched power levels for {timeline_kind}.");
                            if sender.send(TimelineUpdate::UserPowerLevels(
                                UserPowerLevels::from(&power_levels, &user_id),
                            )).is_err() {
                                error!("Failed to send room power levels to UI.")
                            }
                            SignalToUI::set_ui_signal();
                        }
                        Err(e) => {
                            error!("Failed to fetch power levels for {timeline_kind}: {e:?}");
                        }
                    }
                });
            },

            MatrixRequest::ToggleReaction { timeline_kind, timeline_event_id, reaction } => {
                let Some(timeline) = get_timeline(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for toggle reaction request");
                    continue;
                };

                let _toggle_reaction_task = Handle::current().spawn(async move {
                    log!("Sending toggle reaction {reaction:?} to {timeline_kind}: ...");
                    match timeline.toggle_reaction(&timeline_event_id, &reaction).await {
                        Ok(_send_handle) => {
                            log!("Sent toggle reaction {reaction:?} to {timeline_kind}.");
                            SignalToUI::set_ui_signal();
                        },
                        Err(_e) => error!("Failed to send toggle reaction to {timeline_kind}; error: {_e:?}"),
                    }
                });
            },

            MatrixRequest::RedactMessage { timeline_kind, timeline_event_id, reason } => {
                let Some(timeline) = get_timeline(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for redact message request");
                    continue;
                };

                let _redact_task = Handle::current().spawn(async move {
                    match timeline.redact(&timeline_event_id, reason.as_deref()).await {
                        Ok(()) => log!("Successfully redacted message in {timeline_kind}."),
                        Err(e) => {
                            error!("Failed to redact message in {timeline_kind}; error: {e:?}");
                            enqueue_popup_notification(
                                format!("Failed to redact message. Error: {e}"),
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                });
            },

            MatrixRequest::PinEvent { timeline_kind, event_id, pin } => {
                let Some((timeline, sender)) = get_timeline_and_sender(&timeline_kind) else {
                    log!("BUG: {timeline_kind} not found for pin event request");
                    continue;
                };

                let _pin_task = Handle::current().spawn(async move {
                    let room = timeline.room();
                    let result = if pin {
                        room.pin_event(&event_id).await
                    } else {
                        room.unpin_event(&event_id).await
                    };
                    match sender.send(TimelineUpdate::PinResult { event_id, pin, result }) {
                        Ok(_) => SignalToUI::set_ui_signal(),
                        Err(_) => log!("Failed to send UI update for pin event."),
                    }
                });
            }

            MatrixRequest::GetUrlPreview { url, on_fetched, destination, update_sender } => {
                // const MAX_LOG_RESPONSE_BODY_LENGTH: usize = 1000;
                // log!("Starting URL preview fetch for: {}", url);
                let _fetch_url_preview_task = Handle::current().spawn(async move {
                    let result: Result<LinkPreviewData, UrlPreviewError> = async {
                        // log!("Getting Matrix client for URL preview: {}", url);
                        let client = get_client().ok_or(UrlPreviewError::ClientNotAvailable)?;

                        let token = client.access_token().ok_or(UrlPreviewError::AccessTokenNotAvailable)?;
                        // Official Doc: https://spec.matrix.org/v1.11/client-server-api/#get_matrixclientv1mediapreview_url
                        // Element desktop is using /_matrix/media/v3/preview_url
                        let mut endpoint_url = client.homeserver().join("/_matrix/client/v1/media/preview_url")
                            .map_err(UrlPreviewError::UrlParse)?;
                        endpoint_url.query_pairs_mut().append_pair("url", url.as_str());
                        // log!("Fetching URL preview from endpoint: {} for URL: {}", endpoint_url, url);

                        let response = client
                            .http_client()
                            .get(endpoint_url.clone())
                            .bearer_auth(token)
                            .header("Content-Type", "application/json")
                            .send()
                            .await
                            .map_err(UrlPreviewError::Request)?;

                        let status = response.status();
                        // log!("URL preview response status for {}: {}", url, status);

                        if !status.is_success() && status.as_u16() != 429 {
                            // error!("URL preview request failed with status {} for URL: {}", status, url);
                            return Err(UrlPreviewError::HttpStatus(status.as_u16()));
                        }

                        let text = response.text().await.map_err(UrlPreviewError::Request)?;
                        
                        // log!("URL preview response body length for {}: {} bytes", url, text.len());
                        // if text.len() > MAX_LOG_RESPONSE_BODY_LENGTH {
                        //     log!("URL preview response body preview for {}: {}...", url, &text[..MAX_LOG_RESPONSE_BODY_LENGTH]);
                        // } else {
                        //     log!("URL preview response body for {}: {}", url, text);
                        // }
                        // This request is rate limited, retry after a duration we get from the server.
                        if status.as_u16() == 429 {
                            let link_preview_429_res = serde_json::from_str::<LinkPreviewRateLimitResponse>(&text)
                                .map_err(|e| {
                                    // error!("Failed to parse as LinkPreviewRateLimitResponse for URL preview {}: {}", url, e);
                                    UrlPreviewError::Json(e)
                            });
                            match link_preview_429_res {
                                Ok(link_preview_429_res) => {
                                    if let Some(retry_after) = link_preview_429_res.retry_after_ms {
                                        tokio::time::sleep(Duration::from_millis(retry_after.into())).await;
                                        submit_async_request(MatrixRequest::GetUrlPreview{
                                            url: url.clone(),
                                            on_fetched,
                                            destination: destination.clone(),
                                            update_sender: update_sender.clone(),
                                        });
                                        
                                    }
                                }
                                Err(_e) => {
                                    // error!("Failed to parse as LinkPreviewRateLimitResponse for URL preview {}: {}", url, _e);
                                }
                            }
                            return Err(UrlPreviewError::HttpStatus(429));
                        }
                        serde_json::from_str::<LinkPreviewData>(&text)
                            .or_else(|_first_error| {
                                // log!("Failed to parse as LinkPreviewData, trying LinkPreviewDataNonNumeric for URL: {}", url);
                                serde_json::from_str::<LinkPreviewDataNonNumeric>(&text)
                                    .map(|non_numeric| non_numeric.into())
                            })
                            .map_err(|e| {
                                // error!("Failed to parse JSON response for URL preview {}: {}", url, e);
                                // error!("Response body that failed to parse: {}", text);
                                UrlPreviewError::Json(e)
                            })
                    }.await;

                    // match &result {
                    //     Ok(preview_data) => {
                    //         log!("Successfully fetched URL preview for {}: title: {:?}, site_name: {:?}", 
                    //              url, preview_data.title, preview_data.site_name);
                    //     }
                    //     Err(e) => {
                    //         error!("URL preview fetch failed for {}: {}", url, e);
                    //     }
                    // }

                    on_fetched(url, destination, result, update_sender);
                    SignalToUI::set_ui_signal();
                });
            }
        }
    }

    if worker_shutdown_is_unexpected(is_logout_in_progress(), is_account_switch_pending()) {
        error!("matrix_worker_task task ended unexpectedly");
        bail!("matrix_worker_task task ended unexpectedly")
    }

    Ok(())
}

fn worker_shutdown_is_unexpected(logout_in_progress: bool, account_switch_pending: bool) -> bool {
    !logout_in_progress && !account_switch_pending
}

fn should_prebuild_default_sso_client(
    most_recent_user_id: Option<&UserId>,
    cli_has_valid_username_password: bool,
) -> bool {
    most_recent_user_id.is_none() && !cli_has_valid_username_password
}

/// Path to the marker file that records a previous [`DEFAULT_SSO_CLIENT`] pre-build
/// failure on this device, so subsequent startups can skip the noisy attempt.
fn sso_prebuild_failure_flag_path() -> PathBuf {
    app_data_dir().join(".sso_prebuild_failed")
}

/// Records that the [`DEFAULT_SSO_CLIENT`] pre-build failed on this device,
/// so future startups skip the attempt instead of re-spamming matrix-sdk error logs.
///
/// Safe to call from a fresh install: the parent directory is created on demand.
/// Errors are swallowed: at worst the flag isn't persisted and the noise repeats once more.
fn record_sso_prebuild_failure_flag() {
    let _ = std::fs::create_dir_all(app_data_dir());
    let _ = std::fs::write(sso_prebuild_failure_flag_path(), b"");
}

/// Clears the [`DEFAULT_SSO_CLIENT`] pre-build skip flag, if present.
///
/// Called after any successful pre-build or login, so a working network restores
/// the optimization automatically without manual filesystem intervention.
fn clear_sso_prebuild_failure_flag() {
    let _ = std::fs::remove_file(sso_prebuild_failure_flag_path());
}

async fn attach_room_to_space(client: &Client, child_room: &Room, space_id: &OwnedRoomId) -> Result<()> {
    let user_id = client.user_id().ok_or_else(|| anyhow!("Current user ID not found"))?;
    let space_room = client.get_room(space_id)
        .ok_or_else(|| anyhow!("Selected space {space_id} was not found"))?;
    let child_power_levels = child_room.power_levels().await?;

    let child_route = room_route_with_fallback(child_room).await;
    space_room
        .send_state_event_for_key(child_room.room_id(), SpaceChildEventContent::new(child_route))
        .await?;

    if child_power_levels.user_can_send_state(user_id, StateEventType::SpaceParent) {
        let mut parent_content = SpaceParentEventContent::new(room_route_with_fallback(&space_room).await);
        parent_content.canonical = true;
        child_room
            .send_state_event_for_key(space_room.room_id(), parent_content)
            .await?;
    }

    Ok(())
}

async fn room_route_with_fallback(room: &Room) -> Vec<OwnedServerName> {
    match room.route().await {
        Ok(route) if !route.is_empty() => route,
        Ok(_) | Err(_) => room.room_id()
            .server_name()
            .map(ToOwned::to_owned)
            .into_iter()
            .collect(),
    }
}


/// The single global Tokio runtime that is used by all async tasks.
static TOKIO_RUNTIME: Mutex<Option<tokio::runtime::Runtime>> = Mutex::new(None);

/// The sender used by [`submit_async_request`] to send requests to the async worker thread.
/// Currently there is only one, but it can be cloned if we need more concurrent senders.
static REQUEST_SENDER: Mutex<Option<UnboundedSender<MatrixRequest>>> = Mutex::new(None);

/// A client object that is proactively created during initialization
/// in order to speed up the client-building process when the user logs in.
static DEFAULT_SSO_CLIENT: Mutex<Option<(Client, ClientSessionPersisted)>> = Mutex::new(None);

/// Used to notify the SSO login task that the async creation of the `DEFAULT_SSO_CLIENT` has finished.
static DEFAULT_SSO_CLIENT_NOTIFIER: LazyLock<Arc<Notify>> = LazyLock::new(|| Arc::new(Notify::new()));

/// Blocks the current thread until the given future completes.
///
/// ## Warning
/// This should be used with caution, especially on the main UI thread,
/// as blocking a thread prevents it from handling other events or running other tasks.
pub fn block_on_async_with_timeout<T>(
    timeout: Option<Duration>,
    async_future: impl Future<Output = T>,
) -> Result<T, Elapsed> {
    let rt = TOKIO_RUNTIME.lock().unwrap().get_or_insert_with(||
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
    ).handle().clone();

    if let Some(timeout) = timeout {
        rt.block_on(async {
            tokio::time::timeout(timeout, async_future).await
        })
    } else {
        Ok(rt.block_on(async_future))
    }
}


/// The primary initialization routine for starting the Matrix client sync
/// and the async tokio runtime.
///
/// Returns a handle to the Tokio runtime that is used to run async background tasks.
pub fn start_matrix_tokio() -> Result<tokio::runtime::Handle> {
    crate::proxy_config::load_and_apply_saved_proxy_to_process_env();

    // Create a Tokio runtime, and save it in a static variable to ensure it isn't dropped.
    let rt_handle = TOKIO_RUNTIME.lock().unwrap().get_or_insert_with(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
    }).handle().clone();

    // Proactively build a Matrix Client in the background so that the SSO Server
    // can have a quicker start if needed (as it's rather slow to build this client).
    rt_handle.spawn(async move {
        let cli_has_valid_username_password = Cli::try_parse()
            .as_ref()
            .is_ok_and(|cli| !cli.user_id.is_empty() && !cli.password.is_empty());
        let most_recent_user_id = persistence::most_recent_user_id().await;
        if !should_prebuild_default_sso_client(
            most_recent_user_id.as_deref(),
            cli_has_valid_username_password,
        ) {
            DEFAULT_SSO_CLIENT_NOTIFIER.notify_one();
            Cx::post_action(LoginAction::SsoPending(false));
            return;
        }

        // If this device previously failed to reach the default homeserver during pre-build,
        // skip the attempt entirely to avoid spamming error logs from matrix-sdk internals.
        // The SSO login path always falls back to building a fresh client on click.
        // The flag is cleared after any successful login, so a working network
        // restores the optimization automatically without manual intervention.
        if sso_prebuild_failure_flag_path().exists() {
            log!("Skipping DEFAULT_SSO_CLIENT pre-build (previously failed on this device; SSO login will build a fresh client on click).");
            DEFAULT_SSO_CLIENT_NOTIFIER.notify_one();
            Cx::post_action(LoginAction::SsoPending(false));
            return;
        }

        match build_client(&Cli::default(), app_data_dir()).await {
            Ok(client_and_session) => {
                DEFAULT_SSO_CLIENT.lock().unwrap()
                    .get_or_insert(client_and_session);
                // Clear any stale failure flag (e.g., after the user configures a proxy).
                clear_sso_prebuild_failure_flag();
            }
            Err(e) => {
                // If the user has already logged in (e.g. password or custom-homeserver SSO)
                // while the pre-build was still racing the network, do NOT record a failure:
                // we'd be writing the flag right after the post-login cleanup just cleared it,
                // which would permanently disable the optimization for the wrong reason.
                if get_client().is_some() {
                    log!("DEFAULT_SSO_CLIENT pre-build failed after user already logged in; not recording skip flag. Cause: {e}");
                } else {
                    record_sso_prebuild_failure_flag();
                    warning!(
                        "DEFAULT_SSO_CLIENT pre-build failed; SSO login will build a fresh client on click. \
                         Cause: {e}"
                    );
                }
            }
        };
        DEFAULT_SSO_CLIENT_NOTIFIER.notify_one();
        Cx::post_action(LoginAction::SsoPending(false));
    });

    let rt = rt_handle.clone();
    // Spawn the main async task that drives the Matrix client SDK, which itself will
    // start and monitor other related background tasks.
    rt_handle.spawn(start_matrix_client_login_and_sync(rt));

    Ok(rt_handle)
}


/// A tokio::watch channel sender for sending requests from the RoomScreen UI widget
/// to the corresponding background async task for that room (its `timeline_subscriber_handler`).
pub type TimelineRequestSender = watch::Sender<Vec<BackwardsPaginateUntilEventRequest>>;

/// The return type for [`take_timeline_endpoints()`].
///
/// This primarily contains endpoints for channels of communication
/// between the timeline UI (`RoomScreen`] and the background worker tasks.
/// If the relevant room was tombstoned, this also includes info about its successor room.
pub struct TimelineEndpoints {
    pub update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    pub update_receiver: crossbeam_channel::Receiver<TimelineUpdate>,
    pub request_sender: TimelineRequestSender,
    pub successor_room: Option<SuccessorRoom>,
}

/// Info about a timeline for a joined room or a thread in a joined room.
struct PerTimelineDetails {
    /// A shared reference to a room's main timeline or thread's timeline of events.
    timeline: Arc<Timeline>,
    /// A clone-able sender for updates to this timeline.
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    /// A tuple of two separate channel endpoints that can only be taken *once* by the main UI thread.
    ///
    /// 1. The single receiver that can receive updates from this timeline.
    ///    * When a new room is joined (or a thread is opened), an unbounded crossbeam channel will be created
    ///      and its sender given to a background task (the `timeline_subscriber_handler()`)
    ///      that enqueues timeline updates as it receives timeline vector diffs from the server.
    ///    * The UI thread can take ownership of this update receiver in order to receive updates
    ///      to this room or thread timeline, but only one receiver can exist at a time.
    /// 2. The sender that can send requests to the background timeline subscriber handler,
    ///    e.g., to watch for a specific event to be prepended to the timeline (via back pagination).
    timeline_singleton_endpoints: Option<(
        crossbeam_channel::Receiver<TimelineUpdate>,
        TimelineRequestSender,
    )>,
    /// The async task that listens for updates for this timeline.
    timeline_subscriber_handler_task: JoinHandle<()>,
}

struct JoinedRoomDetails {
    /// The room ID of this joined room.
    room_id: OwnedRoomId,
    /// Details about the main timeline for this room.
    main_timeline: PerTimelineDetails,
    /// Thread-focused timelines for this room, keyed by thread root event ID.
    thread_timelines: HashMap<OwnedEventId, PerTimelineDetails>,
    /// The set of thread timelines currently being created, to avoid duplicate in-flight work.
    pending_thread_timelines: HashSet<OwnedEventId>,
    /// A drop guard for the event handler that represents a subscription to typing notices for this room.
    typing_notice_subscriber: Option<EventHandlerDropGuard>,
    /// A drop guard for the event handler that represents a subscription to pinned events for this room.
    pinned_events_subscriber: Option<EventHandlerDropGuard>,
    /// The async task that listens for this room becoming encrypted.
    room_encryption_subscriber_task: Option<JoinHandle<()>>,
}
impl Drop for JoinedRoomDetails {
    fn drop(&mut self) {
        log!("Dropping JoinedRoomDetails for room {}", self.room_id);
        self.main_timeline.timeline_subscriber_handler_task.abort();
        for thread_timeline in self.thread_timelines.values() {
            thread_timeline.timeline_subscriber_handler_task.abort();
        }
        if let Some(room_encryption_subscriber_task) = self.room_encryption_subscriber_task.take() {
            room_encryption_subscriber_task.abort();
        }
        drop(self.typing_notice_subscriber.take());
        drop(self.pinned_events_subscriber.take());
    }
}


/// A const-compatible hasher, used for `static` items containing `HashMap`s or `HashSet`s.
type ConstHasher = BuildHasherDefault<DefaultHasher>;

/// Information about all joined rooms that our client currently know about.
/// We use a `HashMap` for O(1) lookups, as this is accessed frequently (e.g. every timeline update).
static ALL_JOINED_ROOMS: Mutex<HashMap<OwnedRoomId, JoinedRoomDetails, ConstHasher>> = Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()));

/// Returns the timeline and timeline update sender for the given joined room/thread timeline.
fn get_per_timeline_details<'a>(
    all_joined_rooms: &'a mut HashMap<OwnedRoomId, JoinedRoomDetails, ConstHasher>,
    kind: &TimelineKind,
) -> Option<&'a mut PerTimelineDetails> {
    let room_info = all_joined_rooms.get_mut(kind.room_id())?;
    match kind {
        TimelineKind::MainRoom { .. } => Some(&mut room_info.main_timeline),
        TimelineKind::Thread { thread_root_event_id, .. } => room_info.thread_timelines.get_mut(thread_root_event_id),
    }
}

/// Obtains the lock on `ALL_JOINED_ROOMS` and returns the timeline for the given timeline kind.
fn get_timeline(kind: &TimelineKind) -> Option<Arc<Timeline>> {
    get_per_timeline_details(ALL_JOINED_ROOMS.lock().unwrap().deref_mut(), kind)
        .map(|details| details.timeline.clone())
}

/// Obtains the lock on `ALL_JOINED_ROOMS` and returns the timeline and timeline update sender for the given timeline kind.
fn get_timeline_and_sender(kind: &TimelineKind) -> Option<(Arc<Timeline>, crossbeam_channel::Sender<TimelineUpdate>)> {
    get_per_timeline_details(ALL_JOINED_ROOMS.lock().unwrap().deref_mut(), kind)
        .map(|details| (details.timeline.clone(), details.timeline_update_sender.clone()))
}

/// Obtains the lock on `ALL_JOINED_ROOMS` and returns the main timeline for the given room.
fn get_room_timeline(room_id: &RoomId) -> Option<Arc<Timeline>> {
    ALL_JOINED_ROOMS.lock().unwrap()
        .get(room_id)
        .map(|jrd| jrd.main_timeline.timeline.clone())
}

/// The logged-in Matrix client, which can be freely and cheaply cloned.
static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

struct ActiveOidcFlow {
    flow_id: u64,
    cancel_tx: oneshot::Sender<()>,
}

#[derive(Default)]
struct OidcFlowSlot {
    next_flow_id: u64,
    active_flow: Option<ActiveOidcFlow>,
}

impl OidcFlowSlot {
    fn try_start_flow(&mut self) -> std::result::Result<(u64, oneshot::Receiver<()>), &'static str> {
        if self.active_flow.is_some() {
            return Err("OIDC login already in progress");
        }

        self.next_flow_id += 1;
        let flow_id = self.next_flow_id;
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.active_flow = Some(ActiveOidcFlow { flow_id, cancel_tx });
        Ok((flow_id, cancel_rx))
    }

    fn finish_flow(&mut self, flow_id: u64) {
        if self
            .active_flow
            .as_ref()
            .is_some_and(|active| active.flow_id == flow_id)
        {
            self.active_flow = None;
        }
    }

    fn cancel_active_flow(&mut self) -> bool {
        if let Some(active) = self.active_flow.take() {
            let _ = active.cancel_tx.send(());
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    fn has_active_flow(&self) -> bool {
        self.active_flow.is_some()
    }
}

/// Single active OIDC flow slot.
///
/// We keep this generation-scoped rather than storing a bare sender so that a
/// late cleanup from an older flow cannot drop the cancel handle for a newer
/// loopback server. That race would make the browser land on `127.0.0.1`
/// after the local listener had already been torn down.
static OIDC_FLOW_SLOT: Mutex<OidcFlowSlot> = Mutex::new(OidcFlowSlot {
    next_flow_id: 0,
    active_flow: None,
});

fn try_start_oidc_flow() -> std::result::Result<(u64, oneshot::Receiver<()>), &'static str> {
    OIDC_FLOW_SLOT.lock().unwrap().try_start_flow()
}

fn finish_oidc_flow(flow_id: u64) {
    OIDC_FLOW_SLOT.lock().unwrap().finish_flow(flow_id);
}

fn cancel_active_oidc_flow() -> bool {
    OIDC_FLOW_SLOT.lock().unwrap().cancel_active_flow()
}

pub fn get_client() -> Option<Client> {
    CLIENT.lock().unwrap().clone()
}

/// Returns the user ID of the currently logged-in user, if any.
pub fn current_user_id() -> Option<OwnedUserId> {
    CLIENT.lock().unwrap().as_ref().and_then(|c|
        c.session_meta().map(|m| m.user_id.clone())
    )
}

/// The singleton sync service.
static SYNC_SERVICE: Mutex<Option<Arc<SyncService>>> = Mutex::new(None);

/// Flag to indicate an account switch is in progress.
/// Contains the user_id to switch to, if any.
static ACCOUNT_SWITCH_TARGET: Mutex<Option<OwnedUserId>> = Mutex::new(None);

/// Check if an account switch is pending (non-consuming peek).
fn is_account_switch_pending() -> bool {
    ACCOUNT_SWITCH_TARGET.lock().ok().map(|g| g.is_some()).unwrap_or(false)
}

/// Take the account switch target, consuming it. Only call when ready to perform the switch.
fn take_account_switch_target() -> Option<OwnedUserId> {
    ACCOUNT_SWITCH_TARGET.lock().ok()?.take()
}

/// Set the target account to switch to.
fn set_account_switch_target(user_id: OwnedUserId) {
    if let Ok(mut guard) = ACCOUNT_SWITCH_TARGET.lock() {
        *guard = Some(user_id);
    }
}

/// Clear the account switch target without taking it.
#[allow(dead_code)]
fn clear_account_switch_target() {
    if let Ok(mut guard) = ACCOUNT_SWITCH_TARGET.lock() {
        *guard = None;
    }
}

/// Set to `true` when the access token has been rejected by the homeserver,
/// signaling the main task to tear down the current session and wait for re-login.
static TOKEN_EXPIRED: AtomicBool = AtomicBool::new(false);
/// Notifies the main monitoring loop to wake up and check `TOKEN_EXPIRED`.
static TOKEN_EXPIRED_NOTIFY: LazyLock<Notify> = LazyLock::new(Notify::new);


/// Get a reference to the current sync service, if available.
pub fn get_sync_service() -> Option<Arc<SyncService>> {
    SYNC_SERVICE.lock().ok()?.as_ref().cloned()
}

/// The list of users that the current user has chosen to ignore.
/// Ideally we shouldn't have to maintain this list ourselves,
/// but the Matrix SDK doesn't currently properly maintain the list of ignored users.
static IGNORED_USERS: Mutex<HashSet<OwnedUserId, ConstHasher>> = Mutex::new(HashSet::with_hasher(BuildHasherDefault::new()));

/// Returns a deep clone of the current list of ignored users.
pub fn get_ignored_users() -> HashSet<OwnedUserId, ConstHasher> {
    IGNORED_USERS.lock().unwrap().clone()
}

/// Returns whether the given user ID is currently being ignored.
pub fn is_user_ignored(user_id: &UserId) -> bool {
    IGNORED_USERS.lock().unwrap().contains(user_id)
}


/// Returns three channel endpoints related to the timeline for the given joined room or thread.
///
/// 1. A timeline update sender.
/// 2. The timeline update receiver, which is a singleton, and can only be taken once.
/// 3. A `tokio::watch` sender that can be used to send requests to the timeline subscriber handler.
///
/// This will only succeed once per room (or once per room thread),
/// as only a single channel receiver can exist.
pub fn take_timeline_endpoints(kind: &TimelineKind) -> Option<TimelineEndpoints> {
    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
    let jrd = all_joined_rooms.get_mut(kind.room_id())?;
    let details = match kind {
        TimelineKind::MainRoom { .. } => &mut jrd.main_timeline,
        TimelineKind::Thread { thread_root_event_id, .. } => jrd.thread_timelines.get_mut(thread_root_event_id)?,
    };
    let (update_receiver, request_sender) = details.timeline_singleton_endpoints.take()?;
    Some(TimelineEndpoints {
        update_sender: details.timeline_update_sender.clone(),
        update_receiver,
        request_sender,
        successor_room: details.timeline.room().successor_room(),
    })
}

/// Returns a clone of the timeline update sender for the given timeline.
///
/// This can be called multiple times, as it only clones the sender.
pub fn get_timeline_update_sender(kind: &TimelineKind) -> Option<crossbeam_channel::Sender<TimelineUpdate>> {
    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
    let jrd = all_joined_rooms.get(kind.room_id())?;
    let details = match kind {
        TimelineKind::MainRoom { .. } => &jrd.main_timeline,
        TimelineKind::Thread { thread_root_event_id, .. } => jrd.thread_timelines.get(thread_root_event_id)?,
    };
    Some(details.timeline_update_sender.clone())
}

const DEFAULT_HOMESERVER: &str = "matrix.org";

fn username_to_full_user_id(
    username: &str,
    homeserver: Option<&str>,
) -> Option<OwnedUserId> {
    username
        .try_into()
        .ok()
        .or_else(|| {
            let homeserver_url = homeserver.unwrap_or(DEFAULT_HOMESERVER);
            let user_id_str = if username.starts_with("@") {
                format!("{}:{}", username, homeserver_url)
            } else {
                format!("@{}:{}", username, homeserver_url)
            };
            user_id_str.as_str().try_into().ok()
        })
}


/// Info we store about a room received by the room list service.
///
/// This struct is necessary in order for us to track the previous state
/// of a room received from the room list service, so that we can
/// determine what room data has changed since the last update.
/// We can't just store the `matrix_sdk::Room` object itself,
/// because that is a shallow reference to an inner room object within
/// the room list service.
#[derive(Clone)]
struct RoomListServiceRoomInfo {
    room_id: OwnedRoomId,
    state: RoomState,
    is_direct: bool,
    is_marked_unread: bool,
    is_tombstoned: bool,
    tags: Option<Tags>,
    user_power_levels: Option<UserPowerLevels>,
    latest_event_timestamp: Option<MilliSecondsSinceUnixEpoch>,
    num_unread_messages: u64,
    num_unread_mentions: u64,
    display_name: Option<RoomDisplayName>,
    room_avatar: Option<OwnedMxcUri>,
    room: matrix_sdk::Room,
}
impl RoomListServiceRoomInfo {
    async fn from_room(room: matrix_sdk::Room, current_user_id: &Option<OwnedUserId>) -> Self {
        // Parallelize fetching of independent room data.
        let (is_direct, tags, display_name, user_power_levels) = tokio::join!(
            room.is_direct(),
            room.tags(),
            room.display_name(),
            async {
                if let Some(user_id) = current_user_id {
                    UserPowerLevels::from_room(&room, user_id.deref()).await
                } else {
                    None
                }
            }
        );

        Self {
            room_id: room.room_id().to_owned(),
            state: room.state(),
            is_direct: is_direct.unwrap_or(false),
            is_marked_unread: room.is_marked_unread(),
            is_tombstoned: room.is_tombstoned(),
            tags: tags.ok().flatten(),
            user_power_levels,
            latest_event_timestamp: room.latest_event_timestamp(),
            num_unread_messages: room.num_unread_messages(),
            num_unread_mentions: room.num_unread_mentions(),
            display_name: display_name.ok(),
            room_avatar: room.avatar_url(),
            room,
        }
    }
    async fn from_room_ref(room: &matrix_sdk::Room, current_user_id: &Option<OwnedUserId>) -> Self {
        Self::from_room(room.clone(), current_user_id).await
    }
}

/// Performs the Matrix client login or session restore, and starts the main sync service.
///
/// After starting the sync service, this also starts the main room list service loop
/// and the main space service loop.
async fn start_matrix_client_login_and_sync(rt: Handle) {
    // Create a channel for sending requests from the main UI thread to a background worker task.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<MatrixRequest>();
    REQUEST_SENDER.lock().unwrap().replace(sender);

    let (login_sender, mut login_receiver) = tokio::sync::mpsc::channel(1);

    // Spawn the async worker task that handles matrix requests.
    // We must do this now such that the matrix worker task can listen for incoming login requests
    // from the UI, and forward them to this task (via the login_sender --> login_receiver).
    let mut matrix_worker_task_handle = rt.spawn(matrix_worker_task(receiver, login_sender));

    let most_recent_user_id = persistence::most_recent_user_id().await;
    log!("Most recent user ID: {most_recent_user_id:?}");
    let cli_parse_result = Cli::try_parse();
    let cli_has_valid_username_password = cli_parse_result.as_ref()
        .is_ok_and(|cli| !cli.user_id.is_empty() && !cli.password.is_empty());
    log!("CLI parsing succeeded? {}. CLI has valid UN+PW? {}",
        cli_parse_result.as_ref().is_ok(),
        cli_has_valid_username_password,
    );
    let wait_for_login = !cli_has_valid_username_password && (
        most_recent_user_id.is_none()
            || std::env::args().any(|arg| arg == "--login-screen" || arg == "--force-login")
    );
    log!("Waiting for login? {}", wait_for_login);

    let new_login_opt = if !wait_for_login {
        let specified_username = cli_parse_result.as_ref().ok().and_then(|cli|
            username_to_full_user_id(
                &cli.user_id,
                cli.homeserver.as_deref(),
            )
        );
        log!("Trying to restore session for user: {:?}",
            specified_username.as_ref().or(most_recent_user_id.as_ref())
        );
        match persistence::restore_session(specified_username.clone()).await {
            Ok((client, sync_token, session)) => {
                // Do not make whoami a startup restore gate. Some Matrix-compatible
                // homeservers may not expose it yet; invalid tokens are still caught
                // by SDK restore, SyncService::build(), and SessionChange::UnknownToken.
                Some((client, sync_token, false, session))
            }
            Err(e) => {
                let status_err = restore_session_failure_message(&e);
                log!("{status_err} Error: {e:?}");
                apply_restore_session_failure_policy(&e).await;
                Cx::post_action(LoginAction::LoginFailure(status_err));

                if let Ok(cli) = &cli_parse_result {
                    log!("Attempting auto-login from CLI arguments as user '{}'...", cli.user_id);
                    Cx::post_action(LoginAction::CliAutoLogin {
                        user_id: cli.user_id.clone(),
                        homeserver: cli.homeserver.clone(),
                    });
                    match login(cli, LoginRequest::LoginByCli).await {
                        Ok((client, sync_token, _is_add_account, session)) => Some((client, sync_token, false, session)),
                        Err(e) => {
                            error!("CLI-based login failed: {e:?}");
                            Cx::post_action(LoginAction::LoginFailure(
                                format!("Could not login with CLI-provided arguments.\n\nPlease login manually.\n\nError: {e}")
                            ));
                            enqueue_rooms_list_update(RoomsListUpdate::Status {
                                status: format!("Login failed: {e:?}"),
                            });
                            None
                        }
                    }
                } else {
                    None
                }
            }
        }
    } else {
        Cx::post_action(LoginAction::ShowLoginScreen);
        None
    };
    let cli: Cli = cli_parse_result.unwrap_or(Cli::default());
    // `initial_client_opt` holds the client obtained from the session restore or CLI auto-login.
    // On subsequent iterations of the login loop (after a post-auth setup failure), it is `None`,
    // which causes the loop to wait for the user to submit a new manual login request.
    let mut initial_client_opt = new_login_opt;

    loop {
        let (client, sync_service, logged_in_user_id, client_session) = 'login_loop: loop {
            let (client, _sync_token, validate_session, session) = match initial_client_opt.take() {
                Some(login) => login,
                None => {
                    loop {
                        log!("Waiting for login request...");
                        match login_receiver.recv().await {
                            Some(login_request) => {
                                match login(&cli, login_request).await {
                                    Ok((client, sync_token, _is_add_account, session)) => break (client, sync_token, false, session),
                                    Err(e) => {
                                        error!("Login failed: {e:?}");
                                        Cx::post_action(LoginAction::LoginFailure(format!("{e}")));
                                        enqueue_rooms_list_update(RoomsListUpdate::Status {
                                            status: format!("Login failed: {e}"),
                                        });
                                    }
                                }
                            },
                            None => {
                                error!("BUG: login_receiver hung up unexpectedly");
                                let err = String::from("Please restart Robrix.\n\nUnable to listen for login requests.");
                                Cx::post_action(LoginAction::LoginFailure(err.clone()));
                                enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: err,
                                });
                                return;
                            }
                        }
                    }
                }
            };

            if validate_session {
                match client.whoami().await {
                    Ok(_) => {}
                    Err(e) if session_validation_failure_action(is_invalid_token_http_error(&e))
                        == RestoreSessionFailureAction::ClearPersistedSession =>
                    {
                        clear_persisted_session(client.user_id()).await;
                        let err_msg = "Your login token is no longer valid.\n\nPlease log in again.";
                        Cx::post_action(LoginAction::LoginFailure(err_msg.to_string()));
                        enqueue_rooms_list_update(RoomsListUpdate::Status {
                            status: err_msg.to_string(),
                        });
                        continue 'login_loop;
                    }
                    Err(e) => {
                        warning!("Session validation via whoami failed, but the error was not an invalid token; continuing startup: {e}");
                    }
                }
            }

            // Deallocate the default SSO client after a successful login.
            if let Ok(mut client_opt) = DEFAULT_SSO_CLIENT.lock() {
                let _ = client_opt.take();
            }

            let logged_in_user_id: OwnedUserId = client.user_id()
                .expect("BUG: Client::user_id() returned None after successful login!")
                .to_owned();
            let status = format!("Logged in as {}.\n → Loading rooms...", logged_in_user_id);
            enqueue_rooms_list_update(RoomsListUpdate::Status { status });

            // Add the account to the AccountManager
            let account = account_manager::Account {
                client: client.clone(),
                user_id: logged_in_user_id.clone(),
                session: session.clone(),
                display_name: None,
                avatar_url: None,
            };
            let is_new = account_manager::add_account(account);
            log!("Added account {} to AccountManager. New account: {}", logged_in_user_id, is_new);

            // Store this active client in our global Client state so that other tasks can access it.
            if let Some(_existing) = CLIENT.lock().unwrap().replace(client.clone()) {
                error!("BUG: unexpectedly replaced an existing client when initializing the matrix client.");
            }

            // Clear the SSO pre-build skip flag now that CLIENT is set, so any
            // in-flight pre-build that fails after this point will observe
            // `get_client().is_some()` and skip writing a stale flag. A
            // successful login proves the network reaches a homeserver, so
            // future startups should retry the pre-build optimization
            // instead of permanently skipping it.
            clear_sso_prebuild_failure_flag();

            // Listen for changes to our verification status and incoming verification requests.
            add_verification_event_handlers_and_sync_client(client.clone());

            // Listen for updates to the ignored user list.
            handle_ignore_user_list_subscriber(client.clone());

            if !validate_session {
                Cx::post_action(LoginAction::Status {
                    title: "Connecting".into(),
                    status: "Setting up sync service...".into(),
                });
            }
            let sync_service = match SyncService::builder(client.clone())
                .with_offline_mode()
                .build()
                .await
            {
                Ok(ss) => ss,
                Err(e) => {
                    error!("Failed to create SyncService: {e:?}");
                    let err_msg = if is_invalid_token_error(&e) {
                        "Your login token is no longer valid.\n\nPlease log in again.".to_string()
                    } else {
                        format!("Please restart Robrix.\n\nFailed to create Matrix sync service: {e}.")
                    };
                    if is_invalid_token_error(&e) {
                        clear_persisted_session(client.user_id()).await;
                    }
                    Cx::post_action(LoginAction::LoginFailure(err_msg.clone()));
                    enqueue_popup_notification(err_msg.clone(), PopupKind::Error, None);
                    enqueue_rooms_list_update(RoomsListUpdate::Status { status: err_msg });
                    // Clear the stored client so the next login attempt doesn't trigger the
                    // "unexpectedly replaced an existing client" warning.
                    let _ = CLIENT.lock().unwrap().take();
                    continue 'login_loop;
                }
            };

            break 'login_loop (client, sync_service, logged_in_user_id, session);
        };

        let (session_reset_sender, mut session_reset_receiver) =
            tokio::sync::mpsc::unbounded_channel::<SessionResetAction>();
        // Listen for session changes, e.g., when the access token becomes invalid.
        let session_change_handler_task =
            handle_session_changes(client.clone(), client_session.clone(), session_reset_sender);

        // Signal login success now that SyncService::build() has already succeeded (inside
        // 'login_loop), which is the only step that can fail with an invalid/expired token.
        // Doing this before sync_service.start() lets the UI transition to the home screen
        // without waiting for the sync loop to begin.
        Cx::post_action(LoginAction::LoginSuccess);

        // Attempt to load the previously-saved app state.
        handle_load_app_state(logged_in_user_id.to_owned());
        handle_sync_indicator_subscriber(&sync_service);
        handle_sync_service_state_subscriber(sync_service.state());
        sync_service.start().await;

        let room_list_service = sync_service.room_list_service();

        if let Some(_existing) = SYNC_SERVICE.lock().unwrap().replace(Arc::new(sync_service)) {
            error!("BUG: unexpectedly replaced an existing sync service when initializing the matrix client.");
        }

        let mut room_list_service_task = rt.spawn(room_list_service_loop(room_list_service));
        let mut space_service_task = rt.spawn(space_service_loop(client));

        // Now, this task becomes an infinite loop that monitors the
        // matrix/background tasks for the currently-authenticated session.
        #[allow(clippy::never_loop)] // unsure if needed, just following tokio's examples.
        let reauth_message: Option<String> = loop {
            tokio::select! {
                session_reset = session_reset_receiver.recv() => {
                    match session_reset {
                        Some(SessionResetAction::Reauthenticate { message }) => {
                            break Some(message);
                        }
                        None => {
                            warning!("Session reset receiver closed unexpectedly.");
                            continue;
                        }
                    }
                }
                result = &mut matrix_worker_task_handle => {
                    session_change_handler_task.abort();
                    match result {
                        Ok(Ok(())) => {
                            // Check if this is due to logout or account switch
                            if is_logout_in_progress() {
                                log!("matrix worker task ended due to logout");
                            } else if is_account_switch_pending() {
                                log!("matrix worker task ended due to account switch");
                            } else {
                                error!("BUG: matrix worker task ended unexpectedly!");
                            }
                        }
                        Ok(Err(e)) => {
                            // Check if this is due to logout or account switch
                            if is_logout_in_progress() {
                                log!("matrix worker task ended with error due to logout: {e:?}");
                            } else if is_account_switch_pending() {
                                log!("matrix worker task ended with error due to account switch: {e:?}");
                            } else {
                                error!("Error: matrix worker task ended:\n\t{e:?}");
                                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: e.to_string(),
                                });
                                enqueue_popup_notification(
                                    format!("Matrix worker error: {e}"),
                                    PopupKind::Error,
                                    None,
                                );
                            }
                        },
                        Err(e) => {
                            error!("BUG: failed to join matrix worker task: {e:?}");
                        }
                    }
                    break None;
                }
                result = &mut room_list_service_task => {
                    session_change_handler_task.abort();
                    match result {
                        Ok(Ok(())) => {
                            if is_logout_in_progress() || is_account_switch_pending() {
                                log!("room list service loop task ended due to logout/account switch");
                            } else {
                                error!("BUG: room list service loop task ended unexpectedly!");
                            }
                        }
                        Ok(Err(e)) => {
                            if !is_logout_in_progress() && !is_account_switch_pending() {
                                error!("Error: room list service loop task ended:\n\t{e:?}");
                                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: e.to_string(),
                                });
                                enqueue_popup_notification(
                                    format!("Room list service error: {e}"),
                                    PopupKind::Error,
                                    None,
                                );
                            }
                        },
                        Err(e) => {
                            error!("BUG: failed to join room list service loop task: {e:?}");
                        }
                    }
                    break None;
                }
                result = &mut space_service_task => {
                    session_change_handler_task.abort();
                    match result {
                        Ok(Ok(())) => {
                            if is_logout_in_progress() || is_account_switch_pending() {
                                log!("space service loop task ended due to logout/account switch");
                            } else {
                                error!("BUG: space service loop task ended unexpectedly!");
                            }
                        }
                        Ok(Err(e)) => {
                            if !is_logout_in_progress() && !is_account_switch_pending() {
                                error!("Error: space service loop task ended:\n\t{e:?}");
                                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: e.to_string(),
                                });
                                enqueue_popup_notification(
                                    format!("Space service error: {e}"),
                                    PopupKind::Error,
                                    None,
                                );
                            }
                        },
                        Err(e) => {
                            error!("BUG: failed to join space service loop task: {e:?}");
                        }
                    }
                    break None;
                }
            }
        };

        // Check if we need to restart for an account switch (loop to handle consecutive switches)
        while let Some(switch_user_id) = take_account_switch_target() {
            // Clear all backend state
            CLIENT.lock().unwrap().take();
            SYNC_SERVICE.lock().unwrap().take();
            ALL_JOINED_ROOMS.lock().unwrap().clear();
            IGNORED_USERS.lock().unwrap().clear();

            // Clear the rooms list UI
            enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
            enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::Clear));

            // Post action to clear UI state
            Cx::post_action(AccountSwitchAction::Starting(switch_user_id.clone()));

            // Update active account
            account_manager::set_active_account(&switch_user_id);
            // Recreate worker task and service loops
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<MatrixRequest>();
            REQUEST_SENDER.lock().unwrap().replace(sender);
            // Restore session for the switched account
            match persistence::restore_session(Some(switch_user_id.clone())).await {
                Ok((client, _sync_token, session)) => {
                    // Store the client
                    CLIENT.lock().unwrap().replace(client.clone());

                    // Set up the new client
                    add_verification_event_handlers_and_sync_client(client.clone());
                    handle_ignore_user_list_subscriber(client.clone());

                    // Create new sync service
                    let sync_service = match SyncService::builder(client.clone())
                        .with_offline_mode()
                        .build()
                        .await
                    {
                        Ok(ss) => ss,
                        Err(e) => {
                            error!("Failed to create SyncService: {e:?}");
                            Cx::post_action(AccountSwitchAction::Failed(format!("Failed to create sync service: {e}")));
                            return;
                        }
                    };

                    // Load app state for the new user
                    handle_load_app_state(switch_user_id.clone());
                    handle_sync_indicator_subscriber(&sync_service);
                    handle_sync_service_state_subscriber(sync_service.state());
                    sync_service.start().await;
                    let room_list_service = sync_service.room_list_service();

                    SYNC_SERVICE.lock().unwrap().replace(Arc::new(sync_service));
                    
                    let (login_sender, _login_receiver) = tokio::sync::mpsc::channel(1);

                    // Set up session change handler for the switched account
                    let (session_reset_sender, mut session_reset_receiver) =
                        tokio::sync::mpsc::unbounded_channel::<SessionResetAction>();
                    let session_change_handler_task =
                        handle_session_changes(client.clone(), session.clone(), session_reset_sender);

                    let mut matrix_worker_task_handle = rt.spawn(matrix_worker_task(receiver, login_sender));
                    let mut room_list_service_task = rt.spawn(room_list_service_loop(room_list_service));
                    let mut space_service_task = rt.spawn(space_service_loop(client.clone()));

                    // Notify UI that switch is complete (app.rs handles the popup notification)
                    Cx::post_action(AccountSwitchAction::Switched(switch_user_id.clone()));

                    // Re-enter the main monitoring loop
                    loop {
                        tokio::select! {
                            session_reset = session_reset_receiver.recv() => {
                                match session_reset {
                                    Some(SessionResetAction::Reauthenticate { message }) => {
                                        error!("Session reset during account switch: {}", message);
                                        session_change_handler_task.abort();
                                        room_list_service_task.abort();
                                        space_service_task.abort();
                                        Cx::post_action(AccountSwitchAction::Failed(message));
                                        break;
                                    }
                                    None => {
                                        warning!("Session reset receiver closed unexpectedly.");
                                        continue;
                                    }
                                }
                            }
                            result = &mut matrix_worker_task_handle => {
                                session_change_handler_task.abort();
                                match result {
                                    Ok(Ok(())) => {
                                        if !is_logout_in_progress() && !is_account_switch_pending() {
                                            error!("BUG: matrix worker task ended unexpectedly!");
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        if !is_logout_in_progress() && !is_account_switch_pending() {
                                            error!("Error: matrix worker task ended:\n\t{e:?}");
                                        }
                                    }
                                    Err(e) => {
                                        error!("BUG: failed to join matrix worker task: {e:?}");
                                    }
                                }
                                break;
                            }
                            result = &mut room_list_service_task => {
                                session_change_handler_task.abort();
                                if let Err(e) = result {
                                    if !is_logout_in_progress() && !is_account_switch_pending() {
                                        error!("Room list service task error: {e:?}");
                                    }
                                }
                                break;
                            }
                            result = &mut space_service_task => {
                                session_change_handler_task.abort();
                                if let Err(e) = result {
                                    if !is_logout_in_progress() && !is_account_switch_pending() {
                                        error!("Space service task error: {e:?}");
                                    }
                                }
                                break;
                            }
                        }
                    }
                    // After inner loop breaks, outer while loop will check for another pending account switch
                }
                Err(e) => {
                    error!("Failed to restore session for account switch: {e:?}");
                    apply_restore_session_failure_policy(&e).await;
                    Cx::post_action(AccountSwitchAction::Failed(format!("Failed to restore session: {e}")));
                    enqueue_popup_notification(
                        format!("Account switch failed: {e}"),
                        PopupKind::Error,
                        None,
                    );
                    // Don't loop back - a failed switch shouldn't keep trying
                    break;
                }
            }
        }

        // Only run reauth cleanup if we got a reauth message (not account switch or logout)
        if let Some(reauth_msg) = reauth_message {
            session_change_handler_task.abort();
            room_list_service_task.abort();
            space_service_task.abort();

            reset_runtime_state_for_relogin().await;
            Cx::post_action(LoginAction::LoginFailure(reauth_msg.clone()));
            enqueue_rooms_list_update(RoomsListUpdate::Status {
                status: reauth_msg,
            });
        }
    }
}


/// The main async task that listens for changes to all rooms.
async fn room_list_service_loop(room_list_service: Arc<RoomListService>) -> Result<()> {
    let all_rooms_list = room_list_service.all_rooms().await?;
    handle_room_list_service_loading_state(all_rooms_list.loading_state());

    let (room_diff_stream, room_list_dynamic_entries_controller) =
        // TODO: paginate room list to avoid loading all rooms at once
        all_rooms_list.entries_with_dynamic_adapters(usize::MAX);

    // By default, our rooms list should only show rooms that are:
    // 1. not spaces (those are handled by the SpaceService),
    // 2. not left (clients don't typically show rooms that the user has already left),
    // 3. not outdated (don't show tombstoned rooms whose successor is already joined).
    room_list_dynamic_entries_controller.set_filter(Box::new(
        filters::new_filter_all(vec![
            Box::new(filters::new_filter_not(Box::new(filters::new_filter_space()))),
            Box::new(filters::new_filter_non_left()),
            Box::new(filters::new_filter_deduplicate_versions()),
        ])
    ));

    let mut all_known_rooms: Vector<RoomListServiceRoomInfo> = Vector::new();
    let current_user_id = current_user_id();

    pin_mut!(room_diff_stream);
    while let Some(batch) = room_diff_stream.next().await {
        let mut peekable_diffs = batch.into_iter().peekable();
        while let Some(diff) = peekable_diffs.next() {
            let is_reset = matches!(diff, VectorDiff::Reset { .. });
            match diff {
                VectorDiff::Append { values: new_rooms }
                | VectorDiff::Reset { values: new_rooms } => {
                    // Append and Reset are identical, except for Reset first clears all rooms.
                    let _num_new_rooms = new_rooms.len();
                    if is_reset {
                        if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Reset, old length {}, new length {}", all_known_rooms.len(), new_rooms.len()); }
                        // Iterate manually so we can know which rooms are being removed.
                        while let Some(room) = all_known_rooms.pop_back() {
                            remove_room(&room);
                        }
                        // ALL_JOINED_ROOMS should already be empty due to successive calls to `remove_room()`,
                        // so this is just a sanity check.
                        ALL_JOINED_ROOMS.lock().unwrap().clear();
                        enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                        enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::Clear));
                    } else {
                        if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Append, old length {}, adding {} new items", all_known_rooms.len(), _num_new_rooms); }
                    }

                    // Parallelize creating each room's RoomListServiceRoomInfo and adding that new room.
                    // We combine `from_room` and `add_new_room` into a single async task per room.
                    let new_room_infos: Vec<RoomListServiceRoomInfo> = join_all(
                        new_rooms.into_iter().map(|room| async {
                            let room_info = RoomListServiceRoomInfo::from_room(room.into_inner(), &current_user_id).await;
                            if let Err(e) = add_new_room(&room_info, &room_list_service, false).await {
                                error!("Failed to add new room: {:?} ({}); error: {:?}", room_info.display_name, room_info.room_id, e);
                            }
                            room_info
                        })
                    ).await;

                    // Send room order update with the new room IDs
                    let (room_id_refs, room_ids) = {
                        let mut room_id_refs = Vec::with_capacity(new_room_infos.len());
                        let mut room_ids = Vec::with_capacity(new_room_infos.len());
                        for r in &new_room_infos {
                            room_id_refs.push(r.room_id.as_ref());
                            room_ids.push(r.room_id.clone());
                        }
                        (room_id_refs, room_ids)
                    };
                    if !room_ids.is_empty() {
                        enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                            VecDiff::Append { values: room_ids }
                        ));
                        room_list_service.subscribe_to_rooms(&room_id_refs).await;
                        all_known_rooms.extend(new_room_infos);
                    }
                }
                VectorDiff::Clear => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Clear"); }
                    all_known_rooms.clear();
                    ALL_JOINED_ROOMS.lock().unwrap().clear();
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::Clear));
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                }
                VectorDiff::PushFront { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushFront"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner(), &current_user_id).await;
                    let room_id = new_room.room_id.clone();
                    add_new_room(&new_room, &room_list_service, true).await?;
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                        VecDiff::PushFront { value: room_id }
                    ));
                    all_known_rooms.push_front(new_room);
                }
                VectorDiff::PushBack { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushBack"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner(), &current_user_id).await;
                    let room_id = new_room.room_id.clone();
                    add_new_room(&new_room, &room_list_service, true).await?;
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                        VecDiff::PushBack { value: room_id }
                    ));
                    all_known_rooms.push_back(new_room);
                }
                remove_diff @ VectorDiff::PopFront => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopFront"); }
                    if let Some(room) = all_known_rooms.pop_front() {
                        enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::PopFront));
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                            &current_user_id,
                        ).await?;
                    }
                }
                remove_diff @ VectorDiff::PopBack => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopBack"); }
                    if let Some(room) = all_known_rooms.pop_back() {
                        enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::PopBack));
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                            &current_user_id,
                        ).await?;
                    }
                }
                VectorDiff::Insert { index, value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Insert at {index}"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner(), &current_user_id).await;
                    let room_id = new_room.room_id.clone();
                    add_new_room(&new_room, &room_list_service, true).await?;
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                        VecDiff::Insert { index, value: room_id }
                    ));
                    all_known_rooms.insert(index, new_room);
                }
                VectorDiff::Set { index, value: changed_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Set at {index}"); }
                    let changed_room = RoomListServiceRoomInfo::from_room(changed_room.into_inner(), &current_user_id).await;
                    if let Some(old_room) = all_known_rooms.get(index) {
                        update_room(old_room, &changed_room, &room_list_service).await?;
                    } else {
                        error!("BUG: room list diff: Set index {index} was out of bounds.");
                    }
                    // Send order update (room ID at this index may have changed)
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                        VecDiff::Set { index, value: changed_room.room_id.clone() }
                    ));
                    all_known_rooms.set(index, changed_room);
                }
                remove_diff @ VectorDiff::Remove { index } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Remove at {index}"); }
                    if index < all_known_rooms.len() {
                        let room = all_known_rooms.remove(index);
                        enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(VecDiff::Remove { index }));
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                            &current_user_id,
                        ).await?;
                    } else {
                        error!("BUG: room_list: diff Remove index {index} out of bounds, len {}", all_known_rooms.len());
                    }
                }
                VectorDiff::Truncate { length } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Truncate to {length}"); }
                    // Iterate manually so we can know which rooms are being removed.
                    while all_known_rooms.len() > length {
                        if let Some(room) = all_known_rooms.pop_back() {
                            remove_room(&room);
                        }
                    }
                    all_known_rooms.truncate(length); // sanity check
                    enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                        VecDiff::Truncate { length }
                    ));
                }
            }
        }
    }

    bail!("room list service sync loop ended unexpectedly")
}


/// Attempts to optimize a common RoomListService operation of remove + add.
///
/// If a `Remove` diff (or `PopBack` or `PopFront`) is immediately followed by
/// an `Insert` diff (or `PushFront` or `PushBack`) for the same room,
/// we can treat it as a simple `Set` operation, in which we call `update_room()`.
/// This is much more efficient than removing the room and then adding it back.
///
/// This tends to happen frequently in order to change the room's state
/// or to "sort" the room list by changing its positional order.
async fn optimize_remove_then_add_into_update(
    remove_diff: VectorDiff<RoomListItem>,
    room: &RoomListServiceRoomInfo,
    peekable_diffs: &mut Peekable<impl Iterator<Item = VectorDiff<RoomListItem>>>,
    all_known_rooms: &mut Vector<RoomListServiceRoomInfo>,
    room_list_service: &RoomListService,
    current_user_id: &Option<OwnedUserId>,
) -> Result<()> {
    let next_diff_was_handled: bool;
    match peekable_diffs.peek() {
        Some(VectorDiff::Insert { index: insert_index, value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + Insert({insert_index}) into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref(), current_user_id).await;
            update_room(room, &new_room, room_list_service).await?;
            // Send order update for the insert
            enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                VecDiff::Insert { index: *insert_index, value: new_room.room_id.clone() }
            ));
            all_known_rooms.insert(*insert_index, new_room);
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushFront { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushFront into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref(), current_user_id).await;
            update_room(room, &new_room, room_list_service).await?;
            // Send order update for the push front
            enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                VecDiff::PushFront { value: new_room.room_id.clone() }
            ));
            all_known_rooms.push_front(new_room);
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushBack { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushBack into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref(), current_user_id).await;
            update_room(room, &new_room, room_list_service).await?;
            // Send order update for the push back
            enqueue_rooms_list_update(RoomsListUpdate::RoomOrderUpdate(
                VecDiff::PushBack { value: new_room.room_id.clone() }
            ));
            all_known_rooms.push_back(new_room);
            next_diff_was_handled = true;
        }
        _ => next_diff_was_handled = false,
    }
    if next_diff_was_handled {
        peekable_diffs.next(); // consume the next diff
    } else {
        remove_room(room);
    }
    Ok(())
}


/// Invoked when the room list service has received an update that changes an existing room.
async fn update_room(
    old_room: &RoomListServiceRoomInfo,
    new_room: &RoomListServiceRoomInfo,
    room_list_service: &RoomListService,
) -> Result<()> {
    let new_room_id = new_room.room_id.clone();
    if old_room.room_id == new_room_id {
        // Display-flip on a still-Joined room must not destroy JoinedRoomDetails,
        // or the open RoomScreen's singleton timeline receiver is orphaned.
        let old_should_display = should_display_joined_room_entry(
            old_room.state,
            old_room.is_direct,
            old_room.display_name.as_ref(),
        );
        let new_should_display = should_display_joined_room_entry(
            new_room.state,
            new_room.is_direct,
            new_room.display_name.as_ref(),
        );
        match classify_joined_room_display_flip(old_should_display, new_should_display) {
            JoinedRoomDisplayFlip::BecameHidden => {
                enqueue_rooms_list_update(RoomsListUpdate::HideRoom {
                    room_id: new_room_id.clone(),
                });
            }
            JoinedRoomDisplayFlip::BecameDisplayable => {
                enqueue_rooms_list_update(RoomsListUpdate::UnhideRoom {
                    room_id: new_room_id.clone(),
                });
            }
            JoinedRoomDisplayFlip::NoDisplayChange => {}
        }

        // Handle state transitions for a room.
        if LOG_ROOM_LIST_DIFFS {
            log!("Room {:?} ({new_room_id}) state went from {:?} --> {:?}", new_room.display_name, old_room.state, new_room.state);
        }
        if old_room.state != new_room.state {
            match new_room.state {
                RoomState::Banned => {
                    // TODO: handle rooms that this user has been banned from.
                    log!("Removing Banned room: {:?} ({new_room_id})", new_room.display_name);
                    remove_room(new_room);
                    return Ok(());
                }
                RoomState::Left => {
                    log!("Removing Left room: {:?} ({new_room_id})", new_room.display_name);
                    // TODO: instead of removing this, we could optionally add it to
                    //       a separate list of left rooms, which would be collapsed by default.
                    //       Upon clicking a left room, we could show a splash page
                    //       that prompts the user to rejoin the room or forget it permanently.
                    //       Currently, we just remove it and do not show left rooms at all.
                    remove_room(new_room);
                    return Ok(());
                }
                RoomState::Joined => {
                    log!("update_room(): adding new Joined room: {:?} ({new_room_id})", new_room.display_name);
                    return add_new_room(new_room, room_list_service, true).await;
                }
                RoomState::Invited => {
                    log!("update_room(): adding new Invited room: {:?} ({new_room_id})", new_room.display_name);
                    return add_new_room(new_room, room_list_service, true).await;
                }
                RoomState::Knocked => {
                    // TODO: handle Knocked rooms (e.g., can you re-knock? or cancel a prior knock?)
                    return Ok(());
                }
            }
        }

        // First, we check for changes to room data that is relevant to any room,
        // including joined, invited, and other rooms.
        // This includes the room name and room avatar.
        if old_room.room_avatar != new_room.room_avatar {
            log!("Updating room avatar for room {}", new_room_id);
            spawn_fetch_room_avatar(new_room);
        }
        if old_room.display_name != new_room.display_name {
            log!("Updating room {} name: {:?} --> {:?}", new_room_id, old_room.display_name, new_room.display_name);

            enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                new_room_name: (new_room.display_name.clone(), new_room_id.clone()).into(),
            });
        }

        // Then, we check for changes to room data that is only relevant to joined rooms:
        // including the latest event, tags, unread counts, is_direct, tombstoned state, power levels, etc.
        // Invited or left rooms don't care about these details.
        if matches!(new_room.state, RoomState::Joined) { 
            // For some reason, the latest event API does not reliably catch *all* changes
            // to the latest event in a given room, such as redactions.
            // Thus, we have to re-obtain the latest event on *every* update, regardless of timestamp.
            //
            let update_latest = match (old_room.latest_event_timestamp, new_room.room.latest_event_timestamp()) {
                (Some(old_ts), Some(new_ts)) => new_ts >= old_ts,
                (None, Some(_)) => true,
                _ => false,
            };
            if update_latest {
                update_latest_event(&new_room.room).await;
            }


            if old_room.tags != new_room.tags {
                log!("Updating room {} tags from {:?} to {:?}", new_room_id, old_room.tags, new_room.tags);
                enqueue_rooms_list_update(RoomsListUpdate::Tags {
                    room_id: new_room_id.clone(),
                    new_tags: new_room.tags.clone().unwrap_or_default(),
                });
            }

            if old_room.is_marked_unread != new_room.is_marked_unread
                || old_room.num_unread_messages != new_room.num_unread_messages
                || old_room.num_unread_mentions != new_room.num_unread_mentions
            {
                log!("Updating room {}, marked unread {} --> {}, unread messages {} --> {}, unread mentions {} --> {}",
                    new_room_id,
                    old_room.is_marked_unread, new_room.is_marked_unread,
                    old_room.num_unread_messages, new_room.num_unread_messages,
                    old_room.num_unread_mentions, new_room.num_unread_mentions,
                );
                enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                    room_id: new_room_id.clone(),
                    is_marked_unread: new_room.is_marked_unread,
                    unread_messages: UnreadMessageCount::Known(new_room.num_unread_messages),
                    unread_mentions: new_room.num_unread_mentions,
                });
            }

            if old_room.is_direct != new_room.is_direct {
                log!("Updating room {} is_direct from {} to {}",
                    new_room_id,
                    old_room.is_direct,
                    new_room.is_direct,
                );
                enqueue_rooms_list_update(RoomsListUpdate::UpdateIsDirect {
                    room_id: new_room_id.clone(),
                    is_direct: new_room.is_direct,
                });
            }

            if let Some(is_encrypted) = fetch_room_is_encrypted(&new_room.room).await {
                enqueue_rooms_list_update(RoomsListUpdate::UpdateIsEncrypted {
                    room_id: new_room_id.clone(),
                    is_encrypted,
                });
            }

            let mut __timeline_update_sender_opt = None;
            let mut get_timeline_update_sender = |room_id| {
                if __timeline_update_sender_opt.is_none() {
                    if let Some(jrd) = ALL_JOINED_ROOMS.lock().unwrap().get(room_id) {
                        __timeline_update_sender_opt = Some(jrd.main_timeline.timeline_update_sender.clone());
                    }
                }
                __timeline_update_sender_opt.clone()
            };

            if !old_room.is_tombstoned && new_room.is_tombstoned {
                let successor_room = new_room.room.successor_room();
                log!("Updating room {new_room_id} to be tombstoned, {successor_room:?}");
                enqueue_rooms_list_update(RoomsListUpdate::TombstonedRoom { room_id: new_room_id.clone() });
                if let Some(timeline_update_sender) = get_timeline_update_sender(&new_room_id) {
                    spawn_fetch_successor_room_preview(
                        room_list_service.client().clone(),
                        successor_room,
                        new_room_id.clone(),
                        timeline_update_sender,
                    );
                } else {
                    error!("BUG: could not find JoinedRoomDetails for newly-tombstoned room {new_room_id}");
                }
            }

            if let Some(nupl) = new_room.user_power_levels
                && old_room.user_power_levels.is_none_or(|oupl| oupl != nupl)
            {
                if let Some(timeline_update_sender) = get_timeline_update_sender(&new_room_id) {
                    log!("Updating room {new_room_id} user power levels.");
                    match timeline_update_sender.send(TimelineUpdate::UserPowerLevels(nupl)) {
                        Ok(_) => SignalToUI::set_ui_signal(),
                        Err(_) => error!("Failed to send the UserPowerLevels update to room {new_room_id}"),
                    }
                } else {
                    error!("BUG: could not find JoinedRoomDetails for room {new_room_id} where power levels changed.");
                }
            }
        }
        Ok(())
    }
    else {
        warning!("UNTESTED SCENARIO: update_room(): removing old room {}, replacing with new room {}",
            old_room.room_id, new_room_id,
        );
        remove_room(old_room);
        add_new_room(new_room, room_list_service, true).await
    }
}


/// Invoked when the room list service has received an update to remove an existing room.
fn remove_room(room: &RoomListServiceRoomInfo) {
    ALL_JOINED_ROOMS.lock().unwrap().remove(&room.room_id);
    enqueue_rooms_list_update(
        RoomsListUpdate::RemoveRoom {
            room_id: room.room_id.clone(),
            new_state: room.state,
        }
    );
}


/// Invoked when the room list service has received an update with a brand new room.
async fn add_new_room(
    new_room: &RoomListServiceRoomInfo,
    room_list_service: &RoomListService,
    subscribe: bool,
) -> Result<()> {
    match new_room.state {
        RoomState::Knocked => {
            log!("Got new Knocked room: {:?} ({})", new_room.display_name, new_room.room_id);
            // Note: here we could optionally display Knocked rooms as a separate type of room
            //       in the rooms list, but it's not really necessary at this point.
            return Ok(());
        }
        RoomState::Banned => {
            log!("Got new Banned room: {:?} ({})", new_room.display_name, new_room.room_id);
            // Note: here we could optionally display Banned rooms as a separate type of room
            //       in the rooms list, but it's not really necessary at this point.
            return Ok(());
        }
        RoomState::Left => {
            log!("Got new Left room: {:?} ({:?})", new_room.display_name, new_room.room_id);
            // Note: here we could optionally display Left rooms as a separate type of room
            //       in the rooms list, but it's not really necessary at this point.
            return Ok(());
        }
        RoomState::Invited => {
            let invite_details = new_room.room.invite_details().await.ok();
            let room_name_id = RoomNameId::from((new_room.display_name.clone(), new_room.room_id.clone()));
            // Start with a basic text avatar; the avatar image will be fetched asynchronously below.
            let room_avatar = avatar_from_room_name(room_name_id.name_for_avatar());
            let inviter_info = if let Some(inviter) = invite_details.and_then(|d| d.inviter) {
                Some(InviterInfo {
                    user_id: inviter.user_id().to_owned(),
                    display_name: inviter.display_name().map(|n| n.to_string()),
                    avatar: inviter
                        .avatar(AVATAR_THUMBNAIL_FORMAT.into())
                        .await
                        .ok()
                        .flatten()
                        .map(Into::into),
                })
            } else {
                None
            };
            rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddInvitedRoom(InvitedRoomInfo {
                room_name_id: room_name_id.clone(),
                search_text: build_room_search_text(
                    &room_name_id,
                    &new_room.room.canonical_alias(),
                    &new_room.room.alt_aliases(),
                ),
                inviter_info,
                room_avatar,
                canonical_alias: new_room.room.canonical_alias(),
                alt_aliases: new_room.room.alt_aliases(),
                // we don't actually display the latest event for Invited rooms, so don't bother.
                latest: None,
                invite_state: Default::default(),
                is_selected: false,
                is_direct: new_room.is_direct,
            }));
            Cx::post_action(AppStateAction::RoomLoadedSuccessfully {
                room_name_id,
                is_invite: true,
            });
            spawn_fetch_room_avatar(new_room);
            return Ok(());
        }
        RoomState::Joined => { } // Fall through to adding the joined room below.
    }

    // If we didn't already subscribe to this room, do so now.
    // This ensures we will properly receive all of its states and latest event.
    if subscribe {
        room_list_service.subscribe_to_rooms(&[&new_room.room_id]).await;
    }

    let timeline = Arc::new(
        new_room.room.timeline_builder()
            .with_focus(TimelineFocus::Live {
                // we show threads as separate timelines in their own RoomScreen
                hide_threaded_events: true,
            })
            .track_read_marker_and_receipts(TimelineReadReceiptTracking::AllEvents)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("BUG: Failed to build timeline for room {}: {e}", new_room.room_id))?,
    );
    let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

    let (request_sender, request_receiver) = watch::channel(Vec::new());
    let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
        new_room.room.clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
        request_receiver,
        None,
    ));

    // We need to add the room to the `ALL_JOINED_ROOMS` list before we can send
    // an `AddJoinedRoom` update to the RoomsList widget, because that widget might
    // immediately issue a `MatrixRequest` that relies on that room being in `ALL_JOINED_ROOMS`.
    log!("Adding new joined room {}, name: {:?}", new_room.room_id, new_room.display_name);
    ALL_JOINED_ROOMS.lock().unwrap().insert(
        new_room.room_id.clone(),
        JoinedRoomDetails {
            room_id: new_room.room_id.clone(),
            main_timeline: PerTimelineDetails {
                timeline,
                timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
                timeline_update_sender,
                timeline_subscriber_handler_task,
            },
            thread_timelines: HashMap::new(),
            pending_thread_timelines: HashSet::new(),
            typing_notice_subscriber: None,
            pinned_events_subscriber: None,
            room_encryption_subscriber_task: None,
        },
    );
    if let Some(joined_room_details) = ALL_JOINED_ROOMS.lock().unwrap().get_mut(&new_room.room_id) {
        joined_room_details.room_encryption_subscriber_task = Some(
            spawn_room_encryption_subscriber(new_room.room.clone())
        );
    } else {
        error!("BUG: could not find newly-added room {} to attach encryption subscriber", new_room.room_id);
    }

    let latest = get_latest_event_details(
        &new_room.room.latest_event().await,
        room_list_service.client(),
    ).await;
    let is_encrypted = fetch_room_is_encrypted(&new_room.room).await;
    let room_name_id = RoomNameId::from((new_room.display_name.clone(), new_room.room_id.clone()));
    // Start with a basic text avatar; the avatar image will be fetched asynchronously below.
    let room_avatar = avatar_from_room_name(room_name_id.name_for_avatar());
    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddJoinedRoom(JoinedRoomInfo {
        latest,
        tags: new_room.tags.clone().unwrap_or_default(),
        num_unread_messages: new_room.num_unread_messages,
        num_unread_mentions: new_room.num_unread_mentions,
        is_marked_unread: new_room.is_marked_unread,
        room_avatar,
        room_name_id: room_name_id.clone(),
        search_text: build_room_search_text(
            &room_name_id,
            &new_room.room.canonical_alias(),
            &new_room.room.alt_aliases(),
        ),
        canonical_alias: new_room.room.canonical_alias(),
        alt_aliases: new_room.room.alt_aliases(),
        has_been_paginated: false,
        is_selected: false,
        is_direct: new_room.is_direct,
        is_encrypted,
        is_tombstoned: new_room.is_tombstoned,
    }));

    // Keep the entry in `ALL_JOINED_ROOMS`, but hide it from the sidebar until
    // the display name resolves — `update_room` will emit `UnhideRoom` then.
    if !should_display_joined_room_entry(
        new_room.state,
        new_room.is_direct,
        new_room.display_name.as_ref(),
    ) {
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::HideRoom {
            room_id: new_room.room_id.clone(),
        });
    }

    Cx::post_action(AppStateAction::RoomLoadedSuccessfully {
        room_name_id,
        is_invite: false,
    });
    spawn_fetch_room_avatar(new_room);
    Ok(())
}

async fn fetch_room_is_encrypted(room: &Room) -> Option<bool> {
    match room.latest_encryption_state().await {
        Ok(state) => Some(state.is_encrypted()),
        Err(error) => {
            error!("Failed to fetch encryption state for room {}: {error:?}", room.room_id());
            None
        }
    }
}

fn spawn_room_encryption_subscriber(room: Room) -> JoinHandle<()> {
    Handle::current().spawn(async move {
        let room_id = room.room_id().to_owned();
        let mut room_info = room.subscribe_info();

        if room_info.get().encryption_state().is_encrypted() {
            enqueue_rooms_list_update(RoomsListUpdate::UpdateIsEncrypted {
                room_id,
                is_encrypted: true,
            });
            return;
        }

        while let Some(info) = room_info.next().await {
            if info.encryption_state().is_encrypted() {
                enqueue_rooms_list_update(RoomsListUpdate::UpdateIsEncrypted {
                    room_id,
                    is_encrypted: true,
                });
                break;
            }
        }
    })
}

#[allow(unused)]
async fn current_ignore_user_list(client: &Client) -> Option<HashSet<OwnedUserId>> {
    use matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent;
    let ignored_users = client.account()
        .account_data::<IgnoredUserListEventContent>()
        .await
        .ok()??
        .deserialize()
        .ok()?
        .ignored_users
        .into_keys()
        .collect();

    Some(ignored_users)
}

fn handle_ignore_user_list_subscriber(client: Client) {
    let mut subscriber = client.subscribe_to_ignore_user_list_changes();
    log!("Initial ignored-user list is: {:?}", subscriber.get());
    Handle::current().spawn(async move {
        let mut first_update = true;
        while let Some(ignore_list) = subscriber.next().await {
            log!("Received an updated ignored-user list: {ignore_list:?}");
            let ignored_users_new = ignore_list
                .into_iter()
                .filter_map(|u| OwnedUserId::try_from(u).ok())
                .collect::<HashSet<_, ConstHasher>>();

            // TODO: when we support persistent state, don't forget to update `IGNORED_USERS` upon app boot.
            let mut ignored_users_old = IGNORED_USERS.lock().unwrap();
            let has_changed = *ignored_users_old != ignored_users_new;
            *ignored_users_old = ignored_users_new;

            if has_changed && !first_update {
                // After successfully (un)ignoring a user, all timelines are fully cleared by the Matrix SDK.
                // Therefore, we need to re-fetch all timelines for all rooms,
                // and currently the only way to actually accomplish this is via pagination.
                // See: <https://github.com/matrix-org/matrix-rust-sdk/issues/1703#issuecomment-2250297923>
                for joined_room in client.joined_rooms() {
                    submit_async_request(MatrixRequest::PaginateTimeline {
                        timeline_kind: TimelineKind::MainRoom {
                            room_id: joined_room.room_id().to_owned(),
                        },
                        num_events: 50,
                        direction: PaginationDirection::Backwards,
                    });
                }
            }

            first_update = false;
        }
    });
}

/// Asynchronously loads and restores the app state from persistent storage for the given user.
///
/// Restores loaded app state when it contains meaningful persisted content.
///
/// The persistence layer returns `AppState::default()` for fresh installs and corrupt-file
/// fallback, so the all-default value must remain a no-op. Empty-dock mobile state is still
/// meaningful when non-dock fields such as `selected_room`, `bot_settings`, language, or
/// translation settings were persisted.
/// If loading fails, it shows a popup notification with the error message.
fn should_restore_loaded_app_state(app_state: &crate::app::AppState) -> bool {
    fn saved_dock_state_has_content(saved: &crate::app::SavedDockState) -> bool {
        !saved.open_rooms.is_empty()
            || !saved.dock_items.is_empty()
            || !saved.room_order.is_empty()
            || saved.selected_room.is_some()
    }

    app_state.selected_room.is_some()
        || saved_dock_state_has_content(&app_state.saved_dock_state_home)
        || app_state
            .saved_dock_state_per_space
            .values()
            .any(saved_dock_state_has_content)
        || app_state.bot_settings != crate::app::BotSettingsState::default()
        || app_state.app_language != crate::i18n::AppLanguage::default()
        || app_state.app_prefs != crate::settings::app_preferences::AppPreferences::default()
        || app_state.translation != crate::room::translation::TranslationConfig::default()
}

fn handle_load_app_state(user_id: OwnedUserId) {
    Handle::current().spawn(async move {
        match take_skip_app_state_restore_once(&user_id).await {
            Ok(true) => {
                log!("Skipping automatic app state restore once for {user_id} after explicit logout.");
                return;
            }
            Ok(false) => {}
            Err(e) => {
                warning!("Failed to check skip-restore marker for {user_id}: {e}");
            }
        }

        match load_app_state(&user_id).await {
            Ok(app_state) => {
                if should_restore_loaded_app_state(&app_state) {
                    log!("Loaded app state from persistent storage. Restoring now...");
                    Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state)));
                }
            }
            Err(_e) => {
                log!("Failed to restore app state from persistent storage: {_e}");
                enqueue_popup_notification(
                    "Could not restore the previous app state.",
                    PopupKind::Error,
                    None,
                );
            }
        }
    });
}

/// Returns `true` if the given sync service error is due to an invalid/expired access token.
fn is_invalid_token_error(e: &sync_service::Error) -> bool {
    use matrix_sdk::ruma::api::client::error::ErrorKind;
    let sdk_error = match e {
        sync_service::Error::RoomList(
            matrix_sdk_ui::room_list_service::Error::SlidingSync(err)
        ) => err,
        sync_service::Error::EncryptionSync(
            encryption_sync_service::Error::SlidingSync(err)
        ) => err,
        _ => return false,
    };
    matches!(
        sdk_error.client_api_error_kind(),
        Some(ErrorKind::UnknownToken { .. } | ErrorKind::MissingToken)
    )
}

/// Subscribes to session change notifications from the Matrix client.
///
/// When the homeserver rejects the access token with a 401 `M_UNKNOWN_TOKEN` error
/// (e.g., the token was revoked or expired), this emits a [`LoginAction::LoginFailure`]
/// so the user is prompted to log in again.
fn handle_session_changes(
    client: Client,
    client_session: ClientSessionPersisted,
    session_reset_sender: UnboundedSender<SessionResetAction>,
) -> JoinHandle<()> {
    let mut receiver = client.subscribe_to_session_changes();
    Handle::current().spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(SessionChange::UnknownToken(data)) => {
                    let soft_logout = data.soft_logout;
                    let msg = if soft_logout {
                        "Your login session has expired.\n\nPlease log in again."
                    } else {
                        "Your login token is no longer valid.\n\nPlease log in again."
                    };
                    error!("Session token is no longer valid (soft_logout: {soft_logout}). Prompting re-login.");
                    TOKEN_EXPIRED.store(true, Ordering::Release);
                    TOKEN_EXPIRED_NOTIFY.notify_one();
                    Cx::post_action(LoginAction::LoginFailure(msg.to_string()));
                    clear_persisted_session(client.user_id()).await;
                    let _ = session_reset_sender.send(SessionResetAction::Reauthenticate {
                        message: msg.to_string(),
                    });
                    // Only prompt once — the SDK will keep emitting UnknownToken
                    // for every rejected request, but one re-login prompt suffices.
                    break;
                }
                Ok(SessionChange::TokensRefreshed) => {
                    // OAuth refresh lands new access/refresh tokens inside the client;
                    // save_session() re-reads them via client.session() and rewrites the
                    // on-disk FullSessionPersisted so a restart picks up the fresh pair.
                    if let Err(e) = persistence::save_session(&client, client_session.clone()).await {
                        warning!("Failed to persist refreshed session tokens: {e}");
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warning!("Session change receiver lagged, missed {n} messages.");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    })
}

fn handle_sync_service_state_subscriber(mut subscriber: Subscriber<sync_service::State>) {
    log!("Initial sync service state is {:?}", subscriber.get());
    Handle::current().spawn(async move {
        while let Some(state) = subscriber.next().await {
            log!("Received a sync service state update: {state:?}");
            match state {
                sync_service::State::Error(e) => {
                    if is_invalid_token_error(&e) {
                        // The access token is invalid; `handle_session_changes` will have
                        // already posted a LoginAction::LoginFailure, so just log here.
                        // Stop the sync service and exit this loop to prevent further
                        // state transitions (e.g., Offline) from triggering misleading
                        // "cannot reach homeserver" notifications.
                        // Setting TOKEN_EXPIRED signals the main monitoring loop to
                        // tear down the current session and wait for re-login.
                        error!("Sync service stopped due to invalid/expired access token: {e}.");
                        TOKEN_EXPIRED.store(true, Ordering::Release);
                        TOKEN_EXPIRED_NOTIFY.notify_one();
                        if let Some(ss) = get_sync_service() {
                            ss.stop().await;
                        }
                        break;
                    } else {
                        log!("Restarting sync service due to error: {e}.");
                        if let Some(ss) = get_sync_service() {
                            ss.start().await;
                        } else {
                            enqueue_popup_notification(
                                "Unable to restart the Matrix sync service.\n\nPlease quit and restart Robrix.",
                                PopupKind::Error,
                                None,
                            );
                        }
                    }
                }
                _other if TOKEN_EXPIRED.load(Ordering::Acquire) => {
                    log!("Ignoring sync service state update after token expiration.");
                    break;
                }
                other => Cx::post_action(RoomsListHeaderAction::StateUpdate(other)),
            }
        }
    });
}

fn handle_sync_indicator_subscriber(sync_service: &SyncService) {
    /// Duration for sync indicator delay before showing
    const SYNC_INDICATOR_DELAY: Duration = Duration::from_millis(100);
    /// Duration for sync indicator delay before hiding
    const SYNC_INDICATOR_HIDE_DELAY: Duration = Duration::from_millis(200);
    let sync_indicator_stream = sync_service.room_list_service()
        .sync_indicator(
            SYNC_INDICATOR_DELAY,
            SYNC_INDICATOR_HIDE_DELAY
        );
    
    Handle::current().spawn(async move {
       let mut sync_indicator_stream = std::pin::pin!(sync_indicator_stream);

        while let Some(indicator) = sync_indicator_stream.next().await {
            let is_syncing = match indicator {
                SyncIndicator::Show => true,
                SyncIndicator::Hide => false,
            };
            Cx::post_action(RoomsListHeaderAction::SetSyncStatus(is_syncing));
        }
    });
}

fn handle_room_list_service_loading_state(mut loading_state: Subscriber<RoomListLoadingState>) {
    log!("Initial room list loading state is {:?}", loading_state.get());
    Handle::current().spawn(async move {
        while let Some(state) = loading_state.next().await {
            log!("Received a room list loading state update: {state:?}");
            match state {
                RoomListLoadingState::NotLoaded => {
                    enqueue_rooms_list_update(RoomsListUpdate::NotLoaded);
                }
                RoomListLoadingState::Loaded { maximum_number_of_rooms } => {
                    enqueue_rooms_list_update(RoomsListUpdate::LoadedRooms { max_rooms: maximum_number_of_rooms });
                    // The SDK docs state that we cannot move from the `Loaded` state
                    // back to the `NotLoaded` state, so we can safely exit this task here.
                    return;
                }
            }
        }
    });
}

/// Spawns an async task to fetch the RoomPreview for the given successor room.
///
/// After the fetch completes, this emites a [`RoomPreviewAction`]
/// containing the fetched room preview or an error if it failed.
fn spawn_fetch_successor_room_preview(
    client: Client,
    successor_room: Option<SuccessorRoom>,
    tombstoned_room_id: OwnedRoomId,
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
) {
    Handle::current().spawn(async move {
        log!("Updating room {tombstoned_room_id} to be tombstoned, {successor_room:?}");
        let srd = if let Some(SuccessorRoom { room_id, reason }) = successor_room {
            match fetch_room_preview_with_avatar(
                &client,
                room_id.deref().into(),
                Vec::new(),
            ).await {
                Ok(room_preview) => SuccessorRoomDetails::Full { room_preview, reason },
                Err(e) => {
                    log!("Failed to fetch preview of successor room {room_id}, error: {e:?}");
                    SuccessorRoomDetails::Basic(SuccessorRoom { room_id, reason })
                }
            }
        } else {
            log!("BUG: room {tombstoned_room_id} was tombstoned but had no successor room!");
            SuccessorRoomDetails::None
        };

        match timeline_update_sender.send(TimelineUpdate::Tombstoned(srd)) {
            Ok(_) => SignalToUI::set_ui_signal(),
            Err(_) => error!("Failed to send the Tombstoned update to room {tombstoned_room_id}"),
        }
    });
}

/// Fetches the full preview information for the given `room`.
/// Also fetches that room preview's avatar, if it had an avatar URL.
async fn fetch_room_preview_with_avatar(
    client: &Client,
    room: &RoomOrAliasId,
    via: Vec<OwnedServerName>,
) -> Result<FetchedRoomPreview, matrix_sdk::Error> {
    let room_preview = client.get_room_preview(room, via).await?;
    // If this room has an avatar URL, fetch it.
    let room_avatar = if let Some(avatar_url) = room_preview.avatar_url.clone() {
        let media_request = MediaRequestParameters {
            source: MediaSource::Plain(avatar_url),
            format: AVATAR_THUMBNAIL_FORMAT.into(),
        };
        match client.media().get_media_content(&media_request, true).await {
            Ok(avatar_content) => {
                log!("Fetched avatar for room preview {:?} ({})", room_preview.name, room_preview.room_id);
                FetchedRoomAvatar::Image(avatar_content.into())
            }
            Err(e) => {
                log!("Failed to fetch avatar for room preview {:?} ({}), error: {e:?}",
                    room_preview.name, room_preview.room_id
                );
                avatar_from_room_name(room_preview.name.as_deref())
            }
        }
    } else {
        // The successor room did not have an avatar URL
        avatar_from_room_name(room_preview.name.as_deref())
    };
    Ok(FetchedRoomPreview::from(room_preview, room_avatar))
}

/// Fetches key details about the given thread root event.
///
/// Returns a tuple of:
/// 1. the number of replies in the thread (excluding the root event itself),
/// 2. the latest reply event, if it could be fetched.
async fn fetch_thread_summary_details(
    room: &Room,
    thread_root_event_id: &EventId,
) -> (u32, Option<matrix_sdk::deserialized_responses::TimelineEvent>) {
    let mut num_replies = 0;
    let mut latest_reply_event = None;

    if let Ok(thread_root_event) = room.load_or_fetch_event(thread_root_event_id, None).await
        && let Some(thread_summary) = thread_root_event.thread_summary.summary()
    {
        num_replies = thread_summary.num_replies;
        if let Some(latest_reply_event_id) = thread_summary.latest_reply.as_ref()
            && let Ok(latest_reply) = room.load_or_fetch_event(latest_reply_event_id, None).await
        {
            latest_reply_event = Some(latest_reply);
        }
    }

    // Always compute the reply count directly from the fetched thread relations,
    // for some reason we can't rely on the SDK-provided thread_summary to be accurate
    // (it's almost always totally wrong or out-of-date...).
    let count_replies_future = count_thread_replies(room, thread_root_event_id);

    // Fetch the latest reply event and count the thread replies in parallel.
    let (fetched_latest_reply_opt, reply_count_opt) = if latest_reply_event.is_none() {
        tokio::join!(
            fetch_latest_thread_reply_event(room, thread_root_event_id),
            count_replies_future,
        )
    } else {
        (None, count_replies_future.await)
    };

    if let Some(event) = fetched_latest_reply_opt {
        latest_reply_event = Some(event);
    }
    if let Some(count) = reply_count_opt {
        num_replies = count;
    }
    (num_replies, latest_reply_event)
}

/// Fetches the latest reply event in the thread rooted at `thread_root_event_id`.
async fn fetch_latest_thread_reply_event(
    room: &Room,
    thread_root_event_id: &EventId,
) -> Option<matrix_sdk::deserialized_responses::TimelineEvent> {
    let options = RelationsOptions {
        dir: Direction::Backward,
        limit: Some(uint!(1)),
        include_relations: IncludeRelations::RelationsOfType(RelationType::Thread),
        ..Default::default()
    };

    room.relations(thread_root_event_id.to_owned(), options)
        .await
        .ok()
        .and_then(|relations| relations.chunk.into_iter().next())
}

/// Counts all replies in the given thread by paginating `/relations` in batches.
async fn count_thread_replies(
    room: &Room,
    thread_root_event_id: &EventId,
) -> Option<u32> {
    let mut total_replies: u32 = 0;
    let mut next_batch_token = None;

    loop {
        let options = RelationsOptions {
            from: next_batch_token.clone(),
            dir: Direction::Backward,
            limit: Some(uint!(100)),
            include_relations: IncludeRelations::RelationsOfType(RelationType::Thread),
            ..Default::default()
        };

        let relations = room.relations(thread_root_event_id.to_owned(), options).await.ok()?;
        if relations.chunk.is_empty() {
            break;
        }
        total_replies = total_replies.saturating_add(relations.chunk.len() as u32);

        next_batch_token = relations.next_batch_token;
        if next_batch_token.is_none() {
            break;
        }
    }

    Some(total_replies)
}

/// Returns an HTML-formatted text preview of the given latest thread reply event.
async fn text_preview_of_latest_thread_reply(
    room: &Room,
    latest_reply_event: &matrix_sdk::deserialized_responses::TimelineEvent,
) -> Option<String> {
    let raw = latest_reply_event.raw();
    let sender_id = raw.get_field::<OwnedUserId>("sender").ok().flatten()?;
    let sender_room_member = match room.get_member_no_sync(&sender_id).await {
        Ok(Some(rm)) => Some(rm),
        _ => room.get_member(&sender_id).await.ok().flatten(),
    };
    let sender_name = sender_room_member.as_ref()
        .and_then(|rm| rm.display_name())
        .unwrap_or(sender_id.as_str());
    let text_preview = text_preview_of_raw_timeline_event(raw, sender_name).unwrap_or_else(|| {
        let event_type = raw.get_field::<String>("type").ok().flatten();
        TextPreview::from((
            event_type.unwrap_or_else(|| "unknown event type".to_string()),
            BeforeText::UsernameWithColon,
        ))
    });
    let preview_str = text_preview.format_with(sender_name, true);
    match utils::replace_linebreaks_separators(&preview_str, true) {
        Cow::Borrowed(_) => Some(preview_str),
        Cow::Owned(replaced) => Some(replaced),
    }
}

async fn sender_display_name_for_timeline_event(
    room: &Room,
    event: &matrix_sdk::deserialized_responses::TimelineEvent,
) -> Option<(OwnedUserId, String)> {
    let raw = event.raw();
    let sender_id = raw.get_field::<OwnedUserId>("sender").ok().flatten()?;
    let sender_room_member = match room.get_member_no_sync(&sender_id).await {
        Ok(Some(rm)) => Some(rm),
        _ => None,
    };
    let sender_name = sender_room_member.as_ref()
        .and_then(|rm| rm.display_name())
        .unwrap_or(sender_id.as_str())
        .to_string();
    Some((sender_id, sender_name))
}

fn fallback_preview_for_timeline_event(
    event: &matrix_sdk::deserialized_responses::TimelineEvent,
    sender_name: &str,
    as_html: bool,
) -> String {
    text_preview_of_raw_timeline_event(event.raw(), sender_name)
        .unwrap_or_else(|| {
            let event_type = event.raw().get_field::<String>("type").ok().flatten();
            TextPreview::from((
                event_type.unwrap_or_else(|| "unknown event type".to_string()),
                BeforeText::UsernameWithColon,
            ))
        })
        .format_with(sender_name, as_html)
}

async fn fetch_room_threads_page(
    room: &Room,
    from: Option<String>,
) -> Result<(Vec<FetchedRoomThread>, Option<String>), matrix_sdk::Error> {
    let response = room.list_threads(ListThreadsOptions {
        from: from.clone(),
        limit: Some(uint!(20)),
        ..Default::default()
    }).await?;

    let mut threads = Vec::new();
    for event in response.chunk {
        let Some(thread_root_event_id) = event.event_id() else { continue };
        let timestamp = event.timestamp().unwrap_or_else(MilliSecondsSinceUnixEpoch::now);
        let sender_name = sender_display_name_for_timeline_event(room, &event).await
            .map(|(_, sender_name)| sender_name)
            .unwrap_or_else(|| String::from("Unknown user"));
        let title = utils::replace_linebreaks_separators(
            &fallback_preview_for_timeline_event(&event, &sender_name, false),
            true,
        ).into_owned();
        let title = if title.trim().is_empty() {
            String::from("(No message preview)")
        } else {
            title
        };

        let reply_count = event.thread_summary.summary()
            .map(|summary| summary.num_replies)
            .unwrap_or(0);
        let latest_reply_preview = if let Some(latest_event) = event.bundled_latest_thread_event.as_ref() {
            text_preview_of_latest_thread_reply(room, latest_event).await
        } else {
            None
        };

        threads.push(FetchedRoomThread {
            thread_root_event_id,
            timestamp,
            title,
            reply_count,
            latest_reply_preview,
        });
    }

    Ok((threads, response.prev_batch_token))
}


/// Returns the timestamp and an HTML-formatted text preview of the given `latest_event`.
///
/// If the sender profile of the event is not yet available, this function will
/// generate a preview using the sender's user ID instead of their display name.
async fn get_latest_event_details(
    latest_event_value: &LatestEventValue,
    client: &Client,
) -> Option<(MilliSecondsSinceUnixEpoch, String)> {
    macro_rules! get_sender_username {
        ($profile:expr, $sender:expr, $is_own:expr) => {{
            let sender_username_opt = if let TimelineDetails::Ready(profile) = $profile {
                profile.display_name.clone()
            } else if $is_own {
                client.account().get_display_name().await.ok().flatten()
            } else {
                None
            };
            sender_username_opt.unwrap_or_else(|| $sender.to_string())
        }};
    }

    match latest_event_value {
        LatestEventValue::None => None,
        LatestEventValue::Remote { timestamp, sender, is_own, profile, content } => {
            let sender_username = get_sender_username!(profile, sender, *is_own);
            let latest_message_text = text_preview_of_timeline_item(
                content,
                sender,
                &sender_username,
            ).format_with(&sender_username, true);
            Some((*timestamp, latest_message_text))
        }
        LatestEventValue::Local { timestamp, sender, profile, content, state: _ } => {
            // TODO: use the `state` enum to augment the preview text with more details.
            //       Example: "<span color="blue">Sending... {msg}</span>" or
            //                "<span color="red">Failed to send {msg}</span>"
            let is_own = current_user_id().is_some_and(|id| &id == sender);
            let sender_username = get_sender_username!(profile, sender, is_own);
            let latest_message_text = text_preview_of_timeline_item(
                content,
                sender,
                &sender_username,
            ).format_with(&sender_username, true);
            Some((*timestamp, latest_message_text))
        }
        LatestEventValue::RemoteInvite { timestamp, .. } => {
            Some((*timestamp, String::from("You were invited to this room.")))
        }
    }    
}

/// Handles the given updated latest event for the given room.
///
/// This function sends a `RoomsListUpdate::UpdateLatestEvent`
/// to update the latest event in the RoomsListEntry for the given room.
async fn update_latest_event(room: &Room) {
    if let Some((timestamp, latest_message_text)) = get_latest_event_details(
        &room.latest_event().await,
        &room.client(),
    ).await {
        enqueue_rooms_list_update(RoomsListUpdate::UpdateLatestEvent {
            room_id: room.room_id().to_owned(),
            timestamp,
            latest_message_text,
        });
    }
}

/// A request to search backwards for a specific event in a room's timeline.
pub struct BackwardsPaginateUntilEventRequest {
    pub room_id: OwnedRoomId,
    pub target_event_id: OwnedEventId,
    /// The index in the timeline where a backwards search should begin.
    pub starting_index: usize,
    /// The number of items in the timeline at the time of the request,
    /// which is used to detect if the timeline has changed since the request was made,
    /// meaning that the `starting_index` can no longer be relied upon.
    pub current_tl_len: usize,
}

/// Whether to enable verbose logging of all timeline diff updates.
const LOG_TIMELINE_DIFFS: bool = cfg!(feature = "log_timeline_diffs");
/// Whether to enable verbose logging of all room list service diff updates.
const LOG_ROOM_LIST_DIFFS: bool = cfg!(feature = "log_room_list_diffs");

/// A per-timeline async task that listens for timeline updates and sends them to the UI thread.
///
/// One instance of this async task is spawned for each room the client knows about,
/// and also one for each thread that the user opens in a thread view.
async fn timeline_subscriber_handler(
    room: Room,
    timeline: Arc<Timeline>,
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    mut request_receiver: watch::Receiver<Vec<BackwardsPaginateUntilEventRequest>>,
    thread_root_event_id: Option<OwnedEventId>,
) {

    /// An inner function that searches the given new timeline items for a target event.
    ///
    /// If the target event is found, it is removed from the `target_event_id_opt` and returned,
    /// along with the index/position of that event in the given iterator of new items.
    fn find_target_event<'a>(
        target_event_id_opt: &mut Option<OwnedEventId>,
        mut new_items_iter: impl Iterator<Item = &'a Arc<TimelineItem>>,
    ) -> Option<(usize, OwnedEventId)> {
        let found_index = target_event_id_opt
            .as_ref()
            .and_then(|target_event_id| new_items_iter
                .position(|new_item| new_item
                    .as_event()
                    .is_some_and(|new_ev| new_ev.event_id() == Some(target_event_id))
                )
            );

        if let Some(index) = found_index {
            target_event_id_opt.take().map(|ev| (index, ev))
        } else {
            None
        }
    }


    let room_id = room.room_id().to_owned();
    log!("Starting timeline subscriber for room {room_id}, thread {thread_root_event_id:?}...");
    let (mut timeline_items, mut subscriber) = timeline.subscribe().await;
    log!("Received initial timeline update of {} items for room {room_id}, thread {thread_root_event_id:?}.", timeline_items.len());

    timeline_update_sender.send(TimelineUpdate::FirstUpdate {
        initial_items: timeline_items.clone(),
    }).unwrap_or_else(
        |_e| panic!("Error: timeline update sender couldn't send first update ({} items) to room {room_id}, thread {thread_root_event_id:?}...!", timeline_items.len())
    );

    // the event ID to search for while loading previous items into the timeline.
    let mut target_event_id = None;
    // the timeline index and event ID of the target event, if it has been found.
    let mut found_target_event_id: Option<(usize, OwnedEventId)> = None;

    loop { tokio::select! {
        // we should check for new requests before handling new timeline updates,
        // because the request might influence how we handle a timeline update.
        biased;

        // Handle updates to the current backwards pagination requests.
        Ok(()) = request_receiver.changed() => {
            let prev_target_event_id = target_event_id.clone();
            let new_request_details = request_receiver
                .borrow_and_update()
                .iter()
                .find_map(|req| req.room_id
                    .eq(&room_id)
                    .then(|| (req.target_event_id.clone(), req.starting_index, req.current_tl_len))
                );

            target_event_id = new_request_details.as_ref().map(|(ev, ..)| ev.clone());

            // If we received a new request, start searching backwards for the target event.
            if let Some((new_target_event_id, starting_index, current_tl_len)) = new_request_details {
                if prev_target_event_id.as_ref() != Some(&new_target_event_id) {
                    let starting_index = if current_tl_len == timeline_items.len() {
                        starting_index
                    } else {
                        // The timeline has changed since the request was made, so we can't rely on the `starting_index`.
                        // Instead, we have no choice but to start from the end of the timeline.
                        timeline_items.len()
                    };
                    // log!("Received new request to search for event {new_target_event_id} in room {room_id}, thread {thread_root_event_id:?} starting from index {starting_index} (tl len {}).", timeline_items.len());
                    // Search backwards for the target event in the timeline, starting from the given index.
                    if let Some(target_event_tl_index) = timeline_items
                        .focus()
                        .narrow(..starting_index)
                        .into_iter()
                        .rev()
                        .position(|i| i.as_event()
                            .and_then(|e| e.event_id())
                            .is_some_and(|ev_id| ev_id == new_target_event_id)
                        )
                        .map(|i| starting_index.saturating_sub(i).saturating_sub(1))
                    {
                        // log!("Found existing target event {new_target_event_id} in room {room_id}, thread {thread_root_event_id:?} at index {target_event_tl_index}.");

                        // Nice! We found the target event in the current timeline items,
                        // so there's no need to actually proceed with backwards pagination;
                        // thus, we can clear the locally-tracked target event ID.
                        target_event_id = None;
                        found_target_event_id = None;
                        timeline_update_sender.send(
                            TimelineUpdate::TargetEventFound {
                                target_event_id: new_target_event_id.clone(),
                                index: target_event_tl_index,
                            }
                        ).unwrap_or_else(
                            |_e| panic!("Error: timeline update sender couldn't send TargetEventFound({new_target_event_id}, {target_event_tl_index}) to room {room_id}, thread {thread_root_event_id:?}!")
                        );
                        // Send a Makepad-level signal to update this room's timeline UI view.
                        SignalToUI::set_ui_signal();
                    }
                    else {
                        log!("Target event not in timeline. Starting backwards pagination \
                            in room {room_id}, thread {thread_root_event_id:?} to find target event \
                            {new_target_event_id} starting from index {starting_index}.",
                        );
                        // If we didn't find the target event in the current timeline items,
                        // we need to start loading previous items into the timeline.
                        submit_async_request(MatrixRequest::PaginateTimeline {
                            timeline_kind: if let Some(thread_root_event_id) = thread_root_event_id.clone() {
                                TimelineKind::Thread {
                                    room_id: room_id.clone(),
                                    thread_root_event_id,
                                }
                            } else {
                                TimelineKind::MainRoom {
                                    room_id: room_id.clone(),
                                }
                            },
                            num_events: 50,
                            direction: PaginationDirection::Backwards,
                        });
                    }
                }
            }
        }

        // Handle updates to the actual timeline content.
        batch_opt = subscriber.next() => {
            let Some(batch) = batch_opt else { break };
            let mut num_updates = 0;
            let mut index_of_first_change = usize::MAX;
            let mut index_of_last_change = usize::MIN;
            // whether to clear the entire cache of drawn items
            let mut clear_cache = false;
            // whether the changes include items being appended to the end of the timeline
            let mut is_append = false;
            for diff in batch {
                num_updates += 1;
                match diff {
                    VectorDiff::Append { values } => {
                        let _values_len = values.len();
                        index_of_first_change = min(index_of_first_change, timeline_items.len());
                        timeline_items.extend(values);
                        index_of_last_change = max(index_of_last_change, timeline_items.len());
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Append {_values_len}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        is_append = true;
                    }
                    VectorDiff::Clear => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Clear"); }
                        clear_cache = true;
                        timeline_items.clear();
                    }
                    VectorDiff::PushFront { value } => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff PushFront"); }
                        if let Some((index, _ev)) = found_target_event_id.as_mut() {
                            *index += 1; // account for this new `value` being prepended.
                        } else {
                            found_target_event_id = find_target_event(&mut target_event_id, std::iter::once(&value));
                        }

                        clear_cache = true;
                        timeline_items.push_front(value);
                    }
                    VectorDiff::PushBack { value } => {
                        index_of_first_change = min(index_of_first_change, timeline_items.len());
                        timeline_items.push_back(value);
                        index_of_last_change = max(index_of_last_change, timeline_items.len());
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff PushBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        is_append = true;
                    }
                    VectorDiff::PopFront => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff PopFront"); }
                        clear_cache = true;
                        timeline_items.pop_front();
                        if let Some((i, _ev)) = found_target_event_id.as_mut() {
                            *i = i.saturating_sub(1); // account for the first item being removed.
                        }
                        // This doesn't affect whether we should reobtain the latest event.
                    }
                    VectorDiff::PopBack => {
                        timeline_items.pop_back();
                        index_of_first_change = min(index_of_first_change, timeline_items.len());
                        index_of_last_change = usize::MAX;
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff PopBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Insert { index, value } => {
                        if index == 0 {
                            clear_cache = true;
                        } else {
                            index_of_first_change = min(index_of_first_change, index);
                            index_of_last_change = usize::MAX;
                        }
                        if index >= timeline_items.len() {
                            is_append = true;
                        }

                        if let Some((i, _ev)) = found_target_event_id.as_mut() {
                            // account for this new `value` being inserted before the previously-found target event's index.
                            if index <= *i {
                                *i += 1;
                            }
                        } else {
                            found_target_event_id = find_target_event(&mut target_event_id, std::iter::once(&value))
                                .map(|(i, ev)| (i + index, ev));
                        }

                        timeline_items.insert(index, value);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Insert at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Set { index, value } => {
                        index_of_first_change = min(index_of_first_change, index);
                        index_of_last_change  = max(index_of_last_change, index.saturating_add(1));
                        timeline_items.set(index, value);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Set at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Remove { index } => {
                        if index == 0 {
                            clear_cache = true;
                        } else {
                            index_of_first_change = min(index_of_first_change, index.saturating_sub(1));
                            index_of_last_change = usize::MAX;
                        }
                        if let Some((i, _ev)) = found_target_event_id.as_mut() {
                            // account for an item being removed before the previously-found target event's index.
                            if index <= *i {
                                *i = i.saturating_sub(1);
                            }
                        }
                        timeline_items.remove(index);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Remove at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Truncate { length } => {
                        if length == 0 {
                            clear_cache = true;
                        } else {
                            index_of_first_change = min(index_of_first_change, length.saturating_sub(1));
                            index_of_last_change = usize::MAX;
                        }
                        timeline_items.truncate(length);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Truncate to length {length}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Reset { values } => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id}, thread {thread_root_event_id:?} diff Reset, new length {}", values.len()); }
                        clear_cache = true; // we must assume all items have changed.
                        timeline_items = values;
                    }
                }
            }


            if num_updates > 0 {
                // Handle the case where back pagination inserts items at the beginning of the timeline
                // (meaning the entire timeline needs to be re-drawn),
                // but there is a virtual event at index 0 (e.g., a day divider).
                // When that happens, we want the RoomScreen to treat this as if *all* events changed.
                if index_of_first_change == 1 && timeline_items.front().and_then(|item| item.as_virtual()).is_some() {
                    index_of_first_change = 0;
                    clear_cache = true;
                }

                let changed_indices = index_of_first_change..index_of_last_change;

                if LOG_TIMELINE_DIFFS {
                    log!("timeline_subscriber: applied {num_updates} updates for room {room_id}, thread {thread_root_event_id:?}, timeline now has {} items. is_append? {is_append}, clear_cache? {clear_cache}. Changes: {changed_indices:?}.", timeline_items.len());
                }
                timeline_update_sender.send(TimelineUpdate::NewItems {
                    new_items: timeline_items.clone(),
                    changed_indices,
                    clear_cache,
                    is_append,
                }).expect("Error: timeline update sender couldn't send update with new items!");

                // We must send this update *after* the actual NewItems update,
                // otherwise the UI thread (RoomScreen) won't be able to correctly locate the target event.
                if let Some((index, found_event_id)) = found_target_event_id.take() {
                    target_event_id = None;
                    timeline_update_sender.send(
                        TimelineUpdate::TargetEventFound {
                            target_event_id: found_event_id.clone(),
                            index,
                        }
                    ).unwrap_or_else(
                        |_e| panic!("Error: timeline update sender couldn't send TargetEventFound({found_event_id}, {index}) to room {room_id}, thread {thread_root_event_id:?}!")
                    );
                }

                // Send a Makepad-level signal to update this room's timeline UI view.
                SignalToUI::set_ui_signal();
            }
        }

        else => {
            break;
        }
    } }

    error!("Error: unexpectedly ended timeline subscriber for room {room_id}, thread {thread_root_event_id:?}.");
}

/// Spawn a new async task to fetch the room's new avatar.
fn spawn_fetch_room_avatar(room: &RoomListServiceRoomInfo) {
    let room_id = room.room_id.clone();
    let room_name_id = RoomNameId::from((room.display_name.clone(), room.room_id.clone()));
    let inner_room = room.room.clone();
    Handle::current().spawn(async move {
        let room_avatar = room_avatar(&inner_room, &room_name_id).await;
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomAvatar {
            room_id,
            room_avatar,
        });
    });
}

/// Fetches and returns the avatar image for the given room (if one exists),
/// otherwise returns a text avatar string of the first character of the room name.
async fn room_avatar(room: &Room, room_name_id: &RoomNameId) -> FetchedRoomAvatar {
    match room.avatar(AVATAR_THUMBNAIL_FORMAT.into()).await {
        Ok(Some(avatar)) => FetchedRoomAvatar::Image(avatar.into()),
        _ => {
            if let Ok(room_members) = room.members(RoomMemberships::ACTIVE).await {
                if room_members.len() == 2 {
                    if let Some(non_account_member) = room_members.iter().find(|m| !m.is_account_user()) {
                        if let Ok(Some(avatar)) = non_account_member.avatar(AVATAR_THUMBNAIL_FORMAT.into()).await {
                            return FetchedRoomAvatar::Image(avatar.into());
                        }
                    }
                }
            }
            utils::avatar_from_room_name(room_name_id.name_for_avatar())
        }
    }
}

/// Spawn an async task to login to the given Matrix homeserver using the given SSO identity provider ID.
///
/// This function will post a `LoginAction::SsoPending(true)` to the main thread, and another
/// `LoginAction::SsoPending(false)` once the async task has either successfully logged in or
/// failed to do so.
///
/// If the login attempt is successful, the resulting `Client` and `ClientSession` will be sent
/// to the login screen using the `login_sender`.
async fn spawn_sso_server(
    brand: String,
    homeserver_url: String,
    identity_provider_id: String,
    proxy: Option<String>,
    login_sender: Sender<LoginRequest>,
) {
    Cx::post_action(LoginAction::SsoPending(true));
    // Post a status update to inform the user that we're waiting for the client to be built.
    Cx::post_action(LoginAction::Status {
        title: "Initializing client...".into(),
        status: "Please wait while Matrix builds and configures the client object for login.".into(),
    });

    // Wait for the notification that the client has been built
    DEFAULT_SSO_CLIENT_NOTIFIER.notified().await;

    // Try to use the DEFAULT_SSO_CLIENT, if it was successfully built.
    // We do not clone it because a Client cannot be re-used again
    // once it has been used for a login attempt, so this forces us to create a new one
    // if that occurs.
    let client_and_session_opt = DEFAULT_SSO_CLIENT.lock().unwrap().take();

    Handle::current().spawn(async move {
        let effective_proxy = crate::proxy_config::resolve_effective_proxy_url(proxy.as_deref());
        if let Some(proxy) = effective_proxy.as_deref() {
            if let Err(e) = crate::proxy_config::apply_proxy_to_process_env(Some(proxy)) {
                warning!("Failed to apply proxy env before SSO login: {e}");
            }
        }

        // Try to use the DEFAULT_SSO_CLIENT that we proactively created
        // during initialization (to speed up opening the SSO browser window).
        let mut client_and_session = client_and_session_opt;

        // If the DEFAULT_SSO_CLIENT is none (meaning it failed to build),
        // or if the homeserver_url is *not* empty and isn't the default,
        // we cannot use the DEFAULT_SSO_CLIENT, so we must build a new one.
        let mut build_client_error = None;
        if client_and_session.is_none() || effective_proxy.is_some() || (
            !homeserver_url.is_empty()
                && homeserver_url != "matrix.org"
                && Url::parse(&homeserver_url) != Url::parse("https://matrix-client.matrix.org/")
                && Url::parse(&homeserver_url) != Url::parse("https://matrix.org/")
        ) {
            match build_client(
                &Cli {
                    homeserver: homeserver_url.is_empty().not().then_some(homeserver_url),
                    proxy: effective_proxy,
                    ..Default::default()
                },
                app_data_dir(),
            ).await {
                Ok(success) => client_and_session = Some(success),
                Err(e) => build_client_error = Some(e),
            }
        }

        let Some((client, client_session)) = client_and_session else {
            Cx::post_action(LoginAction::LoginFailure(
                if let Some(err) = build_client_error {
                    format!("Could not create client object. Please try to login again.\n\nError: {err}")
                } else {
                    String::from("Could not create client object. Please try to login again.")
                }
            ));
            // This ensures that the called to `DEFAULT_SSO_CLIENT_NOTIFIER.notified()`
            // at the top of this function will not block upon the next login attempt.
            DEFAULT_SSO_CLIENT_NOTIFIER.notify_one();
            Cx::post_action(LoginAction::SsoPending(false));
            return;
        };

        let mut is_logged_in = false;
        Cx::post_action(LoginAction::Status {
            title: "Opening your browser...".into(),
            status: "Please finish logging in using your browser, and then come back to Robrix.".into(),
        });
        match client
            .matrix_auth()
            .login_sso(|sso_url: String| async move {
                let url = Url::parse(&sso_url)?;
                for (key, value) in url.query_pairs() {
                    if key == "redirectUrl" {
                        let redirect_url = Url::parse(&value)?;
                        Cx::post_action(LoginAction::SsoSetRedirectUrl(redirect_url));
                        break
                    }
                }
                Uri::new(&sso_url).open().map_err(|err|
                    Error::Io(io::Error::other(format!("Unable to open SSO login url. Error: {:?}", err)))
                )
            })
            .identity_provider_id(&identity_provider_id)
            .initial_device_display_name(&format!("robrix-sso-{brand}"))
            .await
            .inspect(|_| {
                if let Some(client) = get_client() {
                    if client.matrix_auth().logged_in() {
                        is_logged_in = true;
                        log!("Already logged in, ignore login with sso");
                    }
                }
            }) {
            Ok(identity_provider_res) => {
                if !is_logged_in {
                    if let Err(e) = login_sender.send(LoginRequest::LoginBySSOSuccess(client, client_session, false)).await {
                        error!("Error sending login request to login_sender: {e:?}");
                        Cx::post_action(LoginAction::LoginFailure(String::from(
                            "BUG: failed to send login request to matrix worker thread."
                        )));
                    }
                    enqueue_rooms_list_update(RoomsListUpdate::Status {
                        status: format!(
                            "Logged in as {:?}.\n → Loading rooms...",
                            &identity_provider_res.user_id
                        ),
                    });
                }
            }
            Err(e) => {
                if !is_logged_in {
                    error!("SSO Login failed: {e:?}");
                    Cx::post_action(LoginAction::LoginFailure(format!("SSO login failed: {e}")));
                }
            }
        }

        // This ensures that the called to `DEFAULT_SSO_CLIENT_NOTIFIER.notified()`
        // at the top of this function will not block upon the next login attempt.
        DEFAULT_SSO_CLIENT_NOTIFIER.notify_one();
        Cx::post_action(LoginAction::SsoPending(false));
    });
}


bitflags! {
    /// The powers that a user has in a given room.
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct UserPowerLevels: u64 {
        const Ban = 1 << 0;
        const Invite = 1 << 1;
        const Kick = 1 << 2;
        const Redact = 1 << 3;
        const NotifyRoom = 1 << 4;
        // -------------------------------------
        // -- Copied from TimelineEventType ----
        // -- Unused powers are commented out --
        // -------------------------------------
        // const CallAnswer = 1 << 5;
        // const CallInvite = 1 << 6;
        // const CallHangup = 1 << 7;
        // const CallCandidates = 1 << 8;
        // const CallNegotiate = 1 << 9;
        // const CallReject = 1 << 10;
        // const CallSdpStreamMetadataChanged = 1 << 11;
        // const CallSelectAnswer = 1 << 12;
        // const KeyVerificationReady = 1 << 13;
        // const KeyVerificationStart = 1 << 14;
        // const KeyVerificationCancel = 1 << 15;
        // const KeyVerificationAccept = 1 << 16;
        // const KeyVerificationKey = 1 << 17;
        // const KeyVerificationMac = 1 << 18;
        // const KeyVerificationDone = 1 << 19;
        const Location = 1 << 20;
        const Message = 1 << 21;
        // const PollStart = 1 << 22;
        // const UnstablePollStart = 1 << 23;
        // const PollResponse = 1 << 24;
        // const UnstablePollResponse = 1 << 25;
        // const PollEnd = 1 << 26;
        // const UnstablePollEnd = 1 << 27;
        // const Beacon = 1 << 28;
        const Reaction = 1 << 29;
        // const RoomEncrypted = 1 << 30;
        const RoomMessage = 1 << 31;
        const RoomRedaction = 1 << 32;
        const Sticker = 1 << 33;
        // const CallNotify = 1 << 34;
        // const PolicyRuleRoom = 1 << 35;
        // const PolicyRuleServer = 1 << 36;
        // const PolicyRuleUser = 1 << 37;
        // const RoomAliases = 1 << 38;
        // const RoomAvatar = 1 << 39;
        // const RoomCanonicalAlias = 1 << 40;
        // const RoomCreate = 1 << 41;
        // const RoomEncryption = 1 << 42;
        // const RoomGuestAccess = 1 << 43;
        // const RoomHistoryVisibility = 1 << 44;
        // const RoomJoinRules = 1 << 45;
        // const RoomMember = 1 << 46;
        // const RoomName = 1 << 47;
        const RoomPinnedEvents = 1 << 48;
        const RoomPowerLevels = 1 << 49;
        // const RoomServerAcl = 1 << 50;
        // const RoomThirdPartyInvite = 1 << 51;
        // const RoomTombstone = 1 << 52;
        // const RoomTopic = 1 << 53;
        // const SpaceChild = 1 << 54;
        // const SpaceParent = 1 << 55;
        // const BeaconInfo = 1 << 56;
        // const CallMember = 1 << 57;
        // const MemberHints = 1 << 58;
    }
}
impl UserPowerLevels {
    pub fn from(power_levels: &RoomPowerLevels, user_id: &UserId) -> Self {
        let mut retval = UserPowerLevels::empty();
        let user_power = power_levels.for_user(user_id);
        retval.set(UserPowerLevels::Ban, user_power >= power_levels.ban);
        retval.set(UserPowerLevels::Invite, user_power >= power_levels.invite);
        retval.set(UserPowerLevels::Kick, user_power >= power_levels.kick);
        retval.set(UserPowerLevels::Redact, user_power >= power_levels.redact);
        retval.set(UserPowerLevels::NotifyRoom, user_power >= power_levels.notifications.room);
        retval.set(UserPowerLevels::Location, user_power >= power_levels.for_message(MessageLikeEventType::Location));
        retval.set(UserPowerLevels::Message, user_power >= power_levels.for_message(MessageLikeEventType::Message));
        retval.set(UserPowerLevels::Reaction, user_power >= power_levels.for_message(MessageLikeEventType::Reaction));
        retval.set(UserPowerLevels::RoomMessage, user_power >= power_levels.for_message(MessageLikeEventType::RoomMessage));
        retval.set(UserPowerLevels::RoomRedaction, user_power >= power_levels.for_message(MessageLikeEventType::RoomRedaction));
        retval.set(UserPowerLevels::Sticker, user_power >= power_levels.for_message(MessageLikeEventType::Sticker));
        retval.set(UserPowerLevels::RoomPinnedEvents, user_power >= power_levels.for_state(StateEventType::RoomPinnedEvents));
        retval.set(UserPowerLevels::RoomPowerLevels, power_levels.user_can_send_state(user_id, StateEventType::RoomPowerLevels));
        retval
    }

    pub async fn from_room(room: &Room, user_id: &UserId) -> Option<Self> {
        let room_power_levels = room.power_levels().await.ok()?;
        Some(UserPowerLevels::from(&room_power_levels, user_id))
    }

    pub fn can_ban(self) -> bool {
        self.contains(UserPowerLevels::Ban)
    }

    pub fn can_unban(self) -> bool {
        self.can_ban() && self.can_kick()
    }

    pub fn can_invite(self) -> bool {
        self.contains(UserPowerLevels::Invite)
    }

    pub fn can_kick(self) -> bool {
        self.contains(UserPowerLevels::Kick)
    }

    pub fn can_redact(self) -> bool {
        self.contains(UserPowerLevels::Redact)
    }

    pub fn can_notify_room(self) -> bool {
        self.contains(UserPowerLevels::NotifyRoom)
    }

    pub fn can_redact_own(self) -> bool {
        self.contains(UserPowerLevels::RoomRedaction)
    }

    pub fn can_redact_others(self) -> bool {
        self.can_redact_own() && self.contains(UserPowerLevels::Redact)
    }

    pub fn can_send_location(self) -> bool {
        self.contains(UserPowerLevels::Location)
    }

    pub fn can_send_message(self) -> bool {
        self.contains(UserPowerLevels::RoomMessage)
        || self.contains(UserPowerLevels::Message)
    }

    pub fn can_send_reaction(self) -> bool {
        self.contains(UserPowerLevels::Reaction)
    }

    pub fn can_send_sticker(self) -> bool {
        self.contains(UserPowerLevels::Sticker)
    }

    #[doc(alias("unpin"))]
    pub fn can_pin(self) -> bool {
        self.contains(UserPowerLevels::RoomPinnedEvents)
    }

    pub fn can_change_room_power_levels(self) -> bool {
        self.contains(UserPowerLevels::RoomPowerLevels)
    }
}


/// Shuts down the current Tokio runtime completely and takes ownership to ensure proper cleanup.
pub fn shutdown_background_tasks() {
    cancel_active_oidc_flow();
    if let Some(runtime) = TOKIO_RUNTIME.lock().unwrap().take() {
        runtime.shutdown_background();
    }
}

pub async fn clear_app_state(config: &LogoutConfig) -> Result<()> {
    // Clear resources normally, allowing them to be properly dropped
    // This prevents memory leaks when users logout and login again without closing the app
    cancel_active_oidc_flow();
    CLIENT.lock().unwrap().take();
    SYNC_SERVICE.lock().unwrap().take();
    REQUEST_SENDER.lock().unwrap().take();
    IGNORED_USERS.lock().unwrap().clear();
    ALL_JOINED_ROOMS.lock().unwrap().clear();

    let on_clear_appstate = Arc::new(Notify::new());
    Cx::post_action(LogoutAction::ClearAppState { on_clear_appstate: on_clear_appstate.clone() });
    
    match tokio::time::timeout(config.app_state_cleanup_timeout, on_clear_appstate.notified()).await {
        Ok(_) => {
            log!("Received signal that UI-side app state was cleaned successfully");
            Ok(())
        }
        Err(_) => Err(anyhow!("Timed out waiting for UI-side app state cleanup")),
    }
}

/// Probe a homeserver's registration capabilities.
///
/// Fetches in order:
/// 1. GET `.well-known/matrix/client` — discover base_url and MAS issuer (lenient)
/// 2. GET `/_matrix/client/versions` — liveness check (fatal)
/// 3. GET `/_matrix/client/v3/login` — enumerate SSO providers (non-fatal)
/// 4. POST `/_matrix/client/v3/register` empty body — harvest UIAA flows (fatal)
///
/// Note: `matrix_sdk::reqwest::Response` does not expose `.json()`, so all
/// response bodies are read as text and parsed via `serde_json::from_str`.
fn build_discovery_http_client(
    proxy_override: Option<&str>,
) -> anyhow::Result<matrix_sdk::reqwest::Client> {
    let mut builder = matrix_sdk::reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5));
    if let Some(proxy) = crate::proxy_config::resolve_effective_proxy_url(proxy_override) {
        crate::proxy_config::validate_proxy_url(&proxy)
            .map_err(|e| anyhow::anyhow!(e))?;
        builder = builder.proxy(matrix_sdk::reqwest::Proxy::all(&proxy)?);
    }
    Ok(builder.build()?)
}

async fn discover_homeserver_capabilities(
    raw_url: &str,
    proxy_override: Option<&str>,
) -> anyhow::Result<HsCapabilities> {
    use serde_json::Value;

    let http = build_discovery_http_client(proxy_override)?;

    // Helper: read response text and parse as JSON Value, returning Null on any failure.
    async fn body_json(resp: matrix_sdk::reqwest::Response) -> Value {
        match resp.text().await {
            Ok(text) => serde_json::from_str::<Value>(&text).unwrap_or(Value::Null),
            Err(_) => Value::Null,
        }
    }

    // Step 1: .well-known (lenient — default base_url = raw_url on failure).
    let wk_url = format!("{raw_url}/.well-known/matrix/client");
    let (base_url, is_mas, mas_signup_url, mas_issuer_url) = match http.get(&wk_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = body_json(resp).await;
            let base = body
                .get("m.homeserver")
                .and_then(|m: &Value| m.get("base_url"))
                .and_then(|v: &Value| v.as_str())
                .unwrap_or(raw_url)
                .trim_end_matches('/')
                .to_string();
            // Detect MAS and derive the signup URL in one pass. Prefer stable key.
            // MAS exposes the self-registration form at `<issuer>/register` when
            // open registration is enabled; closed deployments return a polite
            // "registration not available" page at the same path. The MSC2965
            // `account` field is for post-login account management (requires a
            // session) — opening it while unauthenticated loops between
            // /account/ and /login, so we do NOT use it here.
            let (mas, mas_signup_url, mas_issuer_url) = ["m.authentication", "org.matrix.msc2965.authentication"]
                .iter()
                .find_map(|key: &&str| {
                    let issuer = body.get(*key)?.get("issuer").and_then(|v: &Value| v.as_str())?;
                    let issuer = issuer.trim_end_matches('/').to_string();
                    let signup = format!("{issuer}/register");
                    Some((true, Some(signup), Some(issuer)))
                })
                .unwrap_or((false, None, None));
            (base, mas, mas_signup_url, mas_issuer_url)
        }
        _ => (raw_url.trim_end_matches('/').to_string(), false, None, None),
    };

    // Step 2: versions — liveness (fatal if unreachable).
    let versions_url = format!("{base_url}/_matrix/client/versions");
    http.get(&versions_url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("homeserver unreachable: {e}"))?;

    // Step 3: /v3/login — SSO providers (non-fatal on failure).
    let login_url = format!("{base_url}/_matrix/client/v3/login");
    let sso_providers = match http.get(&login_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = body_json(resp).await;
            body.get("flows")
                .and_then(|f: &Value| f.as_array())
                .map(|flows| {
                    flows
                        .iter()
                        .filter(|f: &&Value| {
                            f.get("type").and_then(|t: &Value| t.as_str()) == Some("m.login.sso")
                        })
                        .flat_map(|f: &Value| {
                            f.get("identity_providers")
                                .and_then(|ip: &Value| ip.as_array())
                                .cloned()
                                .unwrap_or_default()
                        })
                        .filter_map(|p: Value| {
                            Some(IdentityProviderSummary {
                                id: p.get("id")?.as_str()?.to_string(),
                                name: p
                                    .get("name")
                                    .and_then(|n: &Value| n.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                icon_url: p
                                    .get("icon")
                                    .and_then(|v: &Value| v.as_str())
                                    .map(String::from),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };

    // Step 4: POST /register empty body — UIAA flow probe.
    let register_url = format!("{base_url}/_matrix/client/v3/register");
    let reg_resp = http
        .post(&register_url)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await?;

    let status = reg_resp.status();
    let body = body_json(reg_resp).await;

    let (registration_enabled, uiaa_probe) = if status == matrix_sdk::reqwest::StatusCode::UNAUTHORIZED {
        // Expected UIAA challenge.
        match serde_json::from_value(body.clone()) {
            Ok(info) => (true, Some(info)),
            Err(_) => (true, None),
        }
    } else {
        (false, None)
    };

    Ok(HsCapabilities {
        base_url,
        is_mas_native_oidc: is_mas,
        registration_enabled,
        uiaa_probe,
        sso_providers,
        mas_signup_url,
        mas_issuer_url,
    })
}

#[cfg(test)]
mod tests {
    use matrix_sdk::ruma::user_id;

    use super::{
        OidcFlowSlot, RestoreSessionFailureAction, build_discovery_http_client,
        restore_session_failure_action, restore_session_failure_message,
        session_validation_failure_action, should_prebuild_default_sso_client,
        worker_shutdown_is_unexpected,
    };
    use crate::persistence::RestoreSessionError;

    #[test]
    fn worker_shutdown_is_not_unexpected_during_logout() {
        assert!(!worker_shutdown_is_unexpected(true, false));
    }

    #[test]
    fn worker_shutdown_is_not_unexpected_during_account_switch() {
        assert!(!worker_shutdown_is_unexpected(false, true));
    }

    #[test]
    fn worker_shutdown_is_unexpected_without_controlled_teardown() {
        assert!(worker_shutdown_is_unexpected(false, false));
    }

    #[test]
    fn oidc_flow_slot_rejects_duplicate_start_until_cleared() {
        let mut slot = OidcFlowSlot::default();

        let _first = slot.try_start_flow().unwrap();
        assert!(slot.try_start_flow().is_err());

        assert!(slot.cancel_active_flow());
        assert!(slot.try_start_flow().is_ok());
    }

    #[test]
    fn oidc_flow_slot_finish_is_scoped_to_matching_generation() {
        let mut slot = OidcFlowSlot::default();

        let (first_id, _first_rx) = slot.try_start_flow().unwrap();
        assert!(slot.cancel_active_flow());

        let (second_id, _second_rx) = slot.try_start_flow().unwrap();
        slot.finish_flow(first_id);
        assert!(slot.has_active_flow());

        slot.finish_flow(second_id);
        assert!(!slot.has_active_flow());
    }

    #[test]
    fn discovery_http_client_accepts_valid_proxy_override() {
        let client = build_discovery_http_client(Some("http://127.0.0.1:8080")).unwrap();
        drop(client);
    }

    #[test]
    fn discovery_http_client_rejects_invalid_proxy_override() {
        let err = build_discovery_http_client(Some("ftp://proxy.invalid"))
            .expect_err("invalid proxy scheme should be rejected");
        assert!(err.to_string().contains("Unsupported proxy URL scheme"));
    }

    #[test]
    fn default_sso_client_is_not_prebuilt_when_restore_session_is_available() {
        assert!(!should_prebuild_default_sso_client(
            Some(user_id!("@bob:192.168.1.58:8128")),
            false,
        ));
    }

    #[test]
    fn default_sso_client_is_not_prebuilt_during_cli_login() {
        assert!(!should_prebuild_default_sso_client(None, true));
    }

    #[test]
    fn default_sso_client_is_prebuilt_for_idle_login_screen() {
        assert!(should_prebuild_default_sso_client(None, false));
    }

    #[test]
    fn restore_session_policy_preserves_data_for_client_build_failure() {
        let err = RestoreSessionError::ClientBuild {
            user_id: user_id!("@alice:example.org").to_owned(),
            message: "homeserver returned 502".to_owned(),
        };

        assert_eq!(restore_session_failure_action(&err), RestoreSessionFailureAction::Preserve);
        assert!(restore_session_failure_message(&err).contains("try again"));
    }

    #[test]
    fn whoami_404_is_retryable_restore_validation_failure() {
        assert_eq!(
            session_validation_failure_action(false),
            RestoreSessionFailureAction::Preserve,
        );
    }

    #[test]
    fn invalid_token_restore_policy_clears_session_and_latest_user() {
        let err = RestoreSessionError::InvalidToken {
            user_id: user_id!("@alice:example.org").to_owned(),
            message: "M_UNKNOWN_TOKEN".to_owned(),
        };

        assert_eq!(
            restore_session_failure_action(&err),
            RestoreSessionFailureAction::ClearPersistedSession,
        );
        assert_eq!(
            session_validation_failure_action(true),
            RestoreSessionFailureAction::ClearPersistedSession,
        );
    }

    #[test]
    fn account_switch_restore_retryable_error_preserves_target_session() {
        let err = RestoreSessionError::RestoreAuth {
            user_id: user_id!("@alice:example.org").to_owned(),
            message: "HTTP 404".to_owned(),
        };

        assert_eq!(restore_session_failure_action(&err), RestoreSessionFailureAction::Preserve);
    }

    #[test]
    fn save_latest_user_failure_is_reported_without_session_cleanup() {
        let err = RestoreSessionError::SaveLatestUserId {
            user_id: user_id!("@alice:example.org").to_owned(),
            message: "permission denied".to_owned(),
        };

        assert_eq!(restore_session_failure_action(&err), RestoreSessionFailureAction::Preserve);
        assert!(restore_session_failure_message(&err).contains("latest user"));
    }
}
