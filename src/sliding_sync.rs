use anyhow::{Result, bail};
use clap::Parser;
use eyeball_im::VectorDiff;
use futures_util::{StreamExt, pin_mut};
use makepad_widgets::{SignalToUI, error, log};
use matrix_sdk::{
    Client,
    ruma::{
        assign,
        OwnedRoomId,
        api::client::{
            session::get_login_types::v3::LoginType,
            sync::sync_events::v4::{SyncRequestListFilters, self},
        },
        events::{StateEventType, FullStateEventContent, room::message::RoomMessageEventContent}, MilliSecondsSinceUnixEpoch, UInt,
    },
    SlidingSyncList,
    SlidingSyncMode,
    config::RequestConfig,
    media::MediaRequest,
    Room,
};
use matrix_sdk_ui::{timeline::{SlidingSyncRoomExt, PaginationOptions, BackPaginationStatus, TimelineItemContent, AnyOtherFullStateEventContent}, Timeline};
use tokio::{
    runtime::Handle,
    sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle,
};
use unicode_segmentation::UnicodeSegmentation;
use std::{cmp::min, collections::BTreeMap, sync::{Arc, Mutex, OnceLock}};
use url::Url;

use crate::{home::{rooms_list::{self, RoomPreviewEntry, RoomsListUpdate, RoomPreviewAvatar, enqueue_rooms_list_update}, room_screen::TimelineUpdate}, media_cache::MediaCacheEntry, utils::MEDIA_THUMBNAIL_FORMAT};
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
    let mut builder = Client::builder().homeserver_url(homeserver_url);

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
        batch_size: u16,
        max_events: u16,
    },
    /// Request to fetch profile information for all members of a room.
    /// This can be *very* slow depending on the number of members in the room.
    FetchRoomMembers {
        room_id: OwnedRoomId,
    },
    /// Request to fetch media from the server.
    /// Upon completion of the async media request, the `on_fetched` function
    /// will be invoked with the `destination` and the result of the media fetch.
    FetchMedia {
        media_request: MediaRequest,
        on_fetched: fn(&Mutex<MediaCacheEntry>, MediaRequest, matrix_sdk::Result<Vec<u8>>),
        destination: Arc<Mutex<MediaCacheEntry>>,
    },
    /// Request to send a message to the given room's timeline.
    SendMessage {
        room_id: OwnedRoomId,
        message: RoomMessageEventContent,
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
            MatrixRequest::PaginateRoomTimeline { room_id, batch_size, max_events: _max_events } => {
                let timeline = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        log!("BUG: room info not found for pagination request {room_id}");
                        continue;
                    };

                    let room_id2 = room_id.clone();
                    let timeline_ref = room_info.timeline.clone();
                    let sender = room_info.timeline_update_sender.clone();

                    if room_info.pagination_status_task.is_none() {
                        let mut back_pagination_subscriber = room_info.timeline.back_pagination_status();
                        room_info.pagination_status_task = Some(Handle::current().spawn( async move {
                            loop {
                                let status = back_pagination_subscriber.next().await;
                                log!("### Timeline {room_id2} back pagination status: {:?}", status);
                                match status {
                                    Some(BackPaginationStatus::Idle) => {
                                        sender.send(TimelineUpdate::PaginationIdle).unwrap();
                                        SignalToUI::set_ui_signal();
                                    }
                                    Some(BackPaginationStatus::TimelineStartReached) => {
                                        sender.send(TimelineUpdate::TimelineStartReached).unwrap();
                                        SignalToUI::set_ui_signal();
                                        break;
                                    }
                                    _ => { }
                                }
                            }
                        }));
                    }

                    // drop the lock on ALL_ROOM_INFO before spawning the actual pagination task.
                    timeline_ref
                };

                // Spawn a new async task that will make the actual pagination request.
                let _paginate_task = Handle::current().spawn(async move {
                    log!("Sending pagination request for room {room_id}...");
                    let res = timeline.paginate_backwards(
                        // PaginationOptions::simple_request(batch_size)
                        PaginationOptions::until_num_items(batch_size, _max_events)
                    ).await;
                    match res {
                        Ok(_) => log!("Completed pagination request for room {room_id}."),
                        Err(e) => error!("Error sending pagination request for room {room_id}: {e:?}"),
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

            MatrixRequest::FetchMedia { media_request, on_fetched, destination } => {
                let Some(client) = CLIENT.get() else { continue };
                let media = client.media();
                
                let _fetch_task = Handle::current().spawn(async move {
                    // log!("Sending fetch media request for {media_request:?}...");
                    let res = media.get_media_content(&media_request, true).await;
                    on_fetched(&destination, media_request, res);
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
                    timeline.send(message.into()).await;
                    log!("Sent message to room {room_id}.");
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

/// Returns the timeline update receiver for the given room, if one exists.
///
/// This will only succeed once per room, as only a single channel receiver can exist.
pub fn take_timeline_update_receiver(
    room_id: &OwnedRoomId,
) -> Option<crossbeam_channel::Receiver<TimelineUpdate>> {
    ALL_ROOM_INFO.lock().unwrap()
        .get_mut(room_id)
        .and_then(|ri| ri.timeline_update_receiver.take())
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

    let mut filter = SyncRequestListFilters::default();
    filter.not_room_types = vec!["m.space".into()]; // Ignore spaces for now.

    let visible_room_list_name = "VisibleRooms".to_owned();
    let visible_room_list = SlidingSyncList::builder(&visible_room_list_name)
        // Load the most recent rooms, one at a time
        .sort(vec!["by_recency".into()])
        .sync_mode(SlidingSyncMode::new_paging(5).maximum_number_of_rooms_to_fetch(50))
        // .sync_mode(SlidingSyncMode::new_growing(1))
        // only load a few timeline events per room to start with. We'll load more later on demand when a room is first viewed.
        .timeline_limit(20)
        .filters(Some(filter))
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
            assign!(v4::AccountDataConfig::default(), { enabled: Some(true) }),
        ) 
        // and the e2ee extension
        .with_e2ee_extension(assign!(v4::E2EEConfig::default(), { enabled: Some(true) })) 
        // and the to-device extension
        .with_to_device_extension(
            assign!(v4::ToDeviceConfig::default(), { enabled: Some(true) }),
        ) 
        // .add_cached_list(visible_room_list).await?
        .add_list(visible_room_list)
        .build()
        .await?;


    // let active_room_list_name = "ActiveRoom".to_owned();
    // let active_room_list = SlidingSyncList::builder(&active_room_list_name)
    //     .sort(vec!["by_recency".into()])
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
                room.display_name().await,
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
                continue
            };
            let Some(timeline) = ssroom.timeline().await else {
                error!("Error: couldn't get timeline for room {room_id:?} that had an update.");
                continue
            };
            
            let room_name = ssroom.name();
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
            if let Some(room_name) = room_name_changed {
                rooms_list::enqueue_rooms_list_update(RoomsListUpdate::UpdateRoomName {
                    room_id: room_id.clone(),
                    room_name,
                });
            }

            
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
                    avatar: avatar_from_room_name(room_name.as_deref().unwrap_or_default()),
                    room_name: room_name.clone(),
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
                    let avatar = room_avatar(&room, &room_name).await;
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


async fn timeline_subscriber_handler(
    room_id: OwnedRoomId,
    timeline: Arc<Timeline>,
    sender: crossbeam_channel::Sender<TimelineUpdate>,
) {
    log!("Starting timeline subscriber for room {room_id}...");
    let (mut timeline_items, mut subscriber) = timeline.subscribe_batched().await;

    sender.send(TimelineUpdate::NewItems {
        items: timeline_items.clone(),
        index_of_first_change: 0,
    }).expect("Error: timeline update sender couldn't send update with initial items!");

    const LOG_DIFFS: bool = false;

    while let Some(batch) = subscriber.next().await {
        let num_updates = batch.len();
        let mut index_of_first_change = usize::MAX;
        for diff in batch {
            match diff {
                VectorDiff::Append { values } => {
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    if LOG_DIFFS { log!("timeline_subscriber: diff Append {}, index_of_first_change: {index_of_first_change}", values.len()); }
                    timeline_items.extend(values);
                }
                VectorDiff::Clear => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Clear"); }
                    index_of_first_change = 0;
                    timeline_items.clear();
                }
                VectorDiff::PushFront { value } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PushFront"); }
                    index_of_first_change = 0;
                    timeline_items.push_front(value);
                }
                VectorDiff::PushBack { value } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PushBack"); }
                    index_of_first_change = min(index_of_first_change, timeline_items.len());
                    timeline_items.push_back(value);
                }
                VectorDiff::PopFront => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PopFront"); }
                    index_of_first_change = 0;
                    timeline_items.pop_front();
                }
                VectorDiff::PopBack => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff PopBack"); }
                    timeline_items.pop_back();
                    index_of_first_change = min(index_of_first_change, timeline_items.len().saturating_sub(1));
                }
                VectorDiff::Insert { index, value } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Insert at {index}"); }
                    index_of_first_change = min(index_of_first_change, index);
                    timeline_items.insert(index, value);
                }
                VectorDiff::Set { index, value } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Set at {index}"); }
                    index_of_first_change = min(index_of_first_change, index);
                    timeline_items.set(index, value);
                }
                VectorDiff::Remove { index } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Remove at {index}"); }
                    index_of_first_change = min(index_of_first_change, index.saturating_sub(1));
                    timeline_items.remove(index);
                }
                VectorDiff::Truncate { length } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Truncate to length {length}"); }
                    index_of_first_change = min(index_of_first_change, length.saturating_sub(1));
                    timeline_items.truncate(length);
                }
                VectorDiff::Reset { values } => {
                    if LOG_DIFFS { log!("timeline_subscriber: diff Reset, new length {}", values.len()); }
                    index_of_first_change = 0; // we must assume that all items have changed.
                    timeline_items = values;
                }
            }
        }
        if num_updates > 0 {
            log!("timeline_subscriber: applied {num_updates} updates for room {room_id}; first change at index {index_of_first_change}.");
        
            sender.send(TimelineUpdate::NewItems {
                items: timeline_items.clone(),
                index_of_first_change,
            }).expect("Error: timeline update sender couldn't send update with new items!");

            // Send a Makepad-level signal to update this room's timeline UI view.
            SignalToUI::set_ui_signal();
        }
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
