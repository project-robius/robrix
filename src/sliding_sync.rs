use anyhow::{bail, Result};
use clap::Parser;
use eyeball::Subscriber;
use eyeball_im::VectorDiff;
use futures_util::{pin_mut, StreamExt};
use imbl::Vector;
use makepad_widgets::{error, log, warning, Cx, SignalToUI};
use matrix_sdk::{
    config::RequestConfig, event_handler::EventHandlerDropGuard, media::MediaRequest, room::{Receipts, RoomMember}, ruma::{
        api::client::{receipt::create_receipt::v3::ReceiptType, session::get_login_types::v3::LoginType}, events::{
            receipt::ReceiptThread, room::{
                message::{ForwardThread, RoomMessageEventContent}, MediaSource
            }, FullStateEventContent
        }, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId, UserId
    }, sliding_sync::VersionBuilder, Client, Error, Room
};
use matrix_sdk_ui::{
    room_list_service::{self, RoomListLoadingState},
    sync_service::{self, SyncService},
    timeline::{AnyOtherFullStateEventContent, EventTimelineItem, RepliedToInfo, TimelineDetails, TimelineItem, TimelineItemContent},
    Timeline,
};
use robius_open::Uri;
use tokio::{
    runtime::Handle,
    sync::{mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender}, watch}, task::JoinHandle,
};
use unicode_segmentation::UnicodeSegmentation;
use std::{cmp::{max, min}, collections::{BTreeMap, BTreeSet}, ops::Not, path:: Path, sync::{Arc, Mutex, OnceLock}};
use std::io;
use crate::{
    app_data_dir, avatar_cache::AvatarUpdate, event_preview::text_preview_of_timeline_item, home::{
        room_screen::TimelineUpdate, rooms_list::{self, enqueue_rooms_list_update, RoomPreviewAvatar, RoomsListEntry, RoomsListUpdate}
    }, login::login_screen::LoginAction, media_cache::MediaCacheEntry, persistent_state::{self, ClientSessionPersisted}, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{enqueue_user_profile_update, UserProfileUpdate},
    }, utils::MEDIA_THUMBNAIL_FORMAT, verification::add_verification_event_handlers_and_sync_client
};


#[derive(Parser, Debug, Default)]
struct Cli {
    /// The user name that should be used for the login.
    #[clap(value_parser)]
    username: String,

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
            username: login.user_id,
            password: login.password,
            homeserver: None,
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
) -> anyhow::Result<(Client, ClientSessionPersisted)> {
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
    Ok((
        client,
        ClientSessionPersisted {
            homeserver: homeserver_url.to_string(),
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
    login_types: &[LoginType],
) -> Result<(Client, Option<String>)> {
    match login_request {
        LoginRequest::LoginByCli | LoginRequest::LoginByPassword(_) => {
            let cli = if let LoginRequest::LoginByPassword(login_by_password) = login_request {
                &Cli::from(login_by_password)
            } else {
                cli
            };
            let (client, client_session) = build_client(cli, app_data_dir()).await?;
            if !login_types
                .iter()
                .any(|flow| matches!(flow, LoginType::Password(_)))
            {
                bail!("Homeserver does not support username + password login flow.");
            }
            // Attempt to login using the CLI-provided username & password.
            let login_result = client
                .matrix_auth()
                .login_username(&cli.username, &cli.password)
                .initial_device_display_name("robrix-un-pw")
                .send()
                .await?;
            if client.logged_in() {
                log!("Logged in successfully? {:?}", client.logged_in());
                enqueue_rooms_list_update(RoomsListUpdate::Status {
                    status: format!("Logged in as {}. Loading rooms...", cli.username),
                });
                if let Err(e) = persistent_state::save_session(&client, client_session).await {
                    error!("Failed to save session state to storage: {e:?}");
                }
                Ok((client, None))
            } else {
                enqueue_rooms_list_update(RoomsListUpdate::Status {
                    status: format!("Failed to login as {}: {:?}", cli.username, login_result),
                });
                bail!("Failed to login as {}: {login_result:?}", cli.username);
            }
        }

        LoginRequest::LoginBySSOSuccess(client, client_session) => {
            if let Err(e) = persistent_state::save_session(&client, client_session).await {
                error!("Failed to save session state to storage: {e:?}");
            }
            Ok((client, None))
        }
        LoginRequest::HomeserverLoginTypesQuery(_) => {
            bail!("LoginRequest::HomeserverLoginTypesQuery not handled earlier");
        }
    }
}

async fn populate_login_types(
    homeserver_url: &str,
    login_types: &mut Vec<LoginType>,
) -> Result<()> {
    Cx::post_action(LoginAction::Status(String::from("Fetching Login Types ...")));
    let homeserver_url = if homeserver_url.is_empty() {
        DEFAULT_HOMESERVER
    } else {
        homeserver_url
    };
    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
        .build()
        .await?;
    match client.matrix_auth().get_login_types().await {
        Ok(login_types_res) => {
            *login_types = login_types_res.flows;
            let identity_providers = login_types.iter().fold(Vec::new(), |mut acc, login_type| {
                if let LoginType::Sso(sso_type) = login_type {
                    acc.extend_from_slice(sso_type.identity_providers.as_slice());
                }
                acc
            });
            Cx::post_action(LoginAction::IdentityProvider(identity_providers));
            Ok(())
        }
        Err(e) => {
            Err(e.into())
        }
    }
}

/// Which direction to paginate in.
///
/// * `Forwards` will retrieve later events (towards the end of the timeline),
///    which only works if the timeline is *focused* on a specific event.
/// * `Backwards`: the more typical choice, in which earlier events are retrieved
///    (towards the start of the timeline), which works in  both live mode and focused mode.
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
    MediaRequest,
    matrix_sdk::Result<Vec<u8>>,
    Option<crossbeam_channel::Sender<TimelineUpdate>>,
);


/// The set of requests for async work that can be made to the worker thread.
pub enum MatrixRequest {
    /// Request from the login screen to log in with the given credentials.
    Login(LoginRequest),
    /// Request to paginate the older (or newer) events of a room's timeline.
    PaginateRoomTimeline {
        room_id: OwnedRoomId,
        /// The maximum number of timeline events to fetch in each pagination batch.
        num_events: u16,
        direction: PaginationDirection,
    },
    /// Request to fetch the full details of the given event in the given room's timeline.
    FetchDetailsForEvent {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Request to fetch profile information for all members of a room.
    /// This can be *very* slow depending on the number of members in the room.
    FetchRoomMembers {
        room_id: OwnedRoomId,
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
        media_request: MediaRequest,
        on_fetched: OnMediaFetchedFn,
        destination: Arc<Mutex<MediaCacheEntry>>,
        update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    },
    /// Request to send a message to the given room.
    SendMessage {
        room_id: OwnedRoomId,
        message: RoomMessageEventContent,
        replied_to: Option<RepliedToInfo>,
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
    /// Sends a read receipt for the given event in the given room.
    ReadReceipt{
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Sends a fully-read receipt for the given event in the given room.
    FullyReadReceipt{
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// Sends a request checking if the currently logged-in user can send a message to the given room.
    ///
    /// The response is delivered back to the main UI thread via a `TimelineUpdate::CanUserSendMessage`.
    CheckCanUserSendMessage{
        room_id: OwnedRoomId,
    }
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    REQUEST_SENDER.get()
        .unwrap() // this is initialized
        .send(req)
        .expect("BUG: async worker task receiver has died!");
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
            MatrixRequest::PaginateRoomTimeline { room_id, num_events, direction } => {
                let (timeline, sender) = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
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
                        timeline.focused_paginate_forwards(num_events).await
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

            MatrixRequest::FetchDetailsForEvent { room_id, event_id } => {
                let (timeline, sender) = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        log!("BUG: room info not found for fetch details for event request {room_id}");
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
                        Err(ref e) => error!("Error fetching details for event {event_id} in room {room_id}: {e:?}"),
                    }
                    sender.send(TimelineUpdate::EventDetailsFetched {
                        event_id,
                        result,
                    }).unwrap();
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::FetchRoomMembers { room_id } => {
                let (timeline, sender) = {
                    let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get(&room_id) else {
                        log!("BUG: room info not found for fetch members request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                // Spawn a new async task that will make the actual fetch request.
                let _fetch_task = Handle::current().spawn(async move {
                    log!("Sending fetch room members request for room {room_id}...");
                    timeline.fetch_members().await;
                    log!("Completed fetch room members request for room {room_id}.");
                    sender.send(TimelineUpdate::RoomMembersFetched).unwrap();
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::GetUserProfile { user_id, room_id, local_only } => {
                let Some(client) = CLIENT.get() else { continue };
                let _fetch_task = Handle::current().spawn(async move {
                    log!("Sending get user profile request: user: {user_id}, \
                        room: {room_id:?}, local_only: {local_only}...",
                    );

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
                        log!("Successfully completed get user profile request: user: {user_id}, room: {room_id:?}, local_only: {local_only}.");
                        enqueue_user_profile_update(upd);
                    } else {
                        log!("Failed to get user profile: user: {user_id}, room: {room_id:?}, local_only: {local_only}.");
                    }
                });
            }

            MatrixRequest::IgnoreUser { ignore, room_member, room_id } => {
                let Some(client) = CLIENT.get() else { continue };
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
                let Some(room) = CLIENT.get().and_then(|c| c.get_room(&room_id)) else {
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
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        log!("BUG: room info not found for subscribe to typing notices request, room {room_id}");
                        continue;
                    };
                    let (room, recv) = if subscribe {
                        if room_info.typing_notice_subscriber.is_some() {
                            warning!("Note: room {room_id} is already subscribed to typing notices.");
                            continue;
                        } else {
                            let Some(room) = CLIENT.get().and_then(|c| c.get_room(&room_id)) else {
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
            MatrixRequest::SpawnSSOServer { brand, homeserver_url, identity_provider_id} => {
                spawn_sso_server(brand, homeserver_url, identity_provider_id, login_sender.clone()).await;
            }
            MatrixRequest::ResolveRoomAlias(room_alias) => {
                let Some(client) = CLIENT.get() else { continue };
                let _resolve_task = Handle::current().spawn(async move {
                    log!("Sending resolve room alias request for {room_alias}...");
                    let res = client.resolve_room_alias(&room_alias).await;
                    log!("Resolved room alias {room_alias} to: {res:?}");
                    todo!("Send the resolved room alias back to the UI thread somehow.");
                });
            }

            MatrixRequest::FetchAvatar { mxc_uri, on_fetched } => {
                let Some(client) = CLIENT.get() else { continue };
                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending fetch avatar request for {mxc_uri:?}...");
                    let media_request = MediaRequest {
                        source: MediaSource::Plain(mxc_uri.clone()),
                        format: MEDIA_THUMBNAIL_FORMAT.into(),
                    };
                    let res = client.media().get_media_content(&media_request, true).await;
                    // log!("Fetched avatar for {mxc_uri:?}, succeeded? {}", res.is_ok());
                    on_fetched(AvatarUpdate { mxc_uri, avatar_data: res.map(|v| v.into()) });
                });
            }

            MatrixRequest::FetchMedia { media_request, on_fetched, destination, update_sender } => {
                let Some(client) = CLIENT.get() else { continue };
                let media = client.media();

                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending fetch media request for {media_request:?}...");
                    let res = media.get_media_content(&media_request, true).await;
                    on_fetched(&destination, media_request, res, update_sender);
                });
            }

            MatrixRequest::SendMessage { room_id, message, replied_to } => {
                let timeline = {
                    let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get(&room_id) else {
                        log!("BUG: room info not found for send message request {room_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };

                // Spawn a new async task that will send the actual message.
                let _send_message_task = Handle::current().spawn(async move {
                    log!("Sending message to room {room_id}: {message:?}...");
                    if let Some(replied_to_info) = replied_to {
                        match timeline.send_reply(message.into(), replied_to_info, ForwardThread::Yes).await {
                            Ok(_send_handle) => log!("Sent reply message to room {room_id}."),
                            Err(_e) => error!("Failed to send reply message to room {room_id}: {_e:?}"),
                        }
                    } else {
                        match timeline.send(message.into()).await {
                            Ok(_send_handle) => log!("Sent message to room {room_id}."),
                            Err(_e) => error!("Failed to send message to room {room_id}: {_e:?}"),
                        }
                    }
                    SignalToUI::set_ui_signal();
                });
            }

            MatrixRequest::ReadReceipt { room_id, event_id }=>{
                let timeline = {
                    let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get(&room_id) else {
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
                });
            },

            MatrixRequest::FullyReadReceipt { room_id, event_id }=>{
                let timeline = {
                    let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get(&room_id) else {
                        log!("BUG: room info not found when sending fully read receipt, room {room_id}, {event_id}");
                        continue;
                    };
                    room_info.timeline.clone()
                };
                let _send_frr_task = Handle::current().spawn(async move {
                    let receipt = Receipts::new().fully_read_marker(event_id.clone());
                    match timeline.send_multiple_receipts(receipt).await {
                        Ok(()) => log!("Sent fully read receipt to room {room_id}, event {event_id}"),
                        Err(_e) => error!("Failed to send fully read receipt to room {room_id}, event {event_id}; error: {_e:?}"),
                    }
                });
            },

            MatrixRequest::CheckCanUserSendMessage { room_id } => {
                let (timeline, sender) = {
                    let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get(&room_id) else {
                        log!("BUG: room info not found for fetch members request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                let Some(user_id) = current_user_id() else { continue };

                let _check_can_user_send_message_task = Handle::current().spawn(async move {
                    let can_user_send_message = timeline.room().can_user_send_message(
                        &user_id,
                        matrix_sdk::ruma::events::MessageLikeEventType::Message
                    )
                    .await
                    .unwrap_or(true);

                    if let Err(e) = sender.send(TimelineUpdate::CanUserSendMessage(can_user_send_message)) {
                        error!("Failed to send the result of if user can send message: {e}")
                    }
                });
            }
        }
    }

    error!("async_worker task ended unexpectedly");
    bail!("async_worker task ended unexpectedly")
}


/// The single global Tokio runtime that is used by all async tasks.
static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// The sender used by [`submit_async_request`] to send requests to the async worker thread.
/// Currently there is only one, but it can be cloned if we need more concurrent senders.
static REQUEST_SENDER: OnceLock<UnboundedSender<MatrixRequest>> = OnceLock::new();


pub fn start_matrix_tokio() -> Result<()> {
    // Create a Tokio runtime, and save it in a static variable to ensure it isn't dropped.
    let rt = TOKIO_RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());

    // Create a channel to be used between UI thread(s) and the async worker thread.
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<MatrixRequest>();
    REQUEST_SENDER.set(sender).expect("BUG: REQUEST_SENDER already set!");

    let (login_sender, login_receiver) = tokio::sync::mpsc::channel(1);
    // Start a high-level async task that will start and monitor all other tasks.
    let _monitor = rt.spawn(async move {
        // Spawn the actual async worker thread.
        let mut worker_join_handle = rt.spawn(async_worker(receiver, login_sender));

        // Start the main loop that drives the Matrix client SDK.
        let mut main_loop_join_handle = rt.spawn(async_main_loop(login_receiver));

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
                            error!("BUG: async worker task ended unexpectedly!");
                        }
                        Ok(Err(e)) => {
                            error!("Error: async worker task ended:\n\t{e:?}");
                            rooms_list::enqueue_rooms_list_update(RoomsListUpdate::Status {
                                status: e.to_string(),
                            });
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

    Ok(())
}


/// A tokio::watch channel sender for sending requests from the RoomScreen UI widget
/// to the corresponding background async task for that room (its `timeline_subscriber_handler`).
pub type TimelineRequestSender = watch::Sender<Vec<BackwardsPaginateUntilEventRequest>>;


/// Info about a room that our client currently knows about.
struct RoomInfo {
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
impl Drop for RoomInfo {
    fn drop(&mut self) {
        log!("Dropping RoomInfo for room {}", self.room_id);
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

/// Information about all of the rooms we currently know about.
static ALL_ROOM_INFO: Mutex<BTreeMap<OwnedRoomId, RoomInfo>> = Mutex::new(BTreeMap::new());

/// Information about all of the rooms that have been tombstoned.
///
/// The map key is the **NEW** replacement room ID, and the value is the **OLD** tombstoned room ID.
/// This allows us to quickly query if a newly-encountered room is a replacement for an old tombstoned room.
static TOMBSTONED_ROOMS: Mutex<BTreeMap<OwnedRoomId, OwnedRoomId>> = Mutex::new(BTreeMap::new());

/// The logged-in Matrix client, which can be freely and cheaply cloned.
static CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_client() -> Option<Client> {
    CLIENT.get().cloned()
}

/// Returns the user ID of the currently logged-in user, if any.
pub fn current_user_id() -> Option<OwnedUserId> {
    CLIENT.get().and_then(|c|
        c.session_meta().map(|m| m.user_id.clone())
    )
}

/// The singleton sync service.
static SYNC_SERVICE: OnceLock<SyncService> = OnceLock::new();

pub fn get_sync_service() -> Option<&'static SyncService> {
    SYNC_SERVICE.get()
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


/// Returns three channel endpoints related to the timeline for the given room.
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
    ALL_ROOM_INFO.lock().unwrap()
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

async fn async_main_loop(
    mut login_receiver: Receiver<LoginRequest>,
) -> Result<()> {
    tracing_subscriber::fmt::init();

    let most_recent_user_id = persistent_state::most_recent_user_id();
    log!("Most recent user ID: {most_recent_user_id:?}");
    let cli_parse_result = Cli::try_parse();
    let cli_has_valid_username_password = cli_parse_result.as_ref()
        .is_ok_and(|cli| !cli.username.is_empty() && !cli.password.is_empty());
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
                &cli.username,
                cli.homeserver.as_deref(),
            )
        );
        log!("Trying to restore session for user: {:?}",
            specified_username.as_ref().or(most_recent_user_id.as_ref())
        );
        if let Ok(session) = persistent_state::restore_session(specified_username).await {
            Some(session)
        } else {
            let status_err = "Failed to restore previous user session. Please login again.";
            log!("{status_err}");
            Cx::post_action(LoginAction::Status(status_err.to_string()));

            if let Ok(cli) = &cli_parse_result {
                let status_str = format!("Attempting auto-login from CLI arguments as user '{}'...", cli.username);
                log!("{status_str}");
                Cx::post_action(LoginAction::Status(status_str));
                let mut login_types: Vec<LoginType> = Vec::new();
                let homeserver_url = cli.homeserver.as_deref().unwrap_or(DEFAULT_HOMESERVER);
                if let Err(e) = populate_login_types(homeserver_url, &mut login_types).await {
                    error!("Populating Login types failed: {e:?}");
                    Cx::post_action(LoginAction::LoginFailure(format!("Populating Login types failed {homeserver_url} {e:?}")));
                }
                match login(cli, LoginRequest::LoginByCli, &login_types).await {
                    Ok(new_login) => Some(new_login),
                    Err(e) => {
                        error!("CLI-based login failed: {e:?}");
                        Cx::post_action(LoginAction::LoginFailure(format!("Login failed: {e:?}")));
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
    } else {
        None
    };
    let cli: Cli = cli_parse_result.unwrap_or(Cli::default());
    let (client, _sync_token) = match new_login_opt {
        Some(new_login) => new_login,
        None => {
            let homeserver_url = cli.homeserver.as_deref().unwrap_or(DEFAULT_HOMESERVER);
            let mut login_types = Vec::new();
            // Display the available Identity providers by fetching the login types
            if let Err(e) = populate_login_types(homeserver_url, &mut login_types).await {
                error!("Populating Login types failed for {homeserver_url}: {e:?}");
                Cx::post_action(LoginAction::LoginFailure(format!("Populating Login types failed for {homeserver_url} {e:?}")));
            }
            loop {
                log!("Waiting for login request...");
                match login_receiver.recv().await {
                    Some(login_request) => {
                        if let LoginRequest::HomeserverLoginTypesQuery(homeserver_url) = login_request {
                            if let Err(e) = populate_login_types(&homeserver_url, &mut login_types).await {
                                error!("Populating Login types failed: {e:?}");
                                Cx::post_action(LoginAction::LoginFailure(format!("Populating Login types failed {homeserver_url} {e:?}")));
                            }
                            continue
                        }
                        match login(&cli, login_request, &login_types).await {
                            Ok((client, sync_token)) => {
                                break (client, sync_token);
                            }
                            Err(e) => {
                                error!("Login failed: {e:?}");
                                Cx::post_action(LoginAction::LoginFailure(format!("Login failed: {e:?}")));
                                enqueue_rooms_list_update(RoomsListUpdate::Status {
                                    status: format!("Login failed: {e:?}"),
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

    enqueue_rooms_list_update(RoomsListUpdate::Status {
        status: format!("Logged in as {}. Loading rooms...", client.user_id().unwrap()),
    });

    CLIENT.set(client.clone()).expect("BUG: CLIENT already set!");

    add_verification_event_handlers_and_sync_client(client.clone());

    // Listen for updates to the ignored user list.
    handle_ignore_user_list_subscriber(client.clone());

    let sync_service = SyncService::builder(client.clone())
        .build()
        .await?;
    handle_sync_service_state_subscriber(sync_service.state());
    sync_service.start().await;
    let room_list_service = sync_service.room_list_service();
    SYNC_SERVICE.set(sync_service).unwrap_or_else(|_| panic!("BUG: SYNC_SERVICE already set!"));

    let all_rooms_list = room_list_service.all_rooms().await?;
    handle_room_list_service_loading_state(all_rooms_list.loading_state());

    let (room_diff_stream, room_list_dynamic_entries_controller) =
        // TODO: paginate room list to avoid loading all rooms at once
        all_rooms_list.entries_with_dynamic_adapters(usize::MAX);

    room_list_dynamic_entries_controller.set_filter(
        Box::new(|_room| true),
    );

    const LOG_ROOM_LIST_DIFFS: bool = false;

    let mut all_known_rooms = Vector::new();
    pin_mut!(room_diff_stream);
    while let Some(batch) = room_diff_stream.next().await {
        let mut peekable_diffs = batch.into_iter().peekable();
        while let Some(diff) = peekable_diffs.next() {
            match diff {
                VectorDiff::Append { values: new_rooms } => {
                    let _num_new_rooms = new_rooms.len();
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Append {_num_new_rooms}"); }
                    for new_room in &new_rooms {
                        add_new_room(new_room).await?;
                    }
                    all_known_rooms.append(new_rooms);
                }
                VectorDiff::Clear => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Clear"); }
                    all_known_rooms.clear();
                    ALL_ROOM_INFO.lock().unwrap().clear();
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                }
                VectorDiff::PushFront { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushFront"); }
                    add_new_room(&new_room).await?;
                    all_known_rooms.push_front(new_room);
                }
                VectorDiff::PushBack { value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PushBack"); }
                    add_new_room(&new_room).await?;
                    all_known_rooms.push_back(new_room);
                }
                VectorDiff::PopFront => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopFront"); }
                    if let Some(room) = all_known_rooms.pop_front() {
                        if LOG_ROOM_LIST_DIFFS { log!("PopFront: removing {}", room.room_id()); }
                        remove_room(&room);
                    }
                }
                VectorDiff::PopBack => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopBack"); }
                    if let Some(room) = all_known_rooms.pop_back() {
                        if LOG_ROOM_LIST_DIFFS { log!("PopBack: removing {}", room.room_id()); }
                        remove_room(&room);
                    }
                }
                VectorDiff::Insert { index, value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Insert at {index}"); }
                    add_new_room(&new_room).await?;
                    all_known_rooms.insert(index, new_room);
                }
                VectorDiff::Set { index, value: changed_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Set at {index}"); }
                    let old_room = all_known_rooms.get(index).expect("BUG: Set index out of bounds");
                    update_room(old_room, &changed_room).await?;
                    all_known_rooms.set(index, changed_room);
                }
                VectorDiff::Remove { index: remove_index } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Remove at {remove_index}"); }
                    if remove_index < all_known_rooms.len() {
                        let room = all_known_rooms.remove(remove_index);
                        // Try to optimize a common operation, in which a `Remove` diff
                        // is immediately followed by an `Insert` diff for the same room,
                        // which happens frequently in order to "sort" the room list
                        // by changing its positional order.
                        // We treat this as a simple `Set` operation (`update_room()`),
                        // which is way more efficient.
                        let mut next_diff_was_handled = false;
                        if let Some(VectorDiff::Insert { index: insert_index, value: new_room }) = peekable_diffs.peek() {
                            if room.room_id() == new_room.room_id() {
                                if LOG_ROOM_LIST_DIFFS {
                                    log!("Optimizing Remove({remove_index}) + Insert({insert_index}) into Set (update) for room {}", room.room_id());
                                }
                                update_room(&room, new_room).await?;
                                all_known_rooms.insert(*insert_index, new_room.clone());
                                next_diff_was_handled = true;
                            }
                        }
                        if next_diff_was_handled {
                            peekable_diffs.next(); // consume the next diff
                        } else {
                            warning!("UNTESTED SCENARIO: room_list: diff Remove({remove_index}) was NOT followed by an Insert. Removed room: {}", room.room_id());
                            remove_room(&room);
                        }
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
                    // ALL_ROOM_INFO should already be empty due to successive calls to `remove_room()`,
                    // so this is just a sanity check.
                    ALL_ROOM_INFO.lock().unwrap().clear();
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                    for room in &new_rooms {
                        add_new_room(room).await?;
                    }
                    all_known_rooms = new_rooms;
                }
            }
        }
    }

    bail!("room list service sync loop ended unexpectedly")
}


/// Invoked when the room list service has received an update that changes an existing room.
async fn update_room(
    old_room: &room_list_service::Room,
    new_room: &room_list_service::Room,
) -> Result<()> {
    let new_room_id = new_room.room_id().to_owned();
    let mut room_avatar_changed = false;
    if old_room.room_id() == new_room_id {
        if let Some(new_latest_event) = new_room.latest_event().await {
            if let Some(old_latest_event) = old_room.latest_event().await {
                if new_latest_event.timestamp() > old_latest_event.timestamp() {
                    log!("Updating latest event for room {}", new_room_id);
                    room_avatar_changed = update_latest_event(new_room_id.clone(), &new_latest_event);
                }
            }
        }

        if room_avatar_changed || (old_room.avatar_url() != new_room.avatar_url()) {
            log!("Updating avatar for room {}", new_room_id);
            spawn_fetch_room_avatar(new_room.inner_room().clone());
        }

        if let Ok(new_room_name) = new_room.compute_display_name().await {
            let new_room_name = new_room_name.to_string();
            if old_room.cached_display_name().as_ref() != Some(&new_room_name) {
                log!("Updating room name for room {} to {}", new_room_id, new_room_name);
                enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: new_room_id.clone(),
                    new_room_name,
                });
            }
        }

        if let Ok(new_tags) = new_room.tags().await {
            enqueue_rooms_list_update(RoomsListUpdate::Tags {
                room_id: new_room_id.clone(),
                new_tags,
            });
        }
        Ok(())
    }
    else {
        warning!("UNTESTED SCENARIO: update_room(): removing old room {}, replacing with new room {}",
            old_room.room_id(), new_room_id,
        );
        remove_room(old_room);
        add_new_room(new_room).await
    }
}


/// Invoked when the room list service has received an update to remove an existing room.
fn remove_room(room: &room_list_service::Room) {
    ALL_ROOM_INFO.lock().unwrap().remove(room.room_id());
    enqueue_rooms_list_update(
        RoomsListUpdate::RemoveRoom(room.room_id().to_owned())
    );
}


/// Invoked when the room list service has received an update with a brand new room.
async fn add_new_room(room: &room_list_service::Room) -> Result<()> {
    let room_id = room.room_id().to_owned();

    // NOTE: the call to `sync_up()` never returns, so I'm not sure how to force a room to fully sync.
    //       I suspect that's the problem -- we can't get the room's tombstone event content because
    //       the room isn't fully synced yet. But I don't know how to force it to fully sync.
    //
    // if !room.is_state_fully_synced() {
    //     log!("Room {room_id} is not fully synced yet; waiting for sync_up...");
    //     room.sync_up().await;
    //     log!("Room {room_id} is now fully synced? {}", room.is_state_fully_synced());
    // }


    // Do not add tombstoned rooms to the rooms list; they require special handling.
    if let Some(tombstoned_info) = room.tombstone() {
        log!("Room {room_id} has been tombstoned: {tombstoned_info:#?}");
        // Since we don't know the order in which we'll learn about new rooms,
        // we need to first check to see if the replacement for this tombstoned room
        // refers to an already-known room as its replacement.
        // If so, we can immediately update the replacement room's room info
        // to indicate that it replaces this tombstoned room.
        let replacement_room_id = tombstoned_info.replacement_room;
        if let Some(room_info) = ALL_ROOM_INFO.lock().unwrap().get_mut(&replacement_room_id) {
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

    let timeline = if let Some(tl_arc) = room.timeline() {
        tl_arc
    } else {
        let builder = room.default_room_timeline_builder().await?
            .track_read_marker_and_receipts();
        room.init_timeline_with_builder(builder).await?;
        room.timeline().ok_or_else(|| anyhow::anyhow!("BUG: room timeline not found for room {room_id}"))?
    };
    let latest_event = timeline.latest_event().await;
    let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();

    let room_name = room.compute_display_name().await
        .map(|n| n.to_string())
        .ok();

    let (request_sender, request_receiver) = watch::channel(Vec::new());
    let timeline_subscriber_handler_task = Handle::current().spawn(timeline_subscriber_handler(
        room.inner_room().clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
        request_receiver,
    ));

    let latest = latest_event.as_ref().map(
        |ev| get_latest_event_details(ev, &room_id)
    );

    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddRoom(RoomsListEntry {
        room_id: room_id.clone(),
        latest,
        tags: room.tags().await.ok().flatten(),
        // start with a basic text avatar; the avatar image will be fetched asynchronously below.
        avatar: avatar_from_room_name(room_name.as_deref().unwrap_or_default()),
        room_name,
        has_been_paginated: false,
        is_selected: false,
    }));

    spawn_fetch_room_avatar(room.inner_room().clone());

    let tombstoned_room_replaced_by_this_room = TOMBSTONED_ROOMS.lock()
        .unwrap()
        .remove(&room_id);

    log!("Adding new room {room_id} to ALL_ROOM_INFO. Replaces tombstoned room: {tombstoned_room_replaced_by_this_room:?}");
    ALL_ROOM_INFO.lock().unwrap().insert(
        room_id.clone(),
        RoomInfo {
            room_id,
            timeline,
            timeline_singleton_endpoints: Some((timeline_update_receiver, request_sender)),
            timeline_update_sender,
            timeline_subscriber_handler_task,
            typing_notice_subscriber: None,
            replaces_tombstoned_room: tombstoned_room_replaced_by_this_room,
        },
    );
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


fn handle_sync_service_state_subscriber(mut subscriber: Subscriber<sync_service::State>) {
    log!("Initial sync service state is {:?}", subscriber.get());
    Handle::current().spawn(async move {
        while let Some(state) = subscriber.next().await {
            log!("Received a sync service state update: {state:?}");
            if state == sync_service::State::Error {
                log!("Restarting sync service due to error.");
                if let Some(ss) = SYNC_SERVICE.get() {
                    ss.start().await;
                }
            }
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
    let sender_username = match latest_event.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
        TimelineDetails::Unavailable => {
            if let Some(event_id) = latest_event.event_id() {
                submit_async_request(MatrixRequest::FetchDetailsForEvent {
                    room_id: room_id.clone(),
                    event_id: event_id.to_owned(),
                });
            }
            None
        }
        _ => None,
    }
    .unwrap_or_else(|| latest_event.sender().as_str());
    (
        latest_event.timestamp(),
        text_preview_of_timeline_item(latest_event.content(), sender_username)
            .format_with(sender_username),
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

const LOG_TIMELINE_DIFFS: bool = false;

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
    let (mut timeline_items, mut subscriber) = timeline.subscribe_batched().await;
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

                // Send a Makepad-level signal to update this room's timeline UI view.
                SignalToUI::set_ui_signal();

                // Update the latest event for this room.
                if let Some(new_latest) = new_latest_event {
                    if latest_event.as_ref().map_or(true, |ev| ev.timestamp() < new_latest.timestamp()) {
                        let room_avatar_changed = update_latest_event(room_id.clone(), &new_latest);
                        latest_event = Some(new_latest);
                        if room_avatar_changed {
                            spawn_fetch_room_avatar(room.clone());
                        }
                    }
                }
            }
        }

        else => {
            break;
        }
    } }

    error!("Error: unexpectedly ended timeline subscriber for room {room_id}.");
}


/// Updates the latest event for the given room.
///
/// This function handles room name changes and checks for (but does not directly handle)
/// room avatar changes.
///
/// Returns `true` if this latest event indicates that the room's avatar has changed
/// and should also be updated.
fn update_latest_event(
    room_id: OwnedRoomId,
    event_tl_item: &EventTimelineItem,
) -> bool {
    let mut room_avatar_changed = false;

    let (timestamp, latest_message_text) = get_latest_event_details(event_tl_item, &room_id);

    // Check for relevant state events: a changed room name or avatar.
    if let TimelineItemContent::OtherState(other) = event_tl_item.content() {
        match other.content() {
            AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: room_id.clone(),
                    new_room_name: content.name.clone(),
                });
            }
            AnyOtherFullStateEventContent::RoomAvatar(_avatar_event) => {
                room_avatar_changed = true;
            }
            AnyOtherFullStateEventContent::RoomPowerLevels(_power_level_event) => {
                submit_async_request(MatrixRequest::CheckCanUserSendMessage { room_id: room_id.clone() })
            }
            _ => { }
        }
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
    let room_id = room.room_id().to_owned();
    let room_name_str = room.cached_display_name().map(|dn| dn.to_string());
    Handle::current().spawn(async move {
        let avatar = room_avatar(&room, &room_name_str).await;
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomAvatar {
            room_id,
            avatar,
        });
    });
}

/// Fetches and returns the avatar image for the given room (if one exists),
/// otherwise returns a text avatar string of the first character of the room name.
async fn room_avatar(room: &Room, room_name: &Option<String>) -> RoomPreviewAvatar {
    match room.avatar(MEDIA_THUMBNAIL_FORMAT.into()).await {
        Ok(Some(avatar)) => RoomPreviewAvatar::Image(avatar),
        _ => avatar_from_room_name(room_name.as_deref().unwrap_or_default()),
    }
}

/// Returns a text avatar string containing the first character of the room name.
fn avatar_from_room_name(room_name: &str) -> RoomPreviewAvatar {
    RoomPreviewAvatar::Text(
        room_name
            .graphemes(true)
            .next()
            .map(ToString::to_string)
            .unwrap_or_default()
    )
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
    Cx::post_action(LoginAction::Status(String::from("Opening Browser ...")));
    let cli = Cli {
        homeserver: homeserver_url.is_empty().not().then_some(homeserver_url),
        ..Default::default()
    };
    Handle::current().spawn(async move {
        let Ok((client, client_session)) = build_client(&cli, app_data_dir()).await else {
            Cx::post_action(LoginAction::LoginFailure("Failed to establish client".to_string()));
            return;
        };
        let mut is_logged_in = false;
        match client
            .matrix_auth()
            .login_sso(|sso_url: String| async move {
                Uri::new(&sso_url).open().map_err(|err| {
                    Error::UnknownError(
                        Box::new(io::Error::new(
                            io::ErrorKind::Other,
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
                    if client.logged_in() {
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
                            "Logged in as {:?}. Loading rooms...",
                            &identity_provider_res.user_id
                        ),
                    });
                }
            }
            Err(e) => {
                if !is_logged_in {
                    error!("Login by SSO failed: {e:?}");
                    Cx::post_action(LoginAction::LoginFailure(format!("Login by SSO failed {e}")));
                }
            }
        }
        Cx::post_action(LoginAction::SsoPending(false));
    });
}
