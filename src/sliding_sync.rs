use anyhow::{anyhow, bail, Result};
use bitflags::bitflags;
use clap::Parser;
use eyeball::Subscriber;
use eyeball_im::VectorDiff;
use futures_util::{pin_mut, StreamExt};
use imbl::Vector;
use makepad_widgets::{error, log, warning, Cx, SignalToUI};
use matrix_sdk::{
    config::RequestConfig, encryption::EncryptionSettings, event_handler::EventHandlerDropGuard, media::MediaRequestParameters, room::{edit::EditedContent, reply::Reply, RoomMember}, ruma::{
        api::client::receipt::create_receipt::v3::ReceiptType, events::{
            receipt::ReceiptThread, room::{
                message::RoomMessageEventContent, power_levels::RoomPowerLevels, MediaSource
            }, FullStateEventContent, MessageLikeEventType, StateEventType
        }, matrix_uri::MatrixId, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId, RoomOrAliasId, UserId
    }, sliding_sync::VersionBuilder, Client, ClientBuildError, Error, OwnedServerName, Room, RoomMemberships, RoomState
};
use matrix_sdk_ui::{
    room_list_service::{RoomListLoadingState, SyncIndicator}, sync_service::{self, SyncService}, timeline::{AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, RoomExt, TimelineEventItemId, TimelineItem, TimelineItemContent}, RoomListService, Timeline
};
use robius_open::Uri;
use tokio::{
    runtime::Handle,
    sync::{mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender}, watch, Notify}, task::JoinHandle, time::error::Elapsed,
};
use unicode_segmentation::UnicodeSegmentation;
use url::Url;
use std::{cmp::{max, min}, collections::{BTreeMap, BTreeSet}, future::Future, iter::Peekable, ops::Not, path:: Path, sync::{atomic::{AtomicBool, Ordering}, Arc, LazyLock, Mutex}, time::Duration};
use std::io;
use crate::{
    app::AppStateAction,
    app_data_dir,
    avatar_cache::AvatarUpdate,
    event_preview::text_preview_of_timeline_item,
    home::{
        invite_screen::{JoinRoomAction, LeaveRoomAction},
        room_screen::TimelineUpdate,
        rooms_list::{self, enqueue_rooms_list_update, InvitedRoomInfo, InviterInfo, JoinedRoomInfo, RoomsListUpdate},
        rooms_list_header::RoomsListHeaderAction,
    },
    login::login_screen::LoginAction,
    logout::{logout_confirm_modal::LogoutAction, logout_state_machine::{logout_with_state_machine, LogoutConfig}}, media_cache::{MediaCacheEntry, MediaCacheEntryRef},
    persistence::{self, load_app_state, ClientSessionPersisted},
    profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{enqueue_user_profile_update, UserProfileUpdate},
    },
    room::RoomPreviewAvatar,
    shared::{
        html_or_plaintext::MatrixLinkPillState,
        jump_to_bottom_button::UnreadMessageCount,
        popup_list::{enqueue_popup_notification, PopupItem, PopupKind}
    },
    utils::{self, AVATAR_THUMBNAIL_FORMAT},
    verification::add_verification_event_handlers_and_sync_client
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
        .with_decryption_trust_requirement(matrix_sdk::crypto::TrustRequirement::Untrusted)
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
        /// Whether to subscribe or unsubscribe from typing notices for this room.
        subscribe: bool,
    },
    /// Subscribe to changes in the read receipts of our own user.
    ///
    /// This request does not return a response or notify the UI thread.
    SubscribeToOwnUserReadReceiptsChanged {
        room_id: OwnedRoomId,
        /// Whether to subscribe or unsubscribe to changes in the read receipts of our own user for this room
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
    /// Sends a request to obtain the room's pill link info for the given Matrix ID.
    ///
    /// The MatrixLinkPillInfo::Loaded variant is sent back to the main UI thread via.
    GetMatrixRoomLinkPillInfo {
        matrix_id: MatrixId,
        via: Vec<OwnedServerName>
    },
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    if let Some(sender) = REQUEST_SENDER.lock().unwrap().as_ref() {
        sender.send(req)
            .expect("BUG: async worker task receiver has died!");
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


/// The entry point for an async worker thread that can run async tasks.
///
/// All this thread does is wait for [`MatrixRequests`] from the main UI-driven non-async thread(s)
/// and then executes them within an async runtime context.
async fn async_worker(
    mut request_receiver: UnboundedReceiver<MatrixRequest>,
    login_sender: Sender<LoginRequest>,
) -> Result<()> {
    log!("Started async_worker task.");
    let mut tasks_list: BTreeMap<OwnedRoomId, JoinHandle<()>> = BTreeMap::new();
    while let Some(request) = request_receiver.recv().await {
        match request {
            MatrixRequest::Login(login_request) => {
                if let Err(e) = login_sender.send(login_request).await {
                    error!("Error sending login request to login_sender: {e:?}");
                    Cx::post_action(LoginAction::LoginFailure(String::from(
                        "BUG: failed to send login request to async worker thread."
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

            MatrixRequest::JoinRoom { room_id } => {
                let Some(client) = get_client() else { continue };
                let _join_room_task = Handle::current().spawn(async move {
                    log!("Sending request to join room {room_id}...");
                    let result_action = if let Some(room) = client.get_room(&room_id) {
                        match room.join().await {
                            Ok(()) => {
                                log!("Successfully joined room {room_id}.");
                                JoinRoomAction::Joined { room_id }
                            }
                            Err(e) => {
                                error!("Error joining room {room_id}: {e:?}");
                                JoinRoomAction::Failed { room_id, error: e }
                            }
                        }
                    } else {
                        error!("BUG: client could not get room with ID {room_id}");
                        JoinRoomAction::Failed {
                            room_id,
                            error: matrix_sdk::Error::UnknownError(
                                String::from("Client couldn't locate room to join it.").into()
                            ),
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
                                LeaveRoomAction::Left { room_id }
                            }
                            Err(e) => {
                                error!("Error leaving room {room_id}: {e:?}");
                                LeaveRoomAction::Failed { room_id, error: e }
                            }
                        }
                    } else {
                        error!("BUG: client could not get room with ID {room_id}");
                        LeaveRoomAction::Failed {
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
                                        username: response.displayname,
                                        user_id: user_id.clone(),
                                        avatar_state: AvatarState::Known(response.avatar_url),
                                    }
                                ));
                            } else {
                                log!("User profile request: client could not get user with ID {user_id}");
                            }
                        }

                        match update.as_mut() {
                            Some(UserProfileUpdate::Full { new_profile: UserProfile { username, .. }, .. }) if username.is_none() => {
                                if let Ok(response) = client.account().fetch_user_profile_of(&user_id).await {
                                    *username = response.displayname;
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
                        count: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                        unread_mentions:timeline.room().num_unread_mentions(),
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
                    if let Some(task_handler) = tasks_list.remove(&room_id) {
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
                                    error!("Failed to get own user read receipt: {e:?}");
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
                                    count: UnreadMessageCount::Known(unread_count),
                                    unread_mentions,
                                });
                            }
                        }
                    }
                });
                tasks_list.insert(room_id.clone(), subscribe_own_read_receipt_task);
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

            MatrixRequest::SendMessage { room_id, message, replied_to } => {
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
                    // The message already contains mentions, no need to add them again
                    if let Some(replied_to_info) = replied_to {
                        match timeline.send_reply(message.into(), replied_to_info).await {
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
                    match timeline.send_single_receipt(ReceiptType::Read, ReceiptThread::Unthreaded, event_id.clone()).await {
                        Ok(sent) => log!("{} read receipt to room {room_id} for event {event_id}", if sent { "Sent" } else { "Already sent" }),
                        Err(_e) => error!("Failed to send read receipt to room {room_id} for event {event_id}; error: {_e:?}"),
                    }
                    // Also update the number of unread messages in the room.
                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                        room_id: room_id.clone(),
                        count: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
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
                    match timeline.send_single_receipt(ReceiptType::FullyRead, ReceiptThread::Unthreaded, event_id.clone()).await {
                        Ok(sent) => log!("{} fully read receipt to room {room_id} for event {event_id}",
                            if sent { "Sent" } else { "Already sent" }
                        ),
                        Err(_e) => error!("Failed to send fully read receipt to room {room_id} for event {event_id}; error: {_e:?}"),
                    }
                    // Also update the number of unread messages in the room.
                    enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                        room_id: room_id.clone(),
                        count: UnreadMessageCount::Known(timeline.room().num_unread_messages()),
                        unread_mentions: timeline.room().num_unread_mentions()
                    });
                });
            },

            MatrixRequest::GetRoomPowerLevels { room_id } => {
                let (timeline, sender) = {
                    let all_joined_rooms = ALL_JOINED_ROOMS.lock().unwrap();
                    let Some(room_info) = all_joined_rooms.get(&room_id) else {
                        log!("BUG: room info not found for fetch members request {room_id}");
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
        }
    }

    error!("async_worker task ended unexpectedly");
    bail!("async_worker task ended unexpectedly")
}


/// The single global Tokio runtime that is used by all async tasks.
static TOKIO_RUNTIME: Mutex<Option<Arc<tokio::runtime::Runtime>>> = Mutex::new(None);


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
    let mut binding = TOKIO_RUNTIME.lock().unwrap();
    let rt = binding.get_or_insert_with(|| Arc::new(tokio::runtime::Runtime::new().unwrap()));
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
/// Returns a reference to the Tokio runtime that is used to run async background tasks.
pub fn start_matrix_tokio() -> Result<Arc<tokio::runtime::Runtime>> {
    // Create a Tokio runtime, and save it in a static variable to ensure it isn't dropped.
    let runtime = {
        let mut rt_guard = TOKIO_RUNTIME.lock().unwrap();
        rt_guard.get_or_insert_with(|| {
            log!("Create newTokio Runtime...");
            Arc::new(tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"))
        }).clone()
    };

    let rt_handle = runtime.handle().clone();

    // Create a channel to be used between UI thread(s) and the async worker thread.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<MatrixRequest>();
    *REQUEST_SENDER.lock().unwrap() = Some(sender);

    let (login_sender, login_receiver) = tokio::sync::mpsc::channel(1);
    // Start a high-level async task that will start and monitor all other tasks.
    let rt = rt_handle.clone();
    let _monitor = rt_handle.spawn(async move {
        // Spawn the actual async worker thread.
        let mut worker_join_handle = rt.spawn(async_worker(receiver, login_sender));

        // Start the main loop that drives the Matrix client SDK.
        let mut main_loop_join_handle = rt.spawn(async_main_loop(login_receiver));
        // Build a Matrix Client in the background so that SSO Server starts earlier.
        rt.spawn(async move {
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

        #[allow(clippy::never_loop)] // unsure if needed, just following tokio's examples.
        loop {
            tokio::select! {
                result = &mut main_loop_join_handle => {
                    match result {
                        Ok(Ok(())) => {
                            error!("BUG: main async loop task ended unexpectedly!");
                        }
                        Ok(Err(e)) => {
                            error!("Error: main async loop task ended:\n\t{e:?}");
                            rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                status: e.to_string(),
                            });
                            enqueue_popup_notification(PopupItem { message: format!("Rooms list update error: {e}"), kind: PopupKind::Error, auto_dismissal_duration: None });
                        },
                        Err(e) => {
                            error!("BUG: failed to join main async loop task: {e:?}");
                        }
                    }
                    break;
                }
                result = &mut worker_join_handle => {
                    match result {
                        Ok(Ok(())) => {
                            // Check if this is due to logout
                            if LOGOUT_IN_PROGRESS.load(Ordering::Acquire) {
                                log!("async worker task ended due to logout");
                            } else {
                                error!("BUG: async worker task ended unexpectedly!");
                            }
                        }
                        Ok(Err(e)) => {
                            // Check if this is due to logout
                            if LOGOUT_IN_PROGRESS.load(Ordering::Acquire) {
                                log!("async worker task ended with error due to logout: {e:?}");
                            } else {
                                error!("Error: async worker task ended:\n\t{e:?}");
                                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: e.to_string(),
                                });
                                enqueue_popup_notification(PopupItem { message: format!("Rooms list update error: {e}"), kind: PopupKind::Error, auto_dismissal_duration: None });

                            }
                        },
                        Err(e) => {
                            error!("BUG: failed to join async worker task: {e:?}");
                        }
                    }
                    break;
                }
            }
        }
    });

    Ok(runtime)
}


/// A tokio::watch channel sender for sending requests from the RoomScreen UI widget
/// to the corresponding background async task for that room (its `timeline_subscriber_handler`).
pub type TimelineRequestSender = watch::Sender<Vec<BackwardsPaginateUntilEventRequest>>;


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
    /// The ID of the old tombstoned room that this room has replaced, if any.
    replaces_tombstoned_room: Option<OwnedRoomId>,
}
impl Drop for JoinedRoomDetails {
    fn drop(&mut self) {
        log!("Dropping JoinedRoomDetails for room {}", self.room_id);
        self.timeline_subscriber_handler_task.abort();
        drop(self.typing_notice_subscriber.take());
        if let Some(replaces_tombstoned_room) = self.replaces_tombstoned_room.take() {
            TOMBSTONED_ROOMS.lock().unwrap().insert(
                self.room_id.clone(),
                replaces_tombstoned_room,
            );
        }
    }
}


/// Information about all joined rooms that our client currently know about.
static ALL_JOINED_ROOMS: Mutex<BTreeMap<OwnedRoomId, JoinedRoomDetails>> = Mutex::new(BTreeMap::new());

/// Information about all of the rooms that have been tombstoned.
///
/// The map key is the **NEW** replacement room ID, and the value is the **OLD** tombstoned room ID.
/// This allows us to quickly query if a newly-encountered room is a replacement for an old tombstoned room.
static TOMBSTONED_ROOMS: Mutex<BTreeMap<OwnedRoomId, OwnedRoomId>> = Mutex::new(BTreeMap::new());

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
) -> Option<(
        crossbeam_channel::Sender<TimelineUpdate>,
        crossbeam_channel::Receiver<TimelineUpdate>,
        TimelineRequestSender,
    )>
{
    ALL_JOINED_ROOMS.lock().unwrap()
        .get_mut(room_id)
        .and_then(|ri| ri.timeline_singleton_endpoints.take()
            .map(|(receiver, request_sender)| {
                (ri.timeline_update_sender.clone(), receiver, request_sender)
            })
        )
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
/// determine if the room has changed state.
/// We can't just store the `matrix_sdk::Room` object itself,
/// because that is a shallow reference to an inner room object within
/// the room list service
#[derive(Clone)]
struct RoomListServiceRoomInfo {
    room: matrix_sdk::Room,
    room_id: OwnedRoomId,
    room_state: RoomState,
}
impl From<&matrix_sdk::Room> for RoomListServiceRoomInfo {
    fn from(room: &matrix_sdk::Room) -> Self {
        room.clone().into()
    }
}
impl From<matrix_sdk::Room> for RoomListServiceRoomInfo {
    fn from(room: matrix_sdk::Room) -> Self {
        Self {
            room_id: room.room_id().to_owned(),
            room_state: room.state(),
            room,
        }
    }
}

async fn async_main_loop(
    mut login_receiver: Receiver<LoginRequest>,
) -> Result<()> {
    // only init subscribe once
    let _ = tracing_subscriber::fmt::try_init();

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
                        return Err(anyhow::anyhow!("BUG: login_receiver hung up unexpectedly"));
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
    // enqueue_popup_notification(status.clone());
    enqueue_rooms_list_update(RoomsListUpdate::Status { status });

    client.event_cache().subscribe().expect("BUG: CLIENT's event cache unable to subscribe");

    if let Some(_existing) = CLIENT.lock().unwrap().replace(client.clone()) {
        error!("BUG: unexpectedly replaced an existing client when initializing the matrix client.");
    }

    add_verification_event_handlers_and_sync_client(client.clone());

    // Listen for updates to the ignored user list.
    handle_ignore_user_list_subscriber(client.clone());

    let sync_service = SyncService::builder(client.clone())
        .with_offline_mode()
        .build()
        .await?;

    // Attempt to load the previously-saved app state.
    // Include this after re-login.
    handle_load_app_state(logged_in_user_id.to_owned());
    handle_sync_indicator_subscriber(&sync_service);
    handle_sync_service_state_subscriber(sync_service.state());
    sync_service.start().await;
    let room_list_service = sync_service.room_list_service();

    if let Some(_existing) = SYNC_SERVICE.lock().unwrap().replace(Arc::new(sync_service)) {
        error!("BUG: unexpectedly replaced an existing sync service when initializing the matrix client.");
    }

    let all_rooms_list = room_list_service.all_rooms().await?;
    handle_room_list_service_loading_state(all_rooms_list.loading_state());

    let (room_diff_stream, room_list_dynamic_entries_controller) =
        // TODO: paginate room list to avoid loading all rooms at once
        all_rooms_list.entries_with_dynamic_adapters(usize::MAX);

    room_list_dynamic_entries_controller.set_filter(
        Box::new(|_room| true),
    );

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
                        add_new_room(&new_room, &room_list_service).await?;
                        all_known_rooms.push_back(new_room.into());
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
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.push_front(new_room.into());
                }
                VectorDiff::PushBack { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushBack"); }
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.push_back(new_room.into());
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
                    add_new_room(&new_room, &room_list_service).await?;
                    all_known_rooms.insert(index, new_room.into());
                }
                VectorDiff::Set { index, value: changed_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Set at {index}"); }
                    if let Some(old_room) = all_known_rooms.get(index) {
                        update_room(old_room, &changed_room, &room_list_service).await?;
                    } else {
                        error!("BUG: room list diff: Set index {index} was out of bounds.");
                    }
                    all_known_rooms.set(index, changed_room.into());
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
                    for room in &new_rooms {
                        add_new_room(room, &room_list_service).await?;
                    }
                    all_known_rooms = new_rooms.into_iter().map(|r| r.into()).collect();
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
    remove_diff: VectorDiff<Room>,
    room: &RoomListServiceRoomInfo,
    peekable_diffs: &mut Peekable<impl Iterator<Item = VectorDiff<matrix_sdk::Room>>>,
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
            update_room(room, new_room, room_list_service).await?;
            all_known_rooms.insert(*insert_index, new_room.clone().into());
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushFront { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushFront into Update for room {}", room.room_id);
            }
            update_room(room, new_room, room_list_service).await?;
            all_known_rooms.push_front(new_room.clone().into());
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushBack { value: new_room })
            if room.room_id == new_room.room_id() =>
        {
            if LOG_ROOM_LIST_DIFFS {
                log!("Optimizing {remove_diff:?} + PushBack into Update for room {}", room.room_id);
            }
            update_room(room, new_room, room_list_service).await?;
            all_known_rooms.push_back(new_room.clone().into());
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
    new_room: &matrix_sdk::Room,
    room_list_service: &RoomListService,
) -> Result<()> {
    let new_room_id = new_room.room_id().to_owned();
    if old_room.room_id == new_room_id {
        let new_room_name = new_room.display_name().await.map(|n| n.to_string()).ok();
        let mut room_avatar_changed = false;

        // Handle state transitions for a room.
        let old_room_state = old_room.room_state;
        let new_room_state = new_room.state();
        if LOG_ROOM_LIST_DIFFS {
            log!("Room {new_room_name:?} ({new_room_id}) state went from {old_room_state:?} --> {new_room_state:?}");
        }
        if old_room_state != new_room_state {
            match new_room_state {
                RoomState::Banned => {
                    // TODO: handle rooms that this user has been banned from.
                    log!("Removing Banned room: {new_room_name:?} ({new_room_id})");
                    remove_room(&new_room.into());
                    return Ok(());
                }
                RoomState::Left => {
                    log!("Removing Left room: {new_room_name:?} ({new_room_id})");
                    // TODO: instead of removing this, we could optionally add it to
                    //       a separate list of left rooms, which would be collapsed by default.
                    //       Upon clicking a left room, we could show a splash page
                    //       that prompts the user to rejoin the room or forget it permanently.
                    //       Currently, we just remove it and do not show left rooms at all.
                    remove_room(&new_room.into());
                    return Ok(());
                }
                RoomState::Joined => {
                    log!("update_room(): adding new Joined room: {new_room_name:?} ({new_room_id})");
                    return add_new_room(new_room, room_list_service).await;
                }
                RoomState::Invited => {
                    log!("update_room(): adding new Invited room: {new_room_name:?} ({new_room_id})");
                    return add_new_room(new_room, room_list_service).await;
                }
                RoomState::Knocked => {
                    // TODO: handle Knocked rooms (e.g., can you re-knock? or cancel a prior knock?)
                    return Ok(());
                }
            }
        }


        let Some(client) = get_client() else {
            return Ok(());
        };
        if let (Some(new_latest_event), Some(old_latest_event)) =
            (new_room.latest_event(), old_room.room.latest_event())
        {
            if let Some(new_latest_event) =
                EventTimelineItem::from_latest_event(client.clone(), &new_room_id, new_latest_event)
                    .await
            {
                if let Some(old_latest_event) = EventTimelineItem::from_latest_event(
                    client.clone(),
                    &new_room_id,
                    old_latest_event,
                )
                .await
                {
                    if new_latest_event.timestamp() > old_latest_event.timestamp() {
                        log!("Updating latest event for room {}", new_room_id);
                        room_avatar_changed =
                            update_latest_event(new_room_id.clone(), &new_latest_event, None);
                    }
                }
            }
        }

        if room_avatar_changed || (old_room.room.avatar_url() != new_room.avatar_url()) {
            log!("Updating avatar for room {}", new_room_id);
            spawn_fetch_room_avatar(new_room.clone());
        }

        if let Some(new_room_name) = new_room_name {
            if old_room.room.cached_display_name().map(|room_name| room_name.to_string()).as_ref() != Some(&new_room_name) {
                log!("Updating room name for room {} to {}", new_room_id, new_room_name);
                enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: new_room_id.clone(),
                    new_room_name,
                });
            }
        }

        // We only update tags or unread count for joined rooms.
        // Invited or left rooms don't care about these details.
        if matches!(new_room_state, RoomState::Joined) {
            if let Ok(new_tags) = new_room.tags().await {
                enqueue_rooms_list_update(RoomsListUpdate::Tags {
                    room_id: new_room_id.clone(),
                    new_tags: new_tags.unwrap_or_default(),
                });
            }

            enqueue_rooms_list_update(RoomsListUpdate::UpdateNumUnreadMessages {
                room_id: new_room_id.clone(),
                count: UnreadMessageCount::Known(new_room.num_unread_messages()),
                unread_mentions: new_room.num_unread_mentions()
            });
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
            new_state: room.room_state,
        }
    );
}


/// Invoked when the room list service has received an update with a brand new room.
async fn add_new_room(room: &matrix_sdk::Room, room_list_service: &RoomListService) -> Result<()> {
    let room_id = room.room_id().to_owned();
    // We must call `display_name()` here to calculate and cache the room's name.
    let room_name = room.display_name().await.map(|n| n.to_string()).ok();

    let is_direct = room.is_direct().await.unwrap_or(false);

    match room.state() {
        RoomState::Knocked => {
            // TODO: handle Knocked rooms (e.g., can you re-knock? or cancel a prior knock?)
            return Ok(());
        }
        RoomState::Banned => {
            log!("Got new Banned room: {room_name:?} ({room_id})");
            // TODO: handle rooms that this user has been banned from.
            return Ok(());
        }
        RoomState::Left => {
            log!("Got new Left room: {room_name:?} ({room_id})");
            // TODO: add this to the list of left rooms,
            //       which is collapsed by default.
            //       Upon clicking a left room, we can show a splash page
            //       that prompts the user to rejoin the room or forget it.

            // TODO: this may also be called when a user rejects an invite, not sure.
            //       So we might also need to make a new RoomsListUpdate::RoomLeft variant.
            return Ok(());
        }
        RoomState::Invited => {
            let invite_details = room.invite_details().await.ok();
            let Some(client) = get_client() else {
                return Ok(());
            };
            let latest_event = if let Some(latest_event) = room.latest_event() {
                EventTimelineItem::from_latest_event(client, &room_id, latest_event).await
            } else {
                None
            };
            let latest = latest_event.as_ref().map(
                |ev| get_latest_event_details(ev, &room_id)
            );
            let room_avatar = room_avatar(room, room_name.as_deref()).await;

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
                room_id: room_id.clone(),
                room_name,
                inviter_info,
                room_avatar,
                canonical_alias: room.canonical_alias(),
                alt_aliases: room.alt_aliases(),
                latest,
                invite_state: Default::default(),
                is_selected: false,
                is_direct,
            }));
            Cx::post_action(AppStateAction::RoomLoadedSuccessfully(room_id));
            return Ok(());
        }
        RoomState::Joined => { } // Fall through to adding the joined room below.
    }

    // Subscribe to all updates for this room in order to properly receive all of its states.
    room_list_service.subscribe_to_rooms(&[&room_id]);

    // Do not add tombstoned rooms to the rooms list; they require special handling.
    if let Some(tombstoned_info) = room.successor_room() {
        log!("Room {room_id} has been tombstoned: {tombstoned_info:#?}");
        // Since we don't know the order in which we'll learn about new rooms,
        // we need to first check to see if the replacement for this tombstoned room
        // refers to an already-known room as its replacement.
        // If so, we can immediately update the replacement room's room info
        // to indicate that it replaces this tombstoned room.
        let replacement_room_id = tombstoned_info.room_id;
        if let Some(room_info) = ALL_JOINED_ROOMS.lock().unwrap().get_mut(&replacement_room_id) {
            room_info.replaces_tombstoned_room = Some(replacement_room_id.clone());
        }
        // But if we don't know about the replacement room yet, we need to save this tombstoned room
        // in a separate list so that the replacement room we will discover in the future
        // can know which old tombstoned room it replaces (see the bottom of this function).
        else {
            TOMBSTONED_ROOMS.lock().unwrap().insert(replacement_room_id, room_id.clone());
        }
        return Ok(());
    }

    let timeline = Arc::new(
        room.timeline_builder()
            .track_read_marker_and_receipts()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("BUG: Failed to build timeline for room {room_id}: {e}"))?,
    );
    let latest_event = timeline.latest_event().await;
    let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

    let (request_sender, request_receiver) = watch::channel(Vec::new());
    let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
        room.clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
        request_receiver,
    ));

    let latest = latest_event.as_ref().map(
        |ev| get_latest_event_details(ev, &room_id)
    );

    let tombstoned_room_replaced_by_this_room = TOMBSTONED_ROOMS.lock()
        .unwrap()
        .remove(&room_id);

    log!("Adding new joined room {room_id}. Replaces tombstoned room: {tombstoned_room_replaced_by_this_room:?}");
    ALL_JOINED_ROOMS.lock().unwrap().insert(
        room_id.clone(),
        JoinedRoomDetails {
            room_id: room_id.clone(),
            timeline,
            timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
            timeline_update_sender,
            timeline_subscriber_handler_task,
            typing_notice_subscriber: None,
            replaces_tombstoned_room: tombstoned_room_replaced_by_this_room,
        },
    );
    // We need to add the room to the `ALL_JOINED_ROOMS` list before we can
    // send the `AddJoinedRoom` update to the UI, because the UI might immediately
    // issue a `MatrixRequest` that relies on that room being in `ALL_JOINED_ROOMS`.
    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddJoinedRoom(JoinedRoomInfo {
        room_id: room_id.clone(),
        latest,
        tags: room.tags().await.ok().flatten().unwrap_or_default(),
        num_unread_messages: room.num_unread_messages(),
        num_unread_mentions: room.num_unread_mentions(),
        // start with a basic text avatar; the avatar image will be fetched asynchronously below.
        avatar: avatar_from_room_name(room_name.as_deref()),
        room_name,
        canonical_alias: room.canonical_alias(),
        alt_aliases: room.alt_aliases(),
        has_been_paginated: false,
        is_selected: false,
        is_direct,
    }));

    Cx::post_action(AppStateAction::RoomLoadedSuccessfully(room_id));
    spawn_fetch_room_avatar(room.clone());

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
/// If the loaded dock state contains open rooms and dock items, it logs a message and posts an action
/// to restore the app state in the UI. If loading fails, it enqueues a notification
/// with the error message.
fn handle_load_app_state(user_id: OwnedUserId) {
    Handle::current().spawn(async move {
        match load_app_state(&user_id).await {
            Ok(app_state) => {
                if !app_state.saved_dock_state.open_rooms.is_empty()
                    && !app_state.saved_dock_state.dock_items.is_empty()
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
            if state == sync_service::State::Error {
                log!("Restarting sync service due to error.");
                let sync_service = get_sync_service().expect("BUG: sync service is None");
                sync_service.start().await;
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
                }
            }
        }
    });
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

    let mut latest_event = timeline.latest_event().await;

    // the event ID to search for while loading previous items into the timeline.
    let mut target_event_id = None;
    // the timeline index and event ID of the target event, if it has been found.
    let mut found_target_event_id: Option<(usize, OwnedEventId)> = None;

    loop { tokio::select! {
        // we must check for new requests before handling new timeline updates.
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
                        // log!("Target event not in timeline. Starting backwards pagination in room {room_id} to find target event {new_target_event_id} starting from index {starting_index}.");

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
            // For now we always requery the latest event, but this can be better optimized.
            let mut reobtain_latest_event = true;
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
                        reobtain_latest_event = true;
                        is_append = true;
                    }
                    VectorDiff::Clear => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Clear"); }
                        clear_cache = true;
                        timeline_items.clear();
                        reobtain_latest_event = true;
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
                        reobtain_latest_event |= latest_event.is_none();
                    }
                    VectorDiff::PushBack { value } => {
                        index_of_first_change = min(index_of_first_change, timeline_items.len());
                        timeline_items.push_back(value);
                        index_of_last_change = max(index_of_last_change, timeline_items.len());
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PushBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        reobtain_latest_event = true;
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
                        reobtain_latest_event = true;
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
                        reobtain_latest_event = true;
                    }
                    VectorDiff::Set { index, value } => {
                        index_of_first_change = min(index_of_first_change, index);
                        index_of_last_change  = max(index_of_last_change, index.saturating_add(1));
                        timeline_items.set(index, value);
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Set at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                        reobtain_latest_event = true;
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
                        reobtain_latest_event = true;
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
                        reobtain_latest_event = true;
                    }
                    VectorDiff::Reset { values } => {
                        if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Reset, new length {}", values.len()); }
                        clear_cache = true; // we must assume all items have changed.
                        timeline_items = values;
                        reobtain_latest_event = true;
                    }
                }
            }


            if num_updates > 0 {
                let new_latest_event = if reobtain_latest_event {
                    timeline.latest_event().await
                } else {
                    None
                };

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

                // Update the latest event for this room.
                // We always do this in case a redaction or other event has changed the latest event.
                if let Some(new_latest) = new_latest_event {
                    let room_avatar_changed = update_latest_event(room_id.clone(), &new_latest, Some(&timeline_update_sender));
                    if room_avatar_changed {
                        spawn_fetch_room_avatar(room.clone());
                    }
                    latest_event = Some(new_latest);
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
/// This currently includes checking the given event for:
/// * room name changes, in which it sends a `RoomsListUpdate`.
/// * room power level changes to see if the current user's permissions
///   have changed; if so, it sends a [`TimelineUpdate::UserPowerLevels`].
/// * room avatar changes, which is not handled here.
///   Instead, we return `true` such that other code can fetch the new avatar.
/// * membership changes to see if the current user has joined or left a room.
///
/// Finally, this function sends a `RoomsListUpdate::UpdateLatestEvent`
/// to update the latest event in the RoomsList's room preview for the given room.
///
/// Returns `true` if room avatar has changed and should be fetched and updated.
fn update_latest_event(
    room_id: OwnedRoomId,
    event_tl_item: &EventTimelineItem,
    timeline_update_sender: Option<&crossbeam_channel::Sender<TimelineUpdate>>
) -> bool {
    let mut room_avatar_changed = false;

    let (timestamp, latest_message_text) = get_latest_event_details(event_tl_item, &room_id);
    match event_tl_item.content() {
        // Check for relevant state events.
        TimelineItemContent::OtherState(other) => {
            match other.content() {
                // Check for room name changes.
                AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
                    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                        room_id: room_id.clone(),
                        new_room_name: content.name.clone(),
                    });
                }
                // Check for room avatar changes.
                AnyOtherFullStateEventContent::RoomAvatar(_avatar_event) => {
                    room_avatar_changed = true;
                }
                // Check for if can user send message.
                AnyOtherFullStateEventContent::RoomPowerLevels(FullStateEventContent::Original { content, prev_content: _ }) => {
                    if let (Some(sender), Some(user_id)) = (timeline_update_sender, current_user_id()) {
                        match sender.send(TimelineUpdate::UserPowerLevels(
                            UserPowerLevels::from(&content.clone().into(), &user_id)
                        )) {
                            Ok(_) => {
                                SignalToUI::set_ui_signal();
                            }
                            Err(e) => {
                                error!("Failed to send the new RoomPowerLevels from an updated latest event: {e}");
                            }
                        }
                    }
                }
                _ => { }
            }
        }
        TimelineItemContent::MembershipChange(room_membership_change) => {
            if matches!(
                room_membership_change.change(),
                Some(MembershipChange::InvitationAccepted | MembershipChange::Joined)
            ) {
                if current_user_id().as_deref() == Some(room_membership_change.user_id()) {
                    submit_async_request(MatrixRequest::GetRoomPowerLevels { room_id: room_id.clone() });
                }
            }
        }
        _ => { }
    }

    enqueue_rooms_list_update(RoomsListUpdate::UpdateLatestEvent {
        room_id,
        timestamp,
        latest_message_text,
    });
    room_avatar_changed
}

/// Spawn a new async task to fetch the room's new avatar.
fn spawn_fetch_room_avatar(room: Room) {
    Handle::current().spawn(async move {
        let room_id = room.room_id().to_owned();
        let room_name_str = room.cached_display_name().map(|dn| dn.to_string());
        let avatar = room_avatar(&room, room_name_str.as_deref()).await;
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomAvatar {
            room_id,
            avatar,
        });
    });
}

/// Fetches and returns the avatar image for the given room (if one exists),
/// otherwise returns a text avatar string of the first character of the room name.
async fn room_avatar(room: &Room, room_name: Option<&str>) -> RoomPreviewAvatar {
    match room.avatar(AVATAR_THUMBNAIL_FORMAT.into()).await {
        Ok(Some(avatar)) => RoomPreviewAvatar::Image(avatar.into()),
        _ => {
            if let Ok(room_members) = room.members(RoomMemberships::ACTIVE).await {
                if room_members.len() == 2 {
                    if let Some(non_account_member) = room_members.iter().find(|m| !m.is_account_user()) {
                        if let Ok(Some(avatar)) = non_account_member.avatar(AVATAR_THUMBNAIL_FORMAT.into()).await {
                            return RoomPreviewAvatar::Image(avatar.into());
                        }
                    }
                }
            }
            avatar_from_room_name(room_name)
        }
    }
}

/// Returns a text avatar string containing the first character of the room name.
///
/// Skips the first character if it is a `#` or `!`, the sigils used for Room aliases and Room IDs.
fn avatar_from_room_name(room_name: Option<&str>) -> RoomPreviewAvatar {
    let first = room_name.and_then(|rn| rn
        .graphemes(true)
        .find(|&g| g != "#" && g != "!")
        .map(ToString::to_string)
    ).unwrap_or_else(|| String::from("?"));
    RoomPreviewAvatar::Text(first)
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
                            "BUG: failed to send login request to async worker thread."
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

/// Global atomic flag indicating if the logout process has reached the "point of no return"
/// where aborting the logout operation is no longer safe.
static LOGOUT_POINT_OF_NO_RETURN: AtomicBool = AtomicBool::new(false);

/// Global atomic flag indicating if logout is in progress
static LOGOUT_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn is_logout_past_point_of_no_return() -> bool {
    LOGOUT_POINT_OF_NO_RETURN.load(Ordering::Relaxed)
}

pub fn is_logout_in_progress() -> bool {
    LOGOUT_IN_PROGRESS.load(Ordering::Relaxed)
}

pub fn set_logout_point_of_no_return(value: bool) {
    LOGOUT_POINT_OF_NO_RETURN.store(value, Ordering::Relaxed);
}

pub fn set_logout_in_progress(value: bool) {
    LOGOUT_IN_PROGRESS.store(value, Ordering::Relaxed);
}

/// Shuts down the current Tokio runtime safely in an independent thread.
/// 
/// This function takes ownership of the global runtime and shuts it down
/// using `shutdown_background()` to avoid blocking indefinitely on spawned tasks.
/// Blocks until shutdown is complete to ensure clean state for restart.
pub fn shutdown_background_tasks() {
    if let Some(old_rt) = TOKIO_RUNTIME.lock().unwrap().take() {
        let (tx, rx) = std::sync::mpsc::channel();
        
        std::thread::spawn(move || {
            match Arc::try_unwrap(old_rt) {
                Ok(runtime) => {
                    runtime.shutdown_background();
                    log!("✅ Old runtime shutdown completed");
                }
                Err(arc_rt) => {
                    // If unwrap fails, force drop the Arc
                    drop(arc_rt);
                    log!("⚠️ Forced drop of old runtime Arc");
                }
            }
            let _ = tx.send(());
        });
        
        // Block current thread until shutdown completes
        // This is safe because we're not in tokio context anymore
        let _ = rx.recv();
    }
}

pub async fn clean_app_state(config: &LogoutConfig) -> Result<()> {
    // Clear resources normally, allowing them to be properly dropped
    // This prevents memory leaks when users logout and login again without closing the app
    CLIENT.lock().unwrap().take();
    log!("Client cleared during logout");
    
    SYNC_SERVICE.lock().unwrap().take();
    log!("Sync service cleared during logout");
    
    REQUEST_SENDER.lock().unwrap().take();
    log!("Request sender cleared during logout");
    
    TOMBSTONED_ROOMS.lock().unwrap().clear();
    IGNORED_USERS.lock().unwrap().clear();
    ALL_JOINED_ROOMS.lock().unwrap().clear();
    
    let on_clear_appstate = Arc::new(Notify::new());
    Cx::post_action(LogoutAction::ClearAppState { on_clear_appstate: on_clear_appstate.clone() });
    
    match tokio::time::timeout(config.app_state_cleanup_timeout, on_clear_appstate.notified()).await {
        Ok(_) => {
            log!("Received signal that app state was cleaned successfully");
            Ok(())
        }
        Err(_) => Err(anyhow!("Timed out waiting for app state cleanup")),
    }
}
