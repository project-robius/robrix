use anyhow::{Result, bail};
use clap::Parser;
use futures_util::{StreamExt, pin_mut};
use imbl::Vector;
use matrix_sdk::{
    Client,
    ruma::{
        assign,
        OwnedRoomId,
        api::client::sync::sync_events::v4::{SyncRequestListFilters, self},
        events::StateEventType,
    }, SlidingSyncList, SlidingSyncMode
};
use matrix_sdk_ui::{timeline::{SlidingSyncRoomExt, TimelineItem, PaginationOptions}, Timeline};
use tokio::runtime::Handle;
use std::{sync::{OnceLock, Mutex, Arc}, collections::BTreeMap};
use url::Url;


#[derive(Parser, Debug)]
struct Cli {
    /// The room id that we should listen for the,
    #[clap(value_parser)]
    room_id: OwnedRoomId,

    /// The user name that should be used for the login.
    #[clap(value_parser)]
    user_name: Option<String>,
    
    /// The password that should be used for the login.
    #[clap(value_parser)]
    password: Option<String>,

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

    let client = builder.build().await?;

    let mut _token = None;

    // If the `user_name` and `password` CLI arguments were provided, try to log in.
    if let (Some(ref un), Some(ref pw)) = (cli.user_name, cli.password) {
        client
            .matrix_auth()
            .login_username(un, pw)
            .initial_device_display_name("robrix-un-pw")
            .await?;
    } 
    // If not, attempt to register for a guest account.
    else {
        bail!("No username and password provided");
    }

    if !client.logged_in() {
        bail!("Failed to login with username and password");
    }
    println!("Logged in successfully? {:?}", client.logged_in());
    
    Ok((client, _token))
}

static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();



pub fn start_matrix_tokio() -> Result<()> {
    // Save the tokio runtime in a static variable to ensure it isn't dropped.
    let rt = TOKIO_RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    rt.spawn(async_main());
    Ok(())
}

/// A temp hacky way to expose full timeline data obtained from the sliding sync proxy.
static ROOM_TIMELINES: Mutex<BTreeMap<OwnedRoomId, (Arc<Timeline>, Vector<Arc<TimelineItem>>)>> = Mutex::new(BTreeMap::new());

pub static CHOSEN_ROOM: OnceLock<OwnedRoomId> = OnceLock::new();

/// A temp hacky way to get the Timeline and list of TimelineItems for a room (in a non-async context).
pub fn get_timeline_items(
    room_id: &OwnedRoomId,
) -> Option<(Arc<Timeline>, Vector<Arc<TimelineItem>>)> {
    ROOM_TIMELINES.lock().unwrap().get(room_id).cloned()
}

async fn async_main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let start = std::time::Instant::now();

    let cli = Cli::parse();
    let room_of_interest = cli.room_id.clone();
    CHOSEN_ROOM.set(room_of_interest.clone()).unwrap();
    let (client, _token) = login(cli).await?;

    let mut filter = SyncRequestListFilters::default();
    filter.not_room_types = vec!["m.space".into()]; // Note: this is what Element-X does to ignore spaces initially
    // filter.room_name_like = Some("testing".into()); // temp: only care about robius-testing room for now

    let visible_room_list_name = "VisibleRooms".to_owned();
    let visible_room_list = SlidingSyncList::builder(&visible_room_list_name)
        // Load the most recent rooms, one at a time
        .sort(vec!["by_recency".into()])
        .sync_mode(SlidingSyncMode::new_growing(1))
        // only load one timeline event per room
        .timeline_limit(10)
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
        if !update.rooms.is_empty() {
            println!("Rooms have an update: {:?}", update.rooms);
            for room_id in &update.rooms {
                if let Some(room) = client.get_room(room_id) {
                    println!("\n{room_id:?} --> {:?}
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
                        latest_event: {:?},",
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


                    let ssroom = sliding_sync.get_room(room_id).await.unwrap();
                    let timeline = ssroom.timeline().await;
                    // println!("    --> SlidingSync room: {:?}, timeline: {:#?}", ssroom, timeline);
                    if let Some(timeline) = timeline {
                        let items = timeline.items().await;
                        let timeline_ref = {
                            let mut room_timelines = ROOM_TIMELINES.lock().unwrap();
                            match room_timelines.entry(room_id.to_owned()) {
                                std::collections::btree_map::Entry::Occupied(mut entry) => {
                                    println!("    --> Updating existing timeline for room {room_id:?}, now has {} items.", items.len(), room_id = room_id);
                                    let entry_mut = entry.get_mut();
                                    entry_mut.1 = items;
                                    entry_mut.0.clone()
                                }
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    println!("    --> Adding new timeline for room {room_id:?}, now has {} items.", items.len(), room_id = room_id);
                                    let tl_arc = Arc::new(timeline);
                                    entry.insert((Arc::clone(&tl_arc), items));
                                    tl_arc
                                }
                            }
                        };

                        // Fetch more events from the room's timeline backwards, in the background.
                        if room_id == &room_of_interest {
                            let room = room_of_interest.clone();
                            // println!("Timeline items: {:#?}", items);
                            Handle::current().spawn(async move {
                                println!("    --> Timeline room {room:?} before pagination had {} items", timeline_ref.items().await.len());

                                if true {
                                    let mut back_pagination_subscriber = timeline_ref.back_pagination_status();
                                    Handle::current().spawn( async move {
                                        loop {
                                            let status = back_pagination_subscriber.next().await;
                                            println!("### Timeline back pagination status: {:?}", status);
                                            if status == Some(matrix_sdk_ui::timeline::BackPaginationStatus::TimelineStartReached) {
                                                break;
                                            }
                                        }
                                    });
                                }
                                let res = timeline_ref.paginate_backwards(
                                    // PaginationOptions::single_request(u16::MAX)
                                    // PaginationOptions::until_num_items(20, 20)
                                    PaginationOptions::until_num_items(500, 500)
                                ).await;
                                let items = timeline_ref.items().await;
                                println!("    --> Timeline room {room:?} pagination result: {:?}, timeline has {} items", res, items.len());
                                let mut room_timelines = ROOM_TIMELINES.lock().unwrap();
                                match room_timelines.entry(room.clone()) {
                                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                                        println!("    --> Post-pagination updating existing timeline for room {room:?}, now has {} items.", items.len());
                                        entry.get_mut().1 = items;
                                    }
                                    std::collections::btree_map::Entry::Vacant(entry) => {
                                        println!("    --> Post-pagination adding new timeline for room {room:?}, now has {} items.", items.len());
                                        entry.insert((timeline_ref, items));
                                    }
                                }
                            });
                        }
                    }
                }
            }

        }
        if !update.lists.is_empty() {
            println!("Lists have an update: {:?}", update.lists);
        }
    }

    bail!("unexpected return from async_main!")
}

