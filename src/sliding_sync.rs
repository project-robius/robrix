use anyhow::{Result, bail};
use clap::Parser;
use futures_util::{StreamExt, pin_mut};
use imbl::Vector;
use makepad_widgets::SignalToUI;
use matrix_sdk::{
    Client,
    ruma::{
        assign,
        OwnedRoomId,
        api::client::{
            media::get_content_thumbnail::v3::Method as ThumbnailMethod,
            session::get_login_types::v3::LoginType,
            sync::sync_events::v4::{SyncRequestListFilters, self},
        },
        events::{StateEventType, FullStateEventContent}, MilliSecondsSinceUnixEpoch, UInt,
    },
    SlidingSyncList,
    SlidingSyncMode,
    config::RequestConfig,
    media::{MediaFormat, MediaThumbnailSize}, Room,
};
use matrix_sdk_ui::{timeline::{SlidingSyncRoomExt, TimelineItem, PaginationOptions, BackPaginationStatus, TimelineItemContent, AnyOtherFullStateEventContent}, Timeline};
use tokio::{
    runtime::Handle,
    sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle,
};
use unicode_segmentation::UnicodeSegmentation;
use std::{sync::{OnceLock, Mutex, Arc}, collections::BTreeMap};
use url::Url;

use crate::home::{rooms_list::{self, RoomPreviewEntry, RoomListUpdate, RoomPreviewAvatar}, room_screen::TimelineUpdate};
use crate::message_display::DisplayerExt;


/// Returns the default thumbnail size (40x40 pixels, scaled) to use for media.
/// This is implemented as a function instead of a const because the internal `UInt` type
/// does not support const construction.
fn media_thumbnail_format() -> MediaFormat {
    MediaFormat::Thumbnail(MediaThumbnailSize {
        width: 40u8.into(),
        height: 40u8.into(),
        method: ThumbnailMethod::Scale,
    })
}


#[derive(Parser, Debug)]
struct Cli {
    /// The user name that should be used for the login.
    #[clap(value_parser)]
    user_name: String,
    
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

    // Use a 30 second timeout for all requests to the homeserver.
    builder = builder.request_config(
        RequestConfig::new()
            .timeout(std::time::Duration::from_secs(30))
    );

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
        .login_username(&cli.user_name, &cli.password)
        .initial_device_display_name("robrix-un-pw")
        .send()
        .await?;

    println!("Login result: {login_result:?}");

    if !client.logged_in() {
        bail!("Failed to login with username and password");
    }
    println!("Logged in successfully? {:?}", client.logged_in());
    
    Ok((client, _token))
}



/// The set of requests for async work that can be made to the worker thread.
pub enum MatrixRequest {
    /// Request to paginate (backwards fetch) the older events of a room's timeline.
    PaginateRoomTimeline {
        room_id: OwnedRoomId,
        batch_size: u16,
        max_events: u16,
    },
    FetchRoomMembers {
        room_id: OwnedRoomId,
    },
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
    println!("Started async_worker task.");
    
    while let Some(request) = receiver.recv().await {
        match request {
            MatrixRequest::PaginateRoomTimeline { room_id, batch_size, max_events: _max_events } => {
                let timeline = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        println!("BUG: room info not found for pagination request {room_id}");
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
                                println!("### Timeline {room_id2} back pagination status: {:?}", status);
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
                    println!("Sending pagination request for room {room_id}...");
                    let res = timeline.paginate_backwards(
                        // PaginationOptions::simple_request(batch_size)
                        PaginationOptions::until_num_items(batch_size, _max_events)
                    ).await;
                    match res {
                        Ok(_) => println!("Sent pagination request for room {room_id}"),
                        Err(e) => eprintln!("Error sending pagination request for room {room_id}: {e:?}"),
                    }
                });
            }

            MatrixRequest::FetchRoomMembers { room_id } => {
                let (timeline, sender) = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        println!("BUG: room info not found for fetch members request {room_id}");
                        continue;
                    };

                    (room_info.timeline.clone(), room_info.timeline_update_sender.clone())
                };

                // Spawn a new async task that will make the actual fetch request.
                let _fetch_task = Handle::current().spawn(async move {
                    println!("Sending fetch room members request for room {room_id}...");
                    timeline.fetch_members().await;
                    println!("Sent fetch room members request for room {room_id}");
                    sender.send(TimelineUpdate::RoomMembersFetched).unwrap();
                    SignalToUI::set_ui_signal();
                });
            }
        }
    }

    eprintln!("async_worker task ended unexpectedly");
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
                            eprintln!("BUG: main async loop task ended unexpectedly!");
                        }
                        Ok(Err(e)) => {
                            eprintln!("Main async loop task ended with error:\n --> {e:?}");
                            
                            // TODO: send error to rooms_list so it can be displayed.
                            
                        },
                        Err(e) => {
                            eprintln!("BUG: failed to join main async loop task: {e:?}");
                        }
                    }
                    break;
                }
                result = &mut worker_join_handle => {
                    match result {
                        Ok(Ok(())) => {
                            eprintln!("BUG: async worker task ended unexpectedly!");
                        }
                        Ok(Err(e)) => {
                            eprintln!("async worker task ended with error:\n --> {e:?}");
                            
                            // TODO: send a message to the UI layer so we can display a error dialog box or notification.

                        },
                        Err(e) => {
                            eprintln!("BUG: failed to join async worker task: {e:?}");
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
    /// The initial set of timeline items that were obtained from the server.
    timeline_initial_items: Vector<Arc<TimelineItem>>,
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


/// Returns the timeline update receiver for the given room, if one exists.
///
/// This will only succeed once per room, as only a single channel receiver can exist.
pub fn take_timeline_update_receiver(
    room_id: &OwnedRoomId,
) -> Option<(
    Vector<Arc<TimelineItem>>,
    crossbeam_channel::Receiver<TimelineUpdate>
)> {
    ALL_ROOM_INFO.lock().unwrap()
        .get_mut(room_id)
        .and_then(|ri| ri.timeline_update_receiver.take()
            .map(|receiver| (
                core::mem::take(&mut ri.timeline_initial_items),
                receiver
            )
        ))
}


async fn async_main_loop() -> Result<()> {
    tracing_subscriber::fmt::init();

    let start = std::time::Instant::now();

    let cli = Cli::parse();
    let (client, _token) = login(cli).await?;

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
            (StateEventType::RoomEncryption, "".to_owned()), // is it encrypted
            (StateEventType::RoomTopic,  "".to_owned()),     // any topic if known
            (StateEventType::RoomName,   "".to_owned()),     // the room's displayable name
            (StateEventType::RoomAvatar, "".to_owned()),     // avatar if set
            (StateEventType::RoomCreate, "".to_owned()),     // room creation type
        ]);

    // Now that we're logged in, try to connect to the sliding sync proxy.
    println!("Sliding sync proxy URL: {:?}", client.sliding_sync_proxy());
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

    loop {
        let update = match stream.next().await {
            Some(Ok(u)) => {
                let curr = start.elapsed().as_secs_f64();
                println!("{curr:>8.2} Received an update. Summary: {u:?}");
                // println!("    --> Current room list: {:?}", sliding_sync.get_all_rooms().await.into_iter().map(|r| r.room_id().to_owned()).collect::<Vec<_>>());
                u
            }
            Some(Err(e)) => {
                eprintln!("loop was stopped by client error processing: {e}");
                continue;
            }
            None => {
                eprintln!("Streaming loop ended unexpectedly");
                break;
            }
        };

        for room_id in update.rooms {
            let Some(room) = client.get_room(&room_id) else {
                eprintln!("Error: couldn't get Room {room_id:?} that had an update");
                continue
            };

            println!("\n{room_id:?} --> {:?} has an update
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
            // println!("    --> Subscribing to above room {:?}", room_id);

            let Some(ssroom) = sliding_sync.get_room(&room_id).await else {
                eprintln!("Error: couldn't get SlidingSyncRoom {room_id:?} that had an update.");
                continue
            };
            let Some(timeline) = ssroom.timeline().await else {
                eprintln!("Error: couldn't get timeline for room {room_id:?} that had an update.");
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
                rooms_list::enqueue_rooms_list_update(RoomListUpdate::UpdateLatestEvent {
                    room_id: room_id.clone(),
                    timestamp,
                    latest_message_text,
                });
            }

            // Send an update to the rooms_list if the room name has changed.
            if let Some(room_name) = room_name_changed {
                rooms_list::enqueue_rooms_list_update(RoomListUpdate::UpdateRoomName {
                    room_id: room_id.clone(),
                    room_name,
                });
            }

            
            // Handle an entirely new room that we haven't seen before.
            if !room_exists {
                // Indicate that we'll fetch the avatar for this new room later.
                room_avatar_changed = true;
                
                let (timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();
                let (timeline_initial_items, mut room_subscriber) = timeline.subscribe_batched().await;
                let tl_arc = Arc::new(timeline);
                let room_id2 = room_id.clone();
                let sender = timeline_update_sender.clone();

                Handle::current().spawn(async move {
                    println!("Starting timeline subscriber for room {room_id2}...");

                    while let Some(batched_update) = room_subscriber.next().await {
                        sender.send(TimelineUpdate::DiffBatch(batched_update))
                            .expect("Error: timeline update sender couldn't send batched update!");

                        // Send a Makepad-level signal to update this room's timeline UI view.
                        SignalToUI::set_ui_signal();
                    }

                    eprintln!("Error: unexpectedly ended timeline subscriber for room {room_id2}...");

                });
                
                rooms_list::enqueue_rooms_list_update(RoomListUpdate::AddRoom(RoomPreviewEntry {
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
                        timeline_initial_items,
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
                    rooms_list::enqueue_rooms_list_update(RoomListUpdate::UpdateRoomAvatar {
                        room_id,
                        avatar,
                    });
                });
            }
        }

        if !update.lists.is_empty() {
            println!("Lists have an update: {:?}", update.lists);
        }
    }

    bail!("unexpected return from async_main_loop!")
}



/// Fetches and returns the avatar image for the given room (if one exists),
/// otherwise returns a text avatar string of the first character of the room name.
async fn room_avatar(room: &Room, room_name: &Option<String>) -> RoomPreviewAvatar {
    match room.avatar(media_thumbnail_format()).await {
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
