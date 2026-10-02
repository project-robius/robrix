//! Functions for generating text previews of timeline events.
//!
//! These text previews are used for:
//! * inline replies within the timeline
//! * preview of a message being replied to above the message input box
//! * previews of each room's latest message in the rooms list

use std::borrow::Cow;

use matrix_sdk::{ruma::{OwnedUserId, events::{room::{guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule, member::{MembershipState, RoomMemberEventContent}, message::{MessageFormat, MessageType}}, AnyRedactionEvent, AnySyncMessageLikeEvent, AnySyncTimelineEvent, StateEventContentChange, SyncMessageLikeEvent}, serde::Raw, UserId}};
use matrix_sdk_base::crypto::types::events::UtdCause;
use matrix_sdk_ui::timeline::{self, AnyOtherStateEventContentChange, EncryptedMessage, EventTimelineItem, MemberProfileChange, MembershipChange, MsgLikeKind, OtherMessageLike, RoomMembershipChange, TimelineItemContent};

use crate::{home::state_event_summary::{Membership, MembershipTransition, membership_transition}, utils};
use matrix_sdk::ruma::events::room::third_party_invite::RoomThirdPartyInviteEventContent;

/// What should be displayed before the text preview of an event.
pub enum BeforeText {
    /// Nothing should be displayed before the text preview.
    Nothing,
    /// The sender's username with a colon should be displayed before the text preview.
    UsernameWithColon,
    /// The sender's username (without a colon) should be displayed before the text preview.
    UsernameWithoutColon,
}

/// A text preview of a timeline event, plus how a username should be displayed before it.
///
/// Call [`TextPreview::format_with()`] to generate displayable text
/// with the appropriately-formatted preceding username.
pub struct TextPreview {
    text: String,
    before_text: BeforeText,
}
impl From<(String, BeforeText)> for TextPreview {
    fn from((text, before_text): (String, BeforeText)) -> Self {
        Self { text, before_text }
    }
}
impl TextPreview {
    /// Formats the text preview with the appropriate preceding username.
    pub fn format_with(
        self,
        username: &str,
        as_html: bool,
    ) -> String {
        let Self { text, before_text } = self;
        match before_text {
            BeforeText::Nothing => text,
            BeforeText::UsernameWithColon => if as_html {
                format!("<b>{}</b>: {}", htmlize::escape_text(username), text)
            } else {
                format!("{}: {}", username, text)
            },
            BeforeText::UsernameWithoutColon => format!(
                "{} {}",
                if as_html { htmlize::escape_text(username) } else { username.into() },
                text,
            ),
        }
    }

    /// Like [`Self::format_with()`], but without the "username: " before a message,
    /// e.g., for a preview shown right beneath its sender's name.
    pub fn format_under_username(self, username: &str, as_html: bool) -> String {
        match self.before_text {
            BeforeText::UsernameWithColon => self.text,
            _ => self.format_with(username, as_html),
        }
    }
}

/// The preview of a message whose content couldn't be parsed.
pub const UNSUPPORTED_MESSAGE_PREVIEW: &str = "[Unsupported message]";

/// Returns an HTML preview of a reply in a thread, beginning with the given name of its sender.
///
/// The `content` is `None` if the reply couldn't be parsed.
pub fn text_preview_of_thread_reply(
    sender: &UserId,
    sender_name: &str,
    content: Option<&TimelineItemContent>,
) -> String {
    let preview = content.map_or_else(
        || TextPreview::from((String::from(UNSUPPORTED_MESSAGE_PREVIEW), BeforeText::UsernameWithColon)),
        |content| text_preview_of_timeline_item(content, sender, sender_name),
    ).format_with(sender_name, true);
    match utils::replace_linebreaks_separators(&preview, true) {
        Cow::Borrowed(_) => preview,
        Cow::Owned(replaced) => replaced,
    }
}

/// Returns a text preview of the given timeline event as an Html-formatted string.
pub fn text_preview_of_timeline_item(
    content: &TimelineItemContent,
    sender_user_id: &UserId,
    sender_username: &str,
) -> TextPreview {
    match content {
        TimelineItemContent::MsgLike(msg_like_content) => {
            match &msg_like_content.kind {
                MsgLikeKind::Message(msg) => text_preview_of_message(msg.msgtype(), sender_username),
                MsgLikeKind::Sticker(sticker) => TextPreview::from((
                    format!("[Sticker]: <i>{}</i>", htmlize::escape_text(&sticker.content().body)),
                    BeforeText::UsernameWithColon,
                )),
                MsgLikeKind::Poll(poll_state) => TextPreview::from((
                    format!(
                        "[Poll]: {}",
                        htmlize::escape_text(
                            poll_state.fallback_text()
                                .unwrap_or_else(|| poll_state.results().question)
                        ),
                    ),
                    BeforeText::UsernameWithColon,
                )),
                MsgLikeKind::Redacted => {
                    let mut preview = text_preview_of_redacted_message(
                        None,
                        sender_user_id,
                        sender_username,
                    );
                    preview.text = htmlize::escape_text(&preview.text).into();
                    preview
                }
                MsgLikeKind::UnableToDecrypt(em) => text_preview_of_encrypted_message(em),
                MsgLikeKind::LiveLocation(_) => TextPreview::from((
                    String::from("[Live Location]"),
                    BeforeText::UsernameWithColon,
                )),
                MsgLikeKind::Other(oml) => text_preview_of_other_message_like(oml),
            }
        }
        TimelineItemContent::MembershipChange(membership_change) =>
            text_preview_of_room_membership_change(membership_change, sender_user_id, true),
        TimelineItemContent::ProfileChange(profile_change) =>
            text_preview_of_member_profile_change(profile_change, sender_username, true),
        TimelineItemContent::OtherState(other_state) =>
            text_preview_of_other_state(other_state, true),
        TimelineItemContent::FailedToParseMessageLike { event_type, .. } => TextPreview::from((
            format!("[Failed to parse <i>{}</i> message]", htmlize::escape_text(event_type.to_string())),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::FailedToParseState { event_type, .. } => TextPreview::from((
            format!("[Failed to parse <i>{}</i> state]", htmlize::escape_text(event_type.to_string())),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::CallInvite => TextPreview::from((
            String::from("[Call Invitation]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::RtcNotification { .. } => TextPreview::from((
            String::from("[RTC Call Notification]"),
            BeforeText::UsernameWithColon,
        )),
    }
}



/// Returns the plaintext `body` of the given timeline event.
pub fn plaintext_body_of_timeline_item(
    event_tl_item: &EventTimelineItem,
) -> String {
    match event_tl_item.content() {
        TimelineItemContent::MsgLike(msg_likecontent) => {
            match &msg_likecontent.kind {
                MsgLikeKind::Message(msg) => {
                    msg.body().into()
                }
                MsgLikeKind::Sticker(sticker) => {
                    sticker.content().body.clone()
                }
                MsgLikeKind::Poll(poll_state) => {
                    format!("[Poll]: {}", 
                        poll_state.fallback_text().unwrap_or_else(|| poll_state.results().question)
                    )
                }
                MsgLikeKind::Redacted => {
                    let sender_username = utils::get_or_fetch_event_sender(event_tl_item, None);
                    text_preview_of_redacted_message(
                        event_tl_item.latest_json(),
                        event_tl_item.sender(),
                        &sender_username,
                    ).format_with(&sender_username, false)
                }
                MsgLikeKind::UnableToDecrypt(em) => {
                    text_preview_of_encrypted_message(em)
                        .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)
                }
                MsgLikeKind::LiveLocation(_) => {
                    String::from("[Live Location]")
                }
                MsgLikeKind::Other(other_msg_like) => {
                    text_preview_of_other_message_like(other_msg_like)
                        .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)}
            }
        }
        TimelineItemContent::MembershipChange(membership_change) => {
            text_preview_of_room_membership_change(membership_change, event_tl_item.sender(), false)
                .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)
        }
        TimelineItemContent::ProfileChange(profile_change) => {
            let sender = utils::get_or_fetch_event_sender(event_tl_item, None);
            text_preview_of_member_profile_change(profile_change, &sender, false)
                .format_with(&sender, false)
        }
        TimelineItemContent::OtherState(other_state) => {
            text_preview_of_other_state(other_state, false)
                .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)
        }
        TimelineItemContent::FailedToParseMessageLike { event_type, error } => {
            format!("Failed to parse {} message. Error: {}", event_type, error)
        }
        TimelineItemContent::FailedToParseState { event_type, error, state_key } => {
            format!("Failed to parse {} state; key: {}. Error: {}", event_type, state_key, error)
        }
        TimelineItemContent::CallInvite => String::from("[Call Invitation]"),
        TimelineItemContent::RtcNotification { .. } => String::from("[RTC Call Notification]"),
    }
}


/// Returns a text preview of the given message as an Html-formatted string.
fn text_preview_of_message(
    msg: &MessageType,
    sender_username: &str,
) -> TextPreview {
    let text = match msg {
        MessageType::Audio(audio) => format!(
            "[Audio]: <i>{}</i>",
            if let Some(formatted_body) = audio.formatted.as_ref() {
                Cow::Borrowed(formatted_body.body.as_str())
            } else {
                htmlize::escape_text(audio.body.as_str())
            }
        ),
        MessageType::Emote(emote) => format!(
            "* {} {}",
            sender_username,
            if let Some(formatted_body) = emote.formatted.as_ref() {
                Cow::Borrowed(formatted_body.body.as_str())
            } else {
                htmlize::escape_text(emote.body.as_str())
            }
        ),
        MessageType::File(file) => format!(
            "[File]: <i>{}</i>",
            if let Some(formatted_body) = file.formatted.as_ref() {
                Cow::Borrowed(formatted_body.body.as_str())
            } else {
                htmlize::escape_text(file.body.as_str())
            }
        ),
        MessageType::Image(image) => format!(
            "[Image]: <i>{}</i>",
            if let Some(formatted_body) = image.formatted.as_ref() {
                Cow::Borrowed(formatted_body.body.as_str())
            } else {
                htmlize::escape_text(image.body.as_str())
            }
        ),
        MessageType::Location(location) => format!(
            "[Location]: <i>{}</i>",
            htmlize::escape_text(&location.body),
        ),
        MessageType::Notice(notice) => format!("<i>{}</i>",
            if let Some(formatted_body) = notice.formatted.as_ref() {
                utils::trim_start_html_whitespace(&formatted_body.body).into()
            } else {
                htmlize::escape_text(notice.body.as_str())
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
                    (fb.format == MessageFormat::Html).then(|| {
                        let filtered_and_trimmed = utils::trim_start_html_whitespace(
                            utils::remove_mx_reply(&fb.body)
                        );
                        utils::linkify(filtered_and_trimmed, true).to_string()
                    })
                )
                .unwrap_or_else(|| match utils::linkify(&text.body, false) {
                    Cow::Borrowed(plaintext) => htmlize::escape_text(plaintext).to_string(),
                    Cow::Owned(linkified) => linkified,
                })
        }
        MessageType::VerificationRequest(verification) => format!(
            "[Verification Request] <i>to user {}</i>",
            verification.to,
        ),
        MessageType::Video(video) => format!(
            "[Video]: <i>{}</i>",
            if let Some(formatted_body) = video.formatted.as_ref() {
               Cow::Borrowed(formatted_body.body.as_str())
            } else {
                htmlize::escape_text(&video.body)
            }
        ),
        MessageType::_Custom(custom) => format!(
            "[Custom message]: {:?}",
            custom,
        ),
        other => format!(
            "[Unknown message type]: {}",
            htmlize::escape_text(other.body()),
        ),
    };
    TextPreview::from((text, BeforeText::UsernameWithColon))
}

/// Returns a preview of the given raw timeline event.
pub fn text_preview_of_raw_timeline_event(
    raw_event: &Raw<AnySyncTimelineEvent>,
    sender_username: &str,
) -> Option<TextPreview> {
    match raw_event.deserialize().ok()? {
        AnySyncTimelineEvent::MessageLike(
            AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Original(ev)
            )
        ) => Some(text_preview_of_message(
            &ev.content.msgtype,
            sender_username,
        )),
        AnySyncTimelineEvent::MessageLike(
            AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Redacted(_)
            )
        ) => {
            let sender_user_id = raw_event.get_field::<OwnedUserId>("sender").ok().flatten()?;
            Some(text_preview_of_redacted_message(
                Some(raw_event),
                sender_user_id.as_ref(),
                sender_username,
            ))
        }
        _ => None,
    }
}


/// Returns a plaintext preview of the given redacted message.
///
/// Note: this function accepts the component parts of an [`EventTimelineItem`]
/// instead of an `EventTimelineItem` itself, in order to also accommodate
/// being invoked with the content/details of an [`EmbeddedEvent`].
///
/// [`EmbeddedEvent`]: matrix_sdk_ui::timeline::EmbeddedEvent
pub fn text_preview_of_redacted_message(
    latest_json: Option<&Raw<AnySyncTimelineEvent>>,
    sender_user_id: &UserId,
    original_sender_username: &str,
) -> TextPreview {
    let mut redactor_and_reason = None;
    if let Some(redacted_msg) = latest_json {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Redacted(redaction)
            )
        )) = redacted_msg.deserialize() {
            if let Ok(redacted_because) = redaction.unsigned.redacted_because.deserialize() {
                let reason = match &redacted_because {
                    AnyRedactionEvent::RoomRedaction(e) => e.content.reason.clone(),
                    _ => None,
                };
                redactor_and_reason = Some((
                    redacted_because.sender().to_owned(),
                    reason,
                ));
            }
        }
    }
    let text = match redactor_and_reason {
        Some((redactor, Some(reason))) => {
            if redactor == sender_user_id {
                format!("{} deleted their own message: \"{}\".", original_sender_username, reason)
            } else {
                format!("{} deleted {}'s message: \"{}\".", redactor, original_sender_username, reason)
            }
        }
        Some((redactor, None)) => {
            if redactor == sender_user_id {
                format!("{} deleted their own message.", original_sender_username)
            } else {
                format!("{} deleted {}'s message.", redactor, original_sender_username)
            }
        }
        None => {
            format!("{}'s message was deleted.", original_sender_username)
        }
    };
    TextPreview::from((text, BeforeText::Nothing))
}


/// Returns a plaintext preview of the given encrypted message that could not be decrypted.
///
/// This is used for "Unable to decrypt" messages, which may have a known cause
/// for why they could not be decrypted.
pub fn text_preview_of_encrypted_message(
    encrypted_message: &EncryptedMessage,
) -> TextPreview {
    let cause_str = match encrypted_message {
        EncryptedMessage::MegolmV1AesSha2 { cause, .. } => match cause {
            UtdCause::Unknown => None,
            UtdCause::SentBeforeWeJoined => Some(
                "this message was sent before you joined the room."
            ),
            UtdCause::VerificationViolation => Some(
                "this message was sent by an unverified user."
            ),
            UtdCause::UnsignedDevice => Some(
                "the sending device wasn't signed by its owner."
            ),
            UtdCause::UnknownDevice => Some(
                "the sending device's signature was not found."
            ),
            UtdCause::HistoricalMessageAndBackupIsDisabled => Some(
                "historical messages are not available on this device because server-side key backup was disabled."
            ),
            UtdCause::WithheldForUnverifiedOrInsecureDevice => Some(
                "your device doesn't meet the sender's security requirements."
            ),
            UtdCause::WithheldBySender => Some(
                "the sender withheld this message from you."
            ),
            UtdCause::HistoricalMessageAndDeviceIsUnverified => Some(
                "historical messages are not available; you must verify this device."
            ),
        }
        _ => None,
    };
    let text = if let Some(cause) = cause_str {
        format!("Unable to decrypt: {cause}")
    } else {
        String::from("Unable to decrypt this message.")
    };
    TextPreview::from((text, BeforeText::UsernameWithColon))
}

/// Returns a plaintext preview of the given other message-like event.
pub fn text_preview_of_other_message_like(
    other_msg_like: &OtherMessageLike,
) -> TextPreview {
    TextPreview::from((
        format!("[Other message type: {}]", other_msg_like.event_type()),
        BeforeText::UsernameWithColon,
    ))
}

/// Returns a text preview of the given other state event as an Html-formatted string.
pub fn text_preview_of_other_state(
    other_state: &timeline::OtherState,
    format_as_html: bool,
) -> TextPreview {
    let text = match other_state.content() {
        AnyOtherStateEventContentChange::RoomAvatar(_) => {
            String::from("set this room's avatar picture.")
        }
        AnyOtherStateEventContentChange::RoomCanonicalAlias(StateEventContentChange::Original { content, .. }) => {
            format!("set the main address of this room to {}.",
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("none")
            )
        }
        AnyOtherStateEventContentChange::RoomCreate(StateEventContentChange::Original { content, .. }) => {
            format!("created this room (v{}).", content.room_version.as_str())
        }
        AnyOtherStateEventContentChange::RoomEncryption(_) => {
            String::from("enabled encryption in this room.")
        }
        AnyOtherStateEventContentChange::RoomGuestAccess(StateEventContentChange::Original { content, .. }) => {
            match &content.guest_access {
                GuestAccess::CanJoin => String::from("has allowed guests to join this room."),
                GuestAccess::Forbidden => String::from("has forbidden guests from joining this room."),
                custom => format!("has set custom guest access rules for this room: {}.", escaped(custom.as_str(), format_as_html)),
            }
        }
        AnyOtherStateEventContentChange::RoomHistoryVisibility(StateEventContentChange::Original { content, .. }) => {
            format!("set this room's history to be visible by {}.",
                match &content.history_visibility {
                    HistoryVisibility::Invited => Cow::Borrowed("invited users, since they were invited"),
                    HistoryVisibility::Joined => Cow::Borrowed("joined users, since they joined"),
                    HistoryVisibility::Shared => Cow::Borrowed("joined users, for all of time"),
                    HistoryVisibility::WorldReadable => Cow::Borrowed("anyone for all time"),
                    custom => escaped(custom.as_str(), format_as_html),
                },
            )
        }
        AnyOtherStateEventContentChange::RoomJoinRules(StateEventContentChange::Original { content, .. }) => {
            match &content.join_rule {
                JoinRule::Public => String::from("set this room to be joinable by anyone."),
                JoinRule::Knock => String::from("set this room to be joinable by invite only or by request."),
                JoinRule::Private => String::from("set this room to be private."),
                JoinRule::Restricted(_) => String::from("set this room to be joinable by invite only or with restrictions."),
                JoinRule::KnockRestricted(_) => String::from("set this room to be joinable by invite only or requestable with restrictions."),
                JoinRule::Invite  => String::from("set this room to be joinable by invite only."),
                custom => format!("set custom join rules for this room: {}.", escaped(custom.as_str(), format_as_html)),
            }
        }
        AnyOtherStateEventContentChange::RoomPinnedEvents(StateEventContentChange::Original { content, prev_content }) => {
            let messages = |n: usize| if n == 1 { "message" } else { "messages" };
            let now_pinned = || -> Cow<'static, str> {
                match content.pinned.len() {
                    1 => "1 is now pinned".into(),
                    n => format!("{n} are now pinned").into(),
                }
            };
            match prev_content.as_ref().map(|prev| prev.pinned.as_deref()) {
                // We can't tell what changed if the previously pinned messages are unknown.
                Some(None) if content.pinned.is_empty() => String::from("unpinned all messages."),
                Some(None) => format!("changed the pinned messages, {}.", now_pinned()),
                // Without any previously pinned messages, all of them were just pinned.
                prev => {
                    let prev = prev.flatten().unwrap_or_default();
                    let pinned = content.pinned.iter().filter(|id| !prev.contains(id)).count();
                    let unpinned = prev.iter().filter(|id| !content.pinned.contains(id)).count();
                    match (pinned, unpinned) {
                        (0, 0) if prev == content.pinned.as_slice() => String::from("made no changes to the pinned messages."),
                        (0, 0) => String::from("reordered the pinned messages."),
                        (0, 1) if content.pinned.is_empty() => String::from("unpinned the last pinned message."),
                        (0, u) if content.pinned.is_empty() => format!("unpinned all {u} messages."),
                        // When nothing was pinned before, the new total would just repeat `p`.
                        (p, 0) if prev.is_empty() => format!("pinned {p} {}.", messages(p)),
                        (p, 0) => format!("pinned {p} {}, {}.", messages(p), now_pinned()),
                        (0, u) => format!("unpinned {u} {}, {}.", messages(u), now_pinned()),
                        (p, u) => format!("pinned {p} {} and unpinned {u} {}, {}.", messages(p), messages(u), now_pinned()),
                    }
                }
            }
        }
        AnyOtherStateEventContentChange::RoomName(StateEventContentChange::Original { content, .. }) => {
            let name = if format_as_html {
                htmlize::escape_text(&content.name)
            } else {
                Cow::Borrowed(content.name.as_str())
            };
            format!("changed this room's name to \"{name}\".")
        }
        AnyOtherStateEventContentChange::RoomPowerLevels(_) => {
            String::from("set the power levels for this room.")
        }
        AnyOtherStateEventContentChange::RoomServerAcl(_) => {
            String::from("set the server access control list for this room.")
        }
        AnyOtherStateEventContentChange::RoomThirdPartyInvite(StateEventContentChange::Original { content, prev_content }) => {
            if is_revoked_email_invite(content) {
                // The invitation it revokes usually says who it was for.
                match prev_content.as_ref().map(|prev| prev.display_name.trim()).filter(|name| !name.is_empty()) {
                    Some(invitee) => format!("revoked {}'s invitation to this room.", escaped(invitee, format_as_html)),
                    None => String::from("revoked an email invitation."),
                }
            } else if content.display_name.trim().is_empty() {
                String::from("invited someone to this room by email.")
            } else {
                format!("invited {} to this room.", escaped(&content.display_name, format_as_html))
            }
        }
        AnyOtherStateEventContentChange::RoomThirdPartyInvite(StateEventContentChange::Redacted(_)) => {
            String::from("updated an email invitation.")
        }
        AnyOtherStateEventContentChange::RoomTombstone(StateEventContentChange::Original { content, .. }) => {
            format!("closed this room and upgraded it to {}.", content.replacement_room.matrix_to_uri())
        }
        AnyOtherStateEventContentChange::RoomTopic(StateEventContentChange::Original { content, .. }) => {
            let topic = if format_as_html {
                htmlize::escape_text(&content.topic)
            } else {
                Cow::Borrowed(content.topic.as_str())
            };
            format!("changed this room's topic to \"{topic}\".")
        }
        AnyOtherStateEventContentChange::SpaceParent(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(other_state.state_key())
            } else {
                Cow::Borrowed(other_state.state_key())
            };
            format!("set this room's parent space to \"{state_key}\".")
        }
        AnyOtherStateEventContentChange::SpaceChild(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(other_state.state_key())
            } else {
                Cow::Borrowed(other_state.state_key())
            };
            format!("added a new child to this space: \"{state_key}\".")
        }
        other => {
            let event_type = other.event_type().to_string();
            format!("changed this room's {} state.",
                if format_as_html { htmlize::escape_text(event_type) } else { event_type.into() },
            )
        }
    };
    TextPreview::from((text, BeforeText::UsernameWithoutColon))
}


/// Returns a text preview of the given member profile change
/// as a plaintext or HTML-formatted string.
pub fn text_preview_of_member_profile_change(
    change: &MemberProfileChange,
    username: &str,
    format_as_html: bool,
) -> TextPreview {
    let name_text = if let Some(name_change) = change.displayname_change() {
        let old = name_change.old.as_deref().filter(|name| !name.trim().is_empty()).unwrap_or(username);
        let old_un = if format_as_html { htmlize::escape_text(old) } else { old.into() };
        // Setting an empty name is the same as removing it.
        if let Some(new) = name_change.new.as_ref().filter(|name| !name.trim().is_empty()) {
            let new_un = if format_as_html { htmlize::escape_text(new) } else { new.into() };
            format!("{old_un} changed their display name to \"{new_un}\"")
        } else {
            format!("{old_un} removed their display name")
        }
    } else {
        String::new()
    };
    let avatar_text = if let Some(avatar_change) = change.avatar_url_change() {
        let verb = if avatar_change.new.is_some() { "changed" } else { "removed" };
        if name_text.is_empty() {
            let un = if format_as_html {
                htmlize::escape_text(username)
            } else {
                username.into()
            };
            format!("{un} {verb} their profile picture")
        } else {
            format!(" and {verb} their profile picture")
        }
    } else {
        String::new()
    };

    if name_text.is_empty() && avatar_text.is_empty() {
        // When a profile change is redacted, both these fields are cleared,
        // so just fall back to a generic message.
        return TextPreview::from((
            String::from("changed their profile."),
            BeforeText::UsernameWithoutColon,
        ));
    }

    TextPreview::from((
        format!("{}{}.", name_text, avatar_text),
        BeforeText::Nothing,
    ))
}


/// The given text, HTML-escaped if it's going into HTML.
fn escaped(text: &str, as_html: bool) -> Cow<'_, str> {
    if as_html { htmlize::escape_text(text) } else { Cow::Borrowed(text) }
}

/// Whether this is the empty invitation that revokes an earlier email invitation.
pub fn is_revoked_email_invite(content: &RoomThirdPartyInviteEventContent) -> bool {
    content.display_name.is_empty() && content.key_validity_url.is_empty()
}

/// The membership that the given change results in, plus the reason given for it, if any.
pub fn membership_and_reason(change: &RoomMembershipChange) -> (&MembershipState, Option<&str>) {
    match change.content() {
        StateEventContentChange::Original { content, .. } => (&content.membership, content.reason.as_deref()),
        StateEventContentChange::Redacted(content) => (&content.membership, None),
    }
}

/// A membership state, simplified for telling membership changes apart.
pub fn membership_of(state: &MembershipState) -> Membership {
    match state {
        MembershipState::Join => Membership::Join,
        MembershipState::Leave => Membership::Leave,
        MembershipState::Invite => Membership::Invite,
        MembershipState::Ban => Membership::Ban,
        MembershipState::Knock => Membership::Knock,
        _ => Membership::Custom,
    }
}

/// Determines what the given membership change actually resulted in.
///
/// This uses the given `previous` callback to help determine what kind of
/// membership transition actually happened, which is needed in common cases
/// where the SDK doesn't tell us what happened, e.g., because the event
/// arrived redacted or without its previous content.
pub fn membership_transition_of(
    change: &RoomMembershipChange,
    sender: &UserId,
    previous: impl FnOnce() -> Option<Membership>,
) -> MembershipTransition {
    membership_transition_from(change.change(), change.content(), sender == change.user_id(), previous)
}

/// The guts of [`membership_transition_of()`], minus the SDK type that tests can't build.
fn membership_transition_from(
    change: Option<MembershipChange>,
    content: &StateEventContentChange<RoomMemberEventContent>,
    by_self: bool,
    previous: impl FnOnce() -> Option<Membership>,
) -> MembershipTransition {
    use MembershipTransition as MT;
    // `prev_content` is `None` if the event was redacted, and `Some(None)` if the server didn't send one.
    let (now, prev_content) = match content {
        StateEventContentChange::Original { content, prev_content } => (
            membership_of(&content.membership),
            Some(prev_content.as_ref().map(|prev| membership_of(&prev.membership))),
        ),
        StateEventContentChange::Redacted(content) => (membership_of(&content.membership), None),
    };
    match change {
        Some(MembershipChange::Joined)             => MT::Joined,
        Some(MembershipChange::Left)               => MT::Left,
        Some(MembershipChange::Banned)             => MT::Banned,
        Some(MembershipChange::Unbanned)           => MT::Unbanned,
        Some(MembershipChange::Kicked)             => MT::Kicked,
        Some(MembershipChange::KickedAndBanned)    => MT::KickedAndBanned,
        Some(MembershipChange::Invited)            => MT::Invited,
        Some(MembershipChange::InvitationAccepted) => MT::InvitationAccepted,
        Some(MembershipChange::InvitationRejected) => MT::InvitationRejected,
        Some(MembershipChange::InvitationRevoked)  => MT::InvitationRevoked,
        Some(MembershipChange::Knocked)            => MT::Knocked,
        Some(MembershipChange::KnockAccepted)      => MT::KnockAccepted,
        Some(MembershipChange::KnockRetracted)     => MT::KnockRetracted,
        Some(MembershipChange::KnockDenied)        => MT::KnockDenied,
        // The SDK treats a missing `prev_content` as a "leave" action,
        // so this could actually be a kick, unban, etc.
        Some(MembershipChange::None) if now == Membership::Leave && prev_content.flatten().is_none() => {
            membership_transition(previous(), now, by_self)
        }
        Some(MembershipChange::None) if now == Membership::Leave && by_self => MT::Left,
        Some(MembershipChange::None)               => MT::Unchanged(now),
        None | Some(MembershipChange::NotImplemented | MembershipChange::Error) => {
            let before = match prev_content {
                Some(Some(prev)) => Some(prev),
                _ => previous(),
            };
            membership_transition(before, now, by_self)
        }
    }
}

/// Returns a text preview of the given room membership change
/// as a plaintext or HTML-formatted string.
pub fn text_preview_of_room_membership_change(
    change: &RoomMembershipChange,
    sender: &UserId,
    format_as_html: bool,
) -> TextPreview {
    let transition = membership_transition_of(change, sender, || None);
    text_preview_of_membership_transition(change, sender, transition, format_as_html)
}

/// Returns a text preview of what the given room membership change did
/// as either a plaintext or HTML-formatted string.
pub fn text_preview_of_membership_transition(
    change: &RoomMembershipChange,
    sender: &UserId,
    transition: MembershipTransition,
    format_as_html: bool,
) -> TextPreview {
    use MembershipTransition as MT;
    let dn = change.display_name().filter(|name| !name.trim().is_empty());
    let target = dn.as_deref().unwrap_or_else(|| change.user_id().as_str());
    let target = if format_as_html {
        htmlize::escape_text(target)
    } else {
        target.into()
    };
    let (membership, reason) = membership_and_reason(change);
    let end = match reason.map(|r| r.trim().trim_end_matches('.')).filter(|r| !r.is_empty()) {
        Some(r) if format_as_html => format!(": {}.", htmlize::escape_text(r)),
        Some(r) => format!(": {r}."),
        None => String::from("."),
    };
    let whose = if sender == change.user_id() {
        Cow::Borrowed("their")
    } else {
        Cow::Owned(format!("{target}'s"))
    };
    let text = match transition {
        MT::Joined => String::from("joined this room."),
        MT::Left => format!("left this room{end}"),
        MT::Banned => format!("banned {target} from this room{end}"),
        MT::Unbanned => format!("unbanned {target} from this room."),
        MT::Kicked => format!("kicked {target} from this room{end}"),
        MT::Invited => format!("invited {target} to this room."),
        MT::KickedAndBanned => format!("kicked and banned {target} from this room{end}"),
        MT::InvitationAccepted => String::from("accepted an invitation to this room."),
        MT::InvitationRejected => format!("rejected an invitation to this room{end}"),
        MT::InvitationRevoked => format!("revoked {target}'s invitation to this room{end}"),
        MT::Knocked => format!("requested to join this room{end}"),
        MT::KnockAccepted => format!("accepted {target}'s request to join this room."),
        MT::KnockRetracted => String::from("retracted their request to join this room."),
        MT::KnockDenied => format!("denied {target}'s request to join this room{end}"),
        MT::Unchanged(Membership::Ban) => format!("updated {target}'s ban{end}"),
        MT::Unchanged(Membership::Invite) => format!("invited {target} to this room again{end}"),
        MT::Unchanged(Membership::Knock) => format!("requested to join this room again{end}"),
        MT::Unchanged(Membership::Join) => String::from("made no changes to their membership."),
        MT::Unchanged(_) => format!("updated {whose} membership{end}"),
        MT::Removed => format!("removed {target}{end}"),
        MT::ProfileChanged => String::from("changed their profile."),
        MT::JoinedOrChangedProfile => String::from("joined this room or changed their profile."),
        MT::Custom => {
            let custom = if format_as_html {
                htmlize::escape_text(membership.as_str())
            } else {
                membership.as_str().into()
            };
            format!("set {whose} membership to \"{custom}\".")
        }
    };
    TextPreview::from((text, BeforeText::UsernameWithoutColon))
}

#[cfg(test)]
mod tests {
    use matrix_sdk::ruma::events::room::member::{MembershipState as St, PossiblyRedactedRoomMemberEventContent, RedactedRoomMemberEventContent};
    use super::*;
    use MembershipTransition as T;

    fn original(now: St, prev: Option<St>) -> StateEventContentChange<RoomMemberEventContent> {
        StateEventContentChange::Original { content: RoomMemberEventContent::new(now), prev_content: prev.map(PossiblyRedactedRoomMemberEventContent::new) }
    }

    fn redacted(now: St) -> StateEventContentChange<RoomMemberEventContent> {
        StateEventContentChange::Redacted(RedactedRoomMemberEventContent::new(now))
    }

    /// For events that should settle it on their own.
    fn no_history() -> Option<Membership> {
        panic!("shouldn't need to look back")
    }

    #[test]
    fn the_sdks_reading_wins_when_it_has_one() {
        let kick = original(St::Leave, Some(St::Join));
        assert_eq!(membership_transition_from(Some(MembershipChange::Kicked), &kick, false, no_history), T::Kicked);
    }

    #[test]
    fn a_leave_without_prev_content_goes_by_what_came_before() {
        // ruma reads the missing `prev_content` as "leave", so it calls someone else's leave no change.
        let none = Some(MembershipChange::None);
        let leave = original(St::Leave, None);
        assert_eq!(membership_transition_from(none, &leave, false, || Some(Membership::Join)), T::Kicked);
        assert_eq!(membership_transition_from(none, &leave, false, || Some(Membership::Ban)), T::Unbanned);
        assert_eq!(membership_transition_from(none, &leave, false, || Some(Membership::Invite)), T::InvitationRevoked);
        assert_eq!(membership_transition_from(none, &leave, false, || None), T::Removed);
        assert_eq!(membership_transition_from(none, &leave, true, || Some(Membership::Invite)), T::InvitationRejected);
        // Same once it's been redacted (the SDK keeps what it read before that).
        assert_eq!(membership_transition_from(none, &redacted(St::Leave), false, || Some(Membership::Knock)), T::KnockDenied);
    }

    #[test]
    fn prev_content_settles_it_without_looking_back() {
        let none = Some(MembershipChange::None);
        assert_eq!(membership_transition_from(none, &original(St::Leave, Some(St::Leave)), true, no_history), T::Left);
        assert_eq!(membership_transition_from(none, &original(St::Leave, Some(St::Leave)), false, no_history), T::Unchanged(Membership::Leave));
        assert_eq!(membership_transition_from(none, &original(St::Ban, Some(St::Ban)), false, no_history), T::Unchanged(Membership::Ban));
        let accepted = original(St::Invite, Some(St::Knock));
        assert_eq!(membership_transition_from(Some(MembershipChange::NotImplemented), &accepted, false, no_history), T::KnockAccepted);
        let invited = original(St::Invite, Some(St::Join));
        assert_eq!(membership_transition_from(Some(MembershipChange::Error), &invited, false, no_history), T::Invited);
    }

    #[test]
    fn events_that_arrived_redacted_go_by_what_came_before() {
        assert_eq!(membership_transition_from(None, &redacted(St::Join), true, || Some(Membership::Join)), T::ProfileChanged);
        assert_eq!(membership_transition_from(None, &redacted(St::Join), true, || None), T::JoinedOrChangedProfile);
        assert_eq!(membership_transition_from(None, &redacted(St::Leave), false, || Some(Membership::Join)), T::Kicked);
    }
}
