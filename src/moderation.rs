//! Moderating a room's members, and reporting messages, rooms, and users to homeserver admins.
//!
//! Everything here runs on the background matrix worker task;
//! see [`ModerationModal`](crate::moderation_modal::ModerationModal) for how the user confirms these.

use anyhow::{Result, anyhow};
use bitflags::bitflags;
use makepad_widgets::{Cx, error};
use matrix_sdk::{
    Client, Room,
    deserialized_responses::TimelineEvent,
    room::{MessagesOptions, RoomMember},
    ruma::{
        OwnedEventId, OwnedRoomId, OwnedUserId, UserId,
        api::client::reporting::report_user,
        events::{
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, MessageLikeEventType, SyncMessageLikeEvent,
            relation::RelationType,
            room::{member::MembershipState, message::{MessageType, Relation, sanitize::remove_plain_reply_fallback}, power_levels::RoomPowerLevels},
        },
        int,
    },
    serde_helpers::extract_relation,
};
use rustc_hash::FxHashMap;
use tokio::sync::oneshot;

use crate::{event_preview::UNSUPPORTED_MESSAGE_PREVIEW, home::event_reaction_list::is_custom_emoji, remove_messages_modal::RemoveMessagesModalAction, shared::popup_list::{PopupKind, enqueue_popup_notification}, utils::{self, RoomNameId}};

/// How long to show the popup notification for a request that succeeded, in seconds.
const SUCCESS_POPUP_DURATION: f64 = 4.0;
/// How long to show the popup notification for a request that failed, in seconds.
const FAILURE_POPUP_DURATION: f64 = 8.0;
/// How many of a user's events to ask the homeserver for at a time.
const USER_EVENTS_PAGE_SIZE: u32 = 100;
/// The length of a message preview, which is cut down so that huge messages don't sit around in memory.
const MAX_PREVIEW_CHARS: usize = 200;
/// The preview of a message that we don't have the keys for.
const UNDECRYPTABLE_MESSAGE_PREVIEW: &str = "Unable to decrypt this message.";

bitflags! {
    /// Possible moderation actions that the current user can perform on a room member.
    ///
    /// This is used to determine which buttons to show in the user profile pane.
    #[derive(Copy, Clone, Debug, Default)]
    pub struct MemberAbilities: u8 {
        const CanKick = 1 << 0;
        const CanBan = 1 << 1;
        const CanUnban = 1 << 2;
        const CanMute = 1 << 3;
        const CanUnmute = 1 << 4;
        const CanRemoveMessages = 1 << 5;
    }
}
impl MemberAbilities {
    /// Returns what `own_user_id` can do to `target_user_id`.
    ///
    /// The target's `membership` should be `None` if they've never been in the room.
    fn from_power_levels(
        power_levels: &RoomPowerLevels,
        own_user_id: &UserId,
        target_user_id: &UserId,
        membership: Option<&MembershipState>,
        message_type: MessageLikeEventType,
    ) -> Self {
        let mut abilities = Self::empty();
        let is_banned = matches!(membership, Some(MembershipState::Ban));
        let is_in_room = matches!(membership, Some(MembershipState::Join | MembershipState::Invite));
        let is_knocking = matches!(membership, Some(MembershipState::Knock));
        abilities.set(Self::CanKick, (is_in_room || is_knocking) && power_levels.user_can_kick_user(own_user_id, target_user_id));
        abilities.set(Self::CanBan, !is_banned && power_levels.user_can_ban_user(own_user_id, target_user_id));
        abilities.set(Self::CanUnban, is_banned && power_levels.user_can_unban_user(own_user_id, target_user_id));
        // Deleting someone's messages doesn't depend on outranking them, and they don't have to still be in the room.
        abilities.set(Self::CanRemoveMessages, membership.is_some() && power_levels.user_can_redact_event_of_other(own_user_id));

        // Unmuting resets a user to the default level, which only helps if that level can send messages.
        let send_level = power_levels.for_message(message_type);
        if is_in_room
            && power_levels.users_default >= send_level
            && power_levels.user_can_change_user_power_level(own_user_id, target_user_id)
        {
            let is_muted = power_levels.for_user(target_user_id) < send_level;
            abilities.set(if is_muted { Self::CanUnmute } else { Self::CanMute }, true);
        }
        abilities
    }
}

/// The result of a [`crate::sliding_sync::MatrixRequest::GetMemberAbilities`] request.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub struct MemberAbilitiesFetched {
    pub room_id: OwnedRoomId,
    pub user_id: OwnedUserId,
    pub abilities: MemberAbilities,
    /// The latest member info of this user, or `None` if they've never been in the room.
    pub room_member: Option<RoomMember>,
}

/// An action that a moderator can perform on a room member.
#[derive(Copy, Clone, Debug)]
pub enum MemberAction {
    Kick,
    Ban,
    Unban,
    /// Lowers the member's power level so that they can't send messages.
    Mute,
    /// Resets a muted member to the room's default power level.
    Unmute,
}

/// A [`MemberAction`] to perform on a specific member of a specific room.
#[derive(Clone, Debug)]
pub struct MemberModeration {
    pub room_id: OwnedRoomId,
    pub room_name: String,
    pub user_id: OwnedUserId,
    pub user_name: String,
    pub action: MemberAction,
}

/// The thing being reported to the homeserver's admins.
#[derive(Clone, Debug)]
pub enum ReportTarget {
    Message {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    /// An event that isn't a message, like a state event.
    Event {
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    },
    Room(RoomNameId),
    User {
        user_id: OwnedUserId,
        user_name: String,
    },
}

/// A message that a user sent in a room.
#[derive(Clone, Debug)]
pub struct UserMessage {
    pub event_id: OwnedEventId,
    /// Each edit has its own copy of the message's content, so it gets removed along with the message.
    pub edit_event_ids: Vec<OwnedEventId>,
    pub preview: String,
    /// When this message was sent, and whether it was edited.
    pub details: String,
}

/// The edits we've found of a message that we haven't reached yet.
#[derive(Default)]
struct MessageEdits {
    event_ids: Vec<OwnedEventId>,
    latest_preview: Option<String>,
}

/// An update on fetching or removing a user's messages.
#[derive(Clone, Debug)]
pub enum RemoveMessagesUpdate {
    /// More of the user's messages were fetched, newest first.
    MessagesFetched {
        fetch_id: u64,
        messages: Vec<UserMessage>,
        is_finished: bool,
        /// Why the rest of the messages couldn't be fetched.
        error: Option<String>,
    },
    /// Another of the user's messages was handled.
    RemovalProgressed {
        num_removed: usize,
        num_failed: usize,
    },
    /// We're done removing the user's messages.
    RemovalFinished {
        num_removed: usize,
        num_failed: usize,
        first_error: Option<String>,
        was_stopped: bool,
    },
}

/// Fetches what the current user can do to the given user in the given room,
/// which gets posted as a [`MemberAbilitiesFetched`] action.
pub async fn fetch_member_abilities(client: Client, room_id: OwnedRoomId, user_id: OwnedUserId) {
    let Some(room) = client.get_room(&room_id) else { return };
    let fetched: Result<_> = async {
        // This may have to fetch the room's members from the homeserver.
        let room_member = room.get_member(&user_id).await?;
        Ok((room_member, room.power_levels().await?))
    }.await;
    let (room_member, power_levels) = match fetched {
        Ok(fetched) => fetched,
        Err(e) => {
            error!("Failed to get the moderation abilities for {user_id} in {room_id}: {e:?}");
            return;
        }
    };
    let abilities = MemberAbilities::from_power_levels(
        &power_levels,
        room.own_user_id(),
        &user_id,
        room_member.as_ref().map(|member| member.membership()),
        get_message_event_type(&room),
    );
    Cx::post_action(MemberAbilitiesFetched { room_id, user_id, abilities, room_member });
}

/// Performs the given moderation of a room member, showing the result in a popup notification.
pub async fn moderate_member(client: Client, moderation: MemberModeration, reason: Option<String>) {
    let MemberModeration { room_id, user_id, user_name, action, .. } = moderation;
    let result: Result<()> = async {
        let room = get_room(&client, &room_id)?;
        let reason = reason.as_deref();
        match action {
            MemberAction::Kick => room.kick_user(&user_id, reason).await?,
            MemberAction::Ban => room.ban_user(&user_id, reason).await?,
            MemberAction::Unban => room.unban_user(&user_id, reason).await?,
            MemberAction::Mute => {
                let send_level = room.power_levels().await?.for_message(get_message_event_type(&room));
                room.update_power_levels(vec![(&user_id, send_level.saturating_sub(int!(1)))]).await?;
            }
            MemberAction::Unmute => {
                let default_level = room.power_levels().await?.users_default;
                room.update_power_levels(vec![(&user_id, default_level)]).await?;
            }
        }
        Ok(())
    }.await;

    let (verb, past_tense) = match action {
        MemberAction::Kick => ("kick", "Kicked"),
        MemberAction::Ban => ("ban", "Banned"),
        MemberAction::Unban => ("unban", "Unbanned"),
        MemberAction::Mute => ("mute", "Muted"),
        MemberAction::Unmute => ("unmute", "Unmuted"),
    };
    match result {
        Ok(()) => enqueue_popup_notification(
            format!("{past_tense} {user_name}."),
            PopupKind::Success,
            Some(SUCCESS_POPUP_DURATION),
        ),
        Err(e) => {
            error!("Failed to {verb} {user_id} in {room_id}: {e:?}");
            enqueue_popup_notification(
                format!("Failed to {verb} {user_name}: {e}"),
                PopupKind::Error,
                Some(FAILURE_POPUP_DURATION),
            );
        }
    }
}

/// Sends the given report to the homeserver's admins, showing the result in a popup notification.
pub async fn report(client: Client, target: ReportTarget, reason: Option<String>) {
    let result: Result<()> = async {
        match &target {
            ReportTarget::Message { room_id, event_id } | ReportTarget::Event { room_id, event_id } => {
                get_room(&client, room_id)?.report_content(event_id.clone(), reason).await?;
            }
            ReportTarget::Room(room_name_id) => {
                get_room(&client, room_name_id.room_id())?.report_room(reason.unwrap_or_default()).await?;
            }
            ReportTarget::User { user_id, .. } => {
                client.send(report_user::v3::Request::new(user_id.clone(), reason.unwrap_or_default())).await?;
            }
        }
        Ok(())
    }.await;

    let description = match &target {
        ReportTarget::Message { .. } => String::from("this message"),
        ReportTarget::Event { .. } => String::from("this event"),
        ReportTarget::Room(room_name_id) => format!("\"{room_name_id}\""),
        ReportTarget::User { user_name, .. } => user_name.clone(),
    };
    match result {
        Ok(()) => enqueue_popup_notification(
            format!("Reported {description} to your homeserver's admins."),
            PopupKind::Success,
            Some(SUCCESS_POPUP_DURATION),
        ),
        Err(e) => {
            error!("Failed to report {target:?}: {e:?}");
            enqueue_popup_notification(
                format!("Failed to report {description}: {e}"),
                PopupKind::Error,
                Some(FAILURE_POPUP_DURATION),
            );
        }
    }
}

/// Fetches all of the given user's messages that we can see in the given room,
/// posting each page of them as a [`RemoveMessagesUpdate::MessagesFetched`], until `stop` is dropped.
pub async fn fetch_user_messages(
    client: Client,
    room_id: OwnedRoomId,
    user_id: OwnedUserId,
    fetch_id: u64,
    mut stop: oneshot::Receiver<()>,
) {
    let post_update = |messages, is_finished, error| Cx::post_action(RemoveMessagesModalAction::Update(
        RemoveMessagesUpdate::MessagesFetched { fetch_id, messages, is_finished, error }
    ));
    let room = match get_room(&client, &room_id) {
        Ok(room) => room,
        Err(e) => return post_update(Vec::new(), true, Some(e.to_string())),
    };
    let mut edits = FxHashMap::default();
    let mut from = None;
    loop {
        let mut options = MessagesOptions::backward().from(from.as_deref());
        options.limit = USER_EVENTS_PAGE_SIZE.into();
        options.filter.senders = Some(vec![user_id.clone()]);
        let page = tokio::select! {
            _ = &mut stop => return,
            page = room.messages(options) => page,
        };
        let page = match page {
            Ok(page) => page,
            Err(e) => {
                error!("Failed to fetch the messages of {user_id} in {room_id}: {e:?}");
                return post_update(Vec::new(), true, Some(e.to_string()));
            }
        };
        let mut messages: Vec<_> = page.chunk.iter()
            .filter_map(|event| build_user_message(event, &user_id, &mut edits))
            .collect();
        // A page with no messages isn't the last page; only a missing `end` token means that.
        let is_finished = page.end.is_none() || page.end == from;
        if is_finished {
            // The edits of a deleted message are still readable, so they get a row of their own.
            messages.extend(edits.drain().filter_map(|(_, MessageEdits { event_ids, latest_preview })| {
                let mut event_ids = event_ids.into_iter();
                Some(UserMessage {
                    event_id: event_ids.next()?,
                    edit_event_ids: event_ids.collect(),
                    preview: latest_preview.unwrap_or_else(|| UNDECRYPTABLE_MESSAGE_PREVIEW.to_owned()),
                    details: String::from("Edit of a deleted message"),
                })
            }));
        }
        post_update(messages, is_finished, None);
        if is_finished { return }
        from = page.end;
    }
}

/// Builds the [`UserMessage`] for the given event, or adds the event to `edits` if it's an edit.
///
/// Returns `None` for anything else that isn't a message we can remove, e.g., a state event.
fn build_user_message(
    event: &TimelineEvent,
    user_id: &UserId,
    edits: &mut FxHashMap<OwnedEventId, MessageEdits>,
) -> Option<UserMessage> {
    // Some homeservers ignore the sender filter, and we can't ever offer to remove someone else's message.
    if event.sender().as_deref() != Some(user_id) {
        return None;
    }
    let raw = event.raw();
    let message = match raw.deserialize() {
        Ok(AnySyncTimelineEvent::MessageLike(message)) => Some(message),
        Ok(AnySyncTimelineEvent::State(_)) => return None,
        Err(_) if matches!(raw.get_field::<String>("state_key"), Ok(Some(_))) => return None,
        // We can't parse this one, but the timeline can still show that it's there.
        Err(_) => None,
    };
    if message.as_ref().is_some_and(|message|
        message.is_redacted() || matches!(message, AnySyncMessageLikeEvent::RoomRedaction(_))
    ) {
        return None;
    }
    let event_id = event.event_id()?.to_owned();

    if let Some((RelationType::Replacement, edited_event_id)) = extract_relation(raw) {
        let edits_of_message = edits.entry(edited_event_id).or_default();
        edits_of_message.event_ids.push(event_id);
        // We're going backwards in time, so the first edit we find is the latest one.
        if edits_of_message.latest_preview.is_none()
            && let Some(AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(edit))) = message
            && let Some(Relation::Replacement(replacement)) = edit.content.relates_to
        {
            edits_of_message.latest_preview = Some(get_message_preview(&replacement.new_content.msgtype));
        }
        return None;
    }

    let MessageEdits { event_ids: edit_event_ids, latest_preview } = edits.remove(&event_id).unwrap_or_default();
    let time = event.timestamp()
        .and_then(utils::unix_time_millis_to_datetime)
        .map_or_else(String::new, |time| time.format("%a %b %-d, %Y, %-I:%M%P").to_string());
    Some(UserMessage {
        preview: latest_preview.unwrap_or_else(|| message.as_ref().map_or_else(
            || UNSUPPORTED_MESSAGE_PREVIEW.to_owned(),
            get_event_preview,
        )),
        details: if edit_event_ids.is_empty() { time } else { format!("{time}  ·  Edited") },
        event_id,
        edit_event_ids,
    })
}

/// Returns a single-line preview of the given event.
fn get_event_preview(event: &AnySyncMessageLikeEvent) -> String {
    match event {
        AnySyncMessageLikeEvent::RoomMessage(SyncMessageLikeEvent::Original(message)) => get_message_preview(&message.content.msgtype),
        AnySyncMessageLikeEvent::Reaction(SyncMessageLikeEvent::Original(reaction)) => {
            let key = &reaction.content.relates_to.key;
            if is_custom_emoji(key) {
                "Reacted with a custom emoji".to_owned()
            } else {
                format!("Reacted with {}", shorten_preview(key))
            }
        }
        AnySyncMessageLikeEvent::Sticker(SyncMessageLikeEvent::Original(sticker)) => {
            format!("[Sticker] {}", shorten_preview(&sticker.content.body))
        }
        AnySyncMessageLikeEvent::RoomEncrypted(_) => UNDECRYPTABLE_MESSAGE_PREVIEW.to_owned(),
        other => format!("[Other message type: {}]", other.event_type()),
    }
}

/// Returns a single-line preview of the given message.
fn get_message_preview(msgtype: &MessageType) -> String {
    let kind = match msgtype {
        MessageType::Audio(_) => "[Audio] ",
        MessageType::File(_) => "[File] ",
        MessageType::Image(_) => "[Image] ",
        MessageType::Location(_) => "[Location] ",
        MessageType::Video(_) => "[Video] ",
        _ => "",
    };
    format!("{kind}{}", shorten_preview(remove_plain_reply_fallback(msgtype.body())))
}

/// Puts the given text on a single line and cuts it down to [`MAX_PREVIEW_CHARS`].
fn shorten_preview(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_PREVIEW_CHARS)
        .collect()
}

/// Removes the given messages one at a time, posting a [`RemoveMessagesUpdate`] after each one,
/// until `stop` is dropped.
///
/// We don't use the room's send queue, since hitting the rate limit there would disable it.
pub async fn remove_user_messages(
    client: Client,
    room_id: OwnedRoomId,
    messages: Vec<UserMessage>,
    reason: Option<String>,
    mut stop: oneshot::Receiver<()>,
) {
    let mut num_removed = 0;
    let mut num_failed = 0;
    let mut first_error = None;
    let mut was_stopped = false;
    match get_room(&client, &room_id) {
        Ok(room) => for message in &messages {
            let result = tokio::select! {
                biased;
                _ = &mut stop => {
                    was_stopped = true;
                    break;
                }
                result = room.redact(&message.event_id, reason.as_deref(), None) => result,
            };
            let mut error = result.err();
            // We can't be stopped here, since a message's edits get harder to find once it's gone.
            if error.is_none() {
                for edit_event_id in &message.edit_event_ids {
                    if let Err(e) = room.redact(edit_event_id, reason.as_deref(), None).await {
                        error.get_or_insert(e);
                    }
                }
            }
            match error {
                None => num_removed += 1,
                Some(error) => {
                    error!("Failed to redact {} in {room_id}: {error:?}", message.event_id);
                    num_failed += 1;
                    let got_response = error.as_client_api_error().is_some();
                    first_error.get_or_insert_with(|| if got_response {
                        error.to_string()
                    } else {
                        String::from("couldn't reach your homeserver.")
                    });
                    // Without a response from the server, the rest of the messages would fail too.
                    if !got_response {
                        break;
                    }
                }
            }
            Cx::post_action(RemoveMessagesModalAction::Update(
                RemoveMessagesUpdate::RemovalProgressed { num_removed, num_failed }
            ));
        },
        Err(e) => {
            num_failed = messages.len();
            first_error = Some(e.to_string());
        }
    }
    Cx::post_action(RemoveMessagesModalAction::Update(
        RemoveMessagesUpdate::RemovalFinished { num_removed, num_failed, first_error, was_stopped }
    ));
}

fn get_room(client: &Client, room_id: &OwnedRoomId) -> Result<Room> {
    client.get_room(room_id).ok_or_else(|| anyhow!("room {room_id} wasn't found"))
}

/// Returns the type of event that the given room's messages are sent as.
fn get_message_event_type(room: &Room) -> MessageLikeEventType {
    if room.encryption_state().is_encrypted() {
        MessageLikeEventType::RoomEncrypted
    } else {
        MessageLikeEventType::RoomMessage
    }
}
