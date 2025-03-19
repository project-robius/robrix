use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use anyhow::{bail, Result};
use matrix_sdk::{
    deserialized_responses::{TimelineEvent, TimelineEventKind}, encryption::identities::ManualVerifyError, ruma::{user_id, OwnedDeviceId, OwnedRoomId}, store::StoreConfig, RoomInfo, SessionMeta
};
use matrix_sdk_base::{BaseClient, RoomState};
use matrix_sdk_sqlite::SqliteStateStore;
use matrix_sdk_ui::timeline::Message;
use ruma_events::{location, message::MessageEventContent, room::message::{MessageFormat, MessageType}, AnySyncTimelineEvent};
use tokio::time::sleep;

use crate::{
    event_preview::text_preview_of_message, home::rooms_list::{self, RoomsListEntry, RoomsListUpdate}, persistent_state::fetch_previous_session, sliding_sync::{avatar_from_room_name, get_latest_event_details}, utils
};

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
    for room in client.rooms() {
        //println!("set_session_meta room: {room:#?}");
        let room_id = room.room_id().to_owned();
        let room_name = room
            .display_name()
            .await
            .map(|room_name| room_name.to_string())
            .unwrap_or_default();
        let latest_event = room.latest_event().and_then(|f| Some(f.event().clone()));

        let latest = latest_event.and_then(|f| f.raw().deserialize().ok().and_then(|f| {
            let timestamp = f.origin_server_ts();
            let content = match f {
                AnySyncTimelineEvent::MessageLike(event) => {
                    let sender_username = event.sender().to_string();
                    if let Some(content) = event.original_content() {
                        println!("content {:?}", content);
                        match content {
                            ruma_events::AnyMessageLikeEventContent::RoomMessage(msg) => {
                                let text = match msg.msgtype {
                                    MessageType::Audio(audio) => format!(
                                        "[Audio]: <i>{}</i>",
                                        if let Some(formatted_body) = audio.formatted.as_ref() {
                                            &formatted_body.body
                                        } else {
                                            &audio.body
                                        }
                                    ),
                                    MessageType::Emote(emote) => format!(
                                        "* {} {}",
                                        sender_username,
                                        if let Some(formatted_body) = emote.formatted.as_ref() {
                                            &formatted_body.body
                                        } else {
                                            &emote.body
                                        }
                                    ),
                                    MessageType::File(file) => format!(
                                        "[File]: <i>{}</i>",
                                        if let Some(formatted_body) = file.formatted.as_ref() {
                                            &formatted_body.body
                                        } else {
                                            &file.body
                                        }
                                    ),
                                    MessageType::Image(image) => format!(
                                        "[Image]: <i>{}</i>",
                                        if let Some(formatted_body) = image.formatted.as_ref() {
                                            &formatted_body.body
                                        } else {
                                            &image.body
                                        }
                                    ),
                                    MessageType::Location(location) => format!(
                                        "[Location]: <i>{}</i>",
                                        location.body,
                                    ),
                                    MessageType::Notice(notice) => format!("<i>{}</i>",
                                        if let Some(formatted_body) = notice.formatted.as_ref() {
                                            utils::trim_start_html_whitespace(&formatted_body.body)
                                        } else {
                                            &notice.body
                                        }
                                    ),
                                    MessageType::ServerNotice(notice) => format!(
                                        "[Server Notice]: <i>{} -- {}</i>",
                                        notice.server_notice_type.as_str(),
                                        notice.body,
                                    ),
                                    MessageType::Text(text) => {
                                        text.formatted
                                            .as_ref()
                                            .and_then(|fb|
                                                (fb.format == MessageFormat::Html).then(||
                                                    utils::linkify(
                                                        utils::trim_start_html_whitespace(&fb.body),
                                                        true,
                                                    )
                                                    .to_string()
                                                )
                                            )
                                            .unwrap_or_else(|| utils::linkify(&text.body, false).to_string())
                                    }
                                    MessageType::VerificationRequest(verification) => format!(
                                        "[Verification Request] <i>to user {}</i>",
                                        verification.to,
                                    ),
                                    MessageType::Video(video) => format!(
                                        "[Video]: <i>{}</i>",
                                        if let Some(formatted_body) = video.formatted.as_ref() {
                                            &formatted_body.body
                                        } else {
                                            &video.body
                                        }
                                    ),
                                    MessageType::_Custom(custom) => format!(
                                        "[Custom message]: {:?}",
                                        custom,
                                    ),
                                    other => format!(
                                        "[Unknown message type]: {}",
                                        other.body(),
                                    )
                                };
                                text
                            }
                            ruma_events::AnyMessageLikeEventContent::Message(msg) => {
                                msg.text.find_plain().unwrap_or_default().to_owned()
                            }
                            _ => {
                                format!("{:?}", content)
                            }
                        }                        
                    } else {
                        String::new()
                    }
                    
                }
                AnySyncTimelineEvent::State(event) => {
                    format!("{:?}", event.content())
                }
            };

            Some((timestamp, content))}));
        println!("latest {:?}", latest);
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
    }
    sleep(Duration::new(8, 0)).await;
    Ok(())
}
