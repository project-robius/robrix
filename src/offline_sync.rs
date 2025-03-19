use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use anyhow::{bail, Result};
use matrix_sdk::{ruma::{user_id, OwnedDeviceId, OwnedRoomId}, store::StoreConfig, RoomInfo, SessionMeta};
use matrix_sdk_base::{
    BaseClient, RoomState,
};
use matrix_sdk_sqlite::SqliteStateStore;
use tokio::time::sleep;

use crate::{home::rooms_list::{self, RoomsListEntry, RoomsListUpdate}, persistent_state::fetch_previous_session, sliding_sync::{avatar_from_room_name, get_latest_event_details}};

static BASE_CLIENT: OnceLock<BaseClient> = OnceLock::new();
/// Initializes and starts the base client for offline synchronization.
///
/// This function sets up the necessary components and configurations
/// to start the base client. It returns a `Result` of room info map
///
/// # Errors
///
/// Returns an error if the initialization process fails due to any
/// configuration or setup issues.
pub async fn start_base_client() -> Result<()> {
    println!("starting base client");
    let previous_session = fetch_previous_session(Some(user_id!("@ruitoalpha:matrix.org").to_owned())).await?;
    let store = SqliteStateStore::open(previous_session.client_session.db_path.clone(), Some(&previous_session.client_session.passphrase)).await.unwrap();
    let event_store = matrix_sdk_sqlite::SqliteEventCacheStore::open(
        previous_session.client_session.db_path,
        Some(&previous_session.client_session.passphrase),
    ).await.unwrap();
    let store_config = StoreConfig::new(String::from("cross-process-store-locks-holder-name"))
        .state_store(store).event_cache_store(event_store);
    let client = BaseClient::with_store_config(store_config);
    let session_meta = previous_session.user_session.meta;
    client.set_session_meta(session_meta, None).await.unwrap();
    for room in client.rooms() {
        //println!("set_session_meta room: {room:#?}");
        let room_id = room.room_id().to_owned();
        let room_name = room.display_name().await.map(|room_name| room_name.to_string()).unwrap_or_default();
        let latest_event = room.latest_event().and_then(|f| Some(f.event().clone()));
        if let Some(latest_event) = latest_event {
            println!("latest_event {:?}",latest_event);
            let event_id = latest_event.event_id().unwrap();
            let event = client.event_cache_store().lock().await.unwrap().find_event(&room_id, &event_id).await.unwrap();
            println!("ni event {:?}", event);
        }
        //let lock = room.clone().latest_encrypted_events;
        // for i in lock.read().unwrap().iter() {
        //     println!("ni event: {i:#?}");
        // }
        // for i in lock.iter() {
            
        // }
        // let latest_events = room.latest_encrypted_events.try_read().unwrap();
        // latest_events.iter().for_each(|event| {            
        //     println!("latest event: {event:#?}");
        // });
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddRoom(RoomsListEntry {
            room_id: room_id.clone(),
            latest: None,
            tags: room.tags().await.ok().flatten(),
            num_unread_messages: room.num_unread_messages(),
            num_unread_mentions: room.num_unread_mentions(),
            // start with a basic text avatar; the avatar image will be fetched asynchronously below.
            avatar: avatar_from_room_name(&room_name),
            room_name: Some(room_name),
            canonical_alias: room.canonical_alias(),
            alt_aliases: room.alt_aliases(),
            has_been_paginated: false,
            is_selected: false,
        }));
    }
    sleep(Duration::new(8, 0)).await;
    Ok(())
}