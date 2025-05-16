//! Functions for generating text previews of timeline events.
//!
//! These text previews are used for:
//! * inline replies within the timeline
//! * preview of a message being replied to above the message input box
//! * previews of each room's latest message in the rooms list

use std::borrow::Cow;

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
    pub fn format_with(
        self,
        username: &str,
        as_html: bool,
    ) -> String {
        let Self { text, before_text } = self;
        match before_text {
            BeforeText::Nothing => text,
            BeforeText::UsernameWithColon => format!(
                "<b>{}</b>: {}",
                if as_html { htmlize::escape_text(username) } else { username.into() },
                text,
            ),
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
    sender_username: &str,
) -> TextPreview {
    match content {
        TimelineItemContent::Message(m) => text_preview_of_message(m, sender_username),
        TimelineItemContent::RedactedMessage => TextPreview::from((
            String::from("[Message was deleted]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::Sticker(sticker) => TextPreview::from((
            format!("[Sticker]: <i>{}</i>", htmlize::escape_text(&sticker.content().body)),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::UnableToDecrypt(_encrypted_msg) => TextPreview::from((
            String::from("[Unable to decrypt message]"),
            BeforeText::UsernameWithColon,
        )),
        TimelineItemContent::MembershipChange(membership_change) => {
            text_preview_of_room_membership_change(membership_change, true)
                .unwrap_or_else(|| TextPreview::from((
                    String::from("<i>underwent a membership change</i>"),
                    BeforeText::UsernameWithoutColon,
                )))
        }
        TimelineItemContent::ProfileChange(profile_change) => {
            text_preview_of_member_profile_change(profile_change, sender_username, true)
        }
        TimelineItemContent::OtherState(other_state) => {
            text_preview_of_other_state(other_state.content(), true, other_state.state_key())
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
            format!(
                "[Poll]: {}",
                htmlize::escape_text(
                    poll_state.fallback_text()
                        .unwrap_or_else(|| poll_state.results().question)
                ),
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



/// Returns the plaintext `body` of the given timeline event.
pub fn plaintext_body_of_timeline_item(
    event_tl_item: &EventTimelineItem,
) -> String {
    match event_tl_item.content() {
        TimelineItemContent::Message(m) => m.body().into(),
        TimelineItemContent::RedactedMessage => "[Message was deleted]".into(),
        TimelineItemContent::Sticker(sticker) => sticker.content().body.clone(),
        TimelineItemContent::UnableToDecrypt(_encrypted_msg) => "[Unable to Decrypt]".into(),
        TimelineItemContent::MembershipChange(membership_change) => {
            text_preview_of_room_membership_change(membership_change, false)
                .unwrap_or_else(|| TextPreview::from((
                    String::from("underwent a membership change."),
                    BeforeText::UsernameWithoutColon,
                )))
                .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)
        }
        TimelineItemContent::ProfileChange(profile_change) => {
            text_preview_of_member_profile_change(
                profile_change,
                &utils::get_or_fetch_event_sender(event_tl_item, None),
                false,
            ).text
        }
        TimelineItemContent::OtherState(other_state) => {
            text_preview_of_other_state(other_state.content(), false, other_state.state_key())
                .unwrap_or_else(|| TextPreview::from((
                    String::from("initiated another state change."),
                    BeforeText::UsernameWithoutColon,
                )))
                .format_with(&utils::get_or_fetch_event_sender(event_tl_item, None), false)
        }
        TimelineItemContent::FailedToParseMessageLike { event_type, error } => {
            format!("Failed to parse {} message. Error: {}", event_type, error)
        }
        TimelineItemContent::FailedToParseState { event_type, error, state_key } => {
            format!("Failed to parse {} state; key: {}. Error: {}", event_type, state_key, error)
        }
        TimelineItemContent::Poll(poll_state) => {
            format!("[Poll]: {}", 
                poll_state.fallback_text().unwrap_or_else(|| poll_state.results().question)
            )
        }
        TimelineItemContent::CallInvite => String::from("[Call Invitation]"),
        TimelineItemContent::CallNotify => String::from("[Call Notification]"),
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
                    (fb.format == MessageFormat::Html).then(||
                        utils::linkify(
                            utils::trim_start_html_whitespace(&fb.body),
                            true,
                        )
                        .to_string()
                    )
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


/// Returns a text preview of a redacted message of the given event as an Html-formatted string.
pub fn text_preview_of_redacted_message(
    event_tl_item: &EventTimelineItem,
    original_sender: &str,
) -> TextPreview {
    let redactor_and_reason = {
        let mut rr = None;
        if let Some(redacted_msg) = event_tl_item.latest_json() {
            if let Ok(AnySyncTimelineEvent::MessageLike(
                AnySyncMessageLikeEvent::RoomMessage(
                    SyncMessageLikeEvent::Redacted(redaction)
                )
            )) = redacted_msg.deserialize() {
                rr = Some((
                    redaction.unsigned.redacted_because.sender,
                    redaction.unsigned.redacted_because.content.reason,
                ));
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
    other_state: &AnyOtherFullStateEventContent,
    format_as_html: bool,
    state_key: &str,
) -> Option<TextPreview> {
    let text = match other_state {
        AnyOtherFullStateEventContent::RoomAliases(FullStateEventContent::Original { content, .. }) => {
            let mut s = String::from("set this room's aliases to ");
            let last_alias = content.aliases.len() - 1;
            for (i, alias) in content.aliases.iter().enumerate() {
                s.push_str(alias.as_str());
                if i != last_alias {
                    s.push_str(", ");
                }
            }
            s.push('.');
            Some(s)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            Some(String::from("set this room's avatar picture."))
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
            Some(String::from("enabled encryption in this room."))
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(FullStateEventContent::Original { content, .. }) => {
            Some(match &content.guest_access {
                GuestAccess::CanJoin => String::from("has allowed guests to join this room."),
                GuestAccess::Forbidden => String::from("has forbidden guests from joining this room."),
                custom => format!("has set custom guest access rules for this room: {}", custom.as_str()),
            })
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(FullStateEventContent::Original { content, .. }) => {
            Some(format!("set this room's history to be visible by {}",
                match &content.history_visibility {
                    HistoryVisibility::Invited => "invited users, since they were invited.",
                    HistoryVisibility::Joined => "joined users, since they joined.",
                    HistoryVisibility::Shared => "joined users, for all of time.",
                    HistoryVisibility::WorldReadable => "anyone for all time.",
                    custom => custom.as_str(),
                },
            ))
        }
        AnyOtherFullStateEventContent::RoomJoinRules(FullStateEventContent::Original { content, .. }) => {
            Some(match &content.join_rule {
                JoinRule::Public => String::from("set this room to be joinable by anyone."),
                JoinRule::Knock => String::from("set this room to be joinable by invite only or by request."),
                JoinRule::Private => String::from("set this room to be private."),
                JoinRule::Restricted(_) => String::from("set this room to be joinable by invite only or with restrictions."),
                JoinRule::KnockRestricted(_) => String::from("set this room to be joinable by invite only or requestable with restrictions."),
                JoinRule::Invite  => String::from("set this room to be joinable by invite only."),
                custom => format!("set custom join rules for this room: {}", custom.as_str()),
            })
        }
        AnyOtherFullStateEventContent::RoomPinnedEvents(FullStateEventContent::Original { content, .. }) => {
            Some(format!("pinned {} events in this room.", content.pinned.len()))
        }
        AnyOtherFullStateEventContent::RoomName(FullStateEventContent::Original { content, .. }) => {
            let name = if format_as_html {
                htmlize::escape_text(&content.name)
            } else {
                Cow::Borrowed(content.name.as_str())
            };
            Some(format!("changed this room's name to \"{name}\"."))
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            Some(String::from("set the power levels for this room."))
        }
        AnyOtherFullStateEventContent::RoomServerAcl(_) => {
            Some(String::from("set the server access control list for this room."))
        }
        AnyOtherFullStateEventContent::RoomTombstone(FullStateEventContent::Original { content, .. }) => {
            Some(format!("closed this room and upgraded it to {}", content.replacement_room.matrix_to_uri()))
        }
        AnyOtherFullStateEventContent::RoomTopic(FullStateEventContent::Original { content, .. }) => {
            let topic = if format_as_html {
                htmlize::escape_text(&content.topic)
            } else {
                Cow::Borrowed(content.topic.as_str())
            };
            Some(format!("changed this room's topic to \"{topic}\"."))
        }
        AnyOtherFullStateEventContent::SpaceParent(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(state_key)
            } else {
                Cow::Borrowed(state_key)
            };
            Some(format!("set this room's parent space to \"{state_key}\"."))
        }
        AnyOtherFullStateEventContent::SpaceChild(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(state_key)
            } else {
                Cow::Borrowed(state_key)
            };
            Some(format!("added a new child to this space: \"{state_key}\"."))
        }
        _other => {
            // log!("*** Unhandled: {:?}.", _other);
            None
        }
    };
    text.map(|t| TextPreview::from((t, BeforeText::UsernameWithoutColon)))
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

    TextPreview::from((
        format!("{}{}.", name_text, avatar_text),
        BeforeText::Nothing,
    ))
}


/// Returns a text preview of the given room membership change
/// as a plaintext or HTML-formatted string.
pub fn text_preview_of_room_membership_change(
    change: &RoomMembershipChange,
    format_as_html: bool,
) -> Option<TextPreview> {
    let dn = change.display_name();
    let change_user_id = dn.as_deref()
        .unwrap_or_else(|| change.user_id().as_str());
    let change_user_id = if format_as_html {
        htmlize::escape_text(change_user_id)
    } else {
        change_user_id.into()
    };
    let text = match change.change() {
        None
        | Some(MembershipChange::NotImplemented)
        | Some(MembershipChange::None)
        | Some(MembershipChange::Error) => {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return None;
        }
        Some(MembershipChange::Joined) =>
            String::from("joined this room."),
        Some(MembershipChange::Left) =>
            String::from("left this room."),
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
            String::from("accepted an invitation to this room."),
        Some(MembershipChange::InvitationRejected) =>
            String::from("rejected an invitation to this room."),
        Some(MembershipChange::InvitationRevoked) =>
            format!("revoked {}'s invitation to this room.", change_user_id),
        Some(MembershipChange::Knocked) =>
            String::from("requested to join this room."),
        Some(MembershipChange::KnockAccepted) =>
            format!("accepted {}'s request to join this room.", change_user_id),
        Some(MembershipChange::KnockRetracted) =>
            String::from("retracted their request to join this room."),
        Some(MembershipChange::KnockDenied) =>
            format!("denied {}'s request to join this room.", change_user_id),
    };
    Some(TextPreview::from((text, BeforeText::UsernameWithoutColon)))
}
