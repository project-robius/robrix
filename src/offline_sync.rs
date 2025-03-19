use std::{collections::BTreeMap, f64::consts::E, sync::OnceLock, time::Duration};

use anyhow::{bail, Result};
use imbl::Vector;
use makepad_widgets::{error, log};
use matrix_sdk::{
    deserialized_responses::{TimelineEvent, TimelineEventKind}, encryption::identities::ManualVerifyError, linked_chunk::ChunkContent, media::{MediaFormat, MediaThumbnailSettings}, ruma::{user_id, OwnedDeviceId, OwnedRoomId}, store::StoreConfig, BaseRoom, RoomInfo, RoomMemberships, SessionMeta
};
use matrix_sdk_base::{BaseClient, RoomState, Room};
use matrix_sdk_sqlite::SqliteStateStore;
use matrix_sdk_ui::timeline::Message;
use ruma_events::{location, message::MessageEventContent, room::message::{MessageFormat, MessageType}, AnyFullStateEventContent, AnySyncMessageLikeEvent, AnySyncStateEvent, AnySyncTimelineEvent};
use tokio::{runtime::Handle, time::sleep};
use matrix_sdk::ruma::{room_id};
use crate::{
    event_preview::{text_preview_of_message, text_preview_of_offline_message, BeforeText, TextPreview}, home::rooms_list::{self, RoomPreviewAvatar, RoomsListEntry, RoomsListUpdate}, media_cache::{self, fetch_from_cache, MediaCache, MediaCacheEntry}, persistent_state::fetch_previous_session, sliding_sync::{avatar_from_room_name, get_latest_event_details}, utils::{self, AVATAR_THUMBNAIL_FORMAT}
};

pub static BASE_CLIENT: OnceLock<BaseClient> = OnceLock::new();
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
    let previous_session =
        fetch_previous_session(Some(user_id!("@ruitoalpha:matrix.org").to_owned())).await?;
    let store = SqliteStateStore::open(
        previous_session.client_session.db_path.clone(),
        Some(&previous_session.client_session.passphrase),
    )
    .await
    .unwrap();
    let event_store = matrix_sdk_sqlite::SqliteEventCacheStore::open(
        previous_session.client_session.db_path,
        Some(&previous_session.client_session.passphrase),
    )
    .await
    .unwrap();
    let store_config = StoreConfig::new(String::from("cross-process-store-locks-holder-name"))
        .state_store(store)
        .event_cache_store(event_store);
    let client = BaseClient::with_store_config(store_config);
    let session_meta = previous_session.user_session.meta;
    
    client.set_session_meta(session_meta, None).await.unwrap();
    BASE_CLIENT.set(client.clone()).unwrap();
    let room_id_to_watch = room_id!("!QQpfJfZvqxbCfeDgCj:matrix.org");

    for room in client.rooms() {
        println!("is_sync {:?} encrypt {:?}", room.is_state_fully_synced(), room.is_encryption_state_synced());
        //println!("set_session_meta room: {room:#?}");
        let room_id = room.room_id().to_owned();
        let room_name = room
            .display_name()
            .await
            .map(|room_name| room_name.to_string())
            .unwrap_or_default();
        let latest_event = room.latest_event().and_then(|f| Some(f.event().clone()));
        let latest_sync_time_event = latest_event.and_then(|f| f.raw().deserialize().ok());
        let latest = if let Some(latest_sync_time_event) = latest_sync_time_event.clone() {
            let timestamp = latest_sync_time_event.origin_server_ts();
            let content = match latest_sync_time_event {
                AnySyncTimelineEvent::MessageLike(event) => {
                    let sender_id = event.sender();
                    let sender_username = room.get_member(sender_id).await?
                    .and_then(|rm| rm.display_name()
                    .and_then(|f| Some(f.to_owned()))).unwrap_or_default();
                    if let Some(content) = event.original_content() {
                        match content {
                            ruma_events::AnyMessageLikeEventContent::RoomMessage(msg) => {
                                text_preview_of_offline_message(&msg, &sender_username).format_with(&sender_username)
                            }
                            ruma_events::AnyMessageLikeEventContent::RoomRedaction(redaction) => {
                                crate::event_preview::text_preview_of_redacted_message_offline(sender_id.to_owned(), redaction, "").format_with(&sender_username)
                            }
                            _ => {
                                TextPreview::from((format!("{:?}", content), BeforeText::UsernameWithColon)).format_with(&sender_username)                         
                            }
                        }
                    } else {
                        TextPreview::from((String::new(), BeforeText::Nothing)).format_with(&sender_username)
                    }
                }
                AnySyncTimelineEvent::State(event) => {
                    match event.content() {
                        AnyFullStateEventContent::RoomName(room_name) => {

                        }
                        _ => {
                        }
                    }
                    TextPreview::from((format!("{:?}", event.content()), BeforeText::Nothing)).format_with("")
                }
            };
            Some((timestamp, content))
        } else {
            None
        };
        println!("room name {:?}", room_name);
        println!("latest {:?}", latest);
        println!("latest_sync_time_event {:?}", latest_sync_time_event);
        
        rooms_list::enqueue_rooms_list_update(RoomsListUpdate::AddRoom(RoomsListEntry {
            room_id: room_id.clone(),
            latest,
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
        
        spawn_fetch_room_avatar(room.clone());
        add_new_room_offline(room_id.clone());
        let ec_store = client.event_cache_store().lock().await?;
        let items = match load_all_chunk(ec_store, &room_id, None).await {
            Ok((initial_items, _chunk_identifier)) => {
                let all_room_info = ALL_ROOM_INFO.lock().unwrap();
                let Some(room_info) = all_room_info.get(&room_id) else {
                    log!("BUG: room info not found for redact message {room_id}");
                    return Ok(());
                };
                let num_unread = 0;
                let latest_read_receipt = None;
                let mut vector_initial_items = Vector::new();
                for item in initial_items {
                    vector_initial_items.push_back(Arc::new(item));
                }
                println!("vector_initial_items len {:?}",vector_initial_items.len());
                room_info.timeline_update_sender.send(TimelineUpdate::FirstUpdateFromCache { initial_items: vector_initial_items, num_unread, latest_read_receipt }).unwrap();

            },
            Err(e) => {
                error!("Failed to load chunk: {e:?}");
            }
        };
        //submit_async_request(MatrixRequest::FetchOfflineTimelineEventChunk { room_id: room_id.clone(), chunk_identifier: None });

    }
    Ok(())
}
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

// Fetches and returns the avatar image for the given room (if one exists),
/// otherwise returns a text avatar string of the first character of the room name.
async fn room_avatar(room: &Room, room_name: &Option<String>) -> RoomPreviewAvatar {
    if let Some(mxc_uri)  = room.avatar_url() {
        println!("mxc_uri {:?}", mxc_uri);
        if let Ok(data) = fetch_from_cache(&mxc_uri) {
            return RoomPreviewAvatar::Image(data)
        }
    }
    if let Some(room_name) = room_name {
        return avatar_from_room_name(room_name)
    } 
    avatar_from_room_name(room.room_id().to_string().as_str())
}

async fn load_all_chunk(ec_store: &EventCacheStoreLockGuard<'_>, room_id: &OwnedRoomId) -> Result<Vector<AnySyncTimelineEvent>> {
    if let Ok(e) = ec_store.load_all_chunks(&room_id).await {
        let mut v: Vec<AnySyncTimelineEvent> = e.iter().map(|f| {
            match &f.content {
                ChunkContent::Items(items) => {
                    items.iter().map(|f| f.raw().deserialize().ok()).filter(|f|f.is_some())
                    .map(|f| f.unwrap())
                    .filter(|f| {
                        if let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(_)) = f {
                            return true;
                        } else {
                            return false;
                        }
                    }).collect()
                }
                ChunkContent::Gap(_gap) => {
                    vec![]
                }
            }
        }).flatten().collect();
        v.sort_by(|a, b| a.origin_server_ts().cmp(&b.origin_server_ts()));
        return Ok(v.into())
    }
    Ok(Vector::new())
}