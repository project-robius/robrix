use anyhow::{Result, bail};
use clap::Parser;
use eyeball_im::VectorDiff;
use futures_util::{StreamExt, pin_mut};
use imbl::Vector;
use makepad_widgets::{error, log, SignalToUI};
use matrix_sdk::{
    config::RequestConfig, media::MediaRequest, room::RoomMember, ruma::{
        api::client::session::get_login_types::v3::LoginType, assign, events::{room::message::RoomMessageEventContent, FullStateEventContent, StateEventType}, MilliSecondsSinceUnixEpoch, OwnedMxcUri, OwnedRoomAliasId, OwnedRoomId, OwnedUserId, UInt
    }, sliding_sync::http::request::{AccountData, E2EE, ListFilters, ToDevice}, Client, Room, SlidingSyncList, SlidingSyncMode
};
use matrix_sdk_ui::{timeline::{AnyOtherFullStateEventContent, LiveBackPaginationStatus, TimelineItem, TimelineItemContent}, Timeline};
use tokio::{
    runtime::Handle,
    sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle,
};
use unicode_segmentation::UnicodeSegmentation;
use std::{cmp::{max, min}, collections::BTreeMap, ops::Range, sync::{Arc, Mutex, OnceLock}};
use url::Url;

use crate::{home::{room_screen::TimelineUpdate, rooms_list::{self, enqueue_rooms_list_update, RoomPreviewAvatar, RoomPreviewEntry, RoomsListUpdate}}, media_cache::{MediaCacheEntry, AVATAR_CACHE}, profile::user_profile::{enqueue_user_profile_update, UserProfile, UserProfileUpdate}, utils::MEDIA_THUMBNAIL_FORMAT};
use crate::message_display::DisplayerExt;


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
    homeserver: Option<Url>,

    /// Set the proxy that should be used for the connection.
    #[clap(short, long)]
    proxy: Option<Url>,

    /// Enable verbose logging output.
    #[clap(short, long, action)]
    verbose: bool,
}


async fn login(cli: Cli) -> Result<(Client, Option<String>)> {
    // Note that when encryption is enabled, you should use a persistent store to be
    // able to restore the session with a working encryption setup.
    // See the `persist_session` example.
    let homeserver_url = cli.homeserver.as_ref()
        .map(|h| h.as_str())
        .unwrap_or("https://matrix-client.matrix.org/");
        // .unwrap_or("https://matrix.org/");
    let mut builder = Client::builder()
        .homeserver_url(homeserver_url)
        // The matrix homeserver's sliding sync proxy doesn't support Simplified MSC3575.
        .simplified_sliding_sync(false);

    if let Some(proxy) = cli.proxy {
        builder = builder.proxy(proxy);
    }

    // Use a 60 second timeout for all requests to the homeserver.
    // Yes, this is a long timeout, but the standard matrix homeserver is often very slow.
    builder = builder.request_config(
        RequestConfig::new()
            .timeout(std::time::Duration::from_secs(60))
    );

    builder = builder.handle_refresh_tokens();

    let client = builder.build().await?;

    let mut _token = None;

    // Query the server for supported login types.
    let login_kinds = client.matrix_auth().get_login_types().await?;
    if !login_kinds.flows.iter().any(|flow| matches!(flow, LoginType::Password(_))) {
        bail!("Server does not support username + password login flow.");
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
        Ok((client, _token))
    } else {
        enqueue_rooms_list_update(RoomsListUpdate::Status {
            status: format!("Failed to login as {}: {:?}", &cli.username, login_result),
        });
        bail!("Failed to login as {}: {login_result:?}", &cli.username)
    }
}



/// The set of requests for async work that can be made to the worker thread.
pub enum MatrixRequest {
    /// Request to paginate (backwards fetch) the older events of a room's timeline.
    PaginateRoomTimeline {
        room_id: OwnedRoomId,
        /// The maximum number of timeline events to fetch in each pagination batch.
        num_events: u16,
        /// Which "direction" to paginate in:
        /// * `true`: paginate forwards to retrieve later events (towards the end of the timeline),
        ///    which only works if the timeline is *focused* on a specific event.
        /// * `false`: (default) paginate backwards to fill in earlier events (towards the start of the timeline),
        ///    which works if the timeline is in either live mode and focused mode.
        forwards: bool,
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
    },
    /// Sends a notice to the given room that the current user is or is not typing.
    ///
    /// This request does not return a response or notify the UI thread, and
    /// furthermore, there is no need to send a follow-up request to stop typing
    /// (though you certainly can do so).
    SendTypingNotice {
        room_id: OwnedRoomId,
        typing: bool,
    }
}

/// Submits a request to the worker thread to be executed asynchronously.
pub fn submit_async_request(req: MatrixRequest) {
    REQUEST_SENDER.get()
        .unwrap() // this is initialized 
        .send(req)
        .expect("BUG: async worker task receiver has died!");
}


/// The entry point for an async worker thread that can run async tasks.
///
/// All this thread does is wait for [`MatrixRequests`] from the main UI-driven non-async thread(s)
/// and then executes them within an async runtime context.
async fn async_worker(mut receiver: UnboundedReceiver<MatrixRequest>) -> Result<()> {
    log!("Started async_worker task.");
    
    while let Some(request) = receiver.recv().await {
        match request {
            MatrixRequest::PaginateRoomTimeline { room_id, num_events, forwards } => {
                let timeline = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        log!("BUG: room info not found for pagination request {room_id}");
                        continue;
                    };

                    let room_id2 = room_id.clone();
                    let timeline_ref = room_info.timeline.clone();
                    let timeline_ref2 = timeline_ref.clone();
                    let sender = room_info.timeline_update_sender.clone();

                    // If the back-pagination status task is finished or doesn't exist, spawn a new one,
                    // but only if the timeline is not already fully paginated.
                    let should_spawn_pagination_status_task = match room_info.pagination_status_task.as_ref() {
                        Some(t) => t.is_finished(),
                        None => true,
                    };
                    if should_spawn_pagination_status_task {
                        room_info.pagination_status_task = Some(Handle::current().spawn( async move {
                            if let Some((pagination_status, mut pagination_stream)) = timeline_ref2.live_back_pagination_status().await {
                                if !matches!(pagination_status, LiveBackPaginationStatus::Idle { hit_start_of_timeline: true }) {
                                    while let Some(status) = pagination_stream.next().await {
                                        log!("### Timeline {room_id2} back pagination status: {:?}", status);
                                        match status {
                                            LiveBackPaginationStatus::Idle { hit_start_of_timeline: false } => {
                                                sender.send(TimelineUpdate::PaginationIdle).unwrap();
                                                SignalToUI::set_ui_signal();
                                            }
                                            LiveBackPaginationStatus::Idle { hit_start_of_timeline: true } => {
                                                sender.send(TimelineUpdate::TimelineStartReached).unwrap();
                                                SignalToUI::set_ui_signal();
                                                break;
                                            }
                                            _ => { }
                                        }
                                    }
                                }
                            }
                        }));
                    }

                    // drop the lock on ALL_ROOM_INFO before spawning the actual pagination task.
                    timeline_ref
                };

                // Spawn a new async task that will make the actual pagination request.
                let _paginate_task = Handle::current().spawn(async move {
                    let direction = if forwards { "forwards" } else { "backwards" };
                    log!("Sending {direction} pagination request for room {room_id}...");
                    let res = if forwards {
                        timeline.focused_paginate_forwards(num_events).await
                    } else {
                        timeline.paginate_backwards(num_events).await
                    };
                    match res {
                        Ok(_hit_start_or_end) => log!(
                            "Completed {direction} pagination request for room {room_id}, hit {} of timeline? {}",
                            if forwards { "end" } else { "start" },
                            if _hit_start_or_end { "yes" } else { "no" },
                        ),
                        Err(e) => error!("Error sending {direction} pagination request for room {room_id}: {e:?}"),
                    }
                });
            }

            MatrixRequest::FetchRoomMembers { room_id } => {
                let (timeline, sender) = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
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
                    let mut avatar_url: Option<OwnedMxcUri> = None;
                    let mut update = None;

                    if let Some(room_id) = room_id.as_ref() {
                        if let Some(room) = client.get_room(room_id) {
                            let member = if local_only {
                                room.get_member_no_sync(&user_id).await
                            } else {
                                room.get_member(&user_id).await
                            };
                            if let Ok(Some(room_member)) = member {
                                avatar_url = room_member.avatar_url().map(|u| u.to_owned());
                                update = Some(UserProfileUpdate::Full {
                                    new_profile: UserProfile {
                                        username: room_member.display_name().map(|u| u.to_owned()),
                                        user_id: user_id.clone(),
                                        avatar_img_data: None, // will be fetched below
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
                            avatar_url = response.avatar_url;
                            update = Some(UserProfileUpdate::UserProfileOnly(
                                UserProfile {
                                    username: response.displayname,
                                    user_id: user_id.clone(),
                                    avatar_img_data: None, // will be fetched below
                                }
                            ));
                        } else {
                            log!("User profile request: client could not get user with ID {user_id}");
                        }
                    }

                    if let Some(mut upd) = update {
                        log!("Successfully completed get user profile request: user: {user_id}, room: {room_id:?}, local_only: {local_only}.");
                        if let Some(uri) = avatar_url {
                            match upd {
                                UserProfileUpdate::UserProfileOnly(ref mut new_profile)
                                | UserProfileUpdate::Full { ref mut new_profile, .. } => {
                                    new_profile.avatar_img_data = AVATAR_CACHE
                                        .get_media_or_fetch_async(client, uri, None)
                                        .await;
                                }
                                _ => { }
                            }
                        }
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
                    // has changed, i.e., the user has been (un)ignored).
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
                    // and all other rooms will be re-paginated in on-demand when they are viewed.
                    submit_async_request(MatrixRequest::PaginateRoomTimeline {
                        room_id,
                        num_events: 50,
                        forwards: false,
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

            MatrixRequest::ResolveRoomAlias(room_alias) => {
                let Some(client) = CLIENT.get() else { continue };
                let _resolve_task = Handle::current().spawn(async move {
                    log!("Sending resolve room alias request for {room_alias}...");
                    let res = client.resolve_room_alias(&room_alias).await;
                    log!("Resolved room alias {room_alias} to: {res:?}");
                    todo!("Send the resolved room alias back to the UI thread somehow.");
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

            MatrixRequest::SendMessage { room_id, message } => {
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
                    match timeline.send(message.into()).await {
                        Ok(_send_handle) => log!("Sent message to room {room_id}."),
                        Err(_e) => error!("Failed to send message to room {room_id}: {_e:?}"),
                    }
                    SignalToUI::set_ui_signal();
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
    
    // Start a high-level async task that will start and monitor all other tasks.
    let _monitor = rt.spawn(async move {
        // Spawn the actual async worker thread.
        let mut worker_join_handle = rt.spawn(async_worker(receiver));

        // Start the main loop that drives the Matrix client SDK.
        let mut main_loop_join_handle = rt.spawn(async_main_loop());

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


/// Info about a room that our client currently knows about.
struct RoomInfo {
    #[allow(unused)]
    room_id: OwnedRoomId,
    /// The timestamp of the latest event that we've seen for this room.
    latest_event_timestamp: MilliSecondsSinceUnixEpoch,
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
    /// The async task that is subscribed to the timeline's back-pagination status.
    pagination_status_task: Option<JoinHandle<()>>,
}

/// Information about all of the rooms we currently know about.
static ALL_ROOM_INFO: Mutex<BTreeMap<OwnedRoomId, RoomInfo>> = Mutex::new(BTreeMap::new());

/// The logged-in Matrix client, which can be freely and cheaply cloned.
static CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_client() -> Option<Client> {
    CLIENT.get().cloned()
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


async fn async_main_loop() -> Result<()> {
    tracing_subscriber::fmt::init();

    let start = std::time::Instant::now();

    let cli = Cli::try_parse().ok().or_else(|| {
        // Quickly try to parse the username and password fields from "login.toml".
        let login_file = std::include_str!("../login.toml");
        let mut username = None;
        let mut password = None;
        for line in login_file.lines() {
            if line.starts_with("username") {
                username = line.find('=')
                    .and_then(|i| line.get((i + 1) ..))
                    .map(|s| s.trim().trim_matches('"').trim().to_string());
            }
            if line.starts_with("password") {
                password = line.find('=')
                    .and_then(|i| line.get((i + 1) ..))
                    .map(|s| s.trim().trim_matches('"').trim().to_string());
            }
            if username.is_some() && password.is_some() {
                break;
            }
        }
        if let (Some(username), Some(password)) = (username, password) {
            if username.is_empty() || password.is_empty() {
                None
            } else {
                log!("Parsed username: {username:?} and password.");
                Some(Cli {
                    username,
                    password,
                    homeserver: None,
                    proxy: None,
                    verbose: false,
                })
            }
        } else {
            log!("Failed to parse username and password from \"login.toml\".");
            None
        }
    });
    
    let Some(cli) = cli else {
        enqueue_rooms_list_update(RoomsListUpdate::Status {
            status: String::from("Error: missing username and password in 'login.toml' file. \
                Please provide a valid username and password in 'login.toml' and rebuild the app."
            ),
        });
        loop { } // nothing else we can do right now
    };
    enqueue_rooms_list_update(RoomsListUpdate::Status {
        status: format!("Logging in as {}...", &cli.username)
    });

    let (client, _token) = login(cli).await?;
    CLIENT.set(client.clone()).expect("BUG: CLIENT already set!");

    // Listen for updates to the ignored user list.
    handle_ignore_user_list_subscriber(client.clone());

    let mut filters = ListFilters::default();
    filters.not_room_types = vec!["m.space".into()]; // Ignore spaces for now.

    let visible_room_list_name = "VisibleRooms".to_owned();
    let visible_room_list = SlidingSyncList::builder(&visible_room_list_name)
        .sync_mode(SlidingSyncMode::new_paging(5).maximum_number_of_rooms_to_fetch(50))
        // .sync_mode(SlidingSyncMode::new_growing(1))
        // only load a few timeline events per room to start with. We'll load more later on demand when a room is first viewed.
        .timeline_limit(20)
        .filters(Some(filters))
        .required_state(vec![ // we want to know immediately:
            (StateEventType::RoomEncryption, "".to_owned()),  // is it encrypted
            (StateEventType::RoomMember, "$LAZY".to_owned()), // lazily fetch room member profiles for users that have sent events
            (StateEventType::RoomMember, "$ME".to_owned()),   // fetch profile for "me", the currently logged-in user (optional)
            (StateEventType::RoomCreate, "".to_owned()),      // room creation type
            (StateEventType::RoomName,   "".to_owned()),      // the room's displayable name
            (StateEventType::RoomAvatar, "".to_owned()),      // avatar if set
            // (StateEventType::RoomTopic,  "".to_owned()),      // any topic if known (optional, can be fetched later)
        ]);

    // Now that we're logged in, try to connect to the sliding sync proxy.
    log!("Sliding sync proxy URL: {:?}", client.sliding_sync_proxy());
    let sliding_sync = client
        .sliding_sync("main-sync")?
        .sliding_sync_proxy("https://slidingsync.lab.matrix.org".try_into()?)
        // .with_all_extensions()
        // we enable the account-data extension
        .with_account_data_extension(
            assign!(AccountData::default(), { enabled: Some(true) }),
        )
        // and the e2ee extension
        .with_e2ee_extension(assign!(E2EE::default(), { enabled: Some(true) }))
        // and the to-device extension
        .with_to_device_extension(
            assign!(ToDevice::default(), { enabled: Some(true) }),
        )
        // .add_cached_list(visible_room_list).await?
        .add_list(visible_room_list)
        .build()
        .await?;


    // let active_room_list_name = "ActiveRoom".to_owned();
    // let active_room_list = SlidingSyncList::builder(&active_room_list_name)
    //     .sync_mode(SlidingSyncMode::new_paging(10))
    //     .timeline_limit(u32::MAX)
    //     .required_state(vec![ // we want to know immediately:
    //         (StateEventType::RoomEncryption, "".to_owned()), // is it encrypted
    //         (StateEventType::RoomTopic, "".to_owned()),      // any topic if known
    //         (StateEventType::RoomAvatar, "".to_owned()),     // avatar if set
    //     ]);
    // sliding_sync.add_cached_list(active_room_list).await?;


    let stream = sliding_sync.sync();
    pin_mut!(stream);

    let mut stream_error = None;
    loop {
        let update = match stream.next().await {
            Some(Ok(u)) => {
                let curr = start.elapsed().as_secs_f64();
                log!("{curr:>8.2} Received an update. Summary: {u:?}");
                // log!("    --> Current room list: {:?}", sliding_sync.get_all_rooms().await.into_iter().map(|r| r.room_id().to_owned()).collect::<Vec<_>>());
                u
            }
            Some(Err(e)) => {
                error!("sync loop was stopped by client error processing: {e}");
                stream_error = Some(e);
                continue;
            }
            None => {
                error!("sync loop ended unexpectedly");
                break;
            }
        };

        for room_id in update.rooms {
            let Some(room) = client.get_room(&room_id) else {
                error!("Error: couldn't get Room {room_id:?} that had an update");
                continue
            };
            let room_name = room.compute_display_name().await;

            log!("\n{room_id:?} --> {:?} has an update
                display_name: {:?},
                topic: {:?},
                is_synced: {:?}, is_state_fully_synced: {:?},
                is_space: {:?},
                create: {:?},
                canonical-alias: {:?},
                alt_aliases: {:?},
                guest_access: {:?},
                history_visibility: {:?},
                is_public: {:?},
                join_rule: {:?},
                latest_event: {:?}
                ",
                room.name(),
                room_name,
                room.topic(),
                room.is_synced(), room.is_state_fully_synced(),
                room.is_space(),
                room.create_content(),
                room.canonical_alias(),
                room.alt_aliases(),
                room.guest_access(),
                room.history_visibility(),
                room.is_public(),
                room.join_rule(),
                room.latest_event(),
            );

            // sliding_sync.subscribe_to_room(room_id.to_owned(), None);
            // log!("    --> Subscribing to above room {:?}", room_id);

            let Some(ssroom) = sliding_sync.get_room(&room_id).await else {
                error!("Error: couldn't get SlidingSyncRoom {room_id:?} that had an update.");
                continue;
            };

            // TODO: when the event cache handles its own cache, we can remove this.
            client
                .event_cache()
                .add_initial_events(
                    &room_id,
                    ssroom.timeline_queue().iter().cloned().collect(),
                    ssroom.prev_batch(),
                )
                .await?;

            let timeline = Timeline::builder(&room)
                .track_read_marker_and_receipts()
                .build()
                .await?;

            let latest_tl = timeline.latest_event().await;

            let mut room_exists = false;
            let mut room_name_changed = None;
            let mut latest_event_changed = None;
            let mut room_avatar_changed = false;

            if let Some(existing) = ALL_ROOM_INFO.lock().unwrap().get_mut(&room_id) {
                room_exists = true;

                // Obtain the details of any changes to this room based on this its latest timeline event.
                if let Some(event_tl_item) = &latest_tl {
                    let timestamp = event_tl_item.timestamp();
                    if timestamp > existing.latest_event_timestamp {
                        existing.latest_event_timestamp = timestamp;
                        latest_event_changed = Some((timestamp, event_tl_item.text_preview().to_string()));

                        match event_tl_item.content() {
                            TimelineItemContent::OtherState(other) => match other.content() {
                                AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
                                    room_name_changed = Some(content.name.clone());
                                }
                                AnyOtherFullStateEventContent::RoomAvatar(_avatar_event) => {
                                    room_avatar_changed = true; // we'll fetch the avatar later.
                                }
                                _ => { }
                            }
                            _ => { }
                        }
                    }
                }
            }

            // Send an update to the rooms_list if the latest event for this room has changed.
            if let Some((timestamp, latest_message_text)) = latest_event_changed {
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateLatestEvent {
                    room_id: room_id.clone(),
                    timestamp,
                    latest_message_text,
                });
            }

            // Send an update to the rooms_list if the room name has changed.
            if let Some(new_room_name) = room_name_changed {
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: room_id.clone(),
                    new_room_name,
                });
            }

            let room_name_str = room_name.ok().map(|n| n.to_string());
            
            // Handle an entirely new room that we haven't seen before.
            if !room_exists {
                // Indicate that we'll fetch the avatar for this new room later.
                room_avatar_changed = true;
                
                let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();
                let tl_arc = Arc::new(timeline);

                Handle::current().spawn(timeline_subscriber_handler(
                    room_id.clone(),
                    tl_arc.clone(),
                    timeline_update_sender.clone(),
                ));
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddRoom(RoomPreviewEntry {
                    room_id: Some(room_id.clone()),
                    latest: latest_tl.as_ref().map(|ev| (ev.timestamp(), ev.text_preview().to_string())),
                    avatar: avatar_from_room_name(room_name_str.as_deref().unwrap_or_default()),
                    room_name: room_name_str.clone(),
                }));

                ALL_ROOM_INFO.lock().unwrap().insert(
                    room_id.clone(),
                    RoomInfo {
                        room_id: room_id.clone(),
                        latest_event_timestamp: latest_tl.as_ref()
                            .map(|ev| ev.timestamp())
                            .unwrap_or(MilliSecondsSinceUnixEpoch(UInt::MIN)),
                        timeline: tl_arc,
                        pagination_status_task: None,
                        timeline_update_receiver: Some(timeline_update_receiver),
                        timeline_update_sender,
                    },
                );
            }

            // Send an update to the rooms_list if the room avatar has changed,
            // which is done within an async task because we need to fetch the avatar from the server.
            // This must be done here because we can't hold the lock on ALL_ROOM_INFO across an `.await` call,
            // and we can't send an `UpdateRoomAvatar` update until after the `AddRoom` update has been sent.
            if room_avatar_changed {
                Handle::current().spawn(async move {
                    let avatar = room_avatar(&room, &room_name_str).await;
                    rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomAvatar {
                        room_id,
                        avatar,
                    });
                });
            }
        }

        if !update.lists.is_empty() {
            log!("Lists have an update: {:?}", update.lists);
        }
    }

    if let Some(e) = stream_error {
        bail!(e)
    } else {
        bail!("sync loop ended unexpectedly")
    }
}


fn handle_ignore_user_list_subscriber(client: Client) {
    let mut subscriber = client.subscribe_to_ignore_user_list_changes();
    Handle::current().spawn(async move {
        while let Some(_ignore_list) = subscriber.next().await {
            log!("Received an updated ignored-user list: {_ignore_list:?}");

            // After successfully (un)ignoring a user, all timelines are fully cleared by the Matrix SDK.
            // Therefore, we need to re-fetch all timelines for all rooms,
            // and currently the only way to actually accomplish this is via pagination.
            // See: <https://github.com/matrix-org/matrix-rust-sdk/issues/1703#issuecomment-2250297923>
            for joined_room in client.joined_rooms() {
                submit_async_request(MatrixRequest::PaginateRoomTimeline {
                    room_id: joined_room.room_id().to_owned(),
                    num_events: 50,
                    forwards: false,
                });
            }
        }
    });
}


async fn timeline_subscriber_handler(
    room_id: OwnedRoomId,
    timeline: Arc<Timeline>,
    sender: crossbeam_channel::Sender<TimelineUpdate>,
) {
    log!("Starting timeline subscriber for room {room_id}...");
    let (mut timeline_items, mut subscriber) = timeline.subscribe_batched().await;
    log!("Received initial timeline update for room {room_id}.");

    sender.send(TimelineUpdate::NewItems {
        items: timeline_items.clone(),
        changed_indices: usize::MIN..usize::MAX,
        clear_cache: true,
    }).expect("Error: timeline update sender couldn't send update with initial items!");

    let send_update = |timeline_items: Vector<Arc<TimelineItem>>, changed_indices: Range<usize>, clear_cache: bool, num_updates: usize| {
        if num_updates > 0 {
            // log!("timeline_subscriber: applied {num_updates} updates for room {room_id}, timeline now has {} items. Clear cache? {clear_cache}. Changes: {changed_indices:?}.", timeline_items.len());
            sender.send(TimelineUpdate::NewItems {
                items: timeline_items,
                changed_indices,
                clear_cache,
            }).expect("Error: timeline update sender couldn't send update with new items!");
            
            // Send a Makepad-level signal to update this room's timeline UI view.
            SignalToUI::set_ui_signal();
        }
    };

    const LOG_DIFFS: bool = false;

    while let Some(batch) = subscriber.next().await {
        let mut num_updates = 0;
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
                    if LOG_DIFFS { log!("timeline_subscriber: diff Append {_values_len}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    num_updates += 1;
                }
                VectorDiff::Clear => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Clear"); }
                    clear_cache = true;
                    timeline_items.clear();
                    num_updates += 1;
                }
                VectorDiff::PushFront { value } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PushFront"); }
                    clear_cache = true;
                    timeline_items.push_front(value);
                    num_updates += 1;
                }
                VectorDiff::PushBack { value } => {
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    timeline_items.push_back(value);
                    index_of_last_change = max(index_of_last_change, timeline_items.len());
                    if LOG_DIFFS { log!("timeline_subscriber: diff PushBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    num_updates += 1;
                }
                VectorDiff::PopFront => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PopFront"); }
                    clear_cache = true;
                    timeline_items.pop_front();
                    num_updates += 1;
                }
                VectorDiff::PopBack => {
                    timeline_items.pop_back();
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    index_of_last_change = usize::MAX;
                    if LOG_DIFFS { log!("timeline_subscriber: diff PopBack. Changes: {index_of_first_change}..{index_of_last_change}"); }
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
                    if LOG_DIFFS { log!("timeline_subscriber: diff Insert at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    num_updates += 1;
                }
                VectorDiff::Set { index, value } => {
                    index_of_first_change = min(index_of_first_change, index);
                    index_of_last_change  = max(index_of_last_change, index.saturating_add(1));
                    timeline_items.set(index, value);
                    if LOG_DIFFS { log!("timeline_subscriber: diff Set at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
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
                    if LOG_DIFFS { log!("timeline_subscriber: diff Remove at {index}. Changes: {index_of_first_change}..{index_of_last_change}"); }
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
                    if LOG_DIFFS { log!("timeline_subscriber: diff Truncate to length {length}. Changes: {index_of_first_change}..{index_of_last_change}"); }
                    num_updates += 1;
                }
                VectorDiff::Reset { values } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Reset, new length {}", values.len()); }
                    clear_cache = true; // we must assume all items have changed.
                    timeline_items = values;
                    num_updates += 1;
                }
            }
        }
        send_update(timeline_items.clone(), index_of_first_change..index_of_last_change, clear_cache, num_updates);
    }

    error!("Error: unexpectedly ended timeline subscriber for room {room_id}.");
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
