//! Functions for generating text previews of timeline events.
//!
//! These text previews are used for:
//! * inline replies within the timeline
//! * preview of a message being replied to above the message input box
//! * previews of each room's latest message in the rooms list

use std::borrow::Cow;

use matrix_sdk::{ruma::{OwnedUserId, events::{room::{guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule, member::MembershipState, message::{MessageFormat, MessageType}}, AnyRedactionEvent, AnySyncMessageLikeEvent, AnySyncTimelineEvent, StateEventContentChange, SyncMessageLikeEvent}, serde::Raw, UserId}};
use matrix_sdk_base::crypto::types::events::UtdCause;
use matrix_sdk_ui::timeline::{self, AnyOtherStateEventContentChange, EncryptedMessage, EventTimelineItem, MemberProfileChange, MembershipChange, MsgLikeKind, OtherMessageLike, RoomMembershipChange, TimelineItemContent};

use crate::utils;

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
                custom => format!("has set custom guest access rules for this room: {}", custom.as_str()),
            }
        }
        AnyOtherStateEventContentChange::RoomHistoryVisibility(StateEventContentChange::Original { content, .. }) => {
            format!("set this room's history to be visible by {}",
                match &content.history_visibility {
                    HistoryVisibility::Invited => "invited users, since they were invited.",
                    HistoryVisibility::Joined => "joined users, since they joined.",
                    HistoryVisibility::Shared => "joined users, for all of time.",
                    HistoryVisibility::WorldReadable => "anyone for all time.",
                    custom => custom.as_str(),
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
                custom => format!("set custom join rules for this room: {}", custom.as_str()),
            }
        }
        AnyOtherStateEventContentChange::RoomPinnedEvents(StateEventContentChange::Original { content, .. }) => {
            format!("pinned {} events in this room.", content.pinned.len())
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
        AnyOtherStateEventContentChange::RoomThirdPartyInvite(StateEventContentChange::Original { content, .. }) => {
            let invitee = if format_as_html {
                htmlize::escape_text(&content.display_name)
            } else {
                Cow::Borrowed(content.display_name.as_str())
            };
            format!("invited {invitee} to this room.")
        }
        AnyOtherStateEventContentChange::RoomTombstone(StateEventContentChange::Original { content, .. }) => {
            format!("closed this room and upgraded it to {}", content.replacement_room.matrix_to_uri())
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
        let old = name_change.old.as_deref().unwrap_or(username);
        let old_un = if format_as_html { htmlize::escape_text(old) } else { old.into() };
        if let Some(new) = name_change.new.as_ref() {
            let new_un = if format_as_html { htmlize::escape_text(new) } else { new.into() };
            format!("{old_un} changed their display name to \"{new_un}\"")
        } else {
            format!("{old_un} removed their display name")
        }
    } else {
        String::new()
    };
    let avatar_text = if let Some(_avatar_change) = change.avatar_url_change() {
        if name_text.is_empty() {
            let un = if format_as_html {
                htmlize::escape_text(username)
            } else {
                username.into()
            };
            format!("{un} changed their profile picture")
        } else {
            String::from(" and changed their profile picture")
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


/// Returns a text preview of the given room membership change
/// as a plaintext or HTML-formatted string.
pub fn text_preview_of_room_membership_change(
    change: &RoomMembershipChange,
    sender: &UserId,
    format_as_html: bool,
) -> TextPreview {
    let dn = change.display_name();
    let change_user_id = dn.as_deref()
        .unwrap_or_else(|| change.user_id().as_str());
    let change_user_id = if format_as_html {
        htmlize::escape_text(change_user_id)
    } else {
        change_user_id.into()
    };
    let (membership, prev_membership, reason) = match change.content() {
        StateEventContentChange::Original { content, prev_content } => (
            &content.membership,
            prev_content.as_ref().map(|prev| &prev.membership),
            content.reason.as_deref(),
        ),
        StateEventContentChange::Redacted(content) => (&content.membership, None, None),
    };
    let end = match reason.map(|r| r.trim_end_matches('.')) {
        Some(r) if format_as_html => format!(": {}.", htmlize::escape_text(r)),
        Some(r) => format!(": {r}."),
        None => String::from("."),
    };
    let text = match change.change() {
        Some(MembershipChange::Joined) =>
            String::from("joined this room."),
        Some(MembershipChange::Left) =>
            format!("left this room{end}"),
        Some(MembershipChange::Banned) =>
            format!("banned {change_user_id} from this room{end}"),
        Some(MembershipChange::Unbanned) =>
            format!("unbanned {change_user_id} from this room."),
        Some(MembershipChange::Kicked) =>
            format!("kicked {change_user_id} from this room{end}"),
        Some(MembershipChange::Invited) =>
            format!("invited {change_user_id} to this room."),
        Some(MembershipChange::KickedAndBanned) =>
            format!("kicked and banned {change_user_id} from this room{end}"),
        Some(MembershipChange::InvitationAccepted) =>
            String::from("accepted an invitation to this room."),
        Some(MembershipChange::InvitationRejected) =>
            format!("rejected an invitation to this room{end}"),
        Some(MembershipChange::InvitationRevoked) =>
            format!("revoked {change_user_id}'s invitation to this room{end}"),
        Some(MembershipChange::Knocked) =>
            format!("requested to join this room{end}"),
        Some(MembershipChange::KnockAccepted) =>
            format!("accepted {change_user_id}'s request to join this room."),
        Some(MembershipChange::KnockRetracted) =>
            String::from("retracted their request to join this room."),
        Some(MembershipChange::KnockDenied) =>
            format!("denied {change_user_id}'s request to join this room{end}"),
        // Anything else will be reported by ruma as one of the variants below,
        // so we treat them all the same and just check the new membership state.
        None
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None)
        | Some(MembershipChange::Error) => match membership {
            MembershipState::Invite =>
                format!("invited {change_user_id} to this room."),
            MembershipState::Knock =>
                format!("requested to join this room{end}"),
            MembershipState::Ban =>
                format!("banned {change_user_id} from this room{end}"),
            MembershipState::Leave if sender == change.user_id() =>
                format!("left this room{end}"),
            MembershipState::Leave =>
                format!("removed {change_user_id} from this room{end}"),
            // A join re-send is a no-op; the timeline hides it, so this string
            // only shows up in text-preview contexts.
            MembershipState::Join if prev_membership == Some(&MembershipState::Join) =>
                String::from("made no changes to their membership."),
            MembershipState::Join =>
                String::from("joined this room."),
            custom => {
                let custom = if format_as_html {
                    htmlize::escape_text(custom.as_str())
                } else {
                    custom.as_str().into()
                };
                format!("set {change_user_id}'s membership to \"{custom}\".")
            }
        }
    };
    TextPreview::from((text, BeforeText::UsernameWithoutColon))
}
