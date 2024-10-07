//! Functions for generating text previews of timeline events.
//!
//! These text previews are used for:
//! * inline replies within the timeline
//! * preview of a message being replied to above the message input box
//! * previews of each room's latest message in the rooms list

use matrix_sdk::ruma::events::{room::{guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule, message::{MessageFormat, MessageType}}, AnySyncMessageLikeEvent, AnySyncTimelineEvent, FullStateEventContent, SyncMessageLikeEvent};
use matrix_sdk_ui::timeline::{self, AnyOtherFullStateEventContent, EventTimelineItem, MemberProfileChange, MembershipChange, RoomMembershipChange, TimelineItemContent};

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
    pub fn format_with(self, username: &str) -> String {
        let Self { text, before_text } = self;
        match before_text {
            BeforeText::Nothing => text,
            BeforeText::UsernameWithColon => format!("<b>{username}</b>: {text}"),
            BeforeText::UsernameWithoutColon => format!("{username} {text}"),
        }
    }
}

/// Returns a text preview of the given timeline event as an Html-formatted string.
pub fn text_preview_of_timeline_item(
    content: &TimelineItemContent,
    sender_username: &str,
) -> TextPreview {
    match content {
        TimelineItemContent::Message(m) => text_preview_of_message(m, sender_username),
        TimelineItemContent::RedactedMessage => TextPreview::from((
            String::from("[Message was deleted]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::Sticker(sticker) => TextPreview::from((
            format!("[Sticker]: <i>{}</i>", sticker.content().body),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::UnableToDecrypt(_encrypted_msg) => TextPreview::from((
            String::from("[Unable to decrypt message]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::MembershipChange(membership_change) => {
            text_preview_of_room_membership_change(membership_change)
                .unwrap_or_else(|| TextPreview::from((
                    String::from("<i>underwent a membership change</i>"),
                    BeforeText::UsernameWithoutColon,
                )))
        }
        TimelineItemContent::ProfileChange(profile_change) => {
            text_preview_of_member_profile_change(profile_change, sender_username)
        }
        TimelineItemContent::OtherState(other_state) => {
            text_preview_of_other_state(other_state)
                .unwrap_or_else(|| TextPreview::from((
                    String::from("<i>initiated another state change</i>"),
                    BeforeText::UsernameWithoutColon,
                )))
        }
        TimelineItemContent::FailedToParseMessageLike { event_type, .. } => TextPreview::from((
            format!("[Failed to parse <i>{}</i> message]", event_type),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::FailedToParseState { event_type, .. } => TextPreview::from((
            format!("[Failed to parse <i>{}</i> state]", event_type),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::Poll(poll_state) => TextPreview::from((
            format!("[Poll]: {}", poll_state.fallback_text()
                .unwrap_or_else(|| poll_state.results().question)
            ),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::CallInvite => TextPreview::from((
            String::from("[Call Invitation]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::CallNotify => TextPreview::from((
            String::from("[Call Notification]"),
            BeforeText::UsernameWithColon,
        )),
    }
}

/// Returns a text preview of the given message as an Html-formatted string.
pub fn text_preview_of_message(
    message: &timeline::Message,
    sender_username: &str,
) -> TextPreview {
    let text = match message.msgtype() {
        MessageType::Audio(audio) => format!(
            "[Audio]: <i>{}</i>",
            if let Some(formatted_body) = audio.formatted.as_ref() {
                &formatted_body.body
            } else {
                &audio.body
            }
        ),
        MessageType::Emote(emote) => format!(
            "<i>{} {}</i>",
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
        MessageType::Notice(notice) => format!("[Notice]: <i>{}</i>",
            if let Some(formatted_body) = notice.formatted.as_ref() {
                &formatted_body.body
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
            text.formatted.as_ref()
                .and_then(|fb| (fb.format == MessageFormat::Html)
                    .then(|| utils::linkify(&fb.body).to_string())
                )
                .unwrap_or_else(|| utils::linkify(&text.body).to_string())
        }
        MessageType::VerificationRequest(verification) => format!(
            "[Verification Request] <i>from device {} to user {}</i>",
            verification.from_device,
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
    TextPreview::from((text, BeforeText::UsernameWithColon))
}


/// Returns a text preview of a redacted message of the given event as an Html-formatted string.
pub fn text_preview_of_redacted_message(
    event_tl_item: &EventTimelineItem,
    original_sender: &str,
) -> TextPreview {
    let redactor_and_reason = {
        let mut rr = None;
        if let Some(redacted_msg) = event_tl_item.latest_json() {
            if let Ok(old) = redacted_msg.deserialize() {
                if let AnySyncTimelineEvent::MessageLike(
                    AnySyncMessageLikeEvent::RoomMessage(
                        SyncMessageLikeEvent::Redacted(redaction)
                    )
                ) = old {
                    rr = Some((
                        redaction.unsigned.redacted_because.sender,
                        redaction.unsigned.redacted_because.content.reason,
                    ));
                }
            }
        }
        rr
    };
    let text = match redactor_and_reason {
        Some((redactor, Some(reason))) => {
            // TODO: get the redactor's display name if possible
            format!("{} deleted {}'s message: {:?}.", redactor, original_sender, reason)
        }
        Some((redactor, None)) => {
            if redactor == event_tl_item.sender() {
                format!("{} deleted their own message.", original_sender)
            } else {
                format!("{} deleted {}'s message.", redactor, original_sender)
            }
        }
        None => {
            format!("{}'s message was deleted.", original_sender)
        }
    };
    TextPreview::from((text, BeforeText::Nothing))
}


/// Returns a text preview of the given other state event as an Html-formatted string.
pub fn text_preview_of_other_state(
    other_state: &timeline::OtherState,
) -> Option<TextPreview> {
    let text = match other_state.content() {
        AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content, .. }) => {
            let mut s = format!("set this room's aliases to ");
            let last_alias = content.aliases.len() - 1;
            for (i, alias) in content.aliases.iter().enumerate() {
                s.push_str(alias.as_str());
                if i != last_alias {
                    s.push_str(", ");
                }
            }
            s.push_str(".");
            Some(s)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            Some(format!("set this room's avatar picture."))
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(FullStateEventContent::Original { content, .. }) => {
            Some(format!("set the main address of this room to {}.",
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("none")
            ))
        }
        AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { content, .. }) => {
            Some(format!("created this room (v{}).", content.room_version.as_str()))
        }
        AnyOtherFullStateEventContent::RoomEncryption(_) => {
            Some(format!("enabled encryption in this room."))
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content, .. }) => {
            Some(match content.guest_access {
                GuestAccess::CanJoin => format!("has allowed guests to join this room."),
                GuestAccess::Forbidden | _ => format!("has forbidden guests from joining this room."),
            })
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content, .. }) => {
            let visibility = match content.history_visibility {
                HistoryVisibility::Invited => "invited users, since they were invited.",
                HistoryVisibility::Joined => "joined users, since they joined.",
                HistoryVisibility::Shared => "joined users, for all of time.",
                HistoryVisibility::WorldReadable | _ => "anyone for all time.",
            };
            Some(format!("set this room's history to be visible by {}", visibility))
        }
        AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content, .. }) => {
            Some(match content.join_rule {
                JoinRule::Public => format!("set this room to be joinable by anyone."),
                JoinRule::Knock => format!("set this room to be joinable by invite only or by request."),
                JoinRule::Private => format!("set this room to be private."),
                JoinRule::Restricted(_) => format!("set this room to be joinable by invite only or with restrictions."),
                JoinRule::KnockRestricted(_) => format!("set this room to be joinable by invite only or requestable with restrictions."),
                JoinRule::Invite | _ => format!("set this room to be joinable by invite only."),
            })
        }
        AnyOtherFullStateEventContent::RoomPinnedEvents(FullStateEventContent::Original { content, .. }) => {
            Some(format!("pinned {} events in this room.", content.pinned.len()))
        }
        AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's name to {:?}.", content.name))
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            Some(format!("set the power levels for this room."))
        }
        AnyOtherFullStateEventContent::RoomServerAcl(_) => {
            Some(format!("set the server access control list for this room."))
        }
        AnyOtherFullStateEventContent::RoomTombstone(FullStateEventContent::Original { content, .. }) => {
            Some(format!("closed this room and upgraded it to {}", content.replacement_room.matrix_to_uri()))
        }
        AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content, .. }) => {
            Some(format!("changed this room's topic to {:?}.", content.topic))
        }
        AnyOtherFullStateEventContent::SpaceParent(_) => {
            Some(format!("set this room's parent space to {}.", other_state.state_key()))
        }
        AnyOtherFullStateEventContent::SpaceChild(_) => {
            Some(format!("added a new child to this space: {}.", other_state.state_key()))
        }
        _other => {
            // log!("*** Unhandled: {:?}.", _other);
            None
        }
    };
    text.map(|t| TextPreview::from((t, BeforeText::UsernameWithoutColon)))
}


/// Returns a text preview of the given member profile change as an Html-formatted string.
pub fn text_preview_of_member_profile_change(
    change: &MemberProfileChange,
    username: &str,
) -> TextPreview {
    let name_text = if let Some(name_change) = change.displayname_change() {
        let old = name_change.old.as_deref().unwrap_or(&username);
        if let Some(new) = name_change.new.as_ref() {
            format!("{old} changed their display name to {new:?}")
        } else {
            format!("{old} removed their display name")
        }
    } else {
        String::new()
    };
    let avatar_text = if let Some(_avatar_change) = change.avatar_url_change() {
        if name_text.is_empty() {
            format!("{} changed their profile picture", username)
        } else {
            format!(" and changed their profile picture")
        }
    } else {
        String::new()
    };

    TextPreview::from((
        format!("{}{}.", name_text, avatar_text),
        BeforeText::Nothing,
    ))
}


/// Returns a text preview of the given room membership change as an Html-formatted string.
pub fn text_preview_of_room_membership_change(
    change: &RoomMembershipChange,
) -> Option<TextPreview> {
    let dn = change.display_name();
    let change_user_id = dn.as_deref()
        .unwrap_or_else(|| change.user_id().as_str());
    let text = match change.change() {
        None
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None)
        | Some(MembershipChange::Error) => {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return None;
        }
        Some(MembershipChange::Joined) =>
            format!("joined this room."),
        Some(MembershipChange::Left) =>
            format!("left this room."),
        Some(MembershipChange::Banned) =>
            format!("banned {} from this room.", change_user_id),
        Some(MembershipChange::Unbanned) =>
            format!("unbanned {} from this room.", change_user_id),
        Some(MembershipChange::Kicked) =>
            format!("kicked {} from this room.", change_user_id),
        Some(MembershipChange::Invited) =>
            format!("invited {} to this room.", change_user_id),
        Some(MembershipChange::KickedAndBanned) =>
            format!("kicked and banned {} from this room.", change_user_id),
        Some(MembershipChange::InvitationAccepted) =>
            format!("accepted an invitation to this room."),
        Some(MembershipChange::InvitationRejected) =>
            format!("rejected an invitation to this room."),
        Some(MembershipChange::InvitationRevoked) =>
            format!("revoked {}'s invitation to this room.", change_user_id),
        Some(MembershipChange::Knocked) =>
            format!("requested to join this room."),
        Some(MembershipChange::KnockAccepted) =>
            format!("accepted {}'s request to join this room.", change_user_id),
        Some(MembershipChange::KnockRetracted) =>
            format!("retracted their request to join this room."),
        Some(MembershipChange::KnockDenied) =>
            format!("denied {}'s request to join this room.", change_user_id),
    };
    Some(TextPreview::from((text, BeforeText::UsernameWithoutColon)))
}
