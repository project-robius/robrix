use anyhow::{Result, bail};
use clap::Parser;
use eyeball::Subscriber;
use eyeball_im::VectorDiff;
use futures_util::StreamExt;
use makepad_widgets::{error, log, warning, Cx, SignalToUI};
use matrix_sdk::{
    config::RequestConfig,
    event_handler::EventHandlerDropGuard,
    media::MediaRequest,
    room::{Receipts, RoomMember},
    ruma::{
        api::client::{receipt::create_receipt::v3::ReceiptType, session::get_login_types::v3::LoginType},
        events::{
            receipt::ReceiptThread, room::{
                message::{ForwardThread, RoomMessageEventContent},
                MediaSource,
            }, FullStateEventContent
        },
        OwnedEventId, OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId, RoomId, UserId
    },
    sliding_sync::VersionBuilder,
    Client,
    Room,
};
use matrix_sdk_ui::{
    room_list_service::{self, RoomListLoadingState},
    sync_service::{self, SyncService},
    timeline::{AnyOtherFullStateEventContent, EventTimelineItem, RepliedToInfo, TimelineDetails, TimelineItemContent},
    Timeline,
};
use tokio::{
    runtime::Handle,
    sync::mpsc::{Sender, Receiver, UnboundedSender, UnboundedReceiver},
};
use unicode_segmentation::UnicodeSegmentation;
use std::{cmp::{max, min}, collections::{BTreeMap, BTreeSet}, path:: Path, sync::{Arc, Mutex, OnceLock}};

use crate::{
    app_data_dir, avatar_cache::AvatarUpdate, event_preview::text_preview_of_timeline_item, home::{
        room_screen::TimelineUpdate, rooms_list::{self, enqueue_rooms_list_update, RoomPreviewAvatar, RoomPreviewEntry, RoomsListUpdate}
    }, login::login_screen::LoginAction, media_cache::MediaCacheEntry, persistent_state::{self, ClientSessionPersisted}, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{enqueue_user_profile_update, UserProfileUpdate},
    }, utils::MEDIA_THUMBNAIL_FORMAT, verification::add_verification_event_handlers_and_sync_client
};
use crate::shared::popup_list::enqueue_popup_notification;

#[derive(Parser, Debug)]
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
impl From<LoginRequest> for Cli {
    fn from(login: LoginRequest) -> Self {
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


async fn login(cli: Cli) -> Result<(Client, Option<String>)> {
    let (client, client_session) = build_client(&cli, app_data_dir()).await?;

    // Query the server for supported login types.
    let login_kinds = client.matrix_auth().get_login_types().await?;
    if !login_kinds.flows.iter().any(|flow| matches!(flow, LoginType::Password(_))) {
        bail!("Homeserver does not support username + password login flow.");
    }

    // Attempt to login using the CLI-provided username & password.
    let login_result = client
        .matrix_auth()
        .login_username(&cli.username, &cli.password)
        .initial_device_display_name("robrix-un-pw")
        .send()
        .await?;

    log!("Login result: {login_result:?}");
    if client.logged_in() {    
        log!("Logged in successfully? {:?}", client.logged_in());
        enqueue_rooms_list_update(RoomsListUpdate::Status {
            status: format!("Logged in as {}. Loading rooms...", &cli.username),
        });
        enqueue_popup_notification(format!("Logged in as {}. Loading rooms...", &cli.username));
        if let Err(e) = persistent_state::save_session(
            &client,
            client_session,
        ).await {
            error!("Failed to save session state to storage: {e:?}");
        }
        Ok((client, None))
    } else {
        enqueue_rooms_list_update(RoomsListUpdate::Status {
            status: format!("Failed to login as {}: {:?}", &cli.username, login_result),
        });
        enqueue_popup_notification(format!("Failed to login as {}: {:?}", &cli.username, login_result));
        bail!("Failed to login as {}: {login_result:?}", &cli.username)
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


/// The set of requests for async work that can be made to the worker thread.
pub enum MatrixRequest {
    /// Request from the login screen to log in with the given credentials.
    Login(LoginRequest),
    /// Request to paginate (backwards fetch) the older events of a room's timeline.
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
        on_fetched: fn(&Mutex<MediaCacheEntry>, MediaRequest, matrix_sdk::Result<Vec<u8>>, Option<crossbeam_channel::Sender<TimelineUpdate>>),
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
    }
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    REQUEST_SENDER.get()
        .unwrap() // this is initialized
        .send(req)
        .expect("BUG: async worker task receiver has died!");
}


/// Information needed to log in to a Matrix homeserver.
pub struct LoginRequest {
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

                    if update.is_none() && !local_only {
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
                            enqueue_popup_notification(e.to_string());
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
                            enqueue_popup_notification(e.to_string());
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


/// Info about a room that our client currently knows about.
struct RoomInfo {
    #[allow(unused)]
    room_id: OwnedRoomId,
    /// A reference to this room's timeline of events.
    timeline: Arc<Timeline>,
    /// An instance of the clone-able sender that can be used to send updates to this room's timeline.
    timeline_update_sender: crossbeam_channel::Sender<TimelineUpdate>,
    /// The single receiver that can receive updates to this room's timeline.
    ///
    /// When a new room is joined, an unbounded crossbeam channel will be created
    /// and its sender given to a background task that enqueues timeline updates
    /// as vector diffs when they are received from the server.
    ///
    /// The UI thread can take ownership of these items  receiver in order for a specific
    /// timeline view (currently room_sccren) to receive and display updates to this room's timeline.
    timeline_update_receiver: Option<crossbeam_channel::Receiver<TimelineUpdate>>,
    /// A drop guard for the event handler that represents a subscription to typing notices for this room.
    typing_notice_subscriber: Option<EventHandlerDropGuard>,
}

/// Information about all of the rooms we currently know about.
static ALL_ROOM_INFO: Mutex<BTreeMap<OwnedRoomId, RoomInfo>> = Mutex::new(BTreeMap::new());

/// The logged-in Matrix client, which can be freely and cheaply cloned.
static CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_client() -> Option<Client> {
    CLIENT.get().cloned()
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


/// Returns the timeline update sender and receiver endpoints for the given room,
/// if and only if the receiver exists.
///
/// This will only succeed once per room, as only a single channel receiver can exist.
pub fn take_timeline_update_receiver(
    room_id: &OwnedRoomId,
) -> Option<(
        crossbeam_channel::Sender<TimelineUpdate>,
        crossbeam_channel::Receiver<TimelineUpdate>,
    )>
{
    ALL_ROOM_INFO.lock().unwrap()
        .get_mut(room_id)
        .and_then(|ri| ri.timeline_update_receiver.take()
            .map(|receiver| (ri.timeline_update_sender.clone(), receiver))
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
    log!("CLI parsing succeeded? {}", cli_parse_result.is_ok());
    let wait_for_login = most_recent_user_id.is_none()
        || std::env::args().any(|arg| arg == "--login-screen" || arg == "--force-login");
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
        if let Some(session) = persistent_state::restore_session(specified_username).await.ok() {
            Some(session)
        } else {
            let status_err = "Error: failed to restore previous user session. Please login again.";
            log!("{status_err}");
            Cx::post_action(LoginAction::Status(status_err.to_string()));

            if let Ok(cli) = cli_parse_result {
                let status_str = format!("Attempting auto-login from CLI arguments as user {}...", cli.username);
                log!("{status_str}");
                Cx::post_action(LoginAction::Status(status_str));

                match login(cli).await {
                    Ok(new_login) => Some(new_login),
                    Err(e) => {
                        error!("CLI-based login failed: {e:?}");
                        Cx::post_action(LoginAction::LoginFailure(e.to_string()));
                        enqueue_rooms_list_update(RoomsListUpdate::Status {
                            status: e.to_string(),
                        });
                        enqueue_popup_notification(e.to_string());
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

    let (client, _sync_token) = match new_login_opt {
        Some(new_login) => new_login,
        None => loop {
            log!("Waiting for login request...");
            if let Some(login_request) = login_receiver.recv().await {
                log!("Received login request for user {}", login_request.user_id);
                match login(Cli::from(login_request)).await {
                    Ok(new_login) => break new_login,
                    Err(e) => {
                        error!("Login failed: {e:?}");
                        Cx::post_action(LoginAction::LoginFailure(e.to_string()));
                        enqueue_rooms_list_update(RoomsListUpdate::Status {
                            status: e.to_string(),
                        });
                        enqueue_popup_notification(e.to_string());
                    }
                }
            } else {
                error!("BUG: login_receiver hung up unexpectedly");
                return Err(anyhow::anyhow!("BUG: login_receiver hung up unexpectedly"));
            }
        }
    };

    Cx::post_action(LoginAction::LoginSuccess);

    enqueue_rooms_list_update(RoomsListUpdate::Status {
        status: format!("Logged in as {}. Loading rooms...", client.user_id().unwrap()),
    });
    enqueue_popup_notification(format!("Logged in as {}. Loading rooms...", client.user_id().unwrap()));

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
    let (mut all_known_rooms, mut room_diff_stream) = all_rooms_list.entries();
    log!("Populating initial set of {} known rooms.", all_known_rooms.len());
    for room in all_known_rooms.iter() {
        add_new_room(room).await?;
    }

    const LOG_ROOM_LIST_DIFFS: bool = true;

    while let Some(batch) = room_diff_stream.next().await {
        for diff in batch {
            match diff {
                VectorDiff::Append { values: new_rooms } => {
                    let _num_new_rooms = new_rooms.len();
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Append {_num_new_rooms}"); }
                    for new_room in &new_rooms {
                        add_new_room(&new_room).await?;
                    }
                    all_known_rooms.append(new_rooms);
                }
                VectorDiff::Clear => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Clear"); }
                    all_known_rooms.clear();
                    ALL_ROOM_INFO.lock().unwrap().clear();
                    // TODO: we probably need to remove each room individually to kill off
                    //       all of the async tasks associated with them (i.e., the timeline subscriber).
                    //       Or, better yet, implement the drop handler for RoomInfo to do so.
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
                        remove_room(room);
                    }
                }
                VectorDiff::PopBack => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff PopBack"); }
                    if let Some(room) = all_known_rooms.pop_back() {
                        remove_room(room);
                    }
                }
                VectorDiff::Insert { index, value: new_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Insert at {index}"); }
                    add_new_room(&new_room).await?;
                    all_known_rooms.insert(index, new_room);
                }
                VectorDiff::Set { index, value: changed_room } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Set at {index} !!!!!!!!"); }
                    update_room(&changed_room).await?;
                    all_known_rooms.set(index, changed_room);
                }
                VectorDiff::Remove { index } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Remove at {index}"); }
                    if index < all_known_rooms.len() {
                        let room = all_known_rooms.remove(index);
                        remove_room(room);
                    } else {
                        error!("BUG: room_list: diff Remove index {index} out of bounds, len {}", all_known_rooms.len());
                    }
                }
                VectorDiff::Truncate { length } => {
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Truncate to {length}"); }
                    while all_known_rooms.len() > length {
                        if let Some(room) = all_known_rooms.pop_back() {
                            remove_room(room);
                        }
                    }
                    all_known_rooms.truncate(length); // sanity check
                }
                VectorDiff::Reset { values } => {
                    // We implement this by clearing all rooms and then adding back the new values.
                    if LOG_ROOM_LIST_DIFFS { log!("room_list: diff Reset, new length {}", values.len()); }
                    all_known_rooms = values;
                    ALL_ROOM_INFO.lock().unwrap().clear();
                    // TODO: we probably need to remove each room individually to kill off
                    //       all of the async tasks associated with them (i.e., the timeline subscriber).
                    //       Or, better yet, implement the drop handler for RoomInfo to do so.
                    enqueue_rooms_list_update(RoomsListUpdate::ClearRooms);
                    for room in &all_known_rooms {
                        add_new_room(&room).await?;
                    }
                }
            }
        }
    }

    bail!("room list service sync loop ended unexpectedly")
}


/// Invoked when the room list service has received an update that changes an existing room.
async fn update_room(_room: &room_list_service::Room) -> matrix_sdk::Result<()> {
    todo!("update_room")
}


/// Invoked when the room list service has received an update to remove an existing room.
fn remove_room(room: room_list_service::Room) {
    ALL_ROOM_INFO.lock().unwrap().remove(room.room_id());
    enqueue_rooms_list_update(
        RoomsListUpdate::RemoveRoom(room.room_id().to_owned())
    );
    // TODO: we probably need to kill all of the async tasks associated
    //       with this room (i.e., the timeline subscriber. etc).
    //       Or, better yet, implement `RoomInfo::drop()` to do so.
}


/// Invoked when the room list service has received an update with a brand new room.
async fn add_new_room(room: &room_list_service::Room) -> Result<()> {
    let room_id = room.room_id().to_owned();

    log!("Adding new room: {:?}, room_id: {room_id}", room.compute_display_name().await.map(|n| n.to_string()).unwrap_or_default());

    let timeline = {
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

    Handle::current().spawn(timeline_subscriber_handler(
        room.inner_room().clone(),
        timeline.clone(),
        timeline_update_sender.clone(),
    ));

    let latest = latest_event.as_ref().map(|ev| {
        let sender_username = match ev.sender_profile() {
            TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
            TimelineDetails::Unavailable => {
                if let Some(event_id) = ev.event_id() {
                    submit_async_request(MatrixRequest::FetchDetailsForEvent {
                        room_id: room_id.clone(),
                        event_id: event_id.to_owned(),
                    });
                }
                None
            }
            _ => None,
        }.unwrap_or_else(|| ev.sender().as_str());
        (
            ev.timestamp(),
            text_preview_of_timeline_item(ev.content(), sender_username)
                .format_with(sender_username),
        )
    });

    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddRoom(RoomPreviewEntry {
        room_id: room_id.clone(),
        latest,
        // start with a basic text avatar; the avatar image will be fetched asynchronously below.
        avatar: avatar_from_room_name(room_name.as_deref().unwrap_or_default()),
        room_name,
        has_been_paginated: false,
        is_selected: false,
    }));

    spawn_fetch_room_avatar(room.inner_room().clone());

    ALL_ROOM_INFO.lock().unwrap().insert(
        room_id.clone(),
        RoomInfo {
            room_id,
            timeline,
            timeline_update_receiver: Some(timeline_update_receiver),
            timeline_update_sender,
            typing_notice_subscriber: None,
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


const LOG_TIMELINE_DIFFS: bool = false;

async fn timeline_subscriber_handler(
    room: Room,
    timeline: Arc<Timeline>,
    sender: crossbeam_channel::Sender<TimelineUpdate>,
) {
    let room_id = room.room_id().to_owned();
    log!("Starting timeline subscriber for room {room_id}...");
    let (mut timeline_items, mut subscriber) = timeline.subscribe_batched().await;
    log!("Received initial timeline update of {} items for room {room_id}.", timeline_items.len());

    sender.send(TimelineUpdate::NewItems {
        new_items: timeline_items.clone(),
        changed_indices: usize::MIN..usize::MAX,
        clear_cache: true,
    }).unwrap_or_else(
        |_e| panic!("Error: timeline update sender couldn't send update to room {room_id} with initial items!")
    );

    let mut latest_event = timeline.latest_event().await;

    while let Some(batch) = subscriber.next().await {
        let mut num_updates = 0;
        // For now we always requery the latest event, but this can be better optimized.
        let mut reobtain_latest_event = true;
        let mut index_of_first_change = usize::MAX;
        let mut index_of_last_change = usize::MIN;
        let mut clear_cache = false; // whether to clear the entire cache of items
        for diff in batch {
            match diff {
                VectorDiff::Append { values } => {
                    let _values_len = values.len();
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    timeline_items.extend(values);
                    index_of_last_change = max(index_of_last_change, timeline_items.len());
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Append {_values_len}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::Clear => {
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Clear"); }
                    clear_cache = true;
                    timeline_items.clear();
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::PushFront { value } => {
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PushFront"); }
                    clear_cache = true;
                    timeline_items.push_front(value);
                    reobtain_latest_event |= latest_event.is_none();
                    num_updates += 1;
                }
                VectorDiff::PushBack { value } => {
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    timeline_items.push_back(value);
                    index_of_last_change = max(index_of_last_change, timeline_items.len());
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PushBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::PopFront => {
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PopFront"); }
                    clear_cache = true;
                    timeline_items.pop_front();
                    // This doesn't affect whether we should reobtain the latest event.
                    num_updates += 1;
                }
                VectorDiff::PopBack => {
                    timeline_items.pop_back();
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    index_of_last_change = usize::MAX;
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff PopBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::Insert { index, value } => {
                    if index == 0 {
                        clear_cache = true;
                    } else {
                        index_of_first_change = min(index_of_first_change, index);
                        index_of_last_change = usize::MAX;
                    }
                    timeline_items.insert(index, value);
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Insert at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::Set { index, value } => {
                    index_of_first_change = min(index_of_first_change, index);
                    index_of_last_change  = max(index_of_last_change, index.saturating_add(1));
                    timeline_items.set(index, value);
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Set at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
                }
                VectorDiff::Remove { index } => {
                    if index == 0 {
                        clear_cache = true;
                    } else {
                        index_of_first_change = min(index_of_first_change, index.saturating_sub(1));
                        index_of_last_change = usize::MAX;
                    }
                    timeline_items.remove(index);
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Remove at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    reobtain_latest_event = true;
                    num_updates += 1;
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
                    num_updates += 1;
                }
                VectorDiff::Reset { values } => {
                    if LOG_TIMELINE_DIFFS { log!("timeline_subscriber: room {room_id} diff Reset, new length {}", values.len()); }
                    clear_cache = true; // we must assume all items have changed.
                    timeline_items = values;
                    reobtain_latest_event = true;
                    num_updates += 1;
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
                log!("timeline_subscriber: applied {num_updates} updates for room {room_id}, timeline now has {} items. Clear cache? {clear_cache}. Changes: {changed_indices:?}.", timeline_items.len());
            }
            sender.send(TimelineUpdate::NewItems {
                new_items: timeline_items.clone(),
                changed_indices,
                clear_cache,
            }).expect("Error: timeline update sender couldn't send update with new items!");
        
            // Send a Makepad-level signal to update this room's timeline UI view.
            SignalToUI::set_ui_signal();
        
            // Update the latest event for this room.
            if let Some(new_latest) = new_latest_event {
                if latest_event.as_ref().map_or(true, |ev| ev.timestamp() < new_latest.timestamp()) {
                    let room_avatar_changed = update_latest_event(&room_id, &new_latest);
                    latest_event = Some(new_latest);
                    if room_avatar_changed {
                        spawn_fetch_room_avatar(room.clone());
                    }
                }
            }
        }
    }

    error!("Error: unexpectedly ended timeline subscriber for room {room_id}.");
}


/// Updates the latest event for the given room.
///
/// Returns `true` if this latest event indicates that the room's avatar has changed
/// and should also be updated.
fn update_latest_event(
    room_id: &RoomId,
    event_tl_item: &EventTimelineItem,
) -> bool {
    let mut room_avatar_changed = false;

    let latest_event_sender_username = match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.as_deref(),
        TimelineDetails::Unavailable => {
            if let Some(event_id) = event_tl_item.event_id() {
                submit_async_request(MatrixRequest::FetchDetailsForEvent {
                    room_id: room_id.to_owned(),
                    event_id: event_id.to_owned(),
                });
            }
            None
        }
        _ => None,
    }
    .unwrap_or_else(|| event_tl_item.sender().as_str());

    let latest_message_text = text_preview_of_timeline_item(
        event_tl_item.content(),
        latest_event_sender_username,
    ).format_with(latest_event_sender_username);

    // Check for relevant state events: a changed room name or avatar.
    match event_tl_item.content() {
        TimelineItemContent::OtherState(other) => match other.content() {
            AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: room_id.to_owned(),
                    new_room_name: content.name.clone(),
                });
            }
            AnyOtherFullStateEventContent::RoomAvatar(_avatar_event) => {
                room_avatar_changed = true;
            }
            _ => { }
        }
        _ => { }
    }
    enqueue_rooms_list_update(RoomsListUpdate::UpdateLatestEvent {
        room_id: room_id.to_owned(),
        timestamp: event_tl_item.timestamp(),
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
