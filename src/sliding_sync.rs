use anyhow::{anyhow, bail, Result};
use bitflags::bitflags;
use clap::Parser;
use eyeball::Subscriber;
use eyeball_im::VectorDiff;
use futures_util::{pin_mut, StreamExt};
use imbl::Vector;
use makepad_widgets::{error, log, warning, Cx, SignalToUI};
use matrix_sdk_base::crypto::{DecryptionSettings, TrustRequirement};
use matrix_sdk::{
    config::RequestConfig, encryption::EncryptionSettings, event_handler::EventHandlerDropGuard, media::MediaRequestParameters, room::{edit::EditedContent, reply::Reply, RoomMember}, ruma::{
        api::client::{profile::{AvatarUrl, DisplayName}, receipt::create_receipt::v3::ReceiptType}, events::{
            room::{
                message::RoomMessageEventContent, power_levels::RoomPowerLevels, MediaSource
            }, MessageLikeEventType, StateEventType
        }, matrix_uri::MatrixId, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId, RoomOrAliasId, UserId
    }, sliding_sync::VersionBuilder, Client, ClientBuildError, Error, OwnedServerName, Room, RoomDisplayName, RoomMemberships, RoomState, SuccessorRoom
};
use matrix_sdk_ui::{
    RoomListService, Timeline, room_list_service::{RoomListItem, RoomListLoadingState, SyncIndicator, filters}, spaces::SpaceService, sync_service::{self, SyncService}, timeline::{EventTimelineItem, LatestEventValue, RoomExt, TimelineDetails, TimelineEventItemId, TimelineItem}
};
use robius_open::Uri;
use ruma::{events::tag::Tags, OwnedRoomOrAliasId};
use tokio::{
    runtime::Handle,
    sync::{mpsc::{Sender, UnboundedReceiver, UnboundedSender}, watch, Notify}, task::JoinHandle, time::error::Elapsed,
};
use url::Url;
use std::{cmp::{max, min}, collections::{BTreeMap, BTreeSet}, future::Future, iter::Peekable, ops::{Deref, Not}, path:: Path, sync::{Arc, LazyLock, Mutex}, time::Duration};
use std::io;
use crate::{
    app::AppStateAction, app_data_dir, avatar_cache::AvatarUpdate, event_preview::text_preview_of_timeline_item, home::{
        add_room::KnockResultAction, invite_screen::{JoinRoomResultAction, LeaveRoomResultAction}, link_preview::{LinkPreviewData, LinkPreviewDataNonNumeric, LinkPreviewRateLimitResponse}, room_screen::TimelineUpdate, rooms_list::{self, InvitedRoomInfo, InviterInfo, JoinedRoomInfo, RoomsListUpdate, enqueue_rooms_list_update}, rooms_list_header::RoomsListHeaderAction, tombstone_footer::SuccessorRoomDetails
    }, login::login_screen::LoginAction, logout::{logout_confirm_modal::LogoutAction, logout_state_machine::{LogoutConfig, is_logout_in_progress, logout_with_state_machine}}, media_cache::{MediaCacheEntry, MediaCacheEntryRef}, persistence::{self, ClientSessionPersisted, load_app_state}, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{UserProfileUpdate, enqueue_user_profile_update},
    }, room::{FetchedRoomAvatar, FetchedRoomPreview, RoomPreviewAction}, shared::{
        html_or_plaintext::MatrixLinkPillState,
        jump_to_bottom_button::UnreadMessageCount,
        popup_list::{PopupItem, PopupKind, enqueue_popup_notification}
    }, space_service_sync::space_service_loop, utils::{self, AVATAR_THUMBNAIL_FORMAT, RoomNameId, avatar_from_room_name}, verification::add_verification_event_handlers_and_sync_client
};

#[derive(Parser, Debug, Default)]
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
impl From<LoginByPassword> for Cli {
    fn from(login: LoginByPassword) -> Self {
        Self {
            user_id: login.user_id,
            password: login.password,
            homeserver: login.homeserver,
            proxy: None,
            login_screen: false,
            verbose: false,
        }
    }
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

    let homeserver_url = cli.homeserver.as_deref()
        .unwrap_or("https://matrix-client.matrix.org/");
        // .unwrap_or("https://matrix.org/");

    let mut builder = Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
        // Use a sqlite database to persist the client's encryption setup.
        .sqlite_store(&db_path, Some(&passphrase))
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

    if let Some(proxy) = cli.proxy.as_ref() {
        builder = builder.proxy(proxy.clone());
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
/// Upon success, this function returns the logged-in client and an optional sync token.
async fn login(
    cli: &Cli,
    login_request: LoginRequest,
) -> Result<(Client, Option<String>)> {
    match login_request {
        LoginRequest::LoginByCli | LoginRequest::LoginByPassword(_) => {
            let cli = if let LoginRequest::LoginByPassword(login_by_password) = login_request {
                &Cli::from(login_by_password)
            } else {
                cli
            };
            let (client, client_session) = build_client(cli, app_data_dir()).await?;
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
                if let Err(e) = persistence::save_session(&client, client_session).await {
                    let err_msg = format!("Failed to save session state to storage: {e}");
                    error!("{err_msg}");
                    enqueue_popup_notification(PopupItem { message: err_msg, kind: PopupKind::Error, auto_dismissal_duration: None });
                }
                Ok((client, None))
            } else {
                let err_msg = format!("Failed to login as {}: {:?}", cli.user_id, login_result);
                enqueue_popup_notification(PopupItem { message: err_msg.clone(), kind: PopupKind::Error, auto_dismissal_duration: None });
                enqueue_rooms_list_update(RoomsListUpdate::Status { status: err_msg.clone() });
                bail!(err_msg);
            }
        }

        LoginRequest::LoginBySSOSuccess(client, client_session) => {
            if let Err(e) = persistence::save_session(&client, client_session).await {
                error!("Failed to save session state to storage: {e:?}");
            }
            Ok((client, None))
        }
        LoginRequest::HomeserverLoginTypesQuery(_) => {
            bail!("LoginRequest::HomeserverLoginTypesQuery not handled earlier");
        }
    }
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
    Request(reqwest::Error),
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
            UrlPreviewError::Request(e) => write!(f, "HTTP request failed: {}", e),
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

/// The set of requests for async work that can be made to the worker thread.
#[allow(clippy::large_enum_variant)]
pub enum MatrixRequest {
    /// Request from the login screen to log in with the given credentials.
    Login(LoginRequest),
    /// Request to logout.
    Logout{
        is_desktop: bool,
    },
    /// Request to paginate the older (or newer) events of a room's timeline.
    PaginateRoomTimeline {
        room_id: OwnedRoomId,
        /// The maximum number of timeline events to fetch in each pagination batch.
        num_events: u16,
        direction: PaginationDirection,
    },
    /// Request to edit the content of an event in the given room's timeline.
    EditMessage {
        room_id: OwnedRoomId,
        timeline_event_item_id: TimelineEventItemId,
        edited_content: EditedContent,
    },
    /// Request to fetch the full details of the given event in the given room's timeline.
    FetchDetailsForEvent {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Request to fetch profile information for all members of a room.
    /// This can be *very* slow depending on the number of members in the room.
    SyncRoomMemberList {
        room_id: OwnedRoomId,
    },
    /// Request to knock on (request an invite to) the given room.
    Knock {
        room_or_alias_id: OwnedRoomOrAliasId,
        reason: Option<String>,
        #[doc(alias("via"))]
        server_names: Vec<OwnedServerName>,
    },
    /// Request to join the given room.
    JoinRoom {
        room_id: OwnedRoomId,
    },
    /// Request to leave the given room.
    LeaveRoom {
        room_id: OwnedRoomId,
    },
    /// Request to get the actual list of members in a room.
    /// This returns the list of members that can be displayed in the UI.
    GetRoomMembers {
        room_id: OwnedRoomId,
        memberships: RoomMemberships,
        /// * If `true` (not recommended), only the local cache will be accessed.
        /// * If `false` (recommended), details will be fetched from the server.
        local_only: bool,
    },
    /// Request to fetch the preview (basic info) for the given room,
    /// either one that is joined locally or one that is unknown.
    ///
    /// Emits a [`RoomPreviewAction::Fetched`] when the fetch operation has completed.
    GetRoomPreview {
        room_or_alias_id: OwnedRoomOrAliasId,
        via: Vec<OwnedServerName>,
    },
    /// Request to fetch the full details (the room preview) of a tombstoned room.
    GetSuccessorRoomDetails {
        tombstoned_room_id: OwnedRoomId,
    },
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
        room_id: OwnedRoomId,
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
    /// Request to resolve a room alias into a room ID and the servers that know about that room.
    ResolveRoomAlias(OwnedRoomAliasId),
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
    /// Request to send a message to the given room.
    SendMessage {
        room_id: OwnedRoomId,
        message: RoomMessageEventContent,
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
    },
    /// Subscribe to typing notices for the given room.
    ///
    /// This request does not return a response or notify the UI thread.
    SubscribeToTypingNotices {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Subscribe to changes in the read receipts of our own user.
    ///
    /// This request does not return a response or notify the UI thread.
    SubscribeToOwnUserReadReceiptsChanged {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Subscribe to changes in the set of pinned events for the given room.
    SubscribeToPinnedEvents {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe.
        subscribe: bool,
    },
    /// Sends a read receipt for the given event in the given room.
    ReadReceipt {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Sends a fully-read receipt for the given event in the given room.
    FullyReadReceipt {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Sends a request to obtain the power levels for this room.
    ///
    /// The response is delivered back to the main UI thread via [`TimelineUpdate::UserPowerLevels`].
    GetRoomPowerLevels {
        room_id: OwnedRoomId,
    },
    /// Toggles the given reaction to the given event in the given room.
    ToggleReaction {
        room_id: OwnedRoomId,
        timeline_event_id: TimelineEventItemId,
        reaction: String,
    },
    /// Redacts (deletes) the given event in the given room.
    #[doc(alias("delete"))]
    RedactMessage {
        room_id: OwnedRoomId,
        timeline_event_id: TimelineEventItemId,
        reason: Option<String>,
    },
    /// Pin or unpin the given event in the given room.
    #[doc(alias("unpin"))]
    PinEvent {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
        pin: bool,
    },
    /// Sends a request to obtain the room's pill link info for the given Matrix ID.
    ///
    /// The MatrixLinkPillInfo::Loaded variant is sent back to the main UI thread via.
    GetMatrixRoomLinkPillInfo {
        matrix_id: MatrixId,
        via: Vec<OwnedServerName>
    },
    /// Request to fetch URL preview from the Matrix homeserver.
    GetUrlPreview {
        url: String,
        on_fetched: OnLinkPreviewFetchedFn,
        destination: Arc<Mutex<crate::home::link_preview::TimestampedCacheEntry>>,
        update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    },
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    if let Some(sender) = REQUEST_SENDER.lock().unwrap().as_ref() {
        sender.send(req)
            .expect("BUG: matrix worker task receiver has died!");
    }
}

/// Details of a login request that get submitted within [`MatrixRequest::Login`].
pub enum LoginRequest{
    LoginByPassword(LoginByPassword),
    LoginBySSOSuccess(Client, ClientSessionPersisted),
    LoginByCli,
    HomeserverLoginTypesQuery(String),

}
/// Information needed to log in to a Matrix homeserver.
pub struct LoginByPassword {
    pub user_id: String,
    pub password: String,
    pub homeserver: Option<String>,
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
    let mut subscribers_own_user_read_receipts: BTreeMap<OwnedRoomId, JoinHandle<()>> = BTreeMap::new();
    let mut subscribers_pinned_events: BTreeMap<OwnedRoomId, JoinHandle<()>> = BTreeMap::new();

    while let Some(request) = request_receiver.recv().await {
        match request {
            MatrixRequest::Login(login_request) => {
                if let Err(e) = login_sender.send(login_request).await {
                    error!("Error sending login request to login_sender: {e:?}");
                    Cx::post_action(LoginAction::LoginFailure(String::from(
                        "BUG: failed to send login request to login worker task."
                    )));
                }
            }

            MatrixRequest::Logout { is_desktop } => {
                log!("Received MatrixRequest::Logout, is_desktop={}", is_desktop);
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

            MatrixRequest::PaginateRoomTimeline { room_id, num_events, direction } => {
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        log!("Skipping pagination request for not-yet-known room {room_id}");
                        continue;
                    };

                    let timeline_ref = room_info.timeline.clone();
                    let sender = room_info.timeline_update_sender.clone();
                    (timeline_ref, sender)
                };

                // Spawn a new async task that will make the actual pagination request.
                let _paginate_task = Handle::current().spawn(async move {
                    log!("Starting {direction} pagination request for room {room_id}...");
                    sender.send(TimelineUpdate::PaginationRunning(direction)).unwrap();
                    SignalToUI::set_ui_signal();

                    let res = if direction == PaginationDirection::Forwards {
                        timeline.paginate_forwards(num_events).await
                    } else {
                        timeline.paginate_backwards(num_events).await
                    };

                    match res {
                        Ok(fully_paginated) => {
                            log!("Completed {direction} pagination request for room {room_id}, hit {} of timeline? {}",
                                if direction == PaginationDirection::Forwards { "end" } else { "start" },
                                if fully_paginated { "yes" } else { "no" },
                            );
                            sender.send(TimelineUpdate::PaginationIdle {
                                fully_paginated,
                                direction,
                            }).unwrap();
                            SignalToUI::set_ui_signal();
                        }
                        Err(error) => {
                            error!("Error sending {direction} pagination request for room {room_id}: {error:?}");
                            sender.send(TimelineUpdate::PaginationError {
                                error,
                                direction,
                            }).unwrap();
                            SignalToUI::set_ui_signal();
                        }
                    }
                });
            }

            MatrixRequest::EditMessage { room_id, timeline_event_item_id: timeline_event_id, edited_content } => {
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        error!("BUG: room info not found for edit request, room {room_id}");
                        continue;
                    };
                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                // Spawn a new async task that will make the actual edit request.
                let _edit_task = Handle::current().spawn(async move {
                    log!("Sending request to edit message {timeline_event_id:?} in room {room_id}...");
                    let result = timeline.edit(&timeline_event_id, edited_content).await;
                    match result {
                        Ok(_) => log!("Successfully edited message {timeline_event_id:?} in room {room_id}."),
                        Err(ref e) => error!("Error editing message {timeline_event_id:?} in room {room_id}: {e:?}"),
                    }
                    sender.send(TimelineUpdate::MessageEdited {
                        timeline_event_id,
                        result,
                    }).unwrap();
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::FetchDetailsForEvent { room_id, event_id } => {
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        error!("BUG: room info not found for fetch details for event request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                // Spawn a new async task that will make the actual fetch request.
                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending request to fetch details for event {event_id} in room {room_id}...");
                    let result = timeline.fetch_details_for_event(&event_id).await;
                    match result {
                        Ok(_) => {
                            // log!("Successfully fetched details for event {event_id} in room {room_id}.");
                        }
                        Err(ref _e) => {
                            // error!("Error fetching details for event {event_id} in room {room_id}: {e:?}");
                        }
                    }
                    sender.send(TimelineUpdate::EventDetailsFetched {
                        event_id,
                        result,
                    }).unwrap();
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::SyncRoomMemberList { room_id } => {
                let (timeline, sender) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        error!("BUG: room info not found for fetch members request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                // Spawn a new async task that will make the actual fetch request.
                let _fetch_task = Handle::current().spawn(async move {
                    log!("Sending sync room members request for room {room_id}...");
                    timeline.fetch_members().await;
                    log!("Completed sync room members request for room {room_id}.");
                    sender.send(TimelineUpdate::RoomMembersSynced).unwrap();
                    SignalToUI::set_ui_signal();
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

            MatrixRequest::JoinRoom { room_id } => {
                let Some(client) = get_client() else { continue };
                let _join_room_task = Handle::current().spawn(async move {
                    log!("Sending request to join room {room_id}...");
                    let result_action = if let Some(room) = client.get_room(&room_id) {
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
                            error: matrix_sdk::Error::UnknownError(
                                String::from("Client couldn't locate room to leave it.").into()
                            ),
                        }
                    };
                    Cx::post_action(result_action);
                });
            }

            MatrixRequest::GetRoomMembers { room_id, memberships, local_only } => {
                let (timeline, sender) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for get room members request {room_id}");
                        continue;
                    };
                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                let _get_members_task = Handle::current().spawn(async move {
                    let room = timeline.room();

                    let send_update = |members: Vec<matrix_sdk::room::RoomMember>, source: &str| {
                        log!("{} {} members for room {}", source, members.len(), room_id);
                        sender.send(TimelineUpdate::RoomMembersListFetched {
                            members
                        }).unwrap();
                        SignalToUI::set_ui_signal();
                    };

                    if local_only {
                        if let Ok(members) = room.members_no_sync(memberships).await {
                            send_update(members, "Got");
                        }
                    } else {
                        if let Ok(members) = room.members(memberships).await {
                            send_update(members, "Successfully fetched");
                        }
                    }
                });
            }

            MatrixRequest::GetRoomPreview { room_or_alias_id, via } => {
                let Some(client) = get_client() else { continue };
                let _fetch_task = Handle::current().spawn(async move {
                    log!("Sending get room preview request for {room_or_alias_id}...");
                    let res = fetch_room_preview_with_avatar(&client, &room_or_alias_id, via).await;
                    Cx::post_action(RoomPreviewAction::Fetched(res));
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
                    (room_info.timeline_update_sender.clone(), room_info.timeline.room().successor_room())
                };
                spawn_fetch_successor_room_preview(
                    client,
                    successor_room,
                    tombstoned_room_id,
                    sender,
                );
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
                                log!("User profile request: user {user_id} was not a member of room {room_id}");
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
            MatrixRequest::GetNumberUnreadMessages { room_id } => {
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        log!("Skipping get number of unread messages request for not-yet-known room {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };
                let _get_unreads_task = Handle::current().spawn(async move {
                    match sender.send(TimelineUpdate::NewUnreadMessagesCount(
                        UnreadMessageCount::Known(timeline.room().num_unread_messages())
                    )) {
                        Ok(_) => SignalToUI::set_ui_signal(),
                        Err(e) => log!("Failed to send timeline update: {e:?} for GetNumberUnreadMessages request for room {room_id}"),
                    }
                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                        room_id: room_id.clone(),
                        unread_messages: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                        unread_mentions: timeline.room().num_unread_mentions(),
                    });
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
                    submit_async_request(MatrixRequest::PaginateRoomTimeline {
                        room_id,
                        num_events: 50,
                        direction: PaginationDirection::Backwards,
                    });
                });
            }

            MatrixRequest::SendTypingNotice { room_id, typing } => {
                let Some(room) = get_client().and_then(|c| c.get_room(&room_id)) else {
                    error!("BUG: client/room not found for typing notice request {room_id}");
                    continue;
                };
                let _typing_task = Handle::current().spawn(async move {
                    if let Err(e) = room.typing_notice(typing).await {
                        error!("Failed to send typing notice to room {room_id}: {e:?}");
                    }
                });
            }

            MatrixRequest::SubscribeToTypingNotices { room_id, subscribe } => {
                let (room, timeline_update_sender, mut typing_notice_receiver) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        log!("BUG: room info not found for subscribe to typing notices request, room {room_id}");
                        continue;
                    };
                    let (room, recv) = if subscribe {
                        if room_info.typing_notice_subscriber.is_some() {
                            warning!("Note: room {room_id} is already subscribed to typing notices.");
                            continue;
                        } else {
                            let Some(room) = get_client().and_then(|c| c.get_room(&room_id)) else {
                                error!("BUG: client/room not found when subscribing to typing notices request, room: {room_id}");
                                continue;
                            };
                            let (drop_guard, recv) = room.subscribe_to_typing_notifications();
                            room_info.typing_notice_subscriber = Some(drop_guard);
                            (room, recv)
                        }
                    } else {
                        room_info.typing_notice_subscriber.take();
                        continue;
                    };
                    // Here: we don't have an existing subscriber running, so we fall through and start one.
                    (room, room_info.timeline_update_sender.clone(), recv)
                };

                let _typing_notices_task = Handle::current().spawn(async move {
                    while let Ok(user_ids) = typing_notice_receiver.recv().await {
                        // log!("Received typing notifications for room {room_id}: {user_ids:?}");
                        let mut users = Vec::with_capacity(user_ids.len());
                        for user_id in user_ids {
                            users.push(
                                room.get_member_no_sync(&user_id)
                                    .await
                                    .ok()
                                    .flatten()
                                    .and_then(|m| m.display_name().map(|d| d.to_owned()))
                                    .unwrap_or_else(|| user_id.to_string())
                            );
                        }
                        if let Err(e) = timeline_update_sender.send(TimelineUpdate::TypingUsers { users }) {
                            error!("Error: timeline update sender couldn't send the list of typing users: {e:?}");
                        }
                        SignalToUI::set_ui_signal();
                    }
                    // log!("Note: typing notifications recv loop has ended for room {}", room_id);
                });
            }
            MatrixRequest::SubscribeToOwnUserReadReceiptsChanged { room_id, subscribe } => {
                if !subscribe {
                    if let Some(task_handler) = subscribers_own_user_read_receipts.remove(&room_id) {
                        task_handler.abort();
                    }
                    continue;
                }
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        log!("BUG: room info not found for subscribe to own user read receipts changed request, room {room_id}");
                        continue;
                    };
                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };
                let room_id_clone = room_id.clone();
                let subscribe_own_read_receipt_task = Handle::current().spawn(async move {
                    let update_receiver = timeline.subscribe_own_user_read_receipts_changed().await;
                    pin_mut!(update_receiver);
                    if let Some(client_user_id) = current_user_id() {
                        if let Some((event_id, receipt)) = timeline.latest_user_read_receipt(&client_user_id).await {
                            log!("Received own user read receipt for room {room_id_clone}: {receipt:?}, event ID: {event_id:?}");
                            if let Err(e) = sender.send(TimelineUpdate::OwnUserReadReceipt(receipt)) {
                                error!("Failed to get own user read receipt: {e:?}");
                            }
                        }

                        while (update_receiver.next().await).is_some() {
                            if let Some((_, receipt)) = timeline.latest_user_read_receipt(&client_user_id).await {
                                if let Err(e) = sender.send(TimelineUpdate::OwnUserReadReceipt(receipt)) {
                                    error!("Failed to send own user read receipt: {e:?}");
                                }
                                // When read receipts change (from other devices), update unread count
                                let unread_count = timeline.room().num_unread_messages();
                                let unread_mentions = timeline.room().num_unread_mentions();
                                // Send updated unread count to the UI
                                if let Err(e) = sender.send(TimelineUpdate::NewUnreadMessagesCount(
                                    UnreadMessageCount::Known(unread_count)
                                )) {
                                    error!("Failed to send unread message count update: {e:?}");
                                }
                                // Update the rooms list with new unread counts
                                enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                                    room_id: room_id_clone.clone(),
                                    unread_messages: UnreadMessageCount::Known(unread_count),
                                    unread_mentions,
                                });
                            }
                        }
                    }
                });
                subscribers_own_user_read_receipts.insert(room_id, subscribe_own_read_receipt_task);
            }
            MatrixRequest::SubscribeToPinnedEvents { room_id, subscribe } => {
                if !subscribe {
                    if let Some(task_handler) = subscribers_pinned_events.remove(&room_id) {
                        task_handler.abort();
                    }
                    continue;
                }
                let (timeline, sender) = {
                    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get_mut(&room_id) else {
                        log!("BUG: room info not found for subscribe to pinned events request, room {room_id}");
                        continue;
                    };
                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };
                let subscribe_pinned_events_task = Handle::current().spawn(async move {
                    // Send an initial update, as the stream may not update immediately.
                    let pinned_events = timeline.room().pinned_event_ids().unwrap_or_default();
                    match sender.send(TimelineUpdate::PinnedEvents(pinned_events)) {
                        Ok(()) => SignalToUI::set_ui_signal(),
                        Err(e) => log!("Failed to send initial pinned events update: {e:?}"),
                    }
                    let update_receiver = timeline.room().pinned_event_ids_stream();
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
            MatrixRequest::SpawnSSOServer { brand, homeserver_url, identity_provider_id} => {
                spawn_sso_server(brand, homeserver_url, identity_provider_id, login_sender.clone()).await;
            }
            MatrixRequest::ResolveRoomAlias(room_alias) => {
                let Some(client) = get_client() else { continue };
                let _resolve_task = Handle::current().spawn(async move {
                    log!("Sending resolve room alias request for {room_alias}...");
                    let res = client.resolve_room_alias(&room_alias).await;
                    log!("Resolved room alias {room_alias} to: {res:?}");
                    todo!("Send the resolved room alias back to the UI thread somehow.");
                });
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

            MatrixRequest::FetchMedia { media_request, on_fetched, destination, update_sender } => {
                let Some(client) = get_client() else { continue };
                let media = client.media();

                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending fetch media request for {media_request:?}...");
                    let res = media.get_media_content(&media_request, true).await;
                    on_fetched(&destination, media_request, res, update_sender);
                });
            }

            MatrixRequest::SendMessage {
                room_id,
                message,
                replied_to,
                #[cfg(feature = "tsp")]
                sign_with_tsp,
            } => {
                let timeline = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for send message request {room_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };

                // Spawn a new async task that will send the actual message.
                let _send_message_task = Handle::current().spawn(async move {
                    log!("Sending message to room {room_id}: {message:?}...");
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
                                                enqueue_popup_notification(PopupItem {
                                                    message: format!("Failed to sign message with TSP: {e}"),
                                                    kind: PopupKind::Error,
                                                    auto_dismissal_duration: None
                                                });
                                                return;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to serialize message to bytes for TSP signing: {e:?}");
                                        enqueue_popup_notification(PopupItem {
                                            message: format!("Failed to serialize message for TSP signing: {e}"),
                                            kind: PopupKind::Error,
                                            auto_dismissal_duration: None
                                        });
                                        return;
                                    }
                                }
                            }
                            message
                        }
                    };

                    if let Some(replied_to_info) = replied_to {
                        match timeline.send_reply(message.into(), replied_to_info.event_id).await {
                            Ok(_send_handle) => log!("Sent reply message to room {room_id}."),
                            Err(_e) => {
                                error!("Failed to send reply message to room {room_id}: {_e:?}");
                                enqueue_popup_notification(PopupItem { message: format!("Failed to send reply: {_e}"), kind: PopupKind::Error, auto_dismissal_duration: None });
                            }
                        }
                    } else {
                        match timeline.send(message.into()).await {
                            Ok(_send_handle) => log!("Sent message to room {room_id}."),
                            Err(_e) => {
                                error!("Failed to send message to room {room_id}: {_e:?}");
                                enqueue_popup_notification(PopupItem { message: format!("Failed to send message: {_e}"), kind: PopupKind::Error, auto_dismissal_duration: None });
                            }
                        }
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::ReadReceipt { room_id, event_id } => {
                let timeline = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found when sending read receipt, room {room_id}, {event_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };
                let _send_rr_task = Handle::current().spawn(async move {
                    match timeline.send_single_receipt(ReceiptType::Read, event_id.clone()).await {
                        Ok(sent) => log!("{} read receipt to room {room_id} for event {event_id}", if sent { "Sent" } else { "Already sent" }),
                        Err(_e) => error!("Failed to send read receipt to room {room_id} for event {event_id}; error: {_e:?}"),
                    }
                    // Also update the number of unread messages in the room.
                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                        room_id: room_id.clone(),
                        unread_messages: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                        unread_mentions: timeline.room().num_unread_mentions()
                    });
                });
            },

            MatrixRequest::FullyReadReceipt { room_id, event_id, .. } => {
                let timeline = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found when sending fully read receipt, room {room_id}, {event_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };
                let _send_frr_task = Handle::current().spawn(async move {
                    match timeline.send_single_receipt(ReceiptType::FullyRead, event_id.clone()).await {
                        Ok(sent) => log!("{} fully read receipt to room {room_id} for event {event_id}",
                            if sent { "Sent" } else { "Already sent" }
                        ),
                        Err(_e) => error!("Failed to send fully read receipt to room {room_id} for event {event_id}; error: {_e:?}"),
                    }
                    // Also update the number of unread messages in the room.
                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                        room_id: room_id.clone(),
                        unread_messages: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                        unread_mentions: timeline.room().num_unread_mentions()
                    });
                });
            },

            MatrixRequest::GetRoomPowerLevels { room_id } => {
                let (timeline, sender) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for get room power levels request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                let Some(user_id) = current_user_id() else { continue };

                let _power_levels_task = Handle::current().spawn(async move {
                    match timeline.room().power_levels().await {
                        Ok(power_levels) => {
                            log!("Successfully fetched power levels for room {room_id}.");
                            if let Err(e) = sender.send(TimelineUpdate::UserPowerLevels(
                                UserPowerLevels::from(&power_levels, &user_id),
                            )) {
                                error!("Failed to send the result of if user can send message: {e}")
                            }
                            SignalToUI::set_ui_signal();
                        }
                        Err(e) => {
                            error!("Failed to fetch power levels for room {room_id}: {e:?}");
                        }
                    }
                });
            },
            MatrixRequest::ToggleReaction { room_id, timeline_event_id, reaction } => {
                let timeline = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for send toggle reaction {room_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };

                let _toggle_reaction_task = Handle::current().spawn(async move {
                    log!("Toggle Reaction to room {room_id}: ...");
                    match timeline.toggle_reaction(&timeline_event_id, &reaction).await {
                        Ok(_send_handle) => {
                            SignalToUI::set_ui_signal();
                            log!("Sent toggle reaction to room {room_id} {reaction}.")
                        },
                        Err(_e) => error!("Failed to send toggle reaction to room {room_id} {reaction}; error: {_e:?}"),
                    }
                });

            },
            MatrixRequest::RedactMessage { room_id, timeline_event_id, reason } => {
                let timeline = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for redact message {room_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };

                let _redact_task = Handle::current().spawn(async move {
                    match timeline.redact(&timeline_event_id, reason.as_deref()).await {
                        Ok(()) => log!("Successfully redacted message in room {room_id}."),
                        Err(e) => {
                            error!("Failed to redact message in {room_id}; error: {e:?}");
                            enqueue_popup_notification(PopupItem { message: format!("Failed to redact message. Error: {e}"), kind: PopupKind::Error, auto_dismissal_duration: None });
                        }
                    }
                });
            },
            MatrixRequest::PinEvent { room_id, event_id, pin } => {
                let (timeline, sender) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for pin message {room_id}");
                        continue;
                    };
                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                let _pin_task = Handle::current().spawn(async move {
                    let result = if pin {
                        timeline.pin_event(&event_id).await
                    } else {
                        timeline.unpin_event(&event_id).await
                    };
                    match sender.send(TimelineUpdate::PinResult { event_id, pin, result }) {
                        Ok(_) => SignalToUI::set_ui_signal(),
                        Err(e) => log!("Failed to send timeline update for pin event: {e:?}"),
                    }
                });
            }
            MatrixRequest::GetMatrixRoomLinkPillInfo { matrix_id, via } => {
                let Some(client) = get_client() else { continue };
                let _fetch_matrix_link_pill_info_task = Handle::current().spawn(async move {
                    let room_or_alias_id: Option<&RoomOrAliasId> = match &matrix_id {
                        MatrixId::Room(room_id) => Some((&**room_id).into()),
                        MatrixId::RoomAlias(room_alias_id) => Some((&**room_alias_id).into()),
                        MatrixId::Event(room_or_alias_id, _event_id) => Some(room_or_alias_id),
                        _ => {
                            log!("MatrixLinkRoomPillInfoRequest: Unsupported MatrixId type: {matrix_id:?}");
                            return;
                        }
                    };
                    if let Some(room_or_alias_id) = room_or_alias_id {
                        match client.get_room_preview(room_or_alias_id, via).await {
                            Ok(preview) => Cx::post_action(MatrixLinkPillState::Loaded {
                                matrix_id: matrix_id.clone(),
                                name: preview.name.unwrap_or_else(|| room_or_alias_id.to_string()),
                                avatar_url: preview.avatar_url
                            }),
                            Err(_e) => {
                                log!("Failed to get room link pill info for {room_or_alias_id:?}: {_e:?}");
                            }
                        };
                    }
                });
            }
            MatrixRequest::GetUrlPreview { url, on_fetched, destination, update_sender,} => {
                // const MAX_LOG_RESPONSE_BODY_LENGTH: usize = 1000;
                // log!("Starting URL preview fetch for: {}", url);
                let _fetch_url_preview_task = Handle::current().spawn(async move {
                    let result: Result<LinkPreviewData, UrlPreviewError> = async {
                        // log!("Getting Matrix client for URL preview: {}", url);
                        let client = get_client().ok_or_else(|| {
                            // error!("Matrix client not available for URL preview: {}", url);
                            UrlPreviewError::ClientNotAvailable
                        })?;
                        
                        let token = client.access_token().ok_or_else(|| {
                            // error!("Access token not available for URL preview: {}", url);
                            UrlPreviewError::AccessTokenNotAvailable
                        })?;
                        // Official Doc: https://spec.matrix.org/v1.11/client-server-api/#get_matrixclientv1mediapreview_url
                        // Element desktop is using /_matrix/media/v3/preview_url
                        let endpoint_url = client.homeserver().join("/_matrix/client/v1/media/preview_url")
                            .map_err(UrlPreviewError::UrlParse)?;
                        // log!("Fetching URL preview from endpoint: {} for URL: {}", endpoint_url, url);
                        
                        let response = client
                            .http_client()
                            .get(endpoint_url.clone())
                            .bearer_auth(token)
                            .query(&[("url", url.as_str())])
                            .header("Content-Type", "application/json")
                            .send()
                            .await
                            .map_err(|e| {
                                // error!("HTTP request failed for URL preview {}: {}", url, e);
                                UrlPreviewError::Request(e)
                            })?;
                        
                        let status = response.status();
                        // log!("URL preview response status for {}: {}", url, status);
                        
                        if !status.is_success() && status.as_u16() != 429 {
                            // error!("URL preview request failed with status {} for URL: {}", status, url);
                            return Err(UrlPreviewError::HttpStatus(status.as_u16()));
                        }
                        
                        let text = response.text().await.map_err(|e| {
                            // error!("Failed to read response text for URL preview {}: {}", url, e);
                            UrlPreviewError::Request(e)
                        })?;
                        
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
                    //         log!("Successfully fetched URL preview for {}: title={:?}, site_name={:?}", 
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

    error!("matrix_worker_task task ended unexpectedly");
    bail!("matrix_worker_task task ended unexpectedly")
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
static DEFAULT_SSO_CLIENT_NOTIFIER: LazyLock<Arc<Notify>> = LazyLock::new(
    || Arc::new(Notify::new())
);

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
    // Create a Tokio runtime, and save it in a static variable to ensure it isn't dropped.
    let rt_handle = TOKIO_RUNTIME.lock().unwrap().get_or_insert_with(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
    }).handle().clone();

    // Proactively build a Matrix Client in the background so that the SSO Server
    // can have a quicker start if needed (as it's rather slow to build this client).
    rt_handle.spawn(async move {
        match build_client(&Cli::default(), app_data_dir()).await {
            Ok(client_and_session) => {
                DEFAULT_SSO_CLIENT.lock().unwrap()
                    .get_or_insert(client_and_session);
            }
            Err(e) => error!("Error: could not create DEFAULT_SSO_CLIENT object: {e}"),
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

/// Backend-specific details about a joined room that our client currently knows about.
struct JoinedRoomDetails {
    #[allow(unused)]
    room_id: OwnedRoomId,
    /// A reference to this room's timeline of events.
    timeline: Arc<Timeline>,
    /// An instance of the clone-able sender that can be used to send updates to this room's timeline.
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    /// A tuple of two separate channel endpoints that can only be taken *once* by the main UI thread.
    ///
    /// 1. The single receiver that can receive updates to this room's timeline.
    ///    * When a new room is joined, an unbounded crossbeam channel will be created
    ///      and its sender given to a background task (the `timeline_subscriber_handler()`)
    ///      that enqueues timeline updates as it receives timeline vector diffs from the server.
    ///    * The UI thread can take ownership of this update receiver in order to receive updates
    ///      to this room's timeline, but only one receiver can exist at a time.
    /// 2. The sender that can send requests to the background timeline subscriber handler,
    ///    e.g., to watch for a specific event to be prepended to the timeline (via back pagination).
    timeline_singleton_endpoints: Option<(
        crossbeam_channel::Receiver<TimelineUpdate>,
        TimelineRequestSender,
    )>,
    /// The async task that listens for timeline updates for this room and sends them to the UI thread.
    timeline_subscriber_handler_task: JoinHandle<()>,
    /// A drop guard for the event handler that represents a subscription to typing notices for this room.
    typing_notice_subscriber: Option<EventHandlerDropGuard>,
    /// A drop guard for the event handler that represents a subscription to pinned events for this room.
    pinned_events_subscriber: Option<EventHandlerDropGuard>,
}
impl Drop for JoinedRoomDetails {
    fn drop(&mut self) {
        log!("Dropping JoinedRoomDetails for room {}", self.room_id);
        self.timeline_subscriber_handler_task.abort();
        drop(self.typing_notice_subscriber.take());
        drop(self.pinned_events_subscriber.take());
    }
}


/// Information about all joined rooms that our client currently know about.
static ALL_JOINED_ROOMS: Mutex<BTreeMap<OwnedRoomId, JoinedRoomDetails>> = Mutex::new(BTreeMap::new());

/// The logged-in Matrix client, which can be freely and cheaply cloned.
static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

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


/// Get a reference to the current sync service, if available.
pub fn get_sync_service() -> Option<Arc<SyncService>> {
    SYNC_SERVICE.lock().ok()?.as_ref().cloned()
}

/// The list of users that the current user has chosen to ignore.
/// Ideally we shouldn't have to maintain this list ourselves,
/// but the Matrix SDK doesn't currently properly maintain the list of ignored users.
static IGNORED_USERS: Mutex<BTreeSet<OwnedUserId>> = Mutex::new(BTreeSet::new());

/// Returns a deep clone of the current list of ignored users.
pub fn get_ignored_users() -> BTreeSet<OwnedUserId> {
    IGNORED_USERS.lock().unwrap().clone()
}

/// Returns whether the given user ID is currently being ignored.
pub fn is_user_ignored(user_id: &UserId) -> bool {
    IGNORED_USERS.lock().unwrap().contains(user_id)
}


/// Returns three channel endpoints related to the timeline for the given joined room.
///
/// 1. A timeline update sender.
/// 2. The timeline update receiver, which is a singleton, and can only be taken once.
/// 3. A `tokio::watch` sender that can be used to send requests to the timeline subscriber handler.
///
/// This will only succeed once per room, as only a single channel receiver can exist.
pub fn take_timeline_endpoints(
    room_id: &OwnedRoomId,
) -> Option<TimelineEndpoints> {
    let mut all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
    all_joined_rooms
        .get_mut(room_id)
        .and_then(|jrd| jrd.timeline_singleton_endpoints.take()
            .map(|(update_receiver, request_sender)| (
                jrd.timeline_update_sender.clone(),
                update_receiver,
                request_sender,
                jrd.timeline.room().successor_room(),
            ))
        )
        .map(|(update_sender, update_receiver, request_sender, successor_room)| {
            TimelineEndpoints {
                update_sender,
                update_receiver,
                request_sender,
                successor_room,
            }
        })
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
    is_tombstoned: bool,
    tags: Option<Tags>,
    user_power_levels: Option<UserPowerLevels>,
    // latest_event_timestamp: Option<MilliSecondsSinceUnixEpoch>,
    num_unread_messages: u64,
    num_unread_mentions: u64,
    display_name: Option<RoomDisplayName>,
    room_avatar: Option<OwnedMxcUri>,
    room: matrix_sdk::Room,
}
impl RoomListServiceRoomInfo {
    async fn from_room(room: matrix_sdk::Room) -> Self {
        Self {
            room_id: room.room_id().to_owned(),
            state: room.state(),
            is_direct: room.is_direct().await.unwrap_or(false),
            is_tombstoned: room.is_tombstoned(),
            tags: room.tags().await.ok().flatten(),
            user_power_levels: if let Some(user_id) = current_user_id() {
                UserPowerLevels::from_room(&room, &user_id).await
            } else {
                None
            },
            // latest_event_timestamp: room.new_latest_event_timestamp(),
            num_unread_messages: room.num_unread_messages(),
            num_unread_mentions: room.num_unread_mentions(),
            display_name: room.display_name().await.ok(),
            room_avatar: room.avatar_url(),
            room,
        }
    }
    async fn from_room_ref(room: &matrix_sdk::Room) -> Self {
        Self::from_room(room.clone()).await
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

    let most_recent_user_id = persistence::most_recent_user_id();
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
        match persistence::restore_session(specified_username).await {
            Ok(session) => Some(session),
            Err(e) => {
                let status_err = "Could not restore previous user session.\n\nPlease login again.";
                log!("{status_err} Error: {e:?}");
                Cx::post_action(LoginAction::LoginFailure(status_err.to_string()));

                if let Ok(cli) = &cli_parse_result {
                    log!("Attempting auto-login from CLI arguments as user '{}'...", cli.user_id);
                    Cx::post_action(LoginAction::CliAutoLogin {
                        user_id: cli.user_id.clone(),
                        homeserver: cli.homeserver.clone(),
                    });
                    match login(cli, LoginRequest::LoginByCli).await {
                        Ok(new_login) => Some(new_login),
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
        None
    };
    let cli: Cli = cli_parse_result.unwrap_or(Cli::default());
    let (client, _sync_token) = match new_login_opt {
        Some(new_login) => new_login,
        None => {
            loop {
                log!("Waiting for login request...");
                match login_receiver.recv().await {
                    Some(login_request) => {
                        match login(&cli, login_request).await {
                            Ok((client, sync_token)) => {
                                break (client, sync_token);
                            }
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

    Cx::post_action(LoginAction::LoginSuccess);

    // Deallocate the default SSO client after a successful login.
    if let Ok(mut client_opt) = DEFAULT_SSO_CLIENT.lock() {
        let _ = client_opt.take();
    }

    let logged_in_user_id = client.user_id()
        .expect("BUG: client.user_id() returned None after successful login!");
    let status = format!("Logged in as {}.\n → Loading rooms...", logged_in_user_id);
    enqueue_rooms_list_update(RoomsListUpdate::Status { status });

    // Store this active client in our global Client state so that other tasks can access it.
    if let Some(_existing) = CLIENT.lock().unwrap().replace(client.clone()) {
        error!("BUG: unexpectedly replaced an existing client when initializing the matrix client.");
    }

    // Listen for changes to our verification status and incoming verification requests.
    add_verification_event_handlers_and_sync_client(client.clone());

    // Listen for updates to the ignored user list.
    handle_ignore_user_list_subscriber(client.clone());

    let sync_service = match SyncService::builder(client.clone())
        .with_offline_mode()
        .build()
        .await
    {
        Ok(ss) => ss,
        Err(e) => {
            error!("BUG: failed to create SyncService: {e:?}");
            let err = format!("Please restart Robrix.\n\nFailed to create Matrix sync service: {e}.");
            enqueue_popup_notification(PopupItem {
                message: err.clone(),
                auto_dismissal_duration: None,
                kind: PopupKind::Error,
            });
            enqueue_rooms_list_update(RoomsListUpdate::Status { status: err });
            return;
        }
    };

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
    let mut space_service_task = rt.spawn(space_service_loop(SpaceService::new(client.clone()), client));

    // Now, this task becomes an infinite loop that monitors the state of the
    // three core matrix-related background tasks that we just spawned above.
    #[allow(clippy::never_loop)] // unsure if needed, just following tokio's examples.
    loop {
        tokio::select! {
            result = &mut matrix_worker_task_handle => {
                match result {
                    Ok(Ok(())) => {
                        // Check if this is due to logout
                        if is_logout_in_progress() {
                            log!("matrix worker task ended due to logout");
                        } else {
                            error!("BUG: matrix worker task ended unexpectedly!");
                        }
                    }
                    Ok(Err(e)) => {
                        // Check if this is due to logout
                        if is_logout_in_progress() {
                            log!("matrix worker task ended with error due to logout: {e:?}");
                        } else {
                            error!("Error: matrix worker task ended:\n\t{e:?}");
                            rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                status: e.to_string(),
                            });
                            enqueue_popup_notification(PopupItem {
                                message: format!("Rooms list update error: {e}"),
                                kind: PopupKind::Error,
                                auto_dismissal_duration: None,
                            });
                        }
                    },
                    Err(e) => {
                        error!("BUG: failed to join matrix worker task: {e:?}");
                    }
                }
                break;
            }
            result = &mut room_list_service_task => {
                match result {
                    Ok(Ok(())) => {
                        error!("BUG: room list service loop task ended unexpectedly!");
                    }
                    Ok(Err(e)) => {
                        error!("Error: room list service loop task ended:\n\t{e:?}");
                        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                            status: e.to_string(),
                        });
                        enqueue_popup_notification(PopupItem {
                            message: format!("Room list service  error: {e}"),
                            kind: PopupKind::Error,
                            auto_dismissal_duration: None,
                        });
                    },
                    Err(e) => {
                        error!("BUG: failed to join room list service loop task: {e:?}");
                    }
                }
                break;
            }
            result = &mut space_service_task => {
                match result {
                    Ok(Ok(())) => {
                        error!("BUG: space service loop task ended unexpectedly!");
                    }
                    Ok(Err(e)) => {
                        error!("Error: space service loop task ended:\n\t{e:?}");
                        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                            status: e.to_string(),
                        });
                        enqueue_popup_notification(PopupItem {
                            message: format!("Space service error: {e}"),
                            kind: PopupKind::Error,
                            auto_dismissal_duration: None,
                        });
                    },
                    Err(e) => {
                        error!("BUG: failed to join space service loop task: {e:?}");
                    }
                }
                break;
            }
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

    pin_mut!(room_diff_stream);
    while let Some(batch) = room_diff_stream.next().await {
        let mut peekable_diffs = batch.into_iter().peekable();
        while let Some(diff) = peekable_diffs.next() {
            match diff {
                VectorDiff::Append { values: new_rooms } => {
                    let _num_new_rooms = new_rooms.len();
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Append {_num_new_rooms}"); }
                    for new_room in new_rooms {
                        let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner()).await;
                        add_new_room(&new_room, &room_list_service).await?;
                        all_known_rooms.push_back(new_room);
                    }
                }
                VectorDiff::Clear => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Clear"); }
                    all_known_rooms.clear();
                    ALL_JOINED_ROOMS.lock().unwrap().clear();
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                }
                VectorDiff::PushFront { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushFront"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner()).await;
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.push_front(new_room);
                }
                VectorDiff::PushBack { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushBack"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner()).await;
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.push_back(new_room);
                }
                remove_diff @ VectorDiff::PopFront => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopFront"); }
                    if let Some(room) = all_known_rooms.pop_front() {
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                        ).await?;
                    }
                }
                remove_diff @ VectorDiff::PopBack => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopBack"); }
                    if let Some(room) = all_known_rooms.pop_back() {
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                        ).await?;
                    }
                }
                VectorDiff::Insert { index, value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Insert at {index}"); }
                    let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner()).await;
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.insert(index, new_room);
                }
                VectorDiff::Set { index, value: changed_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Set at {index}"); }
                    let changed_room = RoomListServiceRoomInfo::from_room(changed_room.into_inner()).await;
                    if let Some(old_room) = all_known_rooms.get(index) {
                        update_room(old_room, &changed_room, &room_list_service).await?;
                    } else {
                        error!("BUG: room list diff: Set index {index} was out of bounds.");
                    }
                    all_known_rooms.set(index, changed_room);
                }
                remove_diff @ VectorDiff::Remove { index: remove_index } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Remove at {remove_index}"); }
                    if remove_index < all_known_rooms.len() {
                        let room = all_known_rooms.remove(remove_index);
                        optimize_remove_then_add_into_update(
                            remove_diff,
                            &room,
                            &mut peekable_diffs,
                            &mut all_known_rooms,
                            &room_list_service,
                        ).await?;
                    } else {
                        error!("BUG: room_list: diff Remove index {remove_index} out of bounds, len {}", all_known_rooms.len());
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
                }
                VectorDiff::Reset { values: new_rooms } => {
                    // We implement this by clearing all rooms and then adding back the new values.
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Reset, old length {}, new length {}", all_known_rooms.len(), new_rooms.len()); }
                    // Iterate manually so we can know which rooms are being removed.
                    while let Some(room) = all_known_rooms.pop_back() {
                        remove_room(&room);
                    }
                    // ALL_JOINED_ROOMS should already be empty due to successive calls to `remove_room()`,
                    // so this is just a sanity check.
                    ALL_JOINED_ROOMS.lock().unwrap().clear();
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                    for new_room in new_rooms.into_iter() {
                        let new_room = RoomListServiceRoomInfo::from_room(new_room.into_inner()).await;
                        add_new_room(&new_room, &room_list_service).await?;
                        all_known_rooms.push_back(new_room);
                    }
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
) -> Result<()> {
    let next_diff_was_handled: bool;
    match peekable_diffs.peek() {
        Some(VectorDiff::Insert { index: insert_index, value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + Insert({insert_index}) into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref()).await;
            update_room(room, &new_room, room_list_service).await?;
            all_known_rooms.insert(*insert_index, new_room);
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushFront { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushFront into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref()).await;
            update_room(room, &new_room, room_list_service).await?;
            all_known_rooms.push_front(new_room);
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushBack { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushBack into Update for room {}", room.room_id);
            }
            let new_room = RoomListServiceRoomInfo::from_room_ref(new_room.deref()).await;
            update_room(room, &new_room, room_list_service).await?;
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
                    return add_new_room(new_room, room_list_service).await;
                }
                RoomState::Invited => {
                    log!("update_room(): adding new Invited room: {:?} ({new_room_id})", new_room.display_name);
                    return add_new_room(new_room, room_list_service).await;
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
            // let should_update_latest = match (old_room.latest_event_timestamp, new_room.new_latest_event_timestamp()) {
            //     (Some(old_ts), Some(new_ts)) if new_ts > old_ts => true,
            //     (None, Some(_)) => true,
            //     _ => false,
            // };
            // if should_update_latest { ... }
            update_latest_event(&new_room.room).await;

            if old_room.tags != new_room.tags {
                log!("Updating room {} tags from {:?} to {:?}", new_room_id, old_room.tags, new_room.tags);
                enqueue_rooms_list_update(RoomsListUpdate::Tags {
                    room_id: new_room_id.clone(),
                    new_tags: new_room.tags.clone().unwrap_or_default(),
                });
            }

            if old_room.num_unread_messages != new_room.num_unread_messages
                || old_room.num_unread_mentions != new_room.num_unread_mentions
            {
                log!("Updating room {}, unread messages {} --> {}, unread mentions {} --> {}",
                    new_room_id,
                    old_room.num_unread_messages, new_room.num_unread_messages,
                    old_room.num_unread_mentions, new_room.num_unread_mentions,
                );
                enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                    room_id: new_room_id.clone(),
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

            let mut __timeline_update_sender_opt = None;
            let mut get_timeline_update_sender = |room_id| {
                if __timeline_update_sender_opt.is_none() {
                    if let Some(jrd) = ALL_JOINED_ROOMS.lock().unwrap().get(room_id) {
                        __timeline_update_sender_opt = Some(jrd.timeline_update_sender.clone());
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
        add_new_room(new_room, room_list_service).await
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
) -> Result<()> {
    match new_room.state {
        RoomState::Knocked => {
            // TODO: handle Knocked rooms (e.g., can you re-knock? or cancel a prior knock?)
            return Ok(());
        }
        RoomState::Banned => {
            log!("Got new Banned room: {:?} ({})", new_room.display_name, new_room.room_id);
            // TODO: handle rooms that this user has been banned from.
            return Ok(());
        }
        RoomState::Left => {
            log!("Got new Left room: {:?} ({:?})", new_room.display_name, new_room.room_id);
            // TODO: add this to the list of left rooms,
            //       which is collapsed by default.
            //       Upon clicking a left room, we can show a splash page
            //       that prompts the user to rejoin the room or forget it.

            // TODO: this may also be called when a user rejects an invite, not sure.
            //       So we might also need to make a new RoomsListUpdate::RoomLeft variant.
            return Ok(());
        }
        RoomState::Invited => {
            let invite_details = new_room.room.invite_details().await.ok();
            let latest_event = if let Some(latest_event) = new_room.room.latest_event() {
                EventTimelineItem::from_latest_event(
                    room_list_service.client().clone(),
                    &new_room.room_id,
                    latest_event,
                ).await
            } else {
                None
            };
            let latest = latest_event.as_ref().map(
                |ev| get_latest_event_details(ev, &new_room.room_id)
            );
            let room_name_id = RoomNameId::from((new_room.display_name.clone(), new_room.room_id.clone()));
            let room_avatar = room_avatar(&new_room.room, &room_name_id).await;

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
                inviter_info,
                room_avatar,
                canonical_alias: new_room.room.canonical_alias(),
                alt_aliases: new_room.room.alt_aliases(),
                latest,
                invite_state: Default::default(),
                is_selected: false,
                is_direct: new_room.is_direct,
            }));
            Cx::post_action(AppStateAction::RoomLoadedSuccessfully(room_name_id));
            return Ok(());
        }
        RoomState::Joined => { } // Fall through to adding the joined room below.
    }

    // Subscribe to all updates for this room in order to properly receive all of its states,
    // as well as its latest event (via `Room::new_latest_event_*()` and the `LatestEvents` API).
    room_list_service.subscribe_to_rooms(&[&new_room.room_id]).await;


    let timeline = Arc::new(
        new_room.room.timeline_builder()
            .track_read_marker_and_receipts()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("BUG: Failed to build timeline for room {}: {e}", new_room.room_id))?,
    );
    let latest_event = timeline.latest_event().await;
    let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

    let (request_sender, request_receiver) = watch::channel(Vec::new());
    let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
        new_room.room.clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
        request_receiver,
    ));

    let latest = latest_event.as_ref().map(
        |ev| get_latest_event_details(ev, &new_room.room_id)
    );

    // We need to add the room to the `ALL_JOINED_ROOMS` list before we can send
    // an `AddJoinedRoom` update to the RoomsList widget, because that widget might
    // immediately issue a `MatrixRequest` that relies on that room being in `ALL_JOINED_ROOMS`.
    log!("Adding new joined room {}, name: {:?}", new_room.room_id, new_room.display_name);
    ALL_JOINED_ROOMS.lock().unwrap().insert(
        new_room.room_id.clone(),
        JoinedRoomDetails {
            room_id: new_room.room_id.clone(),
            timeline,
            timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
            timeline_update_sender,
            timeline_subscriber_handler_task,
            typing_notice_subscriber: None,
            pinned_events_subscriber: None,
        },
    );

    let room_name_id = RoomNameId::from((new_room.display_name.clone(), new_room.room_id.clone()));
    // Start with a basic text avatar; the avatar image will be fetched asynchronously below.
    let room_avatar = avatar_from_room_name(room_name_id.name_for_avatar().as_deref());
    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddJoinedRoom(JoinedRoomInfo {
        latest,
        tags: new_room.tags.clone().unwrap_or_default(),
        num_unread_messages: new_room.num_unread_messages,
        num_unread_mentions: new_room.num_unread_mentions,
        room_avatar,
        room_name_id: room_name_id.clone(),
        canonical_alias: new_room.room.canonical_alias(),
        alt_aliases: new_room.room.alt_aliases(),
        has_been_paginated: false,
        is_selected: false,
        is_direct: new_room.is_direct,
        is_tombstoned: new_room.is_tombstoned,
    }));

    Cx::post_action(AppStateAction::RoomLoadedSuccessfully(room_name_id));
    spawn_fetch_room_avatar(new_room);
    Ok(())
}

#[allow(unused)]
async fn current_ignore_user_list(client: &Client) -> Option<BTreeSet<OwnedUserId>> {
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
                .collect::<BTreeSet<_>>();

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
                    submit_async_request(MatrixRequest::PaginateRoomTimeline {
                        room_id: joined_room.room_id().to_owned(),
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
/// If the loaded dock state contains open rooms and dock items, this function emits an action
/// to instruct the UI to restore the app state for the main home view (all rooms).
/// If loading fails, it shows a popup notification with the error message.
fn handle_load_app_state(user_id: OwnedUserId) {
    Handle::current().spawn(async move {
        match load_app_state(&user_id).await {
            Ok(app_state) => {
                if !app_state.saved_dock_state_home.open_rooms.is_empty()
                    && !app_state.saved_dock_state_home.dock_items.is_empty()
                {
                    log!("Loaded room panel state from app data directory. Restoring now...");
                    Cx::post_action(AppStateAction::RestoreAppStateFromPersistentState(app_state));
                }
            }
            Err(_e) => {
                log!("Failed to restore dock layout from persistent state: {_e}");
                enqueue_popup_notification(PopupItem {
                    message: String::from("Could not restore the previous dock layout."),
                    kind: PopupKind::Error,
                    auto_dismissal_duration: None
                });
            }
        }
    });
}

fn handle_sync_service_state_subscriber(mut subscriber: Subscriber<sync_service::State>) {
    log!("Initial sync service state is {:?}", subscriber.get());
    Handle::current().spawn(async move {
        while let Some(state) = subscriber.next().await {
            log!("Received a sync service state update: {state:?}");
            match state {
                sync_service::State::Error(e) => {
                    log!("Restarting sync service due to error: {e}.");
                    if let Some(ss) = get_sync_service() {
                        ss.start().await;
                    } else {
                        enqueue_popup_notification(PopupItem {
                            message: "Unable to restart the Matrix sync service.\n\nPlease quit and restart Robrix.".into(),
                            auto_dismissal_duration: None,
                            kind: PopupKind::Error,
                        });
                    }
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


/// Returns the timestamp and text preview of the given `latest_event` timeline item.
///
/// If the sender profile of the event is not yet available, this function will
/// generate a preview using the sender's user ID instead of their display name,
/// and will submit a background async request to fetch the details for this event.
fn get_latest_event_details(
    latest_event: &EventTimelineItem,
    room_id: &OwnedRoomId,
) -> (MilliSecondsSinceUnixEpoch, String) {
    let sender_username = &utils::get_or_fetch_event_sender(latest_event, Some(room_id));
    (
        latest_event.timestamp(),
        text_preview_of_timeline_item(
            latest_event.content(),
            latest_event.sender(),
            sender_username,
        ).format_with(sender_username, true),
    )
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

/// A per-room async task that listens for timeline updates and sends them to the UI thread.
///
/// One instance of this async task is spawned for each room the client knows about.
async fn timeline_subscriber_handler(
    room: Room,
    timeline: Arc<Timeline>,
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    mut request_receiver: watch::Receiver<Vec<BackwardsPaginateUntilEventRequest>>,
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
    log!("Starting timeline subscriber for room {room_id}...");
    let (mut timeline_items, mut subscriber) = timeline.subscribe().await;
    log!("Received initial timeline update of {} items for room {room_id}.", timeline_items.len());

    timeline_update_sender.send(TimelineUpdate::FirstUpdate {
        initial_items: timeline_items.clone(),
    }).unwrap_or_else(
        |_e| panic!("Error: timeline update sender couldn't send first update ({} items) to room {room_id}!", timeline_items.len())
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
                    // log!("Received new request to search for event {new_target_event_id} in room {room_id} starting from index {starting_index} (tl len {}).", timeline_items.len());
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
                        // log!("Found existing target event {new_target_event_id} in room {room_id} at index {target_event_tl_index}.");

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
                            |_e| panic!("Error: timeline update sender couldn't send TargetEventFound({new_target_event_id}, {target_event_tl_index}) to room {room_id}!")
                        );
                        // Send a Makepad-level signal to update this room's timeline UI view.
                        SignalToUI::set_ui_signal();
                    }
                    else {
                        log!("Target event not in timeline. Starting backwards pagination \
                            in room {room_id} to find target event {new_target_event_id} \
                            starting from index {starting_index}.",
                        );
                        // If we didn't find the target event in the current timeline items,
                        // we need to start loading previous items into the timeline.
                        submit_async_request(MatrixRequest::PaginateRoomTimeline {
                            room_id: room_id.clone(),
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
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Append {_values_len}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        is_append = true;
                    }
                    VectorDiff::Clear => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Clear"); }
                        clear_cache = true;
                        timeline_items.clear();
                    }
                    VectorDiff::PushFront { value } => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PushFront"); }
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
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PushBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        is_append = true;
                    }
                    VectorDiff::PopFront => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PopFront"); }
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
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PopBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
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
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Insert at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Set { index, value } => {
                        index_of_first_change = min(index_of_first_change, index);
                        index_of_last_change  = max(index_of_last_change, index.saturating_add(1));
                        timeline_items.set(index, value);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Set at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
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
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Remove at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Truncate { length } => {
                        if length == 0 {
                            clear_cache = true;
                        } else {
                            index_of_first_change = min(index_of_first_change, length.saturating_sub(1));
                            index_of_last_change = usize::MAX;
                        }
                        timeline_items.truncate(length);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Truncate to length {length}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    }
                    VectorDiff::Reset { values } => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Reset, new length {}", values.len()); }
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
                    log!("timeline_subscriber: applied {num_updates} updates for room {room_id}, timeline now has {} items. is_append? {is_append}, clear_cache? {clear_cache}. Changes: {changed_indices:?}.", timeline_items.len());
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
                        |_e| panic!("Error: timeline update sender couldn't send TargetEventFound({found_event_id}, {index}) to room {room_id}!")
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

    error!("Error: unexpectedly ended timeline subscriber for room {room_id}.");
}

/// Handles the given updated latest event for the given room.
///
/// This function sends a `RoomsListUpdate::UpdateLatestEvent`
/// to update the latest event in the RoomsListEntry for the given room.
async fn update_latest_event(room: &Room) {
    let Some(client) = get_client() else { return };
    let (sender_username, sender_id, timestamp, content) = match room.new_latest_event().await {
        LatestEventValue::Remote { timestamp, sender, is_own, profile, content } => {
            let sender_username = if let TimelineDetails::Ready(profile) = profile {
                profile.display_name
            } else if is_own {
                client.account().get_display_name().await.ok().flatten()
            } else {
                None
            };
            (
                sender_username.unwrap_or_else(|| sender.to_string()),
                sender,
                timestamp,
                content
            )
        }
        LatestEventValue::Local { timestamp, content, is_sending: _ } => {
            // TODO: use the `is_sending` flag to augment the preview text
            //       (e.g., "Sending... <msg>" or "Failed to send <msg>").
            let our_name = client.account().get_display_name().await.ok().flatten();
            let Some(our_user_id) = current_user_id() else { return };
            (
                our_name.unwrap_or_else(|| String::from("You")),
                our_user_id,
                timestamp,
                content,
            )
        }
        LatestEventValue::None => return,
    };

    let latest_message_text = text_preview_of_timeline_item(
        &content,
        &sender_id,
        &sender_username,
    ).format_with(&sender_username, true);

    enqueue_rooms_list_update(RoomsListUpdate::UpdateLatestEvent {
        room_id: room.room_id().to_owned(),
        timestamp,
        latest_message_text,
    });
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
            utils::avatar_from_room_name(room_name_id.name_for_avatar().as_deref())
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
        // Try to use the DEFAULT_SSO_CLIENT that we proactively created
        // during initialization (to speed up opening the SSO browser window).
        let mut client_and_session = client_and_session_opt;

        // If the DEFAULT_SSO_CLIENT is none (meaning it failed to build),
        // or if the homeserver_url is *not* empty and isn't the default,
        // we cannot use the DEFAULT_SSO_CLIENT, so we must build a new one.
        let mut build_client_error = None;
        if client_and_session.is_none() || (
            !homeserver_url.is_empty()
                && homeserver_url != "matrix.org"
                && Url::parse(&homeserver_url) != Url::parse("https://matrix-client.matrix.org/")
                && Url::parse(&homeserver_url) != Url::parse("https://matrix.org/")
        ) {
            match build_client(
                &Cli {
                    homeserver: homeserver_url.is_empty().not().then_some(homeserver_url),
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
                Uri::new(&sso_url).open().map_err(|err| {
                    Error::UnknownError(
                        Box::new(io::Error::other(
                            format!("Unable to open SSO login url. Error: {:?}", err),
                        ))
                        .into(),
                    )
                })
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
                    if let Err(e) = login_sender.send(LoginRequest::LoginBySSOSuccess(client, client_session)).await {
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
        // const RoomPowerLevels = 1 << 49;
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
}


/// Shuts down the current Tokio runtime completely and takes ownership to ensure proper cleanup.
pub fn shutdown_background_tasks() {
    if let Some(runtime) = TOKIO_RUNTIME.lock().unwrap().take() {
        runtime.shutdown_background();
    }
}

pub async fn clear_app_state(config: &LogoutConfig) -> Result<()> {
    // Clear resources normally, allowing them to be properly dropped
    // This prevents memory leaks when users logout and login again without closing the app
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
