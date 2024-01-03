use anyhow::{Result, bail};
use clap::Parser;
use eyeball_im::VectorDiff;
use futures_util::{StreamExt, pin_mut};
use imbl::Vector;
use makepad_widgets::Signal;
use matrix_sdk::{
    Client,
    ruma::{
        assign,
        OwnedRoomId,
        api::client::{
            media::get_content_thumbnail::v3::Method as ThumbnailMethod,
            sync::sync_events::v4::{SyncRequestListFilters, self},
        },
        events::StateEventType,
    },
    SlidingSyncList,
    SlidingSyncMode,
    config::RequestConfig,
    media::{MediaFormat, MediaThumbnailSize},
};
use matrix_sdk_ui::{timeline::{SlidingSyncRoomExt, TimelineItem, PaginationOptions}, Timeline};
use tokio::{
    runtime::Handle,
    sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle,
};
use unicode_segmentation::UnicodeSegmentation;
use std::{sync::{OnceLock, Mutex, Arc}, collections::BTreeMap};
use url::Url;

use crate::home::rooms_list::{self, RoomPreviewEntry, RoomListUpdate, RoomPreviewAvatar};
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
        .unwrap_or("https://matrix.org");
    let mut builder = Client::builder().homeserver_url(homeserver_url);

    if let Some(proxy) = cli.proxy {
        builder = builder.proxy(proxy);
    }

    // Use a 10 second timeout for all requests to the homeserver.
    builder = builder.request_config(
        RequestConfig::new()
            .timeout(std::time::Duration::from_secs(10))
    );

    let client = builder.build().await?;

    let mut _token = None;

    // If the `user_name` and `password` CLI arguments were provided, try to log in.
    client
        .matrix_auth()
        .login_username(&cli.user_name, &cli.password)
        .initial_device_display_name("robrix-un-pw")
        .await?;

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
    println!("async_worker task started, receiver {:?}", receiver);
    while let Some(request) = receiver.recv().await {
        match request {
            MatrixRequest::PaginateRoomTimeline { room_id, batch_size, max_events: _max_events } => {
                let timeline = {
                    let mut all_room_info = ALL_ROOM_INFO.lock().unwrap();
                    let Some(room_info) = all_room_info.get_mut(&room_id) else {
                        println!("BUG: room info not found for PaginationRequest {room_id}");
                        continue;
                    };

                    let room_id2 = room_id.clone();
                    let timeline_ref2 = room_info.timeline.clone();
                    let timeline_ref = room_info.timeline.clone();

                    if room_info.pagination_status_task.is_none() {
                        let mut back_pagination_subscriber = room_info.timeline.back_pagination_status();
                        room_info.pagination_status_task = Some(Handle::current().spawn( async move {
                            loop {
                                let status = back_pagination_subscriber.next().await;
                                println!("### Timeline {room_id2} back pagination status: {:?}", status);
                                
                                // TODO FIXME: send Makepad-level event/action to update this room's timeline UI view.

                                if status == Some(matrix_sdk_ui::timeline::BackPaginationStatus::TimelineStartReached) {
                                    break;
                                }
                            }
                        }));
                    }

                    // drop the lock on ALL_ROOM_INFO before spawning the actual pagination task.
                    timeline_ref2
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
        }
    }

    eprintln!("async_worker task ended unexpectedly");
    panic!("async_worker task ended unexpectedly");
    // bail!("async_worker task ended unexpectedly")
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
    let _worker = rt.spawn(async_worker(receiver));

    // Start the main loop that drives the Matrix client SDK.
    let _m = rt.spawn(async_main_loop());
    Ok(())
}


/// Info about a room that our client currently knows about.
struct RoomInfo {
    room_id: OwnedRoomId,
    timeline: Arc<Timeline>,
    /// The sender that can be used to send updates to this room's timeline.
    ///
    /// Currently, a clone of this sender is owned by the background task that is spawned
    /// when a new room is first discovered, so we don't actually need to use this sender
    /// for anything. We just keep it around in case we need it later, e.g., if that
    /// background task crashes or something or we need to send a timeline update from elsewhere.
    _timeline_update_sender: crossbeam_channel::Sender<Vec<VectorDiff<Arc<TimelineItem>>>>,
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
    timeline_update_receiver: Option<crossbeam_channel::Receiver<Vec<VectorDiff<Arc<TimelineItem>>>>>,
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
    crossbeam_channel::Receiver<Vec<VectorDiff<Arc<TimelineItem>>>>
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
            (StateEventType::RoomTopic, "".to_owned()),      // any topic if known
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

    /*
     *
    // subscribe to the list APIs for updates
    let ((_, list_state_stream), list_count_stream, (_, list_stream)) = sliding_sync.on_list(&active_room_list_name, |list| {
        std::future::ready((list.state_stream(), list.maximum_number_of_rooms_stream(), list.room_list_stream()))
    }).await.unwrap();

    tokio::spawn(async move {
        pin_mut!(list_state_stream);
        while let Some(new_state) = list_state_stream.next().await {
            println!("### active-list switched state to {new_state:?}");
        }
    });

    tokio::spawn(async move {
        pin_mut!(list_count_stream);
        while let Some(new_count) = list_count_stream.next().await {
            println!("### active-list new count: {new_count:?}");
        }
    });

    tokio::spawn(async move {
        pin_mut!(list_stream);
        while let Some(v_diff) = list_stream.next().await {
            println!("### active-list room list diff update: {v_diff:?}");
        }
    });
    *
    */

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
            
            let room_exists = {
                if let Some(_existing) = ALL_ROOM_INFO.lock().unwrap().get_mut(&room_id) {
                    true
                } else {
                    false
                }
            };
            
            if !room_exists {
                let room_name = ssroom.name();
                let latest_tl = timeline.latest_event().await;
                
                let avatar = match room.avatar(media_thumbnail_format()).await {
                    Ok(Some(avatar)) => RoomPreviewAvatar::Image(avatar),
                    _ => RoomPreviewAvatar::Text(
                        room_name.as_ref()
                            .and_then(|name| name.graphemes(true).next().map(ToString::to_string))
                            .unwrap_or_default()
                    ),
                };
                
                let (_timeline_update_sender, timeline_update_receiver) = crossbeam_channel::unbounded();
                let (timeline_initial_items, mut room_subscriber) = timeline.subscribe_batched().await;
                let tl_arc = Arc::new(timeline);
                let tl_arc2 = Arc::clone(&tl_arc);
                let room_id2 = room_id.clone();
                let sender = _timeline_update_sender.clone();

                Handle::current().spawn(async move {
                    println!("Starting timeline subscriber for room {room_id2}...");

                    while let Some(batched_update) = room_subscriber.next().await {
                        sender.send(batched_update)
                            .expect("Error: timeline update sender couldn't send batched update!");

                        // Send a Makepad-level signal to update this room's timeline UI view.
                        // TODO: should we use an Action or Trigger instead? Signals are too simple for this purpose.
                        Signal::set_ui_signal();
                    }

                    println!("Starting timeline subscriber for room {room_id2}...");

                });

                ALL_ROOM_INFO.lock().unwrap().insert(
                    room_id.clone(),
                    RoomInfo {
                        room_id: room_id.clone(),
                        timeline: tl_arc,
                        pagination_status_task: None,
                        timeline_update_receiver: Some(timeline_update_receiver),
                        _timeline_update_sender,
                        timeline_initial_items,
                    },
                );

                rooms_list::update_rooms_list(RoomListUpdate::AddRoom(RoomPreviewEntry {
                    room_id: Some(room_id.clone()),
                    latest: latest_tl.map(|ev| (ev.timestamp(), ev.text_preview().to_string())),
                    avatar,
                    room_name,
                }));

                // now that we've updated the room list, signal the UI to refresh.
                Signal::set_ui_signal();
            }
        }

        if !update.lists.is_empty() {
            println!("Lists have an update: {:?}", update.lists);
        }
    }

    bail!("unexpected return from async_main_loop!")
}

