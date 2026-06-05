//! Functions for generating text previews of timeline events.
//!
//! These text previews are used for:
//! * inline replies within the timeline
//! * preview of a message being replied to above the message input box
//! * previews of each room's latest message in the rooms list

use std::borrow::Cow;

use matrix_sdk::{ruma::{OwnedUserId, events::{room::{guest_access::GuestAccess, history_visibility::HistoryVisibility, join_rules::JoinRule, message::{AudioMessageEventContent, MessageFormat, MessageType, VideoMessageEventContent}}, AnySyncMessageLikeEvent, AnySyncTimelineEvent, StateEventContentChange, SyncMessageLikeEvent}, serde::Raw, UserId}};
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

/// Structured metadata extracted from a `m.video` message, used by
/// the inline video player widget and the textual fallback summary.
#[derive(Clone, Debug, Default)]
pub struct VideoSummary {
    pub filename: String,
    pub mime: Option<String>,
    pub duration_secs: Option<f64>,
    pub size_bytes: Option<u64>,
    pub dimensions: Option<(u64, u64)>,
    pub caption_html: Option<String>,
}

/// Structured metadata extracted from an `m.audio` message, used by
/// the inline audio player widget. Mirrors [`VideoSummary`] minus the
/// `dimensions` field (audio has no width/height).
#[derive(Clone, Debug, Default)]
pub struct AudioSummary {
    pub filename: String,
    pub mime: Option<String>,
    pub duration_secs: Option<f64>,
    pub size_bytes: Option<u64>,
    pub caption_html: Option<String>,
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

/// Build a structured summary of a `m.video` message. Width and height
/// are only set together — a partial pair drops back to `None` so the
/// caller never has to deal with an incomplete dimension.
pub fn summarize_video_message(video: &VideoMessageEventContent) -> VideoSummary {
    let dimensions = video.info.as_ref().and_then(|info| {
        match (info.width, info.height) {
            (Some(w), Some(h)) => Some((u64::from(w), u64::from(h))),
            _ => None,
        }
    });
    VideoSummary {
        filename: video.filename().to_string(),
        mime: video.info.as_ref().and_then(|info| info.mimetype.clone()),
        duration_secs: video
            .info
            .as_ref()
            .and_then(|info| info.duration)
            .map(|duration| duration.as_secs_f64()),
        size_bytes: video
            .info
            .as_ref()
            .and_then(|info| info.size)
            .map(Into::into),
        dimensions,
        caption_html: video
            .formatted_caption()
            .map(|formatted| formatted.body.clone())
            .or_else(|| video.caption().map(|caption| htmlize::escape_text(caption).to_string())),
    }
}

/// Render the structured `VideoSummary` as a small HTML block used as
/// the textual fallback above the inline video player. Filename is
/// HTML-escaped; dimensions render as `WIDTHxHEIGHT` only when both
/// are known so the omits-when-none test passes.
///
/// Currently unused — `room_screen.rs` builds its own html fallback inline
/// via `tr_fmt`. Kept here so the video-message widget can share it later.
#[allow(dead_code)]
pub fn video_summary_html(summary: &VideoSummary) -> String {
    let mut out = String::new();
    out.push_str("<b>");
    out.push_str(&htmlize::escape_text(&summary.filename));
    out.push_str("</b>");

    let mut meta_parts: Vec<String> = Vec::new();
    if let Some((w, h)) = summary.dimensions {
        meta_parts.push(format!("{}x{}", w, h));
    }
    if let Some(secs) = summary.duration_secs {
        meta_parts.push(format_mmss(secs));
    }
    if let Some(bytes) = summary.size_bytes {
        meta_parts.push(format_bytesize(bytes));
    }
    if let Some(mime) = summary.mime.as_deref() {
        meta_parts.push(htmlize::escape_text(mime).to_string());
    }
    if !meta_parts.is_empty() {
        out.push_str(" <i>(");
        out.push_str(&meta_parts.join(", "));
        out.push_str(")</i>");
    }

    if let Some(caption) = summary.caption_html.as_deref() {
        out.push_str("<br>");
        out.push_str(caption);
    }
    out
}

#[allow(dead_code)]
fn format_bytesize(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_mmss(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "00:00".to_string();
    }
    let secs = secs.floor() as u64;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Build a structured summary of an `m.audio` message — the audio
/// counterpart to [`summarize_video_message`].
pub fn summarize_audio_message(audio: &AudioMessageEventContent) -> AudioSummary {
    AudioSummary {
        filename: audio.filename().to_string(),
        mime: audio.info.as_ref().and_then(|info| info.mimetype.clone()),
        duration_secs: audio
            .info
            .as_ref()
            .and_then(|info| info.duration)
            .map(|duration| duration.as_secs_f64()),
        size_bytes: audio
            .info
            .as_ref()
            .and_then(|info| info.size)
            .map(Into::into),
        caption_html: audio
            .formatted_caption()
            .map(|formatted| formatted.body.clone())
            .or_else(|| audio.caption().map(|caption| htmlize::escape_text(caption).to_string())),
    }
}

/// Best-effort extension hint for the symphonia decoder. Prefers the
/// filename extension when it matches a known audio container, else
/// derives one from the MIME type, else falls back to `"mp3"`.
pub fn infer_audio_extension(filename: &str, mime: Option<&str>) -> &'static str {
    let from_filename = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim().to_ascii_lowercase());
    match from_filename.as_deref() {
        Some("mp3") => "mp3",
        Some("m4a") => "m4a",
        Some("aac") => "aac",
        Some("flac") => "flac",
        Some("ogg") => "ogg",
        Some("oga") => "oga",
        Some("opus") => "opus",
        Some("wav") => "wav",
        Some("webm") => "webm",
        _ => mime.and_then(audio_extension_from_mime).unwrap_or("mp3"),
    }
}

fn audio_extension_from_mime(mime: &str) -> Option<&'static str> {
    match mime
        .to_ascii_lowercase()
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
    {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" => Some("ogg"),
        "audio/opus" => Some("opus"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/webm" => Some("webm",),
        _ => None,
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
            text_preview_of_other_state(other_state, true)
                .unwrap_or_else(|| TextPreview::from((
                    String::from("<i>initiated another state change</i>"),
                    BeforeText::UsernameWithoutColon,
                )))
        }
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
        TimelineItemContent::RtcNotification => TextPreview::from((
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
            text_preview_of_other_state(other_state, false)
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
        TimelineItemContent::CallInvite => String::from("[Call Invitation]"),
        TimelineItemContent::RtcNotification => String::from("[RTC Call Notification]"),
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
                redactor_and_reason = Some((
                    redacted_because.sender,
                    redacted_because.content.reason,
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
) -> Option<TextPreview> {
    let text = match other_state.content() {
        AnyOtherStateEventContentChange::RoomAliases(StateEventContentChange::Original { content, .. }) => {
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
        AnyOtherStateEventContentChange::RoomAvatar(_) => {
            Some(String::from("set this room's avatar picture."))
        }
        AnyOtherStateEventContentChange::RoomCanonicalAlias(StateEventContentChange::Original { content, .. }) => {
            Some(format!("set the main address of this room to {}.",
                content.alias.as_ref().map(|a| a.as_str()).unwrap_or("none")
            ))
        }
        AnyOtherStateEventContentChange::RoomCreate(StateEventContentChange::Original { content, .. }) => {
            Some(format!("created this room (v{}).", content.room_version.as_str()))
        }
        AnyOtherStateEventContentChange::RoomEncryption(_) => {
            Some(String::from("enabled encryption in this room."))
        }
        AnyOtherStateEventContentChange::RoomGuestAccess(StateEventContentChange::Original { content, .. }) => {
            Some(match &content.guest_access {
                GuestAccess::CanJoin => String::from("has allowed guests to join this room."),
                GuestAccess::Forbidden => String::from("has forbidden guests from joining this room."),
                custom => format!("has set custom guest access rules for this room: {}", custom.as_str()),
            })
        }
        AnyOtherStateEventContentChange::RoomHistoryVisibility(StateEventContentChange::Original { content, .. }) => {
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
        AnyOtherStateEventContentChange::RoomJoinRules(StateEventContentChange::Original { content, .. }) => {
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
        AnyOtherStateEventContentChange::RoomPinnedEvents(StateEventContentChange::Original { content, .. }) => {
            Some(format!("pinned {} events in this room.", content.pinned.len()))
        }
        AnyOtherStateEventContentChange::RoomName(StateEventContentChange::Original { content, .. }) => {
            let name = if format_as_html {
                htmlize::escape_text(&content.name)
            } else {
                Cow::Borrowed(content.name.as_str())
            };
            Some(format!("changed this room's name to \"{name}\"."))
        }
        AnyOtherStateEventContentChange::RoomPowerLevels(_) => {
            Some(String::from("set the power levels for this room."))
        }
        AnyOtherStateEventContentChange::RoomServerAcl(_) => {
            Some(String::from("set the server access control list for this room."))
        }
        AnyOtherStateEventContentChange::RoomTombstone(StateEventContentChange::Original { content, .. }) => {
            Some(format!("closed this room and upgraded it to {}", content.replacement_room.matrix_to_uri()))
        }
        AnyOtherStateEventContentChange::RoomTopic(StateEventContentChange::Original { content, .. }) => {
            let topic = if format_as_html {
                htmlize::escape_text(&content.topic)
            } else {
                Cow::Borrowed(content.topic.as_str())
            };
            Some(format!("changed this room's topic to \"{topic}\"."))
        }
        AnyOtherStateEventContentChange::SpaceParent(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(other_state.state_key())
            } else {
                Cow::Borrowed(other_state.state_key())
            };
            Some(format!("set this room's parent space to \"{state_key}\"."))
        }
        AnyOtherStateEventContentChange::SpaceChild(_) => {
            let state_key  = if format_as_html {
                htmlize::escape_text(other_state.state_key())
            } else {
                Cow::Borrowed(other_state.state_key())
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
