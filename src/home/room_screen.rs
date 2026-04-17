//! The `RoomScreen` widget is the UI view that displays a single room or thread's timeline
//! of events (messages，state changes, etc.), along with an input bar at the bottom.

use std::{borrow::Cow, cell::{Cell, RefCell}, ops::{DerefMut, Range}, sync::Arc, time::Duration};

use bytesize::ByteSize;
use hashbrown::{HashMap, HashSet};
use imbl::Vector;
use makepad_widgets::{image_cache::ImageBuffer, *};
use matrix_sdk::{
    OwnedServerName, media::{MediaFormat, MediaRequestParameters}, room::{RoomMember, RoomMemberRole}, ruma::{
        EventId, MatrixToUri, MatrixUri, OwnedEventId, OwnedMxcUri, OwnedRoomId, UserId, events::{
            receipt::Receipt,
            room::{
                ImageInfo, MediaSource, message::{
                    AudioMessageEventContent, EmoteMessageEventContent, FileMessageEventContent, FormattedBody, ImageMessageEventContent, KeyVerificationRequestEventContent, LocationMessageEventContent, MessageFormat, MessageType, NoticeMessageEventContent, RoomMessageEventContent, TextMessageEventContent, VideoMessageEventContent
                }
            },
            sticker::{StickerEventContent, StickerMediaSource},
        }, matrix_uri::MatrixId, uint
    }
};
use matrix_sdk_ui::timeline::{
    self, EmbeddedEvent, EncryptedMessage, EventTimelineItem, InReplyToDetails, LiveLocationState, MemberProfileChange, MembershipChange, MsgLikeContent, MsgLikeKind, OtherMessageLike, PollState, RoomMembershipChange, TimelineDetails, TimelineEventItemId, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem
};
use ruma::{OwnedUserId, api::client::receipt::create_receipt::v3::ReceiptType, events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent}};

use matrix_sdk_ui::sync_service::State;
use crate::{
    app::{AppState, AppStateAction, ConfirmDeleteAction, SelectedRoom}, avatar_cache, event_preview::{plaintext_body_of_timeline_item, text_preview_of_encrypted_message, text_preview_of_member_profile_change, text_preview_of_other_message_like, text_preview_of_other_state, text_preview_of_room_membership_change, text_preview_of_timeline_item}, home::{bot_binding_modal::BotBindingModalAction, create_bot_modal::{CreateBotModalAction, CreateBotModalWidgetExt}, delete_bot_modal::{DeleteBotModalAction, DeleteBotModalWidgetExt}, edited_indicator::EditedIndicatorWidgetRefExt, invite_modal::InviteModalAction, link_preview::{LinkPreviewCache, LinkPreviewRef, LinkPreviewWidgetRefExt}, loading_pane::{LoadingPaneState, LoadingPaneWidgetExt}, room_image_viewer::{get_image_name_and_filesize, populate_matrix_image_modal}, rooms_list::{RoomsListAction, RoomsListRef}, rooms_list_header::RoomsListHeaderAction, tombstone_footer::SuccessorRoomDetails}, i18n::{AppLanguage, tr_fmt, tr_key}, media_cache::{MediaCache, MediaCacheEntry}, profile::{
        user_profile::{ShowUserProfileAction, UserProfile, UserProfileAndRoomId, UserProfilePaneInfo, UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
        user_profile_cache,
    },
    room::{BasicRoomDetails, room_input_bar::{RoomInputBarState, RoomInputBarWidgetRefExt}, translation, typing_notice::TypingNoticeWidgetExt},
    shared::{
        avatar::{AvatarState, AvatarWidgetExt, AvatarWidgetRefExt}, confirmation_modal::{ConfirmationModalAction, ConfirmationModalContent, ConfirmationModalWidgetExt}, html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetRefExt, RobrixHtmlLinkAction}, image_viewer::{ImageViewerAction, ImageViewerMetaData, LoadState}, jump_to_bottom_button::{JumpToBottomButtonWidgetExt, UnreadMessageCount}, popup_list::{PopupKind, enqueue_popup_notification}, restore_status_view::RestoreStatusViewWidgetExt, styles::*, text_or_image::{TextOrImageAction, TextOrImageRef, TextOrImageWidgetRefExt}, timestamp::TimestampWidgetRefExt
    },
    sliding_sync::{BackwardsPaginateUntilEventRequest, FetchedRoomThread, MatrixRequest, PaginationDirection, RoomThreadsAction, TimelineEndpoints, TimelineKind, TimelineRequestSender, UserPowerLevels, current_user_id, get_client, submit_async_request, take_timeline_endpoints}, utils::{self, ImageFormat, MEDIA_THUMBNAIL_FORMAT, RoomNameId, unix_time_millis_to_datetime}
};
use crate::home::event_reaction_list::ReactionListWidgetRefExt;
use crate::home::room_read_receipt::AvatarRowWidgetRefExt;
use crate::home::streaming_animation::StreamingAnimState;
use crate::room::room_input_bar::RoomInputBarWidgetExt;
use crate::shared::mentionable_text_input::MentionableTextInputAction;

use rangemap::RangeSet;

use super::{ContextMenuOpenGesture, event_reaction_list::ReactionData, invite_modal::is_invite_modal_open, loading_pane::LoadingPaneRef, new_message_context_menu::{MessageAbilities, MessageDetails}, room_read_receipt::{self, populate_read_receipts, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT}};

/// The maximum number of timeline items to search through
/// when looking for a particular event.
///
/// This is a safety measure to prevent the main UI thread
/// from getting into a long-running loop if an event cannot be found quickly.
const MAX_ITEMS_TO_SEARCH_THROUGH: usize = 100;

/// The max size (width or height) of a blurhash image to decode.
const BLURHASH_IMAGE_MAX_SIZE: u32 = 500;

/// Use a larger batch when we are trying to fill the initial viewport,
/// otherwise many short messages can trigger a long chain of tiny paginations.
const VIEWPORT_FILL_PAGINATION_SIZE: u16 = 150;
const TOPIC_PREVIEW_CHARS: usize = 140;
const ROOM_INFO_PANE_DESKTOP_WIDTH: f32 = 320.0;
const ROOM_INFO_PANE_MOBILE_BREAKPOINT: f32 = 700.0;
const TRANSLATION_LANG_POPUP_WIDTH: f64 = 220.0;
const TRANSLATION_LANG_POPUP_SCROLL_HEIGHT: f64 = 288.0;
const TRANSLATION_LANG_POPUP_HEIGHT: f64 = TRANSLATION_LANG_POPUP_SCROLL_HEIGHT + 8.0;
const TRANSLATION_LANG_POPUP_GAP: f64 = 6.0;
const TRANSLATION_LANG_POPUP_MARGIN: f64 = 8.0;
const MESSAGE_PROFILE_TOP_MARGIN: f64 = 4.5;
const MESSAGE_PROFILE_AVATAR_SIZE: f64 = 48.0;
const MESSAGE_USERNAME_ROW_HEIGHT: f64 = 18.0;
const MESSAGE_USERNAME_ROW_BOTTOM_MARGIN: f64 = 9.0;
const MESSAGE_USERNAME_RIGHT_MARGIN: f64 = 4.0;
const BOT_BADGE_HEIGHT: f64 = 16.0;
const BOT_BADGE_HORIZONTAL_PADDING: f64 = 6.0;
const BOT_BADGE_BORDER_RADIUS: f64 = 3.0;
const BOT_BADGE_TEXT_FONT_SIZE: f64 = 8.5;
const BOT_BADGE_TEXT_TOP_DROP: f64 = -0.08;
const MAX_OCTOS_ACTION_BUTTONS: usize = 6;

const fn centered_top_margin(outer_top_margin: f64, outer_height: f64, inner_height: f64) -> f64 {
    outer_top_margin + ((outer_height - inner_height) * 0.5)
}

#[cfg(test)]
const fn center_y(top_margin: f64, height: f64) -> f64 {
    top_margin + (height * 0.5)
}

const MESSAGE_USERNAME_ROW_TOP_MARGIN: f64 = centered_top_margin(
    MESSAGE_PROFILE_TOP_MARGIN,
    MESSAGE_PROFILE_AVATAR_SIZE,
    MESSAGE_USERNAME_ROW_HEIGHT,
);

#[cfg(test)]
fn message_profile_avatar_center_y() -> f64 {
    center_y(MESSAGE_PROFILE_TOP_MARGIN, MESSAGE_PROFILE_AVATAR_SIZE)
}

#[cfg(test)]
fn message_username_row_center_y() -> f64 {
    center_y(MESSAGE_USERNAME_ROW_TOP_MARGIN, MESSAGE_USERNAME_ROW_HEIGHT)
}

#[cfg(test)]
fn bot_badge_center_y_within_username_row() -> f64 {
    let bot_badge_top_margin = MESSAGE_USERNAME_ROW_TOP_MARGIN
        + ((MESSAGE_USERNAME_ROW_HEIGHT - BOT_BADGE_HEIGHT) * 0.5);
    center_y(bot_badge_top_margin, BOT_BADGE_HEIGHT)
}

#[cfg(test)]
fn bot_badge_label_center_y() -> f64 {
    let bot_badge_label_top_margin = (BOT_BADGE_HEIGHT - BOT_BADGE_TEXT_FONT_SIZE) * 0.5
        + (BOT_BADGE_TEXT_FONT_SIZE * BOT_BADGE_TEXT_TOP_DROP);
    center_y(bot_badge_label_top_margin, BOT_BADGE_TEXT_FONT_SIZE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BotTimelineLayers {
    status: Option<String>,
    provider: Option<String>,
    body: String,
    footer: Option<String>,
}

impl BotTimelineLayers {
    fn plain(body: &str) -> Self {
        Self {
            status: None,
            provider: None,
            body: body.to_string(),
            footer: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BotTimelineRenderState {
    show_card: bool,
    show_body_card: bool,
    show_status_strip: bool,
    show_metadata_footer: bool,
    status: Option<String>,
    provider: Option<String>,
    body: String,
    footer: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OctosActionStyle {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OctosActionButton {
    id: String,
    label: String,
    style: OctosActionStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionButtonRenderSlot {
    id: String,
    label: String,
    style: OctosActionStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedOctosActionState {
    id: String,
    label: String,
    style: OctosActionStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApprovalCardRenderState {
    title: String,
    summary: String,
    buttons_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionButtonRenderState {
    show_container: bool,
    show_button_row: bool,
    approval_card: Option<ApprovalCardRenderState>,
    buttons_enabled: bool,
    visible_slots: Vec<ActionButtonRenderSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedOctosActionPayload {
    approval_request: Option<OctosApprovalRequest>,
    actions: Vec<OctosActionButton>,
    malformed_approval_request: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OctosApprovalRiskLevel {
    Normal,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OctosApprovalTimeoutBehavior {
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OctosApprovalRequest {
    request_id: String,
    tool_name: String,
    tool_args_digest: String,
    title: String,
    summary: String,
    risk_level: OctosApprovalRiskLevel,
    authorized_approvers: Vec<String>,
    expires_at: String,
    on_timeout: OctosApprovalTimeoutBehavior,
}

fn parse_octos_action_style(style: Option<&str>) -> OctosActionStyle {
    match style {
        Some("primary") => OctosActionStyle::Primary,
        Some("danger") => OctosActionStyle::Danger,
        _ => OctosActionStyle::Secondary,
    }
}

fn effective_octos_message_content(content: &serde_json::Value) -> &serde_json::Value {
    content.get("m.new_content").unwrap_or(content)
}

fn latest_effective_event_content_json(
    event_tl_item: &EventTimelineItem,
) -> Option<serde_json::Value> {
    event_tl_item.latest_edit_json()
        .or_else(|| event_tl_item.original_json())
        .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
        .flatten()
        .map(|content| effective_octos_message_content(&content).clone())
}

fn original_event_content_json(
    event_tl_item: &EventTimelineItem,
) -> Option<serde_json::Value> {
    event_tl_item.original_json()
        .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
        .flatten()
}

fn parse_octos_approval_risk_level(value: Option<&str>) -> Option<OctosApprovalRiskLevel> {
    match value {
        Some("normal") => Some(OctosApprovalRiskLevel::Normal),
        Some("critical") => Some(OctosApprovalRiskLevel::Critical),
        _ => None,
    }
}

fn parse_octos_approval_timeout_behavior(value: Option<&str>) -> Option<OctosApprovalTimeoutBehavior> {
    match value {
        Some("notify") => Some(OctosApprovalTimeoutBehavior::Notify),
        _ => None,
    }
}

fn parse_octos_approval_request_from_content(content: &serde_json::Value) -> Option<OctosApprovalRequest> {
    let approval = content.get("org.octos.approval_request")?;
    let request_id = approval.get("request_id")?.as_str()?.trim();
    let tool_name = approval.get("tool_name")?.as_str()?.trim();
    let tool_args_digest = approval.get("tool_args_digest")?.as_str()?.trim();
    let title = approval.get("title")?.as_str()?.trim();
    let summary = approval.get("summary")?.as_str()?.trim();
    let risk_level = parse_octos_approval_risk_level(
        approval.get("risk_level").and_then(|value| value.as_str()).map(str::trim),
    )?;

    let approvers = approval.get("authorized_approvers")?.as_array()?;
    let authorized_approvers = approvers
        .iter()
        .filter_map(|value| value.as_str().map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if authorized_approvers.is_empty() {
        return None;
    }

    let expires_at = approval.get("expires_at")?.as_str()?.trim();
    let on_timeout = parse_octos_approval_timeout_behavior(
        approval.get("on_timeout").and_then(|value| value.as_str()).map(str::trim),
    )?;

    if request_id.is_empty()
        || tool_name.is_empty()
        || tool_args_digest.is_empty()
        || title.is_empty()
        || summary.is_empty()
        || expires_at.is_empty()
    {
        return None;
    }

    Some(OctosApprovalRequest {
        request_id: request_id.to_owned(),
        tool_name: tool_name.to_owned(),
        tool_args_digest: tool_args_digest.to_owned(),
        title: title.to_owned(),
        summary: summary.to_owned(),
        risk_level,
        authorized_approvers,
        expires_at: expires_at.to_owned(),
        on_timeout,
    })
}

fn parse_octos_actions_from_content(content: &serde_json::Value) -> Vec<OctosActionButton> {
    let Some(actions) = effective_octos_message_content(content)
        .get("org.octos.actions")
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut parsed = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        if parsed.len() >= MAX_OCTOS_ACTION_BUTTONS {
            warning!(
                "org.octos.actions: truncated {} extra buttons",
                actions.len().saturating_sub(MAX_OCTOS_ACTION_BUTTONS)
            );
            break;
        }

        let Some(id) = action.get("id").and_then(|value| value.as_str()).map(str::trim) else {
            warning!("org.octos.actions: skipping malformed entry at index {index}");
            continue;
        };
        let Some(label) = action.get("label").and_then(|value| value.as_str()).map(str::trim) else {
            warning!("org.octos.actions: skipping malformed entry at index {index}");
            continue;
        };
        if id.is_empty() || label.is_empty() {
            warning!("org.octos.actions: skipping malformed entry at index {index}");
            continue;
        }

        parsed.push(OctosActionButton {
            id: id.to_owned(),
            label: label.to_owned(),
            style: parse_octos_action_style(action.get("style").and_then(|value| value.as_str())),
        });
    }

    parsed
}

fn parse_octos_approval_actions_from_content(content: &serde_json::Value) -> Vec<OctosActionButton> {
    parse_octos_actions_from_content(content)
        .into_iter()
        .filter(|action| matches!(action.id.as_str(), "approve" | "deny"))
        .collect()
}

fn parse_octos_action_payload_for_render(
    content: Option<&serde_json::Value>,
    original_content: Option<&serde_json::Value>,
) -> ParsedOctosActionPayload {
    let approval_request = original_content
        .and_then(parse_octos_approval_request_from_content);
    let malformed_approval_request = original_content
        .is_some_and(|content| content.get("org.octos.approval_request").is_some())
        && approval_request.is_none();

    let actions = if malformed_approval_request {
        Vec::new()
    } else if approval_request.is_some() {
        original_content
            .map(parse_octos_approval_actions_from_content)
            .unwrap_or_default()
    } else {
        content
            .map(parse_octos_actions_from_content)
            .unwrap_or_default()
    };

    ParsedOctosActionPayload {
        approval_request,
        actions,
        malformed_approval_request,
    }
}

fn compute_action_button_render_state(
    actions: &[OctosActionButton],
    approval_request: Option<&OctosApprovalRequest>,
    current_user_id: Option<&UserId>,
) -> ActionButtonRenderState {
    let approval_card = approval_request
        .and_then(|approval_request| (!actions.is_empty()).then(|| ApprovalCardRenderState {
            title: approval_request.title.clone(),
            summary: approval_request.summary.clone(),
            buttons_enabled: local_user_can_approve(approval_request, current_user_id),
        }));
    let visible_slots = actions
        .iter()
        .take(MAX_OCTOS_ACTION_BUTTONS)
        .map(|action| ActionButtonRenderSlot {
            id: action.id.clone(),
            label: action.label.clone(),
            style: action.style,
        })
        .collect::<Vec<_>>();

    let buttons_enabled = approval_card
        .as_ref()
        .map(|approval_card| approval_card.buttons_enabled)
        .unwrap_or(true);
    let show_button_row = !visible_slots.is_empty();

    ActionButtonRenderState {
        show_container: approval_card.is_some() || show_button_row,
        show_button_row,
        approval_card,
        buttons_enabled,
        visible_slots,
    }
}

fn action_button_render_slots_for_display(
    render_state: &ActionButtonRenderState,
    selected_action: Option<&SelectedOctosActionState>,
) -> Vec<ActionButtonRenderSlot> {
    if let Some(selected_action) = selected_action {
        vec![ActionButtonRenderSlot {
            id: selected_action.id.clone(),
            label: format!("✓ {}", selected_action.label),
            style: selected_action.style,
        }]
    } else {
        render_state.visible_slots.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OctosActionResponseRequest {
    timeline_kind: TimelineKind,
    content: serde_json::Value,
    target_user_id: OwnedUserId,
    explicit_room: bool,
    source_event_id: OwnedEventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OctosActionButtonRequest {
    Generic {
        action_id: String,
        label: String,
        style: OctosActionStyle,
    },
    Approval {
        request_id: String,
        title: String,
        decision: String,
        label: String,
        tool_args_digest: String,
        style: OctosActionStyle,
    },
}

impl OctosActionButtonRequest {
    fn action_id(&self) -> &str {
        match self {
            Self::Generic { action_id, .. } => action_id,
            Self::Approval { decision, .. } => decision,
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Generic { label, .. } => label,
            Self::Approval { label, .. } => label,
        }
    }

    fn style(&self) -> OctosActionStyle {
        match self {
            Self::Generic { style, .. } | Self::Approval { style, .. } => *style,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OctosActionButtonContext {
    source_event_id: OwnedEventId,
    original_sender: OwnedUserId,
    request: OctosActionButtonRequest,
}

fn build_octos_approval_response_request(
    timeline_kind: &TimelineKind,
    title: &str,
    request_id: &str,
    decision: &str,
    tool_args_digest: &str,
    source_event_id: &EventId,
    original_sender: &UserId,
) -> OctosActionResponseRequest {
    OctosActionResponseRequest {
        timeline_kind: timeline_kind.clone(),
        content: serde_json::json!({
            "msgtype": "m.text",
            "body": format!("[Approval: {decision}] {title}"),
            "org.octos.approval_response": {
                "request_id": request_id,
                "decision": decision,
                "source_event_id": source_event_id.as_str(),
                "tool_args_digest": tool_args_digest,
            },
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": source_event_id.as_str(),
                }
            }
        }),
        target_user_id: original_sender.to_owned(),
        explicit_room: false,
        source_event_id: source_event_id.to_owned(),
    }
}

fn build_octos_action_response_request(
    timeline_kind: &TimelineKind,
    label: &str,
    action_id: &str,
    source_event_id: &EventId,
    original_sender: &UserId,
) -> OctosActionResponseRequest {
    OctosActionResponseRequest {
        timeline_kind: timeline_kind.clone(),
        content: serde_json::json!({
            "msgtype": "m.text",
            "body": format!("[Action: {label}]"),
            "org.octos.action_response": {
                "action_id": action_id,
                "source_event_id": source_event_id.as_str(),
            },
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": source_event_id.as_str(),
                }
            }
        }),
        target_user_id: original_sender.to_owned(),
        explicit_room: false,
        source_event_id: source_event_id.to_owned(),
    }
}

fn local_user_can_approve(
    approval_request: &OctosApprovalRequest,
    current_user_id: Option<&UserId>,
) -> bool {
    let Some(current_user_id) = current_user_id else {
        return false;
    };

    approval_request.authorized_approvers
        .iter()
        .any(|approver| approver == current_user_id.as_str())
}

fn mark_action_buttons_disabled(
    disabled_source_event_ids: &mut HashSet<OwnedEventId>,
    source_event_id: &OwnedEventId,
) {
    disabled_source_event_ids.insert(source_event_id.clone());
}

fn mark_selected_octos_action(
    selected_actions: &mut HashMap<OwnedEventId, SelectedOctosActionState>,
    source_event_id: &OwnedEventId,
    action_id: &str,
    label: &str,
    style: OctosActionStyle,
) {
    selected_actions.insert(source_event_id.clone(), SelectedOctosActionState {
        id: action_id.to_owned(),
        label: label.to_owned(),
        style,
    });
}

fn clear_selected_octos_action(
    selected_actions: &mut HashMap<OwnedEventId, SelectedOctosActionState>,
    source_event_id: &EventId,
) {
    selected_actions.remove(source_event_id);
}

fn clear_action_buttons_disabled(
    disabled_source_event_ids: &mut HashSet<OwnedEventId>,
    source_event_id: &EventId,
) {
    disabled_source_event_ids.remove(source_event_id);
}

fn are_action_buttons_disabled(
    disabled_source_event_ids: &HashSet<OwnedEventId>,
    source_event_id: &EventId,
) -> bool {
    disabled_source_event_ids.contains(source_event_id)
}

fn octos_action_button_paths(index: usize) -> (&'static [LiveId], &'static [LiveId], &'static [LiveId], &'static [LiveId]) {
    match index {
        0 => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_0)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_0), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_0), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_0), live_id!(danger_button)],
        ),
        1 => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_1)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_1), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_1), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_1), live_id!(danger_button)],
        ),
        2 => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_2)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_2), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_2), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_2), live_id!(danger_button)],
        ),
        3 => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_3)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_3), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_3), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_3), live_id!(danger_button)],
        ),
        4 => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_4)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_4), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_4), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_4), live_id!(danger_button)],
        ),
        _ => (
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_5)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_5), live_id!(primary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_5), live_id!(secondary_button)],
            &[live_id!(content), live_id!(action_buttons), live_id!(action_button_row), live_id!(action_button_slot_5), live_id!(danger_button)],
        ),
    }
}

fn is_bot_provider_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("via ") && trimmed.contains('(') && trimmed.ends_with(')')
}

fn strip_streaming_cursor_suffix(line: &str) -> &str {
    line
        .trim_end()
        .strip_suffix('\u{25CF}')
        .map(str::trim_end)
        .unwrap_or_else(|| line.trim_end())
}

fn is_bot_footer_line(line: &str) -> bool {
    let trimmed = strip_streaming_cursor_suffix(line);
    trimmed.starts_with('_')
        && trimmed.ends_with('_')
        && trimmed.contains("·")
        && trimmed.contains(" in")
        && trimmed.contains(" out")
}

fn looks_like_metrics_line(line: &str) -> bool {
    let trimmed = strip_streaming_cursor_suffix(line).trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 40
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
        && (trimmed.contains('s') || trimmed.contains(" in") || trimmed.contains(" out"))
}

fn looks_like_status_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("via ")
        && !trimmed.starts_with('_')
        && trimmed.chars().count() <= 32
        && !trimmed.contains("  ")
}

fn trim_structured_body_lines(lines: &[&str]) -> String {
    let mut start = 0;
    let mut end = lines.len();

    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }

    lines[start..end].join("\n")
}

fn is_viable_bot_body(body: &str) -> bool {
    let trimmed = body.trim();
    !trimmed.is_empty()
        && trimmed.chars().any(|c| c.is_alphanumeric())
}

fn parse_bot_timeline_layers(raw_body: &str, is_bot_sender: bool) -> BotTimelineLayers {
    if !is_bot_sender || raw_body.trim().is_empty() {
        return BotTimelineLayers::plain(raw_body);
    }

    let lines: Vec<&str> = raw_body.lines().collect();
    if lines.is_empty() {
        return BotTimelineLayers::plain(raw_body);
    }

    let (status, provider, mut content_start) =
        if lines.len() >= 2 && looks_like_status_line(lines[0]) && is_bot_provider_line(lines[1]) {
            (
                Some(lines[0].trim().to_string()),
                Some(lines[1].trim().to_string()),
                2usize,
            )
        } else if is_bot_provider_line(lines[0]) {
            (None, Some(lines[0].trim().to_string()), 1usize)
        } else {
            (None, None, 0usize)
        };

    while content_start < lines.len() && lines[content_start].trim().is_empty() {
        content_start += 1;
    }

    let mut footer = None;
    let mut content_end = lines.len();
    let last_non_empty = lines.iter().rposition(|line| !line.trim().is_empty());

    if let Some(last_idx) = last_non_empty {
        if is_bot_footer_line(lines[last_idx]) {
            footer = Some(strip_streaming_cursor_suffix(lines[last_idx]).trim().to_string());
            content_end = last_idx;
            while content_end > content_start && lines[content_end - 1].trim().is_empty() {
                content_end -= 1;
            }
        }
    }

    if content_start >= content_end {
        return if status.is_some() || provider.is_some() || footer.is_some() {
            BotTimelineLayers {
                status,
                provider,
                body: String::new(),
                footer,
            }
        } else {
            BotTimelineLayers::plain(raw_body)
        };
    }

    let content_lines = &lines[content_start..content_end];
    let mut body = trim_structured_body_lines(content_lines);
    if footer.is_none() && content_lines.len() == 1 && looks_like_metrics_line(content_lines[0]) {
        footer = Some(strip_streaming_cursor_suffix(content_lines[0]).trim().to_string());
        body.clear();
    }
    if !is_viable_bot_body(&body) {
        return if status.is_some() || provider.is_some() || footer.is_some() {
            BotTimelineLayers {
                status,
                provider,
                body,
                footer,
            }
        } else {
            BotTimelineLayers::plain(raw_body)
        };
    }

    BotTimelineLayers {
        status,
        provider,
        body,
        footer,
    }
}

fn compute_bot_timeline_render_state(raw_body: &str, is_bot_sender: bool) -> BotTimelineRenderState {
    let layers = parse_bot_timeline_layers(raw_body, is_bot_sender);
    let show_card = is_bot_sender;
    let show_body_card = show_card && !layers.body.trim().is_empty();

    BotTimelineRenderState {
        show_card,
        show_body_card,
        show_status_strip: show_card && layers.status.is_some(),
        show_metadata_footer: show_card && (layers.provider.is_some() || layers.footer.is_some()),
        status: layers.status,
        provider: layers.provider,
        body: layers.body,
        footer: layers.footer,
    }
}

fn display_bot_footer_text(footer: &str) -> &str {
    strip_streaming_cursor_suffix(footer)
        .strip_prefix('_')
        .and_then(|trimmed| trimmed.strip_suffix('_'))
        .unwrap_or(footer)
}

fn has_rich_markdown_syntax(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && (
            trimmed.contains("```")
            || trimmed.starts_with("## ")
            || trimmed.starts_with("### ")
            || trimmed.contains("\n## ")
            || trimmed.contains("\n### ")
            || trimmed.starts_with("|")
            || trimmed.contains("\n|")
            || trimmed.starts_with("- ")
            || trimmed.contains("\n- ")
            || trimmed.starts_with("* ")
            || trimmed.contains("\n* ")
            || trimmed.contains("**")
            || trimmed.contains("`")
        )
}

fn should_render_streaming_full_snapshot(
    body: &str,
    formatted_body: Option<&FormattedBody>,
    is_bot_sender: bool,
) -> bool {
    is_bot_sender
        && (
            formatted_body.is_some_and(|formatted| formatted.format == MessageFormat::Html)
            || has_rich_markdown_syntax(body)
        )
}

fn select_bot_timeline_body_formatted_body(
    render_state: &BotTimelineRenderState,
    formatted_body: Option<&FormattedBody>,
) -> Option<FormattedBody> {
    if render_state.status.is_none()
        && render_state.provider.is_none()
        && render_state.footer.is_none()
    {
        return formatted_body
            .cloned()
            .or_else(|| has_rich_markdown_syntax(&render_state.body)
                .then(|| FormattedBody::markdown(&render_state.body))
                .flatten());
    }

    FormattedBody::markdown(&render_state.body)
}

fn should_render_bot_timeline_body_with_markdown_widget(
    render_state: &BotTimelineRenderState,
) -> bool {
    render_state.show_body_card
        && render_state.body.contains("```")
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch|
        matches!(ch as u32,
            0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x3000..=0x303F
            | 0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0xAC00..=0xD7AF
        )
    )
}

fn fenced_code_blocks_contain_cjk(text: &str) -> bool {
    let mut in_fence = false;
    let mut fence_has_cjk = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if in_fence && fence_has_cjk {
                return true;
            }
            in_fence = !in_fence;
            fence_has_cjk = false;
            continue;
        }

        if in_fence && contains_cjk(line) {
            fence_has_cjk = true;
        }
    }

    in_fence && fence_has_cjk
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BotTimelineCodeBlockMode {
    None,
    Highlighted,
    Plain,
}

fn bot_timeline_code_block_mode(render_state: &BotTimelineRenderState) -> BotTimelineCodeBlockMode {
    if !should_render_bot_timeline_body_with_markdown_widget(render_state) {
        return BotTimelineCodeBlockMode::None;
    }

    if fenced_code_blocks_contain_cjk(&render_state.body) {
        BotTimelineCodeBlockMode::Plain
    } else {
        BotTimelineCodeBlockMode::Highlighted
    }
}

fn streaming_update_requires_content_invalidation(
    state: &StreamingAnimState,
    new_text: &str,
    is_live: bool,
    render_full_target: bool,
) -> bool {
    state.target_text != new_text
        || state.is_live != is_live
        || state.render_full_target != render_full_target
}

thread_local! {
    static ROOM_INFO_ACTION_MODAL_OPEN: Cell<bool> = const { Cell::new(false) };
}

fn set_room_info_action_modal_open(open: bool) {
    ROOM_INFO_ACTION_MODAL_OPEN.with(|state| state.set(open));
}

fn is_room_info_action_modal_open() -> bool {
    ROOM_INFO_ACTION_MODAL_OPEN.with(|state| state.get())
}


/// #FFF4E5
const COLOR_THREAD_SUMMARY_BG: Vec4 = vec4(1.0, 0.957, 0.898, 1.0);
/// #FFEACC
const COLOR_THREAD_SUMMARY_BG_HOVER: Vec4 = vec4(1.0, 0.918, 0.8, 1.0);

fn item_event_id(item: &Arc<TimelineItem>) -> Option<&EventId> {
    let TimelineItemKind::Event(event) = item.kind() else {
        return None;
    };
    event.event_id()
}

/// Check if an event carries the MSC4357 `org.matrix.msc4357.live` field,
/// indicating that the message content is still being streamed.
///
/// For edit events (`m.replace`), the live field lives inside `m.new_content`
/// rather than at the top level of `content`, so we check both locations.
fn content_has_msc4357_live_marker(content: &serde_json::Value) -> bool {
    let effective = content.get("m.new_content").unwrap_or(content);
    match effective.get("org.matrix.msc4357.live") {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(_) => true,
        None => false,
    }
}

fn is_msc4357_live(event_tl_item: &EventTimelineItem) -> bool {
    let message_is_edited = event_tl_item
        .content()
        .as_message()
        .is_some_and(|message| message.is_edited());
    event_tl_item.latest_edit_json()
        .or_else(|| (!message_is_edited).then(|| event_tl_item.original_json()).flatten())
        .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
        .flatten()
        .map(|content| content_has_msc4357_live_marker(&content))
        .unwrap_or(false)
}

fn compute_translation_lang_popup_abs_pos(button_rect: Rect, container_rect: Rect) -> DVec2 {
    let min_x = container_rect.pos.x + TRANSLATION_LANG_POPUP_MARGIN;
    let max_x = (container_rect.pos.x + container_rect.size.x - TRANSLATION_LANG_POPUP_WIDTH - TRANSLATION_LANG_POPUP_MARGIN)
        .max(min_x);
    let popup_x = button_rect.pos.x
        .max(min_x)
        .min(max_x);

    let min_y = container_rect.pos.y + TRANSLATION_LANG_POPUP_MARGIN;
    let max_y = (container_rect.pos.y + container_rect.size.y - TRANSLATION_LANG_POPUP_HEIGHT - TRANSLATION_LANG_POPUP_MARGIN)
        .max(min_y);
    let popup_y_above = button_rect.pos.y - TRANSLATION_LANG_POPUP_HEIGHT - TRANSLATION_LANG_POPUP_GAP;
    let popup_y = if popup_y_above >= min_y {
        popup_y_above
    } else {
        (button_rect.pos.y + button_rect.size.y + TRANSLATION_LANG_POPUP_GAP)
            .max(min_y)
            .min(max_y)
    };

    dvec2(popup_x, popup_y)
}

fn streaming_scan_range(
    clear_cache: bool,
    changed_indices: &Range<usize>,
    _old_len: usize,
    new_len: usize,
) -> Range<usize> {
    if clear_cache {
        0..new_len
    } else {
        let start = changed_indices.start.min(new_len);
        let end = changed_indices.end.min(new_len);
        start..end
    }
}

fn refresh_stream_indices<'a, I>(
    event_ids: I,
    streaming_messages: &mut HashMap<OwnedEventId, super::streaming_animation::StreamingAnimState>,
)
where
    I: IntoIterator<Item = Option<&'a EventId>>,
{
    for state in streaming_messages.values_mut() {
        state.timeline_index = None;
    }

    for (idx, event_id) in event_ids.into_iter().enumerate() {
        let Some(event_id) = event_id else {
            continue;
        };
        if let Some(state) = streaming_messages.get_mut(event_id) {
            state.timeline_index = Some(idx);
        }
    }
}

fn any_timeline_indices_visible<I, F>(
    indices: I,
    is_visible: F,
) -> bool
where
    I: IntoIterator<Item = Option<usize>>,
    F: FnMut(usize) -> bool,
{
    indices.into_iter().flatten().any(is_visible)
}

fn streaming_candidates_from_items<'a>(
    items: &'a Vector<Arc<TimelineItem>>,
) -> impl Iterator<Item = (OwnedEventId, String, bool)> + 'a {
    items.iter().filter_map(|item| {
        let TimelineItemKind::Event(event) = item.kind() else {
            return None;
        };
        let event_id = event.event_id()?.to_owned();
        let text = RoomScreen::extract_message_text(item)?;
        Some((event_id, text, is_msc4357_live(event)))
    })
}

fn rebuild_streaming_messages_for_full_snapshot<I>(
    items: I,
    previous_streaming_messages: Option<&HashMap<OwnedEventId, super::streaming_animation::StreamingAnimState>>,
) -> (HashMap<OwnedEventId, super::streaming_animation::StreamingAnimState>, bool)
where
    I: IntoIterator<Item = (OwnedEventId, String, bool)>,
{
    use crate::home::streaming_animation::StreamingAnimState;

    let mut rebuilt = HashMap::new();
    let mut should_schedule_frame = false;

    for (event_id, new_text, live) in items {
        if !live {
            continue;
        }

        // Only restore animations that were already tracked before the
        // snapshot reset.  Never create brand-new animations here — during
        // initial/reconnect loads the SDK may not have aggregated edits yet,
        // so completed messages can still appear as `live`.  Genuinely new
        // streams will be picked up on the next live sync update.
        if let Some(previous_state) = previous_streaming_messages
            .and_then(|states| states.get(&event_id))
        {
            let state = StreamingAnimState::restore(previous_state, &new_text, true);
            should_schedule_frame |= state.needs_frame();
            rebuilt.insert(event_id, state);
        }
    }

    (rebuilt, should_schedule_frame)
}

fn next_stream_timeout<'a>(
    states: impl IntoIterator<Item = &'a super::streaming_animation::StreamingAnimState>,
) -> Option<Duration> {
    states
        .into_iter()
        .map(|state| state.timeout_after().saturating_sub(state.last_update_time.elapsed()))
        .min()
}

fn escape_slash_command_arg(value: &str) -> String {
    value.trim().replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_create_bot_command(
    username: &str,
    display_name: &str,
    system_prompt: Option<&str>,
) -> String {
    let mut command = format!("/createbot {} {}", username.trim(), display_name.trim());
    if let Some(system_prompt) = system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.push_str(" --prompt \"");
        command.push_str(&escape_slash_command_arg(system_prompt));
        command.push('"');
    }
    command
}

fn format_delete_bot_command(matrix_user_id: &UserId) -> String {
    format!("/deletebot {matrix_user_id}")
}

fn resolve_delete_bot_user_id(
    user_id_or_localpart: &str,
    current_user_id: Option<&UserId>,
    app_language: AppLanguage,
) -> Result<OwnedUserId, String> {
    let raw = user_id_or_localpart.trim();
    if raw.is_empty() {
        return Err(tr_key(app_language, "room_screen.bot.delete.error.empty_user_id").into());
    }

    if raw.starts_with('@') || raw.contains(':') {
        let full_user_id = if raw.starts_with('@') {
            raw.to_string()
        } else {
            format!("@{raw}")
        };
        return UserId::parse(&full_user_id)
            .map(|user_id| user_id.to_owned())
            .map_err(|_| tr_fmt(app_language, "room_screen.bot.delete.error.invalid_user_id", &[
                ("full_user_id", full_user_id.as_str()),
            ]));
    }

    let Some(current_user_id) = current_user_id else {
        return Err(
            tr_key(app_language, "room_screen.bot.delete.error.current_user_unavailable").into(),
        );
    };

    let full_user_id = format!("@{raw}:{}", current_user_id.server_name());
    UserId::parse(&full_user_id)
        .map(|user_id| user_id.to_owned())
        .map_err(|_| tr_fmt(app_language, "room_screen.bot.delete.error.invalid_user_id", &[
            ("full_user_id", full_user_id.as_str()),
        ]))
}

fn detected_bot_binding_for_members(
    app_state: &AppState,
    room_id: &OwnedRoomId,
    members: &[RoomMember],
) -> Option<OwnedUserId> {
    if app_state.bot_settings.is_room_bound(room_id) {
        return None;
    }

    let own_user_id = current_user_id();
    let mut non_self_members = members
        .iter()
        .filter(|room_member|
            own_user_id
                .as_deref()
                .is_none_or(|own_user_id| room_member.user_id() != own_user_id)
        )
        .collect::<Vec<_>>();
    non_self_members.sort_by(|lhs, rhs| lhs.user_id().as_str().cmp(rhs.user_id().as_str()));

    if let Ok(configured_bot_user_id) = app_state
        .bot_settings
        .resolved_bot_user_id(current_user_id().as_deref())
    {
        if non_self_members
            .iter()
            .any(|room_member| room_member.user_id().as_str() == configured_bot_user_id.as_str())
        {
            return Some(configured_bot_user_id);
        }
    }

    let known_bot_user_ids = app_state.bot_settings.known_bot_user_ids();
    if let Some(bot_member) = non_self_members
        .iter()
        .find(|room_member|
            known_bot_user_ids
                .iter()
                .any(|known_bot_user_id| known_bot_user_id.as_str() == room_member.user_id().as_str())
        )
    {
        return Some(bot_member.user_id().to_owned());
    }

    if non_self_members.len() == 1 {
        let dm_counterparty = non_self_members[0];
        let localpart = dm_counterparty.user_id().localpart().to_ascii_lowercase();
        let localpart_likely_bot = localpart == "bot"
            || localpart == "botfather"
            || localpart.starts_with("bot_")
            || localpart.starts_with("bot-")
            || localpart.starts_with("bot.");
        let display_name_likely_bot = dm_counterparty
            .display_name()
            .is_some_and(|display_name| display_name.to_ascii_lowercase().contains("bot"));
        if localpart_likely_bot || display_name_likely_bot {
            return Some(dm_counterparty.user_id().to_owned());
        }
    }

    if non_self_members
        .iter()
        .any(|room_member| room_member.user_id().localpart().eq_ignore_ascii_case("botfather"))
    {
        return non_self_members
            .iter()
            .find(|room_member| room_member.user_id().localpart().eq_ignore_ascii_case("botfather"))
            .map(|room_member| room_member.user_id().to_owned());
    };
    None
}

fn is_likely_bot_user_id(
    user_id: &UserId,
    resolved_parent_bot_user_id: Option<&UserId>,
) -> bool {
    if resolved_parent_bot_user_id.is_some_and(|parent| parent == user_id) {
        return true;
    }

    let localpart = user_id.localpart().to_ascii_lowercase();
    localpart == "bot"
        || localpart == "botfather"
        || localpart.starts_with("bot_")
        || localpart.starts_with("bot-")
        || localpart.starts_with("bot.")
        || localpart.ends_with("_bot")
        || (localpart.ends_with("bot") && localpart.len() > 3)
}

pub(crate) fn is_known_or_likely_bot(
    user_id: &UserId,
    resolved_parent_bot_user_id: Option<&UserId>,
    known_bot_user_ids: &[OwnedUserId],
) -> bool {
    known_bot_user_ids
        .iter()
        .any(|known_bot_user_id| known_bot_user_id.as_str() == user_id.as_str())
        || resolved_parent_bot_user_id.is_some_and(|parent| parent == user_id)
        || is_likely_bot_user_id(user_id, resolved_parent_bot_user_id)
}

fn is_timeline_sender_bot(
    user_id: &UserId,
    resolved_parent_bot_user_id: Option<&UserId>,
    room_bot_user_ids: &[OwnedUserId],
    known_bot_user_ids: &[OwnedUserId],
) -> bool {
    room_bot_user_ids
        .iter()
        .any(|room_bot_user_id| room_bot_user_id.as_str() == user_id.as_str())
        || is_known_or_likely_bot(
            user_id,
            resolved_parent_bot_user_id,
            known_bot_user_ids,
        )
}

fn collect_room_bot_user_ids(
    room_members: &[RoomMember],
    resolved_parent_bot_user_id: Option<&UserId>,
    known_bot_user_ids: &[OwnedUserId],
    persisted_room_bot_user_ids: &[OwnedUserId],
) -> Vec<OwnedUserId> {
    let own_user_id = current_user_id();
    let mut room_bot_user_ids = Vec::<OwnedUserId>::new();

    for persisted_room_bot_user_id in persisted_room_bot_user_ids {
        if room_bot_user_ids
            .iter()
            .all(|existing_user_id| existing_user_id.as_str() != persisted_room_bot_user_id.as_str())
        {
            room_bot_user_ids.push(persisted_room_bot_user_id.clone());
        }
    }

    for room_member in room_members.iter().filter(|room_member|
        own_user_id
            .as_deref()
            .is_none_or(|own_user_id| room_member.user_id() != own_user_id)
    ) {
        if is_known_or_likely_bot(
            room_member.user_id(),
            resolved_parent_bot_user_id,
            known_bot_user_ids,
        ) || is_likely_bot_member(room_member, resolved_parent_bot_user_id)
        {
            let user_id = room_member.user_id().to_owned();
            if room_bot_user_ids
                .iter()
                .all(|existing_user_id| existing_user_id.as_str() != user_id.as_str())
            {
                room_bot_user_ids.push(user_id);
            }
        }
    }

    room_bot_user_ids.sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
    room_bot_user_ids
}

fn compute_timeline_bot_context(
    app_state: Option<&AppState>,
    room_id: &OwnedRoomId,
    room_members: Option<&Arc<Vec<RoomMember>>>,
) -> (Option<OwnedUserId>, Vec<OwnedUserId>, Vec<OwnedUserId>) {
    app_state
        .map(|app_state| {
            let app_service_enabled = app_state.bot_settings.enabled;
            let persisted_room_bot_user_ids = if app_service_enabled {
                app_state.bot_settings.bound_bot_user_ids(room_id)
            } else {
                Vec::new()
            };
            let resolved_parent_bot_user_id = if app_service_enabled {
                app_state
                    .bot_settings
                    .resolved_bot_user_id(current_user_id().as_deref())
                    .ok()
            } else {
                None
            };
            let known_bot_user_ids = if app_service_enabled {
                app_state.bot_settings.known_bot_user_ids()
            } else {
                Vec::new()
            };
            let room_bot_user_ids = room_members
                .map(|members|
                    collect_room_bot_user_ids(
                        members.as_ref(),
                        resolved_parent_bot_user_id.as_deref(),
                        &known_bot_user_ids,
                        &persisted_room_bot_user_ids,
                    )
                )
                .unwrap_or(persisted_room_bot_user_ids);
            (
                resolved_parent_bot_user_id,
                room_bot_user_ids,
                known_bot_user_ids,
            )
        })
        .unwrap_or((None, Vec::new(), Vec::new()))
}

fn is_likely_bot_member(
    room_member: &RoomMember,
    resolved_parent_bot_user_id: Option<&UserId>,
) -> bool {
    if is_likely_bot_user_id(room_member.user_id(), resolved_parent_bot_user_id) {
        return true;
    }

    room_member.display_name().is_some_and(|display_name| {
        let display_name = display_name.trim().to_ascii_lowercase();
        display_name == "bot"
            || display_name == "botfather"
            || display_name.starts_with("bot ")
            || display_name.ends_with(" bot")
            || display_name.contains(" bot ")
    })
}

fn extract_bot_user_ids_from_listbots_reply(
    text: &str,
    default_server_name: Option<&OwnedServerName>,
) -> Vec<OwnedUserId> {
    let mut bot_user_ids = Vec::<OwnedUserId>::new();

    let mut push_bot = |bot_user_id: OwnedUserId| {
        if !bot_user_ids
            .iter()
            .any(|existing_bot_user_id| existing_bot_user_id.as_str() == bot_user_id.as_str())
        {
            bot_user_ids.push(bot_user_id);
        }
    };

    for token in text.split(|ch: char|
        !(ch.is_ascii_alphanumeric() || matches!(ch, '@' | ':' | '_' | '-' | '.'))
    ) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if token.starts_with('@') && token.contains(':') {
            if let Ok(bot_user_id) = UserId::parse(token).map(|user_id| user_id.to_owned()) {
                push_bot(bot_user_id);
            }
            continue;
        }

        if token.contains(':') && !token.starts_with('@') {
            let full_user_id = format!("@{token}");
            if let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned()) {
                push_bot(bot_user_id);
            }
            continue;
        }

        let localpart_lc = token.to_ascii_lowercase();
        let is_likely_bot_localpart = (
                localpart_lc == "bot"
                || localpart_lc.starts_with("bot_")
                || localpart_lc.starts_with("bot-")
                || localpart_lc.starts_with("bot.")
            )
            && localpart_lc != "bots"
            && localpart_lc != "botfather"
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.');
        if !is_likely_bot_localpart {
            continue;
        }

        let Some(default_server_name) = default_server_name else { continue };
        let full_user_id = format!("@{token}:{default_server_name}");
        if let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned()) {
            push_bot(bot_user_id);
        }
    }

    bot_user_ids
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.COLOR_BG = #xfff8ee
    mod.widgets.COLOR_OVERLAY_BG = #x000000d8
    mod.widgets.COLOR_READ_MARKER = #xeb2733

    mod.widgets.REACTION_TEXT_COLOR = #4c00b0

    mod.widgets.COLOR_THREAD_SUMMARY_BG = #FFF4E5
    mod.widgets.COLOR_THREAD_SUMMARY_BG_HOVER = #FFEACC
    mod.widgets.COLOR_THREAD_SUMMARY_BORDER = #E8C99A
    mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT = #A35A00
    mod.widgets.COLOR_BOT_CARD_BG = #xF7FAFE
    mod.widgets.COLOR_BOT_CARD_BORDER = #xD8E3F0
    mod.widgets.COLOR_BOT_STATUS_BG = #xEEF4FB
    mod.widgets.COLOR_BOT_STATUS_TEXT = #x5A6F86
    mod.widgets.COLOR_BOT_PROVIDER_TEXT = #x708399
    mod.widgets.COLOR_BOT_FOOTER_TEXT = #x8B98A7
    mod.widgets.COLOR_BOT_CODE_BG = #xECF2F8
    mod.widgets.COLOR_BOT_CODE_BORDER = #xD5E0ED

    mod.widgets.MessageActionPrimaryButton = RobrixPositiveIconButton {
        width: Fit
        height: Fit
        spacing: 6.0
        padding: Inset{ left: 10.0, right: 10.0, top: 7.0, bottom: 7.0 }
        draw_text +: {
            text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
        }
    }

    mod.widgets.MessageActionSecondaryButton = Button {
        width: Fit
        height: Fit
        spacing: 6.0
        padding: Inset{ left: 10.0, right: 10.0, top: 7.0, bottom: 7.0 }
        draw_text +: {
            text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
        }
        text: ""
    }

    mod.widgets.MessageActionDangerButton = RobrixNegativeIconButton {
        width: Fit
        height: Fit
        spacing: 6.0
        padding: Inset{ left: 10.0, right: 10.0, top: 7.0, bottom: 7.0 }
        draw_text +: {
            text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
        }
    }

    mod.widgets.MessageActionButtonSlot = View {
        visible: false
        width: Fit
        height: Fit
        flow: Overlay

        primary_button := mod.widgets.MessageActionPrimaryButton {
            visible: false
        }
        secondary_button := mod.widgets.MessageActionSecondaryButton {
            visible: false
        }
        danger_button := mod.widgets.MessageActionDangerButton {
            visible: false
        }
    }

    mod.widgets.BotTimelineMarkdown = Markdown {
        width: Fill
        height: Fit
        padding: 0.0
        font_size: (MESSAGE_FONT_SIZE)
        font_color: (MESSAGE_TEXT_COLOR)
        paragraph_spacing: 10.0
        pre_code_spacing: 8.0
        heading_base_scale: 1.45
        inline_code_padding: Inset{ top: 3, bottom: 3, left: 4, right: 4 }
        inline_code_margin: Inset{ left: 3, right: 3, bottom: 2, top: 2 }
        use_code_block_widget: true

        draw_text +: {
            color: (MESSAGE_TEXT_COLOR)
        }
        text_style_normal: mod.widgets.MESSAGE_TEXT_STYLE {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_italic: theme.font_italic {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_bold: theme.font_bold {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_bold_italic: theme.font_bold_italic {
            font_size: (MESSAGE_FONT_SIZE)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        text_style_fixed: mod.widgets.MESSAGE_CODE_TEXT_STYLE {
            font_size: (MESSAGE_FONT_SIZE - 0.5)
            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
        }
        draw_block +: {
            line_color: (MESSAGE_TEXT_COLOR)
            sep_color: (mod.widgets.COLOR_BOT_CODE_BORDER)
            quote_bg_color: #xEFF5FB
            quote_fg_color: #x7892AC
            code_color: (mod.widgets.COLOR_BOT_CODE_BG)
        }
        code_layout: Layout{
            flow: Flow.Right{wrap: true}
            padding: Inset{ left: 0.0, right: 0.0, top: 0.0, bottom: 0.0 }
        }
        code_walk: Walk{ width: Fill, height: Fit, margin: Inset{ top: 10.0, bottom: 10.0 } }
        quote_layout: Layout{
            flow: Flow.Right{wrap: true}
            padding: Inset{ left: 12.0, right: 12.0, top: 8.0, bottom: 8.0 }
        }
        quote_walk: Walk{ width: Fill, height: Fit, margin: Inset{ top: 6.0, bottom: 6.0 } }
        list_item_layout: Layout{
            flow: Flow.Right{wrap: true}
            padding: Inset{ left: 0.0, right: 0.0, top: 1.0, bottom: 1.0 }
        }
        list_item_walk: Walk{ width: Fill, height: Fit, margin: Inset{ top: 0.0, bottom: 1.0 } }

        code_block := RoundedView {
            width: Fill
            height: Fit
            flow: Overlay
            padding: 0.0
            new_batch: true
            show_bg: true
            draw_bg +: {
                color: (mod.widgets.COLOR_BOT_CODE_BG)
                border_radius: 10.0
                border_size: 1.0
                border_color: (mod.widgets.COLOR_BOT_CODE_BORDER)
            }

            code_view := mod.widgets.CodeView {
                keep_cursor_at_end: false
                editor +: {
                    width: Fill
                    height: Fit
                    margin: Inset{ left: 12.0, right: 12.0, top: 10.0, bottom: 10.0 }
                    draw_bg +: { color: #0000 }
                    draw_text +: {
                        text_style: mod.widgets.MESSAGE_CODE_TEXT_STYLE {
                            font_size: (MESSAGE_FONT_SIZE - 0.5)
                            line_spacing: (MESSAGE_TEXT_LINE_SPACING)
                        }
                    }
                    token_colors +: {
                        whitespace: #x6a737d
                        delimiter: #x24292e
                        delimiter_highlight: #x005cc5
                        error_decoration: #xcb2431
                        warning_decoration: #xb08800
                        unknown: #x24292e
                        branch_keyword: #xd73a49
                        constant: #x005cc5
                        identifier: #x24292e
                        loop_keyword: #xd73a49
                        number: #x005cc5
                        other_keyword: #xd73a49
                        punctuator: #x24292e
                        string: #x22863a
                        function: #x6f42c1
                        typename: #xe36209
                        comment: #x6a737d
                    }
                }
            }
        }
    }

    // An empty view that takes up no space in the portal list.
    mod.widgets.Empty = View { }

    // A summary at the bottom of a message that is the root of a thread.
    mod.widgets.ThreadRootSummary = RoundedView {
        visible: false
        width: Fill,
        height: Fit
        flow: Right,
        align: Align{x: 0.0, y: 0.5}
        spacing: 5.0
        margin: Inset{ top: 5.0 }
        padding: 12,
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: (mod.widgets.COLOR_THREAD_SUMMARY_BG)
            border_radius: 4.0
            border_size: 1.5
            border_color: (mod.widgets.COLOR_THREAD_SUMMARY_BORDER)
        }

        thread_summary_count := Label {
            width: Fit,
            draw_text +: {
                text_style: USERNAME_TEXT_STYLE { font_size: 11 }
                color: (mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT)
            }
            text: ""
        }

        Icon {
            width: Fit, height: Fit,
            align: Align{x: 0.5, y: 0.5}
            draw_icon +: {
                svg: crate_resource("self://resources/icons/double_chat.svg")
                color: (mod.widgets.COLOR_THREAD_SUMMARY_REPLY_COUNT)
            }
            icon_walk: Walk{ width: 25, height: 25, margin: Inset{top: 3, right: 7} }
        }

        thread_summary_latest := MessageHtml {
            flow: Right,
            max_lines: 2
            text_overflow: Ellipsis
        }
    }

    // The view used for each text-based message event in a room's timeline.
    mod.widgets.Message = set_type_default() do #(Message::register_widget(vm)) {

        width: Fill,
        height: Fit,
        margin: 0.0
        flow: Down,
        cursor: MouseCursor.Default,
        padding: 0.0,
        spacing: 0.0

        show_bg: true
        draw_bg +: {
            highlight: instance(0.0)
            hover: instance(0.0)
            color: instance((COLOR_PRIMARY)) // default color)

            mentions_bar_color: instance((COLOR_PRIMARY))
            mentions_bar_width: instance(4.0)

            pixel: fn() {
                let base_color = mix(
                    self.color,
                    #fafafa,
                    self.hover
                );

                let with_highlight = mix(
                    base_color,
                    #c5d6fa,
                    self.highlight
                );

                let sdf = Sdf2d.viewport(self.pos * self.rect_size);

                // draw bg
                sdf.rect(0., 0., self.rect_size.x, self.rect_size.y);
                sdf.fill(with_highlight);

                // draw the left vertical line
                sdf.rect(0., 0., self.mentions_bar_width, self.rect_size.y);
                sdf.fill(self.mentions_bar_color);

                return sdf.result;
            }
        }

        animator: Animator{
            highlight: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 2.0} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 0.5} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 1.0} }
                }
            }
            hover: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 1.0} }
                }
            }
        }

        // A preview of the earlier message that this message was in reply to.
        replied_to_message := mod.widgets.RepliedToMessage {
            flow: Right
            margin: Inset{ bottom: 3, top: 10 }
            replied_to_message_content +: {
                margin +: { left: 29 }
                padding +: { bottom: 10 }
            }
        }

        body := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{top: 0, bottom: 10, left: 10, right: 10},

            profile := View {
                align: Align{x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                margin: Inset{top: #(MESSAGE_PROFILE_TOP_MARGIN), right: 10}
                flow: Down,
                avatar := Avatar {
                    width: #(MESSAGE_PROFILE_AVATAR_SIZE),
                    height: #(MESSAGE_PROFILE_AVATAR_SIZE),
                }
                timestamp := Timestamp {
                    margin: Inset{ top: 5.9 }
                }
                edited_indicator := EditedIndicator { }
                tsp_sign_indicator := TspSignIndicator { }
            }

            content := View {
                width: Fill,
                height: Fit
                flow: Down,
                padding: 0.0

                username_view := View {
                    flow: Right,
                    align: Align{y: 0.5},
                    width: Fit,
                    height: #(MESSAGE_USERNAME_ROW_HEIGHT),
                    margin: Inset{
                        top: #(MESSAGE_USERNAME_ROW_TOP_MARGIN),
                        bottom: #(MESSAGE_USERNAME_ROW_BOTTOM_MARGIN),
                    }
                    username := Label {
                        width: Fit,
                        flow: Right, // do not wrap
                        padding: 0,
                        margin: Inset{right: #(MESSAGE_USERNAME_RIGHT_MARGIN)}
                        max_lines: 1
                        text_overflow: Ellipsis
                        draw_text +: {
                            text_style: USERNAME_TEXT_STYLE {},
                            color: (USERNAME_TEXT_COLOR)
                        }
                        text: ""
                    }
                    bot_badge := RoundedView {
                        visible: false
                        width: Fit
                        height: #(BOT_BADGE_HEIGHT)
                        align: Align{x: 0.5, y: 0.5}
                        new_batch: true
                        padding: Inset{left: #(BOT_BADGE_HORIZONTAL_PADDING), right: #(BOT_BADGE_HORIZONTAL_PADDING)}
                        show_bg: true
                        draw_bg +: {
                            color: (COLOR_ACTIVE_PRIMARY)
                            border_radius: #(BOT_BADGE_BORDER_RADIUS)
                        }
                        bot_badge_label := Label {
                            width: Fit
                            height: Fit
                            padding: 0
                            draw_text +: {
                                text_style: REGULAR_TEXT {
                                    font_size: #(BOT_BADGE_TEXT_FONT_SIZE)
                                    top_drop: #(BOT_BADGE_TEXT_TOP_DROP)
                                }
                                color: #fff
                            }
                            text: "bot"
                        }
                    }
                }

                bot_message_card := View {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    margin: Inset{ top: 1.0, bottom: 3.0 }

                    bot_status_strip := RoundedView {
                        visible: false
                        width: Fit
                        height: Fit
                        new_batch: true
                        padding: Inset{ left: 10.0, right: 10.0, top: 5.0, bottom: 5.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_STATUS_BG)
                            border_radius: 10.0
                        }

                        bot_status_label := Label {
                            width: Fit
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 9.5 }
                                color: (mod.widgets.COLOR_BOT_STATUS_TEXT)
                            }
                            text: ""
                        }
                    }

                    bot_body_card := RoundedView {
                        width: Fill
                        height: Fit
                        flow: Down
                        new_batch: true
                        padding: Inset{ left: 14.0, right: 14.0, top: 12.0, bottom: 12.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_CARD_BG)
                            border_radius: 14.0
                            border_size: 1.0
                            border_color: (mod.widgets.COLOR_BOT_CARD_BORDER)
                        }

                        bot_card_body := HtmlOrPlaintext { }
                        bot_card_markdown := mod.widgets.BotTimelineMarkdown {
                            visible: false
                            body: ""
                        }
                        bot_card_markdown_plain := mod.widgets.BotTimelineMarkdown {
                            visible: false
                            use_code_block_widget: false
                            body: ""
                        }
                    }

                    bot_metadata_footer := View {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 2.0
                        padding: Inset{ left: 2.0 }

                        bot_provider_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
                                color: (mod.widgets.COLOR_BOT_PROVIDER_TEXT)
                            }
                            text: ""
                        }

                        bot_footer_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 9.5 }
                                color: (mod.widgets.COLOR_BOT_FOOTER_TEXT)
                            }
                            text: ""
                        }
                    }
                }

                message := HtmlOrPlaintext { }
                splash_card := Splash { visible: false }
                action_buttons := View {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    margin: Inset{ top: 8.0, bottom: 2.0 }

                    approval_request_view := RoundedView {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Down
                        new_batch: true
                        spacing: 4.0
                        padding: Inset{ left: 12.0, right: 12.0, top: 10.0, bottom: 10.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_STATUS_BG)
                            border_radius: 12.0
                            border_size: 1.0
                            border_color: (mod.widgets.COLOR_BOT_CARD_BORDER)
                        }

                        approval_title_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: theme.font_bold { font_size: 10.5 }
                                color: (mod.widgets.COLOR_TEXT)
                            }
                            text: ""
                        }

                        approval_summary_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
                                color: (mod.widgets.COLOR_BOT_STATUS_TEXT)
                            }
                            text: ""
                        }
                    }

                    action_button_row := View {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        spacing: 8.0

                        action_button_slot_0 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_1 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_2 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_3 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_4 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_5 := mod.widgets.MessageActionButtonSlot {}
                    }
                }
                link_preview_view := mod.widgets.LinkPreview {}
                View {
                    width: Fill,
                    height: Fit
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }
        }
    }

    // The view used for a condensed message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    mod.widgets.CondensedMessage = mod.widgets.Message {
        padding: Inset{ top: 2.0, bottom: 2.0 }
        replied_to_message +: {
            replied_to_message_content +: {
                margin: Inset{ left: 74, bottom: 5.0 }
            }
        }
        body := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{ top: 0, bottom: 2.5, left: 10.0, right: 10.0 },
            profile := View {
                align: Align{x: 0.5, y: 0.0} // centered horizontally, top aligned
                width: 65.0,
                height: Fit,
                flow: Down,
                timestamp := Timestamp {
                    margin: Inset{top: 2.5}
                }
                edited_indicator := EditedIndicator { }
                tsp_sign_indicator := TspSignIndicator { }
            }
            content := View {
                width: Fill,
                height: Fit,
                flow: Down,
                padding: Inset{ left: 10.0 }

                bot_message_card := View {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    margin: Inset{ top: 1.0, bottom: 3.0 }

                    bot_status_strip := RoundedView {
                        visible: false
                        width: Fit
                        height: Fit
                        new_batch: true
                        padding: Inset{ left: 10.0, right: 10.0, top: 5.0, bottom: 5.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_STATUS_BG)
                            border_radius: 10.0
                        }

                        bot_status_label := Label {
                            width: Fit
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 9.5 }
                                color: (mod.widgets.COLOR_BOT_STATUS_TEXT)
                            }
                            text: ""
                        }
                    }

                    bot_body_card := RoundedView {
                        width: Fill
                        height: Fit
                        flow: Down
                        new_batch: true
                        padding: Inset{ left: 14.0, right: 14.0, top: 12.0, bottom: 12.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_CARD_BG)
                            border_radius: 14.0
                            border_size: 1.0
                            border_color: (mod.widgets.COLOR_BOT_CARD_BORDER)
                        }

                        bot_card_body := HtmlOrPlaintext { }
                        bot_card_markdown := mod.widgets.BotTimelineMarkdown {
                            visible: false
                            body: ""
                        }
                        bot_card_markdown_plain := mod.widgets.BotTimelineMarkdown {
                            visible: false
                            use_code_block_widget: false
                            body: ""
                        }
                    }

                    bot_metadata_footer := View {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 2.0
                        padding: Inset{ left: 2.0 }

                        bot_provider_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
                                color: (mod.widgets.COLOR_BOT_PROVIDER_TEXT)
                            }
                            text: ""
                        }

                        bot_footer_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 9.5 }
                                color: (mod.widgets.COLOR_BOT_FOOTER_TEXT)
                            }
                            text: ""
                        }
                    }
                }

                message := HtmlOrPlaintext { }
                action_buttons := View {
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    margin: Inset{ top: 8.0, bottom: 2.0 }

                    approval_request_view := RoundedView {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Down
                        new_batch: true
                        spacing: 4.0
                        padding: Inset{ left: 12.0, right: 12.0, top: 10.0, bottom: 10.0 }
                        show_bg: true
                        draw_bg +: {
                            color: (mod.widgets.COLOR_BOT_STATUS_BG)
                            border_radius: 12.0
                            border_size: 1.0
                            border_color: (mod.widgets.COLOR_BOT_CARD_BORDER)
                        }

                        approval_title_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: theme.font_bold { font_size: 10.5 }
                                color: (mod.widgets.COLOR_TEXT)
                            }
                            text: ""
                        }

                        approval_summary_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: mod.widgets.MESSAGE_TEXT_STYLE { font_size: 10.0 }
                                color: (mod.widgets.COLOR_BOT_STATUS_TEXT)
                            }
                            text: ""
                        }
                    }

                    action_button_row := View {
                        visible: false
                        width: Fill
                        height: Fit
                        flow: Flow.Right{wrap: true}
                        spacing: 8.0

                        action_button_slot_0 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_1 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_2 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_3 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_4 := mod.widgets.MessageActionButtonSlot {}
                        action_button_slot_5 := mod.widgets.MessageActionButtonSlot {}
                    }
                }
                link_preview_view := mod.widgets.LinkPreview {}
                View {
                    width: Fill,
                    height: Fit
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }
        }
    }

    // The view used for each static image-based message event in a room's timeline.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    mod.widgets.ImageMessage = mod.widgets.Message {
        body +: {
            content +: {
                width: Fill,
                height: Fit
                padding: Inset{ left: 10.0 }

                message := TextOrImage { }
                View {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }

        }
    }

    // The view used for a condensed image message that came right after another message
    // from the same sender, and thus doesn't need to display the sender's profile again.
    // This excludes stickers and other animated GIFs, video clips, audio clips, etc.
    mod.widgets.CondensedImageMessage = mod.widgets.CondensedMessage {
        body +: {
            content +: {
                message := TextOrImage { }
                View {
                    width: Fill,
                    height: Fit,
                    flow: Right,
                    reaction_list := mod.widgets.ReactionList { }
                    avatar_row := mod.widgets.AvatarRow {}
                }
                thread_root_summary := mod.widgets.ThreadRootSummary {}
            }
        }
    }


    // The view used for each state event (non-messages) in a room's timeline.
    // The timestamp, profile picture, and text are all very small.
    mod.widgets.SmallStateEvent = View {
        width: Fill,
        height: Fit,
        flow: Right,
        margin: Inset{ top: 4.0, bottom: 4.0}
        padding: Inset{ top: 1.0, bottom: 1.0, right: 10.0 }
        spacing: 0.0
        cursor: MouseCursor.Default

        body := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{ left: 7.0, top: 2.0, bottom: 2.0 }
            spacing: 5.0

            left_container := View {
                align: Align{x: 0.5, y: 0}
                width: 70.0,
                height: Fit

                timestamp := Timestamp {
                    margin: Inset{top: 3}
                }
            }

            avatar := Avatar {
                width: 19.,
                height: 19.,
                margin: 0

                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: 7.0 }
                        }
                    }
                }
            }

            // Show an invite button only for a `Knocked` room membership change.
            // All other small state events will not show this button.
            invite_user_button := RobrixPositiveIconButton {
                visible: false
                margin: Inset{ top: -1.5, left: 2, right: 2}
                padding: Inset{top: 4, bottom: 4, left: 9, right: 9}
                draw_bg +: {
                    border_size: 0.75
                }
                draw_icon.svg: (ICON_ADD_USER)
                draw_text.text_style: SMALL_STATE_TEXT_STYLE {}
                icon_walk: Walk{width: 15, height: Fit, margin: Inset{right: -4}}
                text: ""
            }

            content := Label {
                width: Fill,
                height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset{top: 2.5}
                padding: Inset{ top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 }
                draw_text +: {
                    text_style: SMALL_STATE_TEXT_STYLE {},
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            avatar_row := mod.widgets.AvatarRow {}
        }
    }


    // The view used for each day divider in a room's timeline.
    // The date text is centered between two horizontal lines.
    mod.widgets.DateDivider = View {
        width: Fill,
        height: Fit,
        margin: Inset{top: 7.0, bottom: 7.0}
        flow: Right,
        padding: Inset{left: 7.0, right: 7.0},
        spacing: 0.0,
        align: Align{x: 0.5, y: 0.5} // center horizontally and vertically

        left_line := LineH { }

        date := Label {
            padding: Inset{left: 7.0, right: 7.0}
            draw_text +: {
                text_style: TEXT_SUB {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: ""
        }

        right_line := LineH { }
    }

    // The view used for the divider indicating where the user's last-viewed message is.
    // This is implemented as a DateDivider with a different color and a fixed text label.
    mod.widgets.ReadMarker = mod.widgets.DateDivider {
        left_line := LineH {
            draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
        }

        date := Label {
            draw_text.color: (mod.widgets.COLOR_READ_MARKER)
            text: ""
        }

        right_line := LineH {
            draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
        }
    }


    // The top space is used to display a loading message while the room is being paginated.
    mod.widgets.TopSpace = SolidView {
        visible: false,
        width: Fill,
        height: Fit,
        align: Align{x: 0.5, y: 0}
        flow: Right,
        show_bg: true,
        draw_bg.color: #xDAF5E5F0, // mostly opaque light green

        label := Label {
            width: Fill,
            height: Fit,
            align: Align{x: 0.5, y: 0.5},
            flow: Right,
            padding: Inset{ top: 10.0, bottom: 7.0, left: 15.0, right: 15.0 }
            draw_text +: {
                text_style: MESSAGE_TEXT_STYLE { font_size: 10 },
                color: (TIMESTAMP_TEXT_COLOR)
            }
            text: ""
        }
    }

    mod.widgets.ThreadsPaneEntry = #(ThreadsPaneEntry::register_widget(vm)) {
        ..mod.widgets.RoundedView

        width: Fill
        height: Fit
        flow: Down
        spacing: 5
        padding: Inset{top: 12, right: 12, bottom: 12, left: 12}
        margin: Inset{left: 12, right: 12, top: 6, bottom: 0}
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: #F8FAFD
            border_radius: 4.0
            border_size: 1.0
            border_color: #D8E0EA
        }

        title_row := View {
            width: Fill
            height: Fit
            flow: Right
            spacing: 8

            title := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE { font_size: 10.8 }
                    color: #1F1F1F
                }
                text: ""
            }

            time := Label {
                width: Fit
                height: Fit
                draw_text +: {
                    text_style: TIMESTAMP_TEXT_STYLE { font_size: 7.5 }
                    color: (TIMESTAMP_TEXT_COLOR)
                }
                text: ""
            }
        }

        subtitle := Label {
            width: Fill
            height: Fit
            flow: Flow.Right{wrap: true}
            draw_text +: {
                text_style: MESSAGE_TEXT_STYLE { font_size: 9.8 }
                color: #7B7B7B
            }
            text: ""
        }

        preview := Label {
            width: Fill
            height: Fit
            flow: Flow.Right{wrap: true}
            draw_text +: {
                text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                color: (COLOR_TEXT)
            }
            text: ""
        }
    }

    mod.widgets.ThreadsSlidingPane = #(ThreadsSlidingPane::register_widget(vm)) {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: Align{x: 1.0, y: 0}

        bg_view := SolidView {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg.color: #000000BB
        }

        main_content := SolidView {
            width: 320,
            height: Fill
            flow: Down,
            align: Align{x: 1.0}

            show_bg: true,
            draw_bg.color: (COLOR_PRIMARY)

            header := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{top: 12, right: 10, bottom: 12, left: 15}

                title := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: USERNAME_TEXT_STYLE { font_size: 12.5 }
                        color: #000
                    }
                    text: "Threads"
                }

                spacer := View {
                    width: Fill
                    height: Fit
                }

                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    padding: 15,
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                    text: ""
                }
            }

            room_name := Label {
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                padding: Inset{left: 15, right: 15, bottom: 10}
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                    color: #6E6E6E
                }
                text: ""
            }

            loading_indicator := View {
                visible: false
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                spacing: 8
                padding: Inset{left: 15, right: 15, top: 6, bottom: 10}

                spinner := LoadingSpinner {
                    width: 18
                    height: 18
                }

                loading_label := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                        color: #7B7B7B
                    }
                    text: "Loading threads..."
                }
            }

            empty_state := Label {
                visible: false
                width: Fill
                height: Fit
                flow: Flow.Right{wrap: true}
                padding: Inset{left: 15, right: 15, top: 20, bottom: 20}
                draw_text +: {
                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                    color: #7B7B7B
                }
                text: "No threads yet."
            }

            threads_list := PortalList {
                width: Fill
                height: Fill
                flow: Down
                max_pull_down: 0.0

                ThreadEntry := mod.widgets.ThreadsPaneEntry {}
            }
        }

        slide: 1.0,

        animator: Animator {
            panel: {
                default: @hide
                show: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 0.0
                    }
                }
                hide: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 1.0
                    }
                }
            }
        }
    }

    mod.widgets.RoomInfoPeopleEntry = #(RoomInfoPeopleEntry::register_widget(vm)) {
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 9
        padding: Inset{left: 10, right: 10, top: 10, bottom: 10}
        margin: Inset{left: 0, right: 0, top: 0, bottom: 6}
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            color: #F8FAFD
            border_radius: 4.0
            border_size: 1.0
            border_color: #D8E0EA
        }

        avatar := Avatar {
            width: 34
            height: 34
        }

        display_name := Label {
            width: Fill
            height: Fit
            flow: Flow.Right{wrap: true}
            draw_text +: {
                text_style: USERNAME_TEXT_STYLE { font_size: 11.2 }
                color: #1F1F1F
            }
            text: ""
        }

        level := Label {
            width: Fit
            height: Fit
            draw_text +: {
                text_style: MESSAGE_TEXT_STYLE { font_size: 10.2 }
                color: #6D7682
            }
            text: ""
        }
    }

    mod.widgets.RoomInfoSlidingPane = #(RoomInfoSlidingPane::register_widget(vm)) {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: Align{x: 1.0, y: 0}

        bg_view := SolidView {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg.color: #000000BB
        }

        main_content := SolidView {
            width: 320,
            height: Fill
            flow: Down,
            align: Align{x: 1.0}

            show_bg: true,
            draw_bg.color: (COLOR_PRIMARY)

            header := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{top: 12, right: 10, bottom: 12, left: 15}

                back_button := RobrixNeutralIconButton {
                    visible: false
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    padding: 12,
                    draw_icon.svg: (ICON_JUMP)
                    icon_walk: Walk{width: 14, height: 14}
                    text: ""
                }

                title := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: USERNAME_TEXT_STYLE { font_size: 12.5 }
                        color: #000
                    }
                    text: "Info"
                }

                spacer := View {
                    width: Fill
                    height: Fit
                }

                close_button := RobrixNeutralIconButton {
                    width: Fit,
                    height: Fit,
                    spacing: 0,
                    padding: 15,
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14}
                    text: ""
                }
            }

            content_scroll := ScrollYView {
                width: Fill
                height: Fill
                flow: Down

                info_view := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 10
                    padding: Inset{left: 12, right: 12, top: 12, bottom: 12}

                    summary_card := RoundedView {
                        width: Fill
                        height: Fit
                        flow: Right
                        spacing: 10
                        align: Align{y: 0.5}
                        padding: Inset{left: 10, right: 10, top: 10, bottom: 10}

                        show_bg: true
                        draw_bg +: {
                            color: #F8FAFD
                            border_radius: 4.0
                            border_size: 1.0
                            border_color: #D8E0EA
                        }

                        room_avatar := Avatar {
                            width: 40
                            height: 40
                        }

                        room_meta := View {
                            width: Fill
                            height: Fit
                            flow: Down
                            spacing: 4

                            room_name_value := Label {
                                width: Fill
                                height: Fit
                                flow: Flow.Right{wrap: true}
                                draw_text +: {
                                    text_style: USERNAME_TEXT_STYLE { font_size: 11.0 }
                                    color: #1F1F1F
                                }
                                text: ""
                            }

                            room_id_row := View {
                                width: Fill
                                height: Fit
                                flow: Right
                                align: Align{y: 0.5}
                                spacing: 5

                                room_id_value := Label {
                                    width: Fill
                                    height: Fit
                                    flow: Flow.Right{wrap: true}
                                    draw_text +: {
                                        text_style: MESSAGE_TEXT_STYLE { font_size: 9.5 }
                                        color: #6A6A6A
                                    }
                                    text: ""
                                }

                                copy_room_id_button := RobrixNeutralIconButton {
                                    width: 24
                                    height: 22
                                    padding: 4
                                    spacing: 0
                                    draw_icon.svg: (ICON_COPY)
                                    icon_walk: Walk{width: 11, height: 11}
                                    text: ""
                                }
                            }
                        }
                    }

                    topic_card := RoundedView {
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 5
                        padding: Inset{left: 10, right: 10, top: 8, bottom: 8}

                        show_bg: true
                        draw_bg +: {
                            color: #F8FAFD
                            border_radius: 4.0
                            border_size: 1.0
                            border_color: #D8E0EA
                        }

                        topic_label := Label {
                            width: Fill
                            height: Fit
                            draw_text +: {
                                text_style: USERNAME_TEXT_STYLE { font_size: 9.5 }
                                color: #4A4A4A
                            }
                            text: "Topic"
                        }

                        topic_value := Label {
                            width: Fill
                            height: Fit
                            flow: Flow.Right{wrap: true}
                            draw_text +: {
                                text_style: MESSAGE_TEXT_STYLE { font_size: 10.2 }
                                color: #6A6A6A
                            }
                            text: ""
                        }

                        topic_toggle_button := RobrixNeutralIconButton {
                            visible: false
                            width: Fit
                            height: 30
                            align: Align{x: 0.0, y: 0.5}
                            padding: Inset{left: 9, right: 9, top: 6, bottom: 6}
                            spacing: 0
                            icon_walk: Walk{width: 0, height: 0}
                            text: "Expand"
                        }
                    }

                    facts_card := RoundedView {
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 6
                        padding: Inset{left: 10, right: 10, top: 9, bottom: 9}

                        show_bg: true
                        draw_bg +: {
                            color: #F8FAFD
                            border_radius: 4.0
                            border_size: 1.0
                            border_color: #D8E0EA
                        }

                        visibility_row := View {
                            width: Fill
                            height: Fit
                            flow: Right

                            visibility_label := Label {
                                width: 78
                                height: Fit
                                draw_text +: {
                                    text_style: USERNAME_TEXT_STYLE { font_size: 9.5 }
                                    color: #4A4A4A
                                }
                                text: "Visibility"
                            }

                            visibility_value := Label {
                                width: Fill
                                height: Fit
                                draw_text +: {
                                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                                    color: (COLOR_TEXT)
                                }
                                text: ""
                            }
                        }

                        encryption_row := View {
                            width: Fill
                            height: Fit
                            flow: Right

                            encryption_label := Label {
                                width: 78
                                height: Fit
                                draw_text +: {
                                    text_style: USERNAME_TEXT_STYLE { font_size: 9.5 }
                                    color: #4A4A4A
                                }
                                text: "Encryption"
                            }

                            encryption_value := Label {
                                width: Fill
                                height: Fit
                                draw_text +: {
                                    text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
                                    color: (COLOR_TEXT)
                                }
                                text: ""
                            }
                        }
                    }

                    actions_row := View {
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 8

                        invite_button := RobrixNeutralIconButton {
                            width: Fill
                            height: 40
                            padding: 10
                            draw_icon.svg: (ICON_ADD_USER)
                            icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                            text: "Invite"
                        }

                        people_button := RobrixNeutralIconButton {
                            width: Fill
                            height: 40
                            padding: 10
                            draw_icon.svg: (ICON_ADD_USER)
                            icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                            text: "People"
                        }

                        report_room_button := RobrixNeutralIconButton {
                            width: Fill
                            height: 40
                            padding: 10
                            draw_icon.svg: (ICON_INFO)
                            icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                            text: "Report room"
                        }

                        leave_room_button := RobrixNegativeIconButton {
                            width: Fill
                            height: 40
                            padding: 10
                            draw_icon.svg: (ICON_CLOSE)
                            icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                            text: "Leave Room"
                        }
                    }
                }

            }

            people_view := View {
                visible: false
                width: Fill
                height: Fill
                flow: Down
                spacing: 6
                padding: Inset{left: 12, right: 12, top: 12, bottom: 10}

                member_count := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: USERNAME_TEXT_STYLE { font_size: 10.5 }
                        color: #4A4A4A
                    }
                    text: ""
                }

                loading_label := Label {
                    visible: false
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                        color: #6D7682
                    }
                    text: "Loading members..."
                }

                empty_label := Label {
                    visible: false
                    width: Fill
                    height: Fit
                    draw_text +: {
                        text_style: MESSAGE_TEXT_STYLE { font_size: 10.0 }
                        color: #6D7682
                    }
                    text: "No members found."
                }

                people_list := PortalList {
                    width: Fill
                    height: Fill
                    flow: Down
                    max_pull_down: 0.0

                    PersonEntry := mod.widgets.RoomInfoPeopleEntry {}
                }
            }
        }

        slide: 1.0,

        animator: Animator {
            panel: {
                default: @hide
                show: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 0.0
                    }
                }
                hide: AnimatorState{
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        slide: 1.0
                    }
                }
            }
        }
    }

    mod.widgets.ReportRoomModalLabel = Label {
        width: Fill
        height: Fit
        draw_text +: {
            text_style: REGULAR_TEXT { font_size: 10.5 }
            color: #333
        }
        text: ""
    }

    mod.widgets.ReportRoomModal = #(ReportRoomModal::register_widget(vm)) {
        width: Fit
        height: Fit

        RoundedView {
            width: 430
            height: Fit
            align: Align{x: 0.5}
            flow: Down
            padding: Inset{top: 26, right: 22, bottom: 18, left: 22}
            spacing: 14

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 6.0
            }

            title := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: TITLE_TEXT { font_size: 13 }
                    color: #000
                }
                text: "Report Room"
            }

            body := mod.widgets.ReportRoomModalLabel {
                text: ""
            }

            reason_input := RobrixTextInput {
                width: Fill
                height: Fit
                padding: 10
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 11.5 }
                    color: #000
                }
                empty_text: "Describe why you are reporting this room"
            }

            status_label := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10.2 }
                    color: #000
                }
                text: ""
            }

            buttons := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{x: 1.0, y: 0.5}
                spacing: 16

                cancel_button := RobrixNeutralIconButton {
                    width: 110
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_FORBIDDEN)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Cancel"
                }

                report_button := RobrixNegativeIconButton {
                    width: 130
                    align: Align{x: 0.5, y: 0.5}
                    padding: 12
                    draw_icon.svg: (ICON_INFO)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: "Report room"
                }
            }
        }
    }

    mod.widgets.AppServicePanel = #(AppServicePanel::register_widget(vm)) {
        width: Fill
        height: Fit
        margin: Inset{left: 14, right: 54, top: 10, bottom: 16}
        flow: Down
        align: Align{x: 0.0, y: 0.0}
        spacing: 8

        sender_row := View {
            width: Fit
            height: Fit
            flow: Right
            spacing: 6

            sender_name := Label {
                width: Fit
                height: Fit
                draw_text +: {
                    text_style: USERNAME_TEXT_STYLE { font_size: 10.8 }
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                text: ""
            }

            sender_tag := Label {
                width: Fit
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 9.5 }
                    color: #8A8A8A
                }
                text: ""
            }
        }

        bubble := RoundedView {
            width: 408
            height: Fit
            flow: Down
            spacing: 8
            padding: Inset{top: 14, right: 14, bottom: 12, left: 14}

            show_bg: true
            draw_bg +: {
                color: (COLOR_PRIMARY)
                border_radius: 0.0
                border_size: 1.0
                border_color: (COLOR_SECONDARY_DARKER)
            }

            header := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}

                title := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: USERNAME_TEXT_STYLE { font_size: 11.2 }
                        color: #1F1F1F
                    }
                    text: ""
                }

                spacer := View {
                    width: Fill
                    height: Fit
                }

                dismiss_button := RobrixNeutralIconButton {
                    width: 28
                    height: 24
                    align: Align{x: 0.5, y: 0.5}
                    spacing: 0
                    padding: 0
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 12, height: 12}
                    text: ""
                }
            }

            subtitle := Label {
                width: Fill
                height: Fit
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10.5 }
                    color: (COLOR_TEXT)
                }
                text: ""
            }

            footer := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{x: 1.0, y: 0.5}

                timestamp := Label {
                    width: Fit
                    height: Fit
                    draw_text +: {
                        text_style: REGULAR_TEXT { font_size: 8.8 }
                        color: #9A9A9A
                    }
                    text: ""
                }
            }
        }

        keyboard := View {
            width: Fit
            height: Fit
            flow: Down
            spacing: 8

            first_row := View {
                width: Fit
                height: Fit
                flow: Right
                spacing: 8

                create_button := RobrixPositiveIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_CHECKMARK)
                    icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1}}
                    text: ""
                }

                list_button := RobrixNeutralIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_SEARCH)
                    icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                    text: ""
                }
            }

            second_row := View {
                width: Fit
                height: Fit
                flow: Right
                spacing: 8

                delete_button := RobrixNegativeIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                    text: ""
                }

                help_button := RobrixNeutralIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_INFO)
                    icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                    text: ""
                }
            }

            third_row := View {
                width: Fit
                height: Fit
                flow: Right
                spacing: 8

                view_bound_button := RobrixNeutralIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_SEARCH)
                    icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                    text: "View Bound Bots"
                }

                unbind_button := RobrixNeutralIconButton {
                    width: 156
                    height: 46
                    padding: 10
                    draw_icon.svg: (ICON_CLOSE)
                    icon_walk: Walk{width: 14, height: 14, margin: Inset{left: -2, right: -1}}
                    text: ""
                }
            }
        }
    }

    mod.widgets.Timeline = View {
        width: Fill,
        height: Fill,
        align: Align{x: 0.5, y: 0.0} // center horizontally, align to top vertically
        flow: Overlay,

        list := PortalList {
            height: Fill,
            width: Fill
            flow: Down

            auto_tail: true, // set to `true` to lock the view to the last item.
            max_pull_down: 0.0, // set to `0.0` to disable the pulldown bounce animation.

            // Below, we must place all of the possible templates (views) that can be used in the portal list.
            Message := mod.widgets.Message {}
            CondensedMessage := mod.widgets.CondensedMessage {}
            ImageMessage := mod.widgets.ImageMessage {}
            CondensedImageMessage := mod.widgets.CondensedImageMessage {}
            SmallStateEvent := mod.widgets.SmallStateEvent {}
            Empty := mod.widgets.Empty {}
            DateDivider := mod.widgets.DateDivider {}
            ReadMarker := mod.widgets.ReadMarker {}
            AppServicePanel := mod.widgets.AppServicePanel {}
        }

        // A jump to bottom button (with an unread message badge) that is shown
        // when the timeline is not at the bottom.
        jump_to_bottom_button := JumpToBottomButton { }
    }

    mod.widgets.TranslationLangPopupButton = RobrixIconButton {
        width: Fill
        height: 36
        spacing: 0
        margin: 0
        padding: Inset{left: 12, right: 12, top: 8, bottom: 8}
        icon_walk: Walk{width: 0, height: 0}
        draw_text +: {
            color: (COLOR_TEXT)
            color_hover: (COLOR_TEXT)
            color_down: (COLOR_TEXT)
            text_style: MESSAGE_TEXT_STYLE { font_size: 10.5 }
        }
        draw_bg +: {
            color: #0000
            color_hover: #xF0F4FA
            color_down: #xE8EEF8
            border_size: 0.0
            border_radius: 0.0
        }
    }

    mod.widgets.RoomScreen = #(RoomScreen::register_widget(vm)) {
        width: Fill, height: Fill,
        cursor: MouseCursor.Default,
        flow: Down,
        spacing: 0.0

        room_screen_wrapper := SolidView {
            width: Fill, height: Fill,
            flow: Overlay,

            show_bg: true
            draw_bg.color: (COLOR_PRIMARY_DARKER)

            restore_status_view := RestoreStatusView {}

            // Widgets within this view will get shifted upwards when the on-screen keyboard is shown.
            keyboard_view := KeyboardView {
                width: Fill, height: Fill,
                flow: Down,

                // First, display the timeline of all messages/events.
                timeline := mod.widgets.Timeline {
                    // margin: Inset{bottom: 10}
                }

                // Below that, display a typing notice when other users in the room are typing.
                typing_notice := TypingNotice { }

                room_input_bar := RoomInputBar {
                    // margin: Inset{top: 20}
                }
            }

            translation_lang_modal := Modal {
                align: Align{x: 0, y: 0}
                bg_view.draw_bg.color: #00000000
                content +: {
                    width: Fill
                    height: Fill
                    flow: Overlay
                    align: Align{x: 0, y: 0}

                    translation_lang_popup := RoundedView {
                        width: 220
                        height: Fit
                        margin: Inset{left: 0, top: 0}
                        padding: Inset{top: 4, bottom: 4}
                        show_bg: true
                        new_batch: true
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                            border_radius: 6.0
                            border_size: 1.0
                            border_color: #ddd
                            shadow_color: #0003
                            shadow_radius: 8.0
                            shadow_offset: vec2(0.0, 2.0)
                        }

                        translation_lang_scroll := ScrollYView {
                            width: Fill
                            height: 288
                            flow: Down
                            spacing: 0

                            lang_en := mod.widgets.TranslationLangPopupButton { text: "en  English" }
                            lang_zh := mod.widgets.TranslationLangPopupButton { text: "zh  简体中文" }
                            lang_zh_tw := mod.widgets.TranslationLangPopupButton { text: "zh-TW  繁體中文" }
                            lang_ja := mod.widgets.TranslationLangPopupButton { text: "ja  日本語" }
                            lang_ko := mod.widgets.TranslationLangPopupButton { text: "ko  한국어" }
                            lang_es := mod.widgets.TranslationLangPopupButton { text: "es  Español" }
                            lang_fr := mod.widgets.TranslationLangPopupButton { text: "fr  Français" }
                            lang_de := mod.widgets.TranslationLangPopupButton { text: "de  Deutsch" }
                            lang_ru := mod.widgets.TranslationLangPopupButton { text: "ru  Русский" }
                            lang_pt := mod.widgets.TranslationLangPopupButton { text: "pt  Português" }
                            lang_ar := mod.widgets.TranslationLangPopupButton { text: "ar  العربية" }
                            lang_vi := mod.widgets.TranslationLangPopupButton { text: "vi  Tiếng Việt" }
                            lang_th := mod.widgets.TranslationLangPopupButton { text: "th  ไทย" }
                            lang_id := mod.widgets.TranslationLangPopupButton { text: "id  Bahasa Indonesia" }
                            lang_ms := mod.widgets.TranslationLangPopupButton { text: "ms  Bahasa Melayu" }
                            lang_tr := mod.widgets.TranslationLangPopupButton { text: "tr  Türkçe" }
                            lang_hi := mod.widgets.TranslationLangPopupButton { text: "hi  हिन्दी" }
                        }
                    }
                }
            }

            // Note: here, we're within a View that has an Overlay flow,
            // so the order that we define the below views determines which one is on top.

            // The top space should be displayed as an overlay at the top of the timeline.
            top_space := mod.widgets.TopSpace { }

            threads_sliding_pane := mod.widgets.ThreadsSlidingPane { }
            room_info_sliding_pane := mod.widgets.RoomInfoSlidingPane { }

            // The user profile sliding pane should be displayed on top of other "static" subviews
            // (on top of all other views that are always visible).
            user_profile_sliding_pane := mod.widgets.UserProfileSlidingPane { }

            // The loading pane appears while the user is waiting for something in the room screen
            // to finish loading, e.g., when loading an older replied-to message.
            loading_pane := LoadingPane { }

            create_bot_modal := Modal {
                content +: {
                    create_bot_modal_inner := mod.widgets.CreateBotModal {}
                }
            }

            delete_bot_modal := Modal {
                content +: {
                    delete_bot_modal_inner := mod.widgets.DeleteBotModal {}
                }
            }

            report_room_modal := Modal {
                content +: {
                    report_room_modal_inner := mod.widgets.ReportRoomModal {}
                }
            }

            leave_room_confirm_modal := Modal {
                content +: {
                    leave_room_confirm_modal_inner := mod.widgets.NegativeConfirmationModal {}
                }
            }


            /*
             * TODO: add the action bar back in as a series of floating buttons.
             *
            message_action_bar_popup := PopupNotification {
                align: Align{x: 0.0, y: 0.0}
                content: {
                    height: Fit,
                    width: Fit,
                    show_bg: false,
                    align: Align{
                        x: 0.5,
                        y: 0.5
                    }

                    message_action_bar := MessageActionBar {}
                }
            }
            */
        }
    }
}

#[derive(Clone, Default, Debug)]
pub enum ThreadsPaneAction {
    OpenThread(OwnedEventId),
    LoadMoreRequested,
    #[default]
    None,
}

impl ActionDefaultRef for ThreadsPaneAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: ThreadsPaneAction = ThreadsPaneAction::None;
        &DEFAULT
    }
}

#[derive(Clone, Default, Debug)]
pub enum RoomInfoPaneAction {
    InviteUser,
    ShowPeoplePage,
    OpenPeopleProfile(OwnedUserId),
    ReportRoom,
    LeaveRoom,
    #[default]
    None,
}

impl ActionDefaultRef for RoomInfoPaneAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: RoomInfoPaneAction = RoomInfoPaneAction::None;
        &DEFAULT
    }
}

#[derive(Clone, Debug)]
struct ThreadsPaneEntryInfo {
    thread_root_event_id: OwnedEventId,
    title: String,
    subtitle: String,
    time: String,
    preview: String,
}

#[derive(Clone, Debug)]
struct ThreadsPaneInfo {
    room_name: String,
    entries: Vec<ThreadsPaneEntryInfo>,
    status_text: String,
    show_entries: bool,
    loading_text: String,
    show_loading: bool,
}

#[derive(Clone, Debug)]
struct RoomInfoPaneInfo {
    room_name: String,
    room_id: String,
    topic: String,
    visibility: String,
    encryption: String,
    room_avatar_uri: Option<OwnedMxcUri>,
    room_avatar_fallback_text: String,
    people_entries: Vec<RoomInfoPeopleEntryInfo>,
    people_count_text: String,
    show_people_loading: bool,
}

#[derive(Clone, Debug)]
struct RoomInfoPeopleEntryInfo {
    user_id: OwnedUserId,
    display_name: String,
    level: String,
    is_bot: bool,
    avatar_uri: Option<OwnedMxcUri>,
    avatar_fallback_text: String,
}

#[derive(Default)]
struct ThreadsPaneState {
    room_id: Option<OwnedRoomId>,
    entries: Vec<FetchedRoomThread>,
    prev_batch_token: Option<String>,
    is_loading: bool,
    initialized: bool,
    status_text: String,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ThreadsPaneEntry {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[rust] thread_root_event_id: Option<OwnedEventId>,
}

impl Widget for ThreadsPaneEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(thread_root_event_id) = self.thread_root_event_id.clone() else { return };
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                cx.widget_action(
                    self.widget_uid(),
                    ThreadsPaneAction::OpenThread(thread_root_event_id),
                );
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ThreadsPaneEntry {
    fn set_entry(&mut self, cx: &mut Cx, entry: &ThreadsPaneEntryInfo) {
        self.thread_root_event_id = Some(entry.thread_root_event_id.clone());
        self.label(cx, ids!(title)).set_text(cx, &entry.title);
        self.label(cx, ids!(time)).set_text(cx, &entry.time);
        self.label(cx, ids!(subtitle)).set_text(cx, &entry.subtitle);
        self.label(cx, ids!(preview)).set_text(cx, &entry.preview);
    }
}

impl ThreadsPaneEntryRef {
    fn set_entry(&self, cx: &mut Cx, entry: &ThreadsPaneEntryInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_entry(cx, entry);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomInfoPeopleEntry {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[rust] user_id: Option<OwnedUserId>,
}

impl Widget for RoomInfoPeopleEntry {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(user_id) = self.user_id.clone() else { return };
        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                cx.widget_action(
                    self.widget_uid(),
                    RoomInfoPaneAction::OpenPeopleProfile(user_id),
                );
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomInfoPeopleEntry {
    fn set_entry(&mut self, cx: &mut Cx, entry: &RoomInfoPeopleEntryInfo) {
        self.user_id = Some(entry.user_id.clone());
        let display_name = if entry.is_bot {
            format!("{} [bot]", entry.display_name)
        } else {
            entry.display_name.clone()
        };
        self.label(cx, ids!(display_name)).set_text(cx, &display_name);
        self.label(cx, ids!(level)).set_text(cx, &entry.level);
        self.label(cx, ids!(level)).set_visible(cx, !entry.level.is_empty());

        let avatar = self.avatar(cx, ids!(avatar));
        if let Some(uri) = entry.avatar_uri.as_ref()
            && let avatar_cache::AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri)
        {
            let res = avatar.show_image(
                cx,
                None,
                |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, &image_data),
            );
            if res.is_err() {
                avatar.show_text(cx, None, None, &entry.avatar_fallback_text);
            }
        } else {
            avatar.show_text(cx, None, None, &entry.avatar_fallback_text);
        }
    }
}

impl RoomInfoPeopleEntryRef {
    fn set_entry(&self, cx: &mut Cx, entry: &RoomInfoPeopleEntryInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_entry(cx, entry);
    }
}

#[derive(Script, ScriptHook, Widget, Animator)]
pub struct ThreadsSlidingPane {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[live] slide: f32,

    #[rust] info: Option<ThreadsPaneInfo>,
    #[rust] is_animating_out: bool,
}

impl Widget for ThreadsSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            self.redraw(cx);
        }

        if self.is_animating_out && !self.animator.is_track_animating(id!(panel)) {
            self.visible = false;
            self.is_animating_out = false;
            cx.revert_key_focus();
            self.view(cx, ids!(bg_view)).set_visible(cx, false);
            self.redraw(cx);
            return;
        }

        let area = self.view.area();
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(close_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    fue.mouse_button().is_some_and(|b| b.is_back())
                    || !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if close_pane {
            self.hide(cx);
        }

        if let Event::Actions(actions) = event {
            let threads_list = self.portal_list(cx, ids!(threads_list));
            if threads_list.scrolled(actions)
                && threads_list.first_id() == 0
                && threads_list.scroll_position() >= -0.5
            {
                cx.widget_action(
                    self.widget_uid(),
                    ThreadsPaneAction::LoadMoreRequested,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(info) = self.info.as_ref() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };

        let container_width = self.view.area().rect(cx).size.x as f32;
        let panel_width = if container_width > 1.0 && container_width < ROOM_INFO_PANE_MOBILE_BREAKPOINT {
            container_width
        } else {
            ROOM_INFO_PANE_DESKTOP_WIDTH
        };
        let right_margin = -(self.slide * panel_width);
        let mut main_content = self.view(cx, ids!(main_content));
        script_apply_eval!(cx, main_content, {
            width: #(panel_width)
            margin.right: #(right_margin)
        });
        let bg_alpha = (1.0 - self.slide) * 0.733;
        let bg_color = vec4(0.0, 0.0, 0.0, bg_alpha);
        let mut bg_view = self.view(cx, ids!(bg_view));
        script_apply_eval!(cx, bg_view, {
            draw_bg +: { color: #(bg_color) }
        });

        self.label(cx, ids!(room_name)).set_text(cx, &info.room_name);
        self.label(cx, ids!(loading_label)).set_text(cx, &info.loading_text);
        self.view(cx, ids!(loading_indicator)).set_visible(cx, info.show_loading);
        self.label(cx, ids!(empty_state)).set_text(cx, &info.status_text);
        self.view(cx, ids!(empty_state)).set_visible(cx, !info.show_entries && !info.show_loading);
        self.view(cx, ids!(threads_list)).set_visible(cx, info.show_entries);

        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            list.set_item_range(cx, 0, info.entries.len());
            while let Some(item_id) = list.next_visible_item(cx) {
                let Some(entry) = info.entries.get(item_id) else { continue };
                let item = list.item(cx, item_id, id!(ThreadEntry));
                item.as_threads_pane_entry().set_entry(cx, entry);
                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}

impl ThreadsSlidingPane {
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    fn set_info(&mut self, _cx: &mut Cx, info: ThreadsPaneInfo) {
        self.info = Some(info);
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.is_animating_out = false;
        cx.set_key_focus(self.view.area());
        self.animator_play(cx, ids!(panel.show));
        self.view(cx, ids!(bg_view)).set_visible(cx, true);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        self.redraw(cx);
    }

    pub fn hide(&mut self, cx: &mut Cx) {
        if !self.visible {
            return;
        }
        self.is_animating_out = true;
        self.animator_play(cx, ids!(panel.hide));
        self.redraw(cx);
    }
}

impl ThreadsSlidingPaneRef {
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    fn set_info(&self, cx: &mut Cx, info: ThreadsPaneInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_info(cx, info);
    }

    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }

    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }
}

#[derive(Script, ScriptHook, Widget, Animator)]
pub struct RoomInfoSlidingPane {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[live] slide: f32,

    #[rust] info: Option<RoomInfoPaneInfo>,
    #[rust] is_animating_out: bool,
    #[rust] show_people_page: bool,
    #[rust] topic_expanded: bool,
    #[rust] people_display_count: usize,
}

impl Widget for RoomInfoSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            self.redraw(cx);
        }

        if self.is_animating_out && !self.animator.is_track_animating(id!(panel)) {
            self.visible = false;
            self.is_animating_out = false;
            cx.revert_key_focus();
            self.view(cx, ids!(bg_view)).set_visible(cx, false);
            self.redraw(cx);
            return;
        }

        let area = self.view.area();
        let close_pane = if is_invite_modal_open() || is_room_info_action_modal_open() {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(close_button)).clicked(actions)
            )
        } else {
            matches!(
                event,
                Event::Actions(actions) if self.button(cx, ids!(close_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    fue.mouse_button().is_some_and(|b| b.is_back())
                    || !self.view(cx, ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if close_pane {
            self.hide(cx);
        }

        if let Event::Actions(actions) = event {
            if self.button(cx, ids!(header.back_button)).clicked(actions) {
                self.show_people_page = false;
                self.redraw(cx);
            }
            if self.button(cx, ids!(content_scroll.info_view.topic_card.topic_toggle_button)).clicked(actions) {
                self.topic_expanded = !self.topic_expanded;
                self.redraw(cx);
            }
            if self.button(cx, ids!(content_scroll.info_view.summary_card.room_meta.room_id_row.copy_room_id_button)).clicked(actions)
                && let Some(info) = self.info.as_ref()
            {
                cx.copy_to_clipboard(&info.room_id);
                enqueue_popup_notification(
                    "Room ID copied.",
                    PopupKind::Success,
                    Some(2.0),
                );
            }
            if self.button(cx, ids!(content_scroll.info_view.actions_row.invite_button)).clicked(actions) {
                cx.widget_action(
                    self.widget_uid(),
                    RoomInfoPaneAction::InviteUser,
                );
            }
            if self.button(cx, ids!(content_scroll.info_view.actions_row.people_button)).clicked(actions) {
                self.show_people_page = true;
                self.people_display_count = self.info.as_ref()
                    .map(|info| info.people_entries.len().min(40))
                    .unwrap_or(0);
                cx.widget_action(
                    self.widget_uid(),
                    RoomInfoPaneAction::ShowPeoplePage,
                );
                self.redraw(cx);
            }
            if self.button(cx, ids!(content_scroll.info_view.actions_row.report_room_button)).clicked(actions) {
                cx.widget_action(
                    self.widget_uid(),
                    RoomInfoPaneAction::ReportRoom,
                );
            }
            if self.button(cx, ids!(content_scroll.info_view.actions_row.leave_room_button)).clicked(actions) {
                cx.widget_action(
                    self.widget_uid(),
                    RoomInfoPaneAction::LeaveRoom,
                );
            }

            if self.show_people_page
                && let Some(info) = self.info.as_ref()
                && self.people_display_count < info.people_entries.len()
            {
                let people_list = self.portal_list(cx, ids!(people_view.people_list));
                if people_list.scrolled(actions) {
                    let threshold = self.people_display_count.saturating_sub(5);
                    if people_list.first_id() + people_list.visible_items() >= threshold {
                        self.people_display_count = (self.people_display_count + 40).min(info.people_entries.len());
                        self.redraw(cx);
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(info) = self.info.as_ref() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };

        let panel_width = 320.0;
        let right_margin = -(self.slide * panel_width);
        let mut main_content = self.view(cx, ids!(main_content));
        script_apply_eval!(cx, main_content, {
            margin.right: #(right_margin)
        });
        let bg_alpha = (1.0 - self.slide) * 0.733;
        let bg_color = vec4(0.0, 0.0, 0.0, bg_alpha);
        let mut bg_view = self.view(cx, ids!(bg_view));
        script_apply_eval!(cx, bg_view, {
            draw_bg +: { color: #(bg_color) }
        });

        self.button(cx, ids!(header.back_button)).set_visible(cx, self.show_people_page);
        self.label(cx, ids!(header.title)).set_text(cx, if self.show_people_page { "People" } else { "Info" });
        self.view(cx, ids!(content_scroll)).set_visible(cx, !self.show_people_page);
        self.view(cx, ids!(content_scroll.info_view)).set_visible(cx, !self.show_people_page);
        self.view(cx, ids!(people_view)).set_visible(cx, self.show_people_page);

        self.label(cx, ids!(content_scroll.info_view.summary_card.room_meta.room_name_value)).set_text(cx, &info.room_name);
        self.label(cx, ids!(content_scroll.info_view.summary_card.room_meta.room_id_row.room_id_value)).set_text(cx, &info.room_id);
        self.label(cx, ids!(content_scroll.info_view.facts_card.visibility_row.visibility_value)).set_text(cx, &info.visibility);
        self.label(cx, ids!(content_scroll.info_view.facts_card.encryption_row.encryption_value)).set_text(cx, &info.encryption);

        let topic_chars_len = info.topic.chars().count();
        let topic_has_more = topic_chars_len > TOPIC_PREVIEW_CHARS;
        let topic_display_text = if topic_has_more && !self.topic_expanded {
            let mut preview: String = info.topic.chars().take(TOPIC_PREVIEW_CHARS).collect();
            preview.push_str("...");
            preview
        } else {
            info.topic.clone()
        };
        self.label(cx, ids!(content_scroll.info_view.topic_card.topic_value)).set_text(cx, &topic_display_text);
        self.button(cx, ids!(content_scroll.info_view.topic_card.topic_toggle_button)).set_visible(cx, topic_has_more);
        self.button(cx, ids!(content_scroll.info_view.topic_card.topic_toggle_button)).set_text(
            cx,
            if self.topic_expanded { "Collapse" } else { "Expand" },
        );

        let room_avatar = self.avatar(cx, ids!(content_scroll.info_view.summary_card.room_avatar));
        if let Some(uri) = info.room_avatar_uri.as_ref()
            && let avatar_cache::AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri)
        {
            let res = room_avatar.show_image(
                cx,
                None,
                |cx, img_ref| utils::load_png_or_jpg(&img_ref, cx, &image_data),
            );
            if res.is_err() {
                room_avatar.show_text(cx, None, None, &info.room_avatar_fallback_text);
            }
        } else {
            room_avatar.show_text(cx, None, None, &info.room_avatar_fallback_text);
        }

        if self.show_people_page && self.people_display_count == 0 {
            self.people_display_count = info.people_entries.len().min(40);
        }
        let visible_people_count = self.people_display_count.min(info.people_entries.len());
        self.label(cx, ids!(people_view.member_count)).set_text(cx, &info.people_count_text);
        self.view(cx, ids!(people_view.loading_label)).set_visible(cx, info.show_people_loading);
        self.view(cx, ids!(people_view.empty_label)).set_visible(cx, !info.show_people_loading && info.people_entries.is_empty());
        self.view(cx, ids!(people_view.people_list)).set_visible(cx, visible_people_count > 0);

        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list_ref = widget.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            list.set_item_range(cx, 0, visible_people_count);
            while let Some(item_id) = list.next_visible_item(cx) {
                let Some(entry) = info.people_entries.get(item_id) else { continue };
                let item = list.item(cx, item_id, id!(PersonEntry));
                item.as_room_info_people_entry().set_entry(cx, entry);
                item.draw_all(cx, &mut Scope::empty());
            }
        }
        DrawStep::done()
    }
}

impl RoomInfoSlidingPane {
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    fn set_info(&mut self, cx: &mut Cx, info: RoomInfoPaneInfo) {
        self.info = Some(info);
        if self.show_people_page {
            if let Some(info) = self.info.as_ref() {
                self.people_display_count = self.people_display_count
                    .max(40.min(info.people_entries.len()))
                    .min(info.people_entries.len());
            }
        }
        self.redraw(cx);
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.is_animating_out = false;
        self.show_people_page = false;
        self.topic_expanded = false;
        self.people_display_count = 0;
        cx.set_key_focus(self.view.area());
        self.animator_play(cx, ids!(panel.show));
        self.view(cx, ids!(bg_view)).set_visible(cx, true);
        self.view.button(cx, ids!(close_button)).reset_hover(cx);
        self.redraw(cx);
    }

    pub fn hide(&mut self, cx: &mut Cx) {
        if !self.visible {
            return;
        }
        self.is_animating_out = true;
        self.animator_play(cx, ids!(panel.hide));
        self.redraw(cx);
    }
}

impl RoomInfoSlidingPaneRef {
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    fn set_info(&self, cx: &mut Cx, info: RoomInfoPaneInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_info(cx, info);
    }

    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }

    pub fn hide(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.hide(cx);
    }
}

#[derive(Clone, Debug)]
pub enum ReportRoomModalAction {
    Close,
    Submit(String),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ReportRoomModal {
    #[deref]
    view: View,
    #[rust]
    is_showing_error: bool,
}

impl Widget for ReportRoomModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for ReportRoomModal {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let cancel_button = self.view.button(cx, ids!(buttons.cancel_button));
        let report_button = self.view.button(cx, ids!(buttons.report_button));
        let reason_input = self.view.text_input(cx, ids!(reason_input));
        let mut status_label = self.view.label(cx, ids!(status_label));

        if cancel_button.clicked(actions)
            || actions
                .iter()
                .any(|a| matches!(a.downcast_ref(), Some(ModalAction::Dismissed)))
        {
            cx.action(ReportRoomModalAction::Close);
            return;
        }

        if self.is_showing_error && reason_input.changed(actions).is_some() {
            self.is_showing_error = false;
            status_label.set_text(cx, "");
            self.view.redraw(cx);
        }

        if report_button.clicked(actions) || reason_input.returned(actions).is_some() {
            let reason = reason_input.text().trim().to_string();
            if reason.is_empty() {
                self.is_showing_error = true;
                script_apply_eval!(cx, status_label, {
                    text: "Please enter a reason before reporting."
                    draw_text +: {
                        color: mod.widgets.COLOR_FG_DANGER_RED
                    }
                });
                self.view.redraw(cx);
                return;
            }
            cx.action(ReportRoomModalAction::Submit(reason));
        }
    }
}

impl ReportRoomModal {
    pub fn show(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        self.is_showing_error = false;
        self.view
            .label(cx, ids!(title))
            .set_text(cx, "Report Room");
        self.view.label(cx, ids!(body)).set_text(
            cx,
            &format!(
                "Report {} to your homeserver administrators. Please provide a reason.",
                room_name_id
            ),
        );
        self.view
            .text_input(cx, ids!(reason_input))
            .set_text(cx, "");
        self.view.label(cx, ids!(status_label)).set_text(cx, "");
        self.view
            .button(cx, ids!(buttons.report_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .set_enabled(cx, true);
        self.view
            .button(cx, ids!(buttons.report_button))
            .reset_hover(cx);
        self.view
            .button(cx, ids!(buttons.cancel_button))
            .reset_hover(cx);
        self.view.redraw(cx);
    }
}

impl ReportRoomModalRef {
    pub fn show(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.show(cx, room_name_id);
    }
}

/// The main widget that displays a single Matrix room.
#[derive(Script, Widget)]
pub struct RoomScreen {
    #[deref] view: View,

    /// The name and ID of the currently-shown room, if any.
    #[rust] room_name_id: Option<RoomNameId>,
    /// The avatar URL of the currently-shown room, if any.
    #[rust] room_avatar_url: Option<OwnedMxcUri>,
    /// The timeline currently displayed by this RoomScreen, if any.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// The persistent UI-relevant states for the room that this widget is currently displaying.
    #[rust] tl_state: Option<TimelineUiState>,
    /// The set of pinned events in this room.
    #[rust] pinned_events: Vec<OwnedEventId>,
    /// Whether this room has been successfully loaded (received from the homeserver).
    #[rust] is_loaded: bool,
    /// Whether or not all rooms have been loaded (received from the homeserver).
    #[rust] all_rooms_loaded: bool,
    /// NextFrame subscription for driving streaming typewriter animation.
    #[rust]
    streaming_next_frame: NextFrame,
    /// Timeout used to evict stalled streaming states without per-frame polling.
    #[rust]
    streaming_timeout_timer: Timer,
    /// Whether the in-room app service quick actions card is currently visible.
    #[rust] show_app_service_actions: bool,
    #[rust] threads_pane_state: ThreadsPaneState,
    #[rust] app_language: AppLanguage,
    #[rust] app_language_initialized: bool,
    #[rust] pending_invited_users: HashSet<OwnedUserId>,
    #[rust] octos_action_button_contexts: HashMap<WidgetUid, OctosActionButtonContext>,
    #[rust] disabled_octos_action_source_event_ids: HashSet<OwnedEventId>,
    #[rust] selected_octos_action_by_source_event_id: HashMap<OwnedEventId, SelectedOctosActionState>,
}

impl Drop for RoomScreen {
    fn drop(&mut self) {
        // This ensures that the `TimelineUiState` instance owned by this room is *always* returned
        // back to to `TIMELINE_STATES`, which ensures that its UI state(s) are not lost
        // and that other RoomScreen instances can show this room in the future.
        // RoomScreen will be dropped whenever its widget instance is destroyed, e.g.,
        // when a Tab is closed or the app is resized to a different AdaptiveView layout.
        self.hide_timeline();
    }
}

impl ScriptHook for RoomScreen {
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            if let Some(tl_state) = &mut self.tl_state.as_mut() {
                // Clear the timeline's drawn items caches and redraw it.
                tl_state.content_drawn_since_last_update.clear();
                tl_state.profile_drawn_since_last_update.clear();
                self.view.redraw(cx);
            }
        });
    }
}

impl Widget for RoomScreen {
    // Handle events and actions for the RoomScreen widget and its inner Timeline view.
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        let room_screen_widget_uid = self.widget_uid();
        let portal_list = self.portal_list(cx, ids!(timeline.list));
        let user_profile_sliding_pane = self.user_profile_sliding_pane(cx, ids!(user_profile_sliding_pane));
        let threads_sliding_pane = self.threads_sliding_pane(cx, ids!(threads_sliding_pane));
        let threads_sliding_pane_widget_uid = threads_sliding_pane.widget_uid();
        let room_info_sliding_pane = self.room_info_sliding_pane(cx, ids!(room_info_sliding_pane));
        let room_info_sliding_pane_widget_uid = room_info_sliding_pane.widget_uid();
        let loading_pane = self.loading_pane(cx, ids!(loading_pane));
        set_room_info_action_modal_open(
            self.view.modal(cx, ids!(report_room_modal)).is_open()
                || self.view.modal(cx, ids!(leave_room_confirm_modal)).is_open()
        );

        // Streaming animation frame handler
        if let Some(_ne) = self.streaming_next_frame.is_event(event) {
            #[cfg(debug_assertions)]
            #[allow(unused_variables)]
            let frame_start = std::time::Instant::now();

            if let Some(tl) = self.tl_state.as_mut() {
                let mut needs_another_frame = false;
                let mut completed_ids = Vec::new();
                let mut redraw_candidate_indices = Vec::new();

                for (event_id, state) in tl.streaming_messages.iter_mut() {
                    if state.needs_frame() {
                        if state.tick() {
                            // Invalidate draw cache so item gets re-populated
                            if let Some(idx) = state.timeline_index {
                                tl.content_drawn_since_last_update.remove(idx..idx + 1);
                            }
                            redraw_candidate_indices.push(state.timeline_index);
                        }
                        needs_another_frame |= state.needs_frame();
                    }

                    if state.is_complete() || state.is_timed_out() {
                        completed_ids.push(event_id.clone());
                        redraw_candidate_indices.push(state.timeline_index);
                    }
                }

                for id in &completed_ids {
                    tl.streaming_messages.remove(id);
                }

                // Safety cap: max 50 streaming entries
                while tl.streaming_messages.len() > 50 {
                    if let Some((oldest_id, oldest_idx)) = tl.streaming_messages.iter()
                        .min_by_key(|(_, s)| s.animation_start_time)
                        .map(|(id, state)| (id.clone(), state.timeline_index))
                    {
                        tl.streaming_messages.remove(&oldest_id);
                        redraw_candidate_indices.push(oldest_idx);
                    }
                }

                if needs_another_frame {
                    self.streaming_next_frame = cx.new_next_frame();
                }

                if any_timeline_indices_visible(
                    redraw_candidate_indices.iter().copied(),
                    |idx| portal_list.get_item(idx).is_some(),
                ) {
                    self.redraw_timeline_list(cx);
                }
            }

            #[cfg(debug_assertions)]
            {
                if let Some(tl) = self.tl_state.as_ref() {
                    let elapsed = frame_start.elapsed();
                    if elapsed.as_millis() > 2 {
                        log!("Streaming animation frame took {}ms ({} active streams)",
                            elapsed.as_millis(), tl.streaming_messages.len());
                    }
                }
            }

            self.schedule_stream_timeout(cx);
        }

        if self.streaming_timeout_timer.is_event(event).is_some() {
            if let Some(tl) = self.tl_state.as_mut() {
                let timed_out_entries: Vec<(OwnedEventId, Option<usize>)> = tl
                    .streaming_messages
                    .iter()
                    .filter_map(|(event_id, state)| {
                        if state.is_timed_out() || state.is_complete() {
                            Some((event_id.clone(), state.timeline_index))
                        } else {
                            None
                        }
                    })
                    .collect();

                for (event_id, _) in &timed_out_entries {
                    tl.streaming_messages.remove(event_id);
                }

                if any_timeline_indices_visible(
                    timed_out_entries.iter().map(|(_, idx)| *idx),
                    |idx| portal_list.get_item(idx).is_some(),
                ) {
                    self.redraw_timeline_list(cx);
                }
            }

            self.schedule_stream_timeout(cx);
        }

        // Handle actions here before processing timeline updates.
        // Normally (in most other widgets), the order of event handling doesn't matter much.
        // However, since actions may refer to a specific timeline item's index,
        // we want to handle those before processing any updates that might change
        // the set of timeline indices (which would invalidate the index values in any actions).
        if let Event::Actions(actions) = event {
            for (index, wr) in portal_list.items_with_actions(actions) {
                // Handle a hover-in action on the reaction list: show a reaction summary.
                let reaction_list = wr.reaction_list(cx, ids!(reaction_list));
                if let RoomScreenTooltipActions::HoverInReactionButton {
                    widget_rect,
                    reaction_data,
                } = reaction_list.hovered_in(actions) {
                    let Some(_tl_state) = self.tl_state.as_ref() else { continue };
                    let tooltip_text_arr: Vec<String> = reaction_data.reaction_senders
                        .iter()
                        .map(|(sender, _react_info)| {
                            user_profile_cache::get_user_display_name_for_room(
                                cx,
                                sender.clone(),
                                Some(&reaction_data.room_id),
                                true,
                            )
                            .into_option()
                            .unwrap_or_else(|| sender.to_string())
                        })
                        .collect();

                    let mut tooltip_text = utils::human_readable_list(&tooltip_text_arr, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT);
                    tooltip_text.push_str(&tr_fmt(self.app_language, "room_screen.tooltip.reacted_with_suffix", &[
                        ("reaction", reaction_data.reaction.as_str()),
                    ]));
                    cx.widget_action(
                        room_screen_widget_uid, 
                        TooltipAction::HoverIn {
                            text: tooltip_text,
                            widget_rect,
                            options: CalloutTooltipOptions {
                                position: TooltipPosition::Bottom,
                                ..Default::default()
                            },
                        },
                    );
                }

                // Handle a hover-out action on the reaction list or avatar row.
                let avatar_row_ref = wr.avatar_row(cx, ids!(avatar_row));
                if reaction_list.hovered_out(actions)
                    || avatar_row_ref.hover_out(actions)
                {
                    cx.widget_action(
                        room_screen_widget_uid, 
                        TooltipAction::HoverOut,
                    );
                }

                // Handle a hover-in action on the avatar row: show a read receipts summary.
                if let RoomScreenTooltipActions::HoverInReadReceipt {
                    widget_rect,
                    read_receipts
                } = avatar_row_ref.hover_in(actions) {
                    let Some(room_id) = self.room_id() else { return; };
                    let tooltip_text= room_read_receipt::populate_tooltip(cx, read_receipts, room_id);
                    cx.widget_action(
                        room_screen_widget_uid, 
                        TooltipAction::HoverIn {
                            text: tooltip_text,
                            widget_rect,
                            options: CalloutTooltipOptions {
                                position: TooltipPosition::Left,
                                ..Default::default()
                            },
                        },
                    );
                }

                // Handle an image within the message being clicked.
                let content_message = wr.text_or_image(cx, ids!(content.message));
                if let TextOrImageAction::Clicked(mxc_uri) = actions.find_widget_action(content_message.widget_uid()).cast() {
                    let texture = content_message.get_texture(cx);
                    self.handle_image_click(
                        cx,
                        mxc_uri,
                        texture,
                        index,
                    );
                    continue;
                }

                // Handle the invite_user_button (in a SmallStateEvent) being clicked.
                if wr.button(cx, ids!(invite_user_button)).clicked(actions) {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    if let Some(event_tl_item) = tl.items.get(index).and_then(|item| item.as_event()) {
                        let user_id = event_tl_item.sender().to_owned();
                        let username = if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
                            profile.display_name.as_deref().unwrap_or(user_id.as_str())
                        } else {
                            user_id.as_str()
                        };
                        let room_id = tl.kind.room_id().clone();
                        let app_language = self.app_language;
                        let content = ConfirmationModalContent {
                            title_text: tr_key(app_language, "room_screen.modal.invite.title").into(),
                            body_text: tr_fmt(app_language, "room_screen.modal.invite.body", &[("username", username)]).into(),
                            accept_button_text: Some(tr_key(app_language, "room_screen.modal.invite.accept").into()),
                            on_accept_clicked: Some(Box::new(move |_cx| {
                                submit_async_request(MatrixRequest::InviteUser { room_id, user_id });
                            })),
                            ..Default::default()
                        };
                        cx.action(InviteAction::ShowInviteConfirmationModal(RefCell::new(Some(content))));
                    }
                }
            }

            self.handle_message_actions(cx, actions, &portal_list, &loading_pane);

            for action in actions {
                if let Some(RoomsListAction::Selected(selected_room)) = action.downcast_ref() {
                    if self.timeline_kind.as_ref() != selected_room.timeline_kind().as_ref() {
                        self.close_report_room_modal(cx);
                        self.close_leave_room_confirm_modal(cx);
                    }
                }
                if let Some(AppStateAction::RoomFocused(selected_room)) = action.downcast_ref() {
                    if self.timeline_kind.as_ref() != selected_room.timeline_kind().as_ref() {
                        self.close_report_room_modal(cx);
                        self.close_leave_room_confirm_modal(cx);
                    }
                }
                if let Some(AppStateAction::FocusNone) = action.downcast_ref() {
                    self.close_report_room_modal(cx);
                    self.close_leave_room_confirm_modal(cx);
                }

                // Handle actions related to restoring the previously-saved state of rooms.
                if let Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, ..}) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_name_id.room_id()) {
                        // `set_displayed_room()` does nothing if the room_name_id is unchanged, so we clear it first.
                        self.room_name_id = None;
                        let thread_root_event_id = self.timeline_kind.as_ref()
                            .and_then(|k| k.thread_root_event_id().cloned());
                        self.set_displayed_room(cx, room_name_id, thread_root_event_id);
                        return;
                    }
                }

                // Handle InviteResultAction to show popup notifications.
                if let Some(InviteResultAction::Sent { room_id, user_id }) = action.downcast_ref() {
                    // Only handle if this is for the current room.
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        self.pending_invited_users.insert(user_id.clone());
                        enqueue_popup_notification(
                            "Invite sent. Waiting for acceptance.",
                            PopupKind::Info,
                            Some(4.0),
                        );
                        if let Some(app_state) = scope.data.get::<AppState>()
                            && app_state.bot_settings.enabled
                        {
                            if let Ok(bot_user_id) = app_state
                                .bot_settings
                                .resolved_bot_user_id_for_room(room_id, current_user_id().as_deref())
                            {
                                if &bot_user_id == user_id
                                    && app_state
                                        .bot_settings
                                        .bound_bot_user_id(room_id.as_ref())
                                        .is_none_or(|existing_bot_user_id| existing_bot_user_id.as_str() != user_id.as_str())
                                {
                                    cx.action(AppStateAction::BotRoomBindingUpdated {
                                        room_id: room_id.clone(),
                                        bound: true,
                                        bot_user_id: Some(user_id.clone()),
                                        warning: None,
                                    });
                                }
                            }
                        }
                    }
                }
                if let Some(InviteResultAction::Failed { room_id, user_id, error }) = action.downcast_ref() {
                    // Only handle if this is for the current room.
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        self.pending_invited_users.remove(user_id);
                        let error_text = error.to_string();
                        enqueue_popup_notification(
                            tr_fmt(self.app_language, "room_screen.popup.invite.failed", &[
                                ("error", error_text.as_str()),
                            ]),
                            PopupKind::Error,
                            None,
                        );
                    }
                }
                if let Some(ReportRoomResultAction::Sent { room_id }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        enqueue_popup_notification(
                            "Room reported successfully.",
                            PopupKind::Success,
                            Some(4.0),
                        );
                    }
                }
                if let Some(ReportRoomResultAction::Failed { room_id, error }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        enqueue_popup_notification(
                            format!("Failed to report room.\n\nError: {error}"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }
                if let Some(ActionResponseResultAction::Failed { room_id, source_event_id, error }) = action.downcast_ref() {
                    if self.room_name_id.as_ref().is_some_and(|rn| rn.room_id() == room_id) {
                        clear_action_buttons_disabled(
                            &mut self.disabled_octos_action_source_event_ids,
                            source_event_id.as_ref(),
                        );
                        clear_selected_octos_action(
                            &mut self.selected_octos_action_by_source_event_id,
                            source_event_id.as_ref(),
                        );
                        self.redraw_timeline_list(cx);
                        enqueue_popup_notification(
                            tr_fmt(
                                self.app_language,
                                "room_screen.popup.action_response.failed",
                                &[("error", error.as_str())],
                            ),
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }

                match action
                    .as_widget_action()
                    .widget_uid_eq(threads_sliding_pane_widget_uid)
                    .cast_ref()
                {
                    ThreadsPaneAction::OpenThread(thread_root_event_id) => {
                        let Some(room_name_id) = self.room_name_id.as_ref().cloned() else { continue };
                        threads_sliding_pane.hide(cx);
                        cx.widget_action(
                            room_screen_widget_uid,
                            RoomsListAction::Selected(SelectedRoom::Thread {
                                room_name_id,
                                thread_root_event_id: thread_root_event_id.clone(),
                            }),
                        );
                    }
                    ThreadsPaneAction::LoadMoreRequested => {
                        self.request_more_threads(cx, true);
                    }
                    ThreadsPaneAction::None => {}
                }

                match action
                    .as_widget_action()
                    .widget_uid_eq(room_info_sliding_pane_widget_uid)
                    .cast_ref()
                {
                    RoomInfoPaneAction::InviteUser => {
                        if let Some(room_name_id) = self.room_name_id.as_ref().cloned() {
                            cx.action(InviteModalAction::Open(room_name_id));
                        }
                    }
                    RoomInfoPaneAction::ShowPeoplePage => {
                        if let Some(tl) = self.tl_state.as_ref()
                            && tl.room_members.is_none()
                        {
                            submit_async_request(MatrixRequest::GetRoomMembers {
                                timeline_kind: tl.kind.clone(),
                                memberships: matrix_sdk::RoomMemberships::JOIN,
                                local_only: false,
                            });
                        }
                    }
                    RoomInfoPaneAction::OpenPeopleProfile(user_id) => {
                        let Some(room_name_id) = self.room_name_id.as_ref().cloned() else { continue };
                        let room_member = self.tl_state.as_ref()
                            .and_then(|tl| tl.room_members.as_ref())
                            .and_then(|members| members.iter().find(|member| member.user_id() == user_id).cloned());
                        let username = room_member.as_ref()
                            .and_then(|member| member.display_name().map(ToOwned::to_owned));
                        let avatar_state = AvatarState::Known(
                            room_member
                                .as_ref()
                                .and_then(|member| member.avatar_url().map(ToOwned::to_owned))
                        );
                        self.show_user_profile(
                            cx,
                            &user_profile_sliding_pane,
                            UserProfilePaneInfo {
                                profile_and_room_id: UserProfileAndRoomId {
                                    user_profile: UserProfile {
                                        user_id: user_id.clone(),
                                        username,
                                        avatar_state,
                                    },
                                    room_id: room_name_id.room_id().clone(),
                                },
                                room_name: room_name_id.to_string(),
                                room_member,
                            },
                        );
                    }
                    RoomInfoPaneAction::ReportRoom => {
                        self.open_report_room_modal(cx);
                    }
                    RoomInfoPaneAction::LeaveRoom => {
                        self.open_leave_room_confirm_modal(cx);
                    }
                    RoomInfoPaneAction::None => {}
                }

                if let Some(RoomThreadsAction::Loaded { room_id, from, threads, prev_batch_token }) = action.downcast_ref() {
                    if self.threads_pane_state.room_id.as_ref().is_some_and(|current| current == room_id) {
                        self.on_threads_loaded(
                            cx,
                            from.as_ref(),
                            threads,
                            prev_batch_token.clone(),
                        );
                    }
                }
                if let Some(RoomThreadsAction::Failed { room_id, from: _, error }) = action.downcast_ref() {
                    if self.threads_pane_state.room_id.as_ref().is_some_and(|current| current == room_id) {
                        self.on_threads_failed(cx, error);
                    }
                }

                // When transitioning from offline to online, clear stale `Requested`/`Failed`
                // entries from per-room caches so they can be re-fetched.
                if let Some(RoomsListHeaderAction::StateUpdate(new_state)) = action.downcast_ref() {
                    if !matches!(new_state, State::Offline) {
                        if let Some(tl) = self.tl_state.as_mut() {
                            tl.media_cache.clear_all_pending_and_failed_requests();
                            tl.link_preview_cache.clear_all_pending_and_failed_requests();
                        }
                    }
                    continue;
                }

                // Handle the highlight animation for a message.
                let Some(tl) = self.tl_state.as_mut() else { continue };
                if let MessageHighlightAnimationState::Pending { item_id } = tl.message_highlight_animation_state {
                    if portal_list.smooth_scroll_reached(actions) {
                        cx.widget_action(
                            room_screen_widget_uid, 
                            MessageAction::HighlightMessage(item_id),
                        );
                        tl.message_highlight_animation_state = MessageHighlightAnimationState::Off;
                        // Adjust the scrolled-to item's position to be slightly beneath the top of the viewport.
                        // portal_list.set_first_id_and_scroll(portal_list.first_id(), 15.0);
                    }
                }
            }

            /*
            // close message action bar if scrolled.
            if portal_list.scrolled(actions) {
                let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                message_action_bar_popup.close(cx);
            }
            */

            // Set visibility of loading message banner based of pagination logic
            self.send_pagination_request_based_on_scroll_pos(cx, actions, &portal_list);
            // Handle sending any read receipts for the current logged-in user.
            self.send_user_read_receipts_based_on_scroll_pos(cx, actions, &portal_list);

            // Handle the jump to bottom button: update its visibility, and handle clicks.
            self.jump_to_bottom_button(cx, ids!(jump_to_bottom_button)).update_from_actions(
                cx,
                &portal_list,
                actions,
            );
        }

        // Currently, a Signal event is only used to tell this widget:
        // 1. to check if the room has been loaded from the homeserver yet, or
        // 2. that its timeline events have been updated in the background.
        if let Event::Signal = event {
            if let (false, Some(room_name_id), true) = (self.is_loaded, self.room_name_id.as_ref(), cx.has_global::<RoomsListRef>()) {
                let rooms_list_ref = cx.get_global::<RoomsListRef>();
                if rooms_list_ref.is_room_loaded(room_name_id.room_id()) {
                    let room_name_clone = room_name_id.clone();
                    let thread_root_event_id = self.timeline_kind.as_ref()
                        .and_then(|k| k.thread_root_event_id().cloned());
                    // This room has been loaded now, so we call `set_displayed_room()`.
                    // We first clear the `room_name_id`, otherwise that function will do nothing.
                    self.room_name_id = None;
                    self.set_displayed_room(cx, &room_name_clone, thread_root_event_id);
                } else {
                    self.all_rooms_loaded = rooms_list_ref.all_rooms_loaded();
                    return;
                }
            }

            // If this RoomScreen is waiting to show a thread timeline (not the main room timeline),
            // then we need to retry showing the timeline now (upon a Signal),
            // because the thread timeline may have been successfully created.
            if self.tl_state.is_none() && self.timeline_kind.is_some() {
                self.show_timeline(cx);
            }

            self.process_timeline_updates(cx, &portal_list, scope.data.get::<AppState>());
            if threads_sliding_pane.is_currently_shown(cx) {
                self.refresh_threads_pane(cx);
            }
            if room_info_sliding_pane.is_currently_shown(cx) {
                self.refresh_room_info_pane(cx);
            }

            // Ideally we would do this elsewhere on the main thread, because it's not room-specific,
            // but it doesn't hurt to do it here.
            // TODO: move this up a layer to something higher in the UI tree,
            //       and wrap it in a `if let Event::Signal` conditional.
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
        }

        // We only forward "interactive hit" events to the inner timeline view
        // if none of the various overlay views are visible.
        // We always forward "non-interactive hit" events to the inner timeline view.
        // We check which overlay views are visible in the order of those views' z-ordering,
        // such that the top-most views get a chance to handle the event first.
        //
        let room_info_action_modal_open =
            self.view.modal(cx, ids!(report_room_modal)).is_open()
            || self.view.modal(cx, ids!(leave_room_confirm_modal)).is_open();
        let is_interactive_hit = utils::is_interactive_hit_event(event);
        let is_pane_shown: bool;
        if room_info_action_modal_open {
            is_pane_shown = true;
        }
        else if loading_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            if is_interactive_hit {
                loading_pane.handle_event(cx, event, scope);
            }
        }
        else if threads_sliding_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            if is_interactive_hit {
                threads_sliding_pane.handle_event(cx, event, scope);
            }
        }
        else if user_profile_sliding_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            if is_interactive_hit {
                user_profile_sliding_pane.handle_event(cx, event, scope);
            }
        }
        else if room_info_sliding_pane.is_currently_shown(cx) {
            is_pane_shown = true;
            if is_interactive_hit {
                room_info_sliding_pane.handle_event(cx, event, scope);
            }
        }
        else {
            is_pane_shown = false;
        }

        // TODO: once we use the `hits()` API, should be able to remove the above conditionals
        //       about whether the loading pane or user profile pane are shown, because
        //       Makepad already delivers most events to all views regardless of visibility,
        //       so the only thing we'd need here is the conditional below.

        if room_info_action_modal_open || !is_pane_shown || !is_interactive_hit {
            let Some(room_props) = self.build_room_screen_props(cx, scope, room_screen_widget_uid) else {
                if !is_pane_shown || !is_interactive_hit {
                    return;
                }
                log!("RoomScreen handling event with no room_name_id and no tl_state, skipping room-dependent event handling");
                return;
            };
            let mut room_scope = if let Some(app_state) = scope.data.get_mut::<AppState>() {
                Scope::with_data_props(app_state, &room_props)
            } else {
                Scope::with_props(&room_props)
            };
            let leave_room_confirm_modal_uid = self
                .confirmation_modal(cx, ids!(leave_room_confirm_modal_inner))
                .widget_uid();


            // Forward the event to the inner timeline view, but capture any actions it produces
            // such that we can handle the ones relevant to only THIS RoomScreen widget right here and now,
            // ensuring they are not mistakenly handled by other RoomScreen widget instances.
            let mut actions_generated_within_this_room_screen = cx.capture_actions(|cx|
                self.view.handle_event(cx, event, &mut room_scope)
            );
            // Here, we handle and remove any general actions that are relevant to only this RoomScreen.
            // Removing the handled actions ensures they are not mistakenly handled by other RoomScreen widget instances.
            actions_generated_within_this_room_screen.retain(|action| {
                if self.handle_link_clicked(cx, action, &user_profile_sliding_pane) {
                    return false;
                }

                match action
                    .as_widget_action()
                    .widget_uid_eq(room_screen_widget_uid)
                    .cast()
                {
                    AppServicePanelAction::Dismiss => {
                        self.set_app_service_actions_visible(cx, false);
                        return false;
                    }
                    AppServicePanelAction::OpenCreateBotModal => {
                        if let Some(app_state) = scope.data.get::<AppState>() {
                            if !app_state.bot_settings.enabled {
                                self.send_app_service_feedback_message(
                                    tr_key(self.app_language, "room_screen.popup.app_service.enable_before_create"),
                                );
                                self.set_app_service_actions_visible(cx, false);
                            } else if !room_props.app_service_room_bound {
                                self.send_app_service_feedback_message(
                                    tr_key(self.app_language, "room_screen.popup.app_service.bind_before_create"),
                                );
                                self.set_app_service_actions_visible(cx, false);
                            } else {
                                self.open_create_bot_modal(cx);
                            }
                        } else {
                            self.send_app_service_feedback_message(
                                tr_key(self.app_language, "room_screen.popup.app_service.state_unavailable_create"),
                            );
                            self.set_app_service_actions_visible(cx, false);
                        }
                        return false;
                    }
                    AppServicePanelAction::OpenDeleteBotModal => {
                        if let Some(app_state) = scope.data.get::<AppState>() {
                            if !app_state.bot_settings.enabled {
                                self.send_app_service_feedback_message(
                                    tr_key(self.app_language, "room_screen.popup.app_service.enable_before_delete"),
                                );
                                self.set_app_service_actions_visible(cx, false);
                            } else if !room_props.app_service_room_bound {
                                self.send_app_service_feedback_message(
                                    tr_key(self.app_language, "room_screen.popup.app_service.bind_before_delete"),
                                );
                                self.set_app_service_actions_visible(cx, false);
                            } else {
                                self.open_delete_bot_modal(cx);
                            }
                        } else {
                            self.send_app_service_feedback_message(
                                tr_key(self.app_language, "room_screen.popup.app_service.state_unavailable_delete"),
                            );
                            self.set_app_service_actions_visible(cx, false);
                        }
                        return false;
                    }
                    AppServicePanelAction::SendListBots => {
                        if let Some(app_state) = scope.data.get::<AppState>() {
                            self.send_botfather_command(
                                cx,
                                app_state,
                                "/listbots",
                                tr_key(self.app_language, "room_screen.popup.bot.sent_listbots").to_string(),
                            );
                        }
                        return false;
                    }
                    AppServicePanelAction::SendBotHelp => {
                        if let Some(app_state) = scope.data.get::<AppState>() {
                            self.send_botfather_command(
                                cx,
                                app_state,
                                "/bothelp",
                                tr_key(self.app_language, "room_screen.popup.bot.sent_bothelp").to_string(),
                            );
                        }
                        return false;
                    }
                    AppServicePanelAction::ShowBoundBots => {
                        cx.action(BotBindingModalAction::Open(
                            room_props.room_name_id.clone(),
                        ));
                        self.set_app_service_actions_visible(cx, false);
                        return false;
                    }
                    AppServicePanelAction::Unbind => {
                        if let Some(app_state) = scope.data.get::<AppState>() {
                            if !room_props.app_service_room_bound {
                                self.send_app_service_feedback_message(
                                    tr_key(self.app_language, "room_screen.popup.app_service.room_not_bound"),
                                );
                            } else {
                                match app_state
                                    .bot_settings
                                    .resolved_bot_user_id_for_room(
                                        room_props.room_name_id.room_id(),
                                        current_user_id().as_deref(),
                                    )
                                {
                                    Ok(bot_user_id) => {
                                        submit_async_request(MatrixRequest::SetRoomBotBinding {
                                            room_id: room_props.room_name_id.room_id().clone(),
                                            bound: false,
                                            bot_user_id: bot_user_id.clone(),
                                        });
                                        self.send_app_service_feedback_message(
                                            tr_fmt(self.app_language, "room_screen.popup.app_service.removing_botfather", &[
                                                ("bot_user_id", bot_user_id.as_str()),
                                            ]),
                                        );
                                    }
                                    Err(error) => {
                                        self.send_app_service_feedback_message(
                                            error,
                                        );
                                    }
                                }
                            }
                        } else {
                            self.send_app_service_feedback_message(
                                tr_key(self.app_language, "room_screen.popup.app_service.state_unavailable_unbind"),
                            );
                        }
                        self.set_app_service_actions_visible(cx, false);
                        return false;
                    }
                    _ => {}
                }

                // Handle precomputed member sort ready (from background thread).
                // Validate by Arc::ptr_eq to reject stale results from a different
                // member snapshot. The Arc is kept alive in the action to prevent ABA.
                if let Some(sort_ready) = action.downcast_ref::<crate::cpu_worker::PrecomputedMemberSortReady>() {
                    if let Some(tl) = self.tl_state.as_mut() {
                        if tl.kind == sort_ready.timeline_kind {
                            let is_same = tl.room_members.as_ref()
                                .is_some_and(|m| Arc::ptr_eq(m, &sort_ready.members_arc));
                            if is_same {
                                tl.room_members_sort = Some(sort_ready.sort.clone());
                            }
                        }
                    }
                }

                match action.downcast_ref::<CreateBotModalAction>() {
                    Some(CreateBotModalAction::Close) => {
                        self.close_create_bot_modal(cx);
                        return false;
                    }
                    Some(CreateBotModalAction::Submit(request)) => {
                        let Some(app_state) = scope.data.get::<AppState>() else {
                            self.send_app_service_feedback_message(
                                tr_key(self.app_language, "room_screen.popup.bot.state_unavailable_create_command"),
                            );
                            self.close_create_bot_modal(cx);
                            return false;
                        };
                        self.send_create_bot_command(
                            cx,
                            app_state,
                            &request.username,
                            &request.display_name,
                            request.system_prompt.as_deref(),
                        );
                        return false;
                    }
                    None => {}
                }

                match action.downcast_ref::<DeleteBotModalAction>() {
                    Some(DeleteBotModalAction::Close) => {
                        self.close_delete_bot_modal(cx);
                        return false;
                    }
                    Some(DeleteBotModalAction::Submit(request)) => {
                        let Some(app_state) = scope.data.get::<AppState>() else {
                            self.send_app_service_feedback_message(
                                tr_key(self.app_language, "room_screen.popup.bot.state_unavailable_delete_command"),
                            );
                            self.close_delete_bot_modal(cx);
                            return false;
                        };
                        self.send_delete_bot_command(cx, app_state, &request.user_id_or_localpart);
                        return false;
                    }
                    None => {}
                }

                match action.downcast_ref::<ReportRoomModalAction>() {
                    Some(ReportRoomModalAction::Close) => {
                        self.close_report_room_modal(cx);
                        return false;
                    }
                    Some(ReportRoomModalAction::Submit(reason)) => {
                        let Some(room_id) = self.room_id().cloned() else {
                            self.close_report_room_modal(cx);
                            return false;
                        };
                        submit_async_request(MatrixRequest::ReportRoom {
                            room_id,
                            reason: reason.clone(),
                        });
                        self.close_report_room_modal(cx);
                        return false;
                    }
                    None => {}
                }

                if let ConfirmationModalAction::Close(accepted) = action
                    .as_widget_action()
                    .widget_uid_eq(leave_room_confirm_modal_uid)
                    .cast()
                {
                    self.close_leave_room_confirm_modal(cx);
                    if accepted {
                        if let Some(room_id) = self.room_id().cloned() {
                            submit_async_request(MatrixRequest::LeaveRoom {
                                room_id,
                            });
                        }
                    }
                    return false;
                }

                if let MessageAction::ToggleAppServiceActions = action
                    .as_widget_action()
                    .widget_uid_eq(room_screen_widget_uid)
                    .cast()
                {
                    if room_props.timeline_kind.thread_root_event_id().is_some() {
                        self.send_app_service_feedback_message(
                            tr_key(self.app_language, "room_screen.popup.bot.main_timeline_only"),
                        );
                    } else if !room_props.app_service_enabled {
                        self.send_app_service_feedback_message(
                            tr_key(self.app_language, "room_screen.popup.bot.enable_in_settings_before_bot"),
                        );
                    } else {
                        self.toggle_app_service_actions(cx);
                    }
                    return false;
                }

                // Handle the action that requests to show the user profile sliding pane.
                if let ShowUserProfileAction::ShowUserProfile(profile_and_room_id) = action.as_widget_action().cast() {
                    self.show_user_profile(
                        cx,
                        &user_profile_sliding_pane,
                        UserProfilePaneInfo {
                            profile_and_room_id,
                            room_name: self.room_name_id.as_ref().map_or_else(
                                || tr_key(self.app_language, "room_screen.fallback.unnamed_room").to_string(),
                                |r| r.to_string(),
                            ),
                            room_member: None,
                        },
                    );
                }

                /*
                match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast() {
                    MessageAction::ActionBarClose => {
                        let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(cx, ids!(message_action_bar));

                        // close only if the active message is requesting it to avoid double closes.
                        if let Some(message_widget_uid) = message_action_bar.message_widget_uid() {
                            if action.as_widget_action().widget_uid_eq(message_widget_uid).is_some() {
                                message_action_bar_popup.close(cx);
                            }
                        }
                    }
                    MessageAction::ActionBarOpen { item_id, message_rect } => {
                        let message_action_bar_popup = self.popup_notification(cx, ids!(message_action_bar_popup));
                        let message_action_bar = message_action_bar_popup.message_action_bar(cx, ids!(message_action_bar));

                        let margin_x = 50.;

                        let coords = dvec2(
                            (message_rect.pos.x + message_rect.size.x) - margin_x,
                            message_rect.pos.y,
                        );

                        script_apply_eval!(cx, message_action_bar_popup, {
                            content +: { margin +: { left: #(coords.x), top: #(coords.y) } }
                        });

                        if let Some(message_widget_uid) = action.as_widget_action().map(|a| a.widget_uid) {
                            message_action_bar_popup.open(cx);
                            message_action_bar.initialize_with_data(cx, widget_uid, message_widget_uid, item_id);
                        }
                    }
                    _ => {}
                }
                */

                // Keep all unhandled actions so we can add them back to the global action list below.
                true
            });
            self.handle_translation_lang_popup_actions(cx, &actions_generated_within_this_room_screen);
            // Add back any unhandled actions to the global action list.
            cx.extend_actions(actions_generated_within_this_room_screen);
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        // If the room isn't loaded yet, we show the restore status label only.
        if !self.is_loaded {
            let Some(room_name) = &self.room_name_id else {
                // No room selected yet, nothing to show.
                return DrawStep::done();
            };
            let mut restore_status_view = self.view.restore_status_view(cx, ids!(restore_status_view));
            restore_status_view.set_content(cx, self.all_rooms_loaded, room_name);
            return restore_status_view.draw(cx, scope);
        }
        if self.tl_state.is_none() {
            // Tl_state may not be ready after dock loading.
            // If return DrawStep::done() inside self.view.draw_walk, turtle will misalign and panic.
            return DrawStep::done();
        }


        let room_screen_widget_uid = self.widget_uid();
        let Some(room_props) = self.build_room_screen_props(cx, scope, room_screen_widget_uid) else {
            return DrawStep::done();
        };
        let mut room_scope = if let Some(app_state) = scope.data.get_mut::<AppState>() {
            Scope::with_data_props(app_state, &room_props)
        } else {
            Scope::with_props(&room_props)
        };
        self.octos_action_button_contexts.clear();
        while let Some(subview) = self.view.draw_walk(cx, &mut room_scope, walk).step() {
            // Here, we only need to handle drawing the portal list.
            let portal_list_ref = subview.as_portal_list();
            let Some(mut list_ref) = portal_list_ref.borrow_mut() else { continue };
            let Some(tl_state) = self.tl_state.as_mut() else {
                return DrawStep::done();
            };

            // Set the portal list's range based on the number of timeline items.
            let tl_items = &tl_state.items;
            let last_item_id = tl_items.len() + usize::from(self.show_app_service_actions);

            let list = list_ref.deref_mut();
            list.set_item_range(cx, 0, last_item_id);

            let (
                resolved_parent_bot_user_id,
                room_bot_user_ids,
                known_bot_user_ids,
            ) = compute_timeline_bot_context(
                room_scope.data.get::<AppState>(),
                tl_state.kind.room_id(),
                tl_state.room_members.as_ref(),
            );

            while let Some(item_id) = list.next_visible_item(cx) {
                let item = {
                    let tl_idx = item_id;
                    if self.show_app_service_actions && tl_idx == tl_items.len() {
                        list.item(cx, item_id, id!(AppServicePanel))
                    } else {
                    let Some(timeline_item) = tl_items.get(tl_idx) else {
                        // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                        // but we can always safely fill the item with an empty widget that takes up no space.
                        list.item(cx, item_id, id!(Empty));
                        continue;
                    };

                    // Determine whether this item's content and profile have been drawn since the last update.
                    // Pass this state to each of the `populate_*` functions so they can attempt to re-use
                    // an item in the timeline's portallist that was previously populated, if one exists.
                    let item_drawn_status = ItemDrawnStatus {
                        content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
                        profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
                    };
                    let (item, item_new_draw_status) = match timeline_item.kind() {
                        TimelineItemKind::Event(event_tl_item) => match event_tl_item.content() {
                            TimelineItemContent::MsgLike(msg_like_content) => {
                                if tl_state.kind.thread_root_event_id().is_none()
                                    && msg_like_content.thread_root.is_some()
                                {
                                    // Hide threaded replies from the main room timeline UI.
                                    (list.item(cx, item_id, id!(Empty)), ItemDrawnStatus::both_drawn())
                                } else {
                                    match &msg_like_content.kind {
                                        MsgLikeKind::Message(_)
                                        | MsgLikeKind::Sticker(_)
                                        | MsgLikeKind::Redacted => {
                                            let prev_event = tl_idx.checked_sub(1).and_then(|i| tl_items.get(i));
                                            populate_message_view(
                                                cx,
                                                list,
                                                item_id,
                                                &tl_state.kind,
                                                self.app_language,
                                                event_tl_item,
                                                msg_like_content,
                                                prev_event,
                                                &mut tl_state.media_cache,
                                                &mut tl_state.link_preview_cache,
                                                &tl_state.fetched_thread_summaries,
                                                &mut tl_state.pending_thread_summary_fetches,
                                                &tl_state.user_power,
                                                &self.pinned_events,
                                                item_drawn_status,
                                                room_screen_widget_uid,
                                                resolved_parent_bot_user_id.as_deref(),
                                                &room_bot_user_ids,
                                                &known_bot_user_ids,
                                                &mut tl_state.streaming_messages,
                                                &mut self.octos_action_button_contexts,
                                                &self.disabled_octos_action_source_event_ids,
                                                &self.selected_octos_action_by_source_event_id,
                                            )
                                        },
                                        // TODO: properly implement `Poll` as a regular Message-like timeline item.
                                        MsgLikeKind::Poll(poll_state) => populate_small_state_event(
                                            cx,
                                            list,
                                            item_id,
                                            &tl_state.kind,
                                            self.app_language,
                                            event_tl_item,
                                            poll_state,
                                            item_drawn_status,
                                        ),
                                        MsgLikeKind::UnableToDecrypt(utd) => populate_small_state_event(
                                            cx,
                                            list,
                                            item_id,
                                            &tl_state.kind,
                                            self.app_language,
                                            event_tl_item,
                                            utd,
                                            item_drawn_status,
                                        ),
                                        MsgLikeKind::LiveLocation(live_loc) => populate_small_state_event(
                                            cx,
                                            list,
                                            item_id,
                                            &tl_state.kind,
                                            app_language,
                                            event_tl_item,
                                            live_loc,
                                            item_drawn_status,
                                        ),
                                        MsgLikeKind::Other(other) => populate_small_state_event(
                                            cx,
                                            list,
                                            item_id,
                                            &tl_state.kind,
                                            self.app_language,
                                            event_tl_item,
                                            other,
                                            item_drawn_status,
                                        ),
                                    }
                                }
                            },
                            TimelineItemContent::MembershipChange(membership_change) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                &tl_state.kind,
                                self.app_language,
                                event_tl_item,
                                membership_change,
                                item_drawn_status,
                            ),
                            TimelineItemContent::ProfileChange(profile_change) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                &tl_state.kind,
                                self.app_language,
                                event_tl_item,
                                profile_change,
                                item_drawn_status,
                            ),
                            TimelineItemContent::OtherState(other) => populate_small_state_event(
                                cx,
                                list,
                                item_id,
                                &tl_state.kind,
                                self.app_language,
                                event_tl_item,
                                other,
                                item_drawn_status,
                            ),
                            unhandled => {
                                let item = list.item(cx, item_id, id!(SmallStateEvent));
                                item.label(cx, ids!(content)).set_text(
                                    cx,
                                    &format!("{} {:?}", tr_key(self.app_language, "room_screen.unsupported.prefix"), unhandled),
                                );
                                (item, ItemDrawnStatus::both_drawn())
                            }
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(millis)) => {
                            let item = list.item(cx, item_id, id!(DateDivider));
                            let text = unix_time_millis_to_datetime(*millis)
                                // format the time as a shortened date (Sat, Sept 5, 2021)
                                .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                                .unwrap_or_else(|| format!("{:?}", millis));
                            item.label(cx, ids!(date)).set_text(cx, &text);
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                            let item = list.item(cx, item_id, id!(ReadMarker));
                            item.label(cx, ids!(date)).set_text(
                                cx,
                                tr_key(self.app_language, "room_screen.read_marker.new_messages"),
                            );
                            (item, ItemDrawnStatus::both_drawn())
                        }
                        TimelineItemKind::Virtual(VirtualTimelineItem::TimelineStart) => {
                            let item = list.item(cx, item_id, id!(Empty));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    };

                    // Now that we've drawn the item, add its index to the set of drawn items.
                    if item_new_draw_status.content_drawn {
                        tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                    }
                    if item_new_draw_status.profile_drawn {
                        tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                    }
                    item
                    }
                };
                item.draw_all(cx, &mut room_scope);
            }

            // If the list is not filling the viewport, we need to back paginate the timeline
            // until we have enough events items to fill the viewport.
            if tl_state.kind.thread_root_event_id().is_none()
                && !tl_state.fully_paginated
                && !tl_state.backwards_pagination_in_flight
                && !list.is_filling_viewport()
            {
                tl_state.backwards_pagination_in_flight = true;
                log!("Automatically paginating timeline to fill viewport for room {:?}", self.room_name_id);
                submit_async_request(MatrixRequest::PaginateTimeline {
                    timeline_kind: tl_state.kind.clone(),
                    num_events: VIEWPORT_FILL_PAGINATION_SIZE,
                    direction: PaginationDirection::Backwards,
                });
            }
        }
        DrawStep::done()
    }
}

impl RoomScreen {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.app_language_initialized = true;
        self.sync_app_language(cx);
    }

    fn sync_app_language(&mut self, cx: &mut Cx) {
        self.view
            .label(cx, ids!(top_space.label))
            .set_text(cx, tr_key(self.app_language, "room_screen.top_space.loading_earlier"));
        self.view
            .room_input_bar(cx, ids!(room_input_bar))
            .set_app_language(cx, self.app_language);
        self.sync_translation_lang_popup(cx);
        self.view.redraw(cx);
    }

    fn redraw_timeline_list(&self, cx: &mut Cx) {
        let portal_list = self.portal_list(cx, ids!(timeline.list));
        if let Some(mut list) = portal_list.borrow_mut() {
            list.redraw(cx);
        }
    }

    fn sync_translation_lang_popup(&mut self, cx: &mut Cx) {
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_en))
            .set_text(cx, &translation::language_popup_label("en"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_zh))
            .set_text(cx, &translation::language_popup_label("zh"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_zh_tw))
            .set_text(cx, &translation::language_popup_label("zh-TW"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_ja))
            .set_text(cx, &translation::language_popup_label("ja"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_ko))
            .set_text(cx, &translation::language_popup_label("ko"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_es))
            .set_text(cx, &translation::language_popup_label("es"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_fr))
            .set_text(cx, &translation::language_popup_label("fr"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_de))
            .set_text(cx, &translation::language_popup_label("de"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_ru))
            .set_text(cx, &translation::language_popup_label("ru"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_pt))
            .set_text(cx, &translation::language_popup_label("pt"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_ar))
            .set_text(cx, &translation::language_popup_label("ar"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_vi))
            .set_text(cx, &translation::language_popup_label("vi"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_th))
            .set_text(cx, &translation::language_popup_label("th"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_id))
            .set_text(cx, &translation::language_popup_label("id"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_ms))
            .set_text(cx, &translation::language_popup_label("ms"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_tr))
            .set_text(cx, &translation::language_popup_label("tr"));
        self.view
            .button(cx, ids!(translation_lang_modal.content.translation_lang_popup.translation_lang_scroll.lang_hi))
            .set_text(cx, &translation::language_popup_label("hi"));
    }

    fn build_room_screen_props(
        &self,
        cx: &mut Cx,
        scope: &mut Scope,
        room_screen_widget_uid: WidgetUid,
    ) -> Option<RoomScreenProps> {
        if let Some(tl) = self.tl_state.as_ref() {
            let room_id = tl.kind.room_id().clone();
            let room_members = tl.room_members.clone();
            let is_direct_room = cx.get_global::<RoomsListRef>()
                .is_direct_room(&room_id)
                .unwrap_or(false);
            let (
                app_service_enabled,
                app_service_room_bound,
                has_persisted_management_binding,
                bound_bot_user_id,
                resolved_parent_bot_user_id,
                room_bot_user_ids,
                known_bot_user_ids,
            ) = scope
                .data
                .get::<AppState>()
                .map(|app_state| {
                    let app_service_enabled = app_state.bot_settings.enabled;
                    let persisted_bound_bot_user_id =
                        app_state.bot_settings.bound_bot_user_id(&room_id).map(ToOwned::to_owned);
                    let persisted_room_bot_user_ids = if app_service_enabled {
                        app_state.bot_settings.bound_bot_user_ids(&room_id)
                    } else {
                        Vec::new()
                    };
                    let resolved_parent_bot_user_id = if app_service_enabled {
                        app_state
                            .bot_settings
                            .resolved_bot_user_id(current_user_id().as_deref())
                            .ok()
                    } else {
                        None
                    };
                    let known_bot_user_ids = if app_service_enabled {
                        app_state.bot_settings.known_bot_user_ids()
                    } else {
                        Vec::new()
                    };
                    let has_persisted_management_binding = resolved_parent_bot_user_id
                        .as_ref()
                        .is_some_and(|resolved_parent_bot_user_id|
                            persisted_room_bot_user_ids
                                .iter()
                                .any(|bot_user_id| bot_user_id == resolved_parent_bot_user_id)
                        );
                    let room_bot_user_ids = room_members
                        .as_ref()
                        .map(|members|
                            collect_room_bot_user_ids(
                                members.as_ref(),
                                resolved_parent_bot_user_id.as_deref(),
                                &known_bot_user_ids,
                                &persisted_room_bot_user_ids,
                            )
                        )
                        .unwrap_or(persisted_room_bot_user_ids);
                    let detected_bound_bot_user_id = room_members
                        .as_ref()
                        .and_then(|members|
                            detected_bot_binding_for_members(
                                app_state,
                                &room_id,
                                members.as_ref(),
                            )
                        );
                    let bound_bot_user_id = if app_service_enabled {
                        persisted_bound_bot_user_id.or(detected_bound_bot_user_id)
                    } else {
                        None
                    };
                    let app_service_room_bound = bound_bot_user_id.is_some();
                    (
                        app_service_enabled,
                        app_service_room_bound,
                        has_persisted_management_binding,
                        bound_bot_user_id,
                        resolved_parent_bot_user_id,
                        room_bot_user_ids,
                        known_bot_user_ids,
                    )
                })
                .unwrap_or((false, false, false, None, None, Vec::new(), Vec::new()));

            Some(RoomScreenProps {
                room_screen_widget_uid,
                room_name_id: self.room_name_id.clone().unwrap_or_else(|| RoomNameId::empty(room_id.clone())),
                timeline_kind: tl.kind.clone(),
                room_members,
                is_direct_room,
                room_bot_user_ids,
                room_members_sync_pending: tl.room_members_sync_pending,
                room_members_sort: tl.room_members_sort.clone(),
                room_avatar_url: self.room_avatar_url.clone(),
                app_service_enabled,
                app_service_room_bound,
                has_persisted_management_binding,
                bound_bot_user_id,
                resolved_parent_bot_user_id,
                known_bot_user_ids,
            })
        } else {
            self.room_name_id.as_ref().map(|room_name| RoomScreenProps {
                room_screen_widget_uid,
                room_name_id: room_name.clone(),
                timeline_kind: self.timeline_kind.clone()
                    .expect("BUG: room_name_id was set but timeline_kind was missing"),
                room_members: None,
                is_direct_room: false,
                room_bot_user_ids: Vec::new(),
                room_members_sort: None,
                room_members_sync_pending: false,
                room_avatar_url: None,
                app_service_enabled: false,
                app_service_room_bound: false,
                has_persisted_management_binding: false,
                bound_bot_user_id: None,
                resolved_parent_bot_user_id: None,
                known_bot_user_ids: Vec::new(),
            })
        }
    }

    fn room_id(&self) -> Option<&OwnedRoomId> {
        self.room_name_id.as_ref().map(|r| r.room_id())
    }

    /// Extract the text body from a timeline item, if it's a text message.
    fn extract_message_text(item: &Arc<TimelineItem>) -> Option<String> {
        let TimelineItemKind::Event(event) = item.kind() else { return None };
        let TimelineItemContent::MsgLike(_) = event.content() else { return None };
        Some(plaintext_body_of_timeline_item(event))
    }

    fn discover_known_bot_user_ids_from_timeline_items(
        app_state: &AppState,
        timeline_items: &Vector<Arc<TimelineItem>>,
    ) -> Vec<OwnedUserId> {
        let Ok(parent_bot_user_id) = app_state
            .bot_settings
            .resolved_bot_user_id(current_user_id().as_deref())
        else {
            return Vec::new();
        };

        let default_server_name = current_user_id()
            .map(|user_id| user_id.server_name().to_owned());
        let mut discovered_bot_user_ids = Vec::<OwnedUserId>::new();
        let mut push_bot_user_id = |bot_user_id: OwnedUserId| {
            if bot_user_id.as_str() == parent_bot_user_id.as_str() {
                return;
            }
            if !discovered_bot_user_ids
                .iter()
                .any(|existing_bot_user_id| existing_bot_user_id.as_str() == bot_user_id.as_str())
            {
                discovered_bot_user_ids.push(bot_user_id);
            }
        };

        for item in timeline_items {
            let TimelineItemKind::Event(event_tl_item) = item.kind() else { continue };
            if event_tl_item.sender().as_str() != parent_bot_user_id.as_str() {
                continue;
            }
            let Some(message_text) = Self::extract_message_text(item) else { continue };
            for bot_user_id in extract_bot_user_ids_from_listbots_reply(
                &message_text,
                default_server_name.as_ref(),
            ) {
                push_bot_user_id(bot_user_id);
            }
        }

        discovered_bot_user_ids
    }

    fn schedule_stream_timeout(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.streaming_timeout_timer);
        self.streaming_timeout_timer = next_stream_timeout(
            self.tl_state
                .as_ref()
                .into_iter()
                .flat_map(|tl| tl.streaming_messages.values()),
        )
        .map(|duration| cx.start_timeout(duration.as_secs_f64()))
        .unwrap_or_else(Timer::empty);
    }

    fn set_app_service_actions_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.show_app_service_actions = visible;
        self.redraw(cx);
    }

    fn toggle_app_service_actions(&mut self, cx: &mut Cx) {
        self.set_app_service_actions_visible(cx, !self.show_app_service_actions);
    }

    fn close_create_bot_modal(&self, cx: &mut Cx) {
        self.view.modal(cx, ids!(create_bot_modal)).close(cx);
    }

    fn close_delete_bot_modal(&self, cx: &mut Cx) {
        self.view.modal(cx, ids!(delete_bot_modal)).close(cx);
    }

    fn close_report_room_modal(&self, cx: &mut Cx) {
        self.view.modal(cx, ids!(report_room_modal)).close(cx);
    }

    fn close_leave_room_confirm_modal(&self, cx: &mut Cx) {
        self.view.modal(cx, ids!(leave_room_confirm_modal)).close(cx);
    }

    fn open_create_bot_modal(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.clone() else {
            return;
        };
        self.set_app_service_actions_visible(cx, false);
        self.view
            .create_bot_modal(cx, ids!(create_bot_modal_inner))
            .show(cx, room_name_id);
        self.view.modal(cx, ids!(create_bot_modal)).open(cx);
    }

    fn open_delete_bot_modal(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.clone() else {
            return;
        };
        self.set_app_service_actions_visible(cx, false);
        self.view
            .delete_bot_modal(cx, ids!(delete_bot_modal_inner))
            .show(cx, room_name_id);
        self.view.modal(cx, ids!(delete_bot_modal)).open(cx);
    }

    fn open_report_room_modal(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else {
            return;
        };
        self.view
            .report_room_modal(cx, ids!(report_room_modal_inner))
            .show(cx, room_name_id);
        self.view.modal(cx, ids!(report_room_modal)).open(cx);
    }

    fn open_leave_room_confirm_modal(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else {
            return;
        };
        self.view
            .confirmation_modal(cx, ids!(leave_room_confirm_modal_inner))
            .show(cx, ConfirmationModalContent {
                title_text: String::from("Leave Room").into(),
                body_text: format!("Are you sure you want to leave {}?", room_name_id).into(),
                accept_button_text: Some(String::from("Leave").into()),
                cancel_button_text: Some(String::from("Cancel").into()),
                ..Default::default()
            });
        self.view.modal(cx, ids!(leave_room_confirm_modal)).open(cx);
    }

    fn reset_app_service_ui(&mut self, cx: &mut Cx) {
        self.set_app_service_actions_visible(cx, false);
        self.close_create_bot_modal(cx);
        self.close_delete_bot_modal(cx);
        self.close_report_room_modal(cx);
        self.close_leave_room_confirm_modal(cx);
    }

    fn is_app_service_room_bound(&self, app_state: &AppState, room_id: &OwnedRoomId) -> bool {
        app_state.bot_settings.is_room_bound(room_id)
    }

    fn send_app_service_feedback_message(&self, message: impl Into<String>) {
        let Some(room_id) = self.room_id().cloned() else {
            return;
        };
        let message = format!("[App Service] {}", message.into());
        submit_async_request(MatrixRequest::SendMessage {
            timeline_kind: TimelineKind::MainRoom { room_id },
            message: RoomMessageEventContent::notice_plain(message),
            replied_to: None,
            target_user_id: None,
            explicit_room: false,
            #[cfg(feature = "tsp")]
            sign_with_tsp: false,
        });
    }

    fn send_botfather_command(
        &mut self,
        cx: &mut Cx,
        app_state: &AppState,
        command: &str,
        success_message: String,
    ) -> bool {
        let Some(timeline_kind) = self.timeline_kind.clone() else {
            return false;
        };
        if timeline_kind.thread_root_event_id().is_some() {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.bot.main_timeline_only"),
            );
            return false;
        }

        let Some(room_id) = self.room_id().cloned() else {
            return false;
        };
        if !app_state.bot_settings.enabled {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.bot.enable_before_commands"),
            );
            return false;
        }
        if !self.is_app_service_room_bound(app_state, &room_id) {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.bot.bind_before_commands"),
            );
            return false;
        }

        submit_async_request(MatrixRequest::SendMessage {
            timeline_kind,
            message: RoomMessageEventContent::text_plain(command),
            replied_to: None,
            target_user_id: app_state
                .bot_settings
                .bound_bot_user_id(room_id.as_ref())
                .map(ToOwned::to_owned),
            explicit_room: false,
            #[cfg(feature = "tsp")]
            sign_with_tsp: false,
        });

        self.send_app_service_feedback_message(success_message);
        self.set_app_service_actions_visible(cx, false);
        true
    }

    fn send_create_bot_command(
        &mut self,
        cx: &mut Cx,
        app_state: &AppState,
        username: &str,
        display_name: &str,
        system_prompt: Option<&str>,
    ) {
        let Some(timeline_kind) = self.timeline_kind.clone() else {
            return;
        };
        if timeline_kind.thread_root_event_id().is_some() {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.bot.creation_main_timeline_only"),
            );
            return;
        }

        let Some(room_id) = self.room_id().cloned() else {
            return;
        };
        if !app_state.bot_settings.enabled {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.app_service.enable_before_create"),
            );
            return;
        }
        if !self.is_app_service_room_bound(app_state, &room_id) {
            self.send_app_service_feedback_message(
                tr_key(self.app_language, "room_screen.popup.app_service.bind_before_create"),
            );
            return;
        }

        let command = format_create_bot_command(username, display_name, system_prompt);
        if self.send_botfather_command(
            cx,
            app_state,
            &command,
            tr_fmt(self.app_language, "room_screen.popup.bot.sent_createbot", &[("username", username)]),
        ) {
            self.close_create_bot_modal(cx);
        }
    }

    fn send_delete_bot_command(
        &mut self,
        cx: &mut Cx,
        app_state: &AppState,
        user_id_or_localpart: &str,
    ) {
        let matrix_user_id =
            match resolve_delete_bot_user_id(user_id_or_localpart, current_user_id().as_deref(), self.app_language) {
                Ok(user_id) => user_id,
                Err(error) => {
                    self.send_app_service_feedback_message(error);
                    return;
                }
            };

        let command = format_delete_bot_command(matrix_user_id.as_ref());
        if self.send_botfather_command(
            cx,
            app_state,
            &command,
            tr_fmt(self.app_language, "room_screen.popup.bot.sent_deletebot", &[("matrix_user_id", matrix_user_id.as_str())]),
        ) {
            self.close_delete_bot_modal(cx);
        }
    }

    /// Processes all pending background updates to the currently-shown timeline.
    ///
    /// Redraws this RoomScreen view if any updates were applied.
    fn process_timeline_updates(
        &mut self,
        cx: &mut Cx,
        portal_list: &PortalListRef,
        app_state: Option<&AppState>,
    ) {
        let top_space = self.view(cx, ids!(top_space));
        let jump_to_bottom_button = self.jump_to_bottom_button(cx, ids!(jump_to_bottom_button));
        let curr_first_id = portal_list.first_id();
        let ui = self.widget_uid();
        let Some(tl) = self.tl_state.as_mut() else { return };
        let (
            resolved_parent_bot_user_id,
            room_bot_user_ids,
            known_bot_user_ids,
        ) = compute_timeline_bot_context(
            app_state,
            tl.kind.room_id(),
            tl.room_members.as_ref(),
        );

        let mut done_loading = false;
        let mut should_continue_backwards_pagination = false;
        let mut typing_users = None;
        let mut num_updates = 0;
        while let Ok(update) = tl.update_receiver.try_recv() {
            num_updates += 1;
            match update {
                TimelineUpdate::FirstUpdate { initial_items } => {
                    if let Some(app_state) = app_state {
                        let discovered_bot_user_ids =
                            Self::discover_known_bot_user_ids_from_timeline_items(
                                app_state,
                                &initial_items,
                            );
                        if !discovered_bot_user_ids.is_empty() {
                            Cx::post_action(AppStateAction::KnownBotUserIdsDiscovered {
                                bot_user_ids: discovered_bot_user_ids,
                            });
                        }
                    }
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                    tl.fully_paginated = false;
                    // Set the portal list to the very bottom of the timeline.
                    portal_list.set_first_id_and_scroll(initial_items.len().saturating_sub(1), 0.0);
                    portal_list.set_tail_range(true);
                    jump_to_bottom_button.update_visibility(cx, true);

                    let previous_streaming_messages = std::mem::take(&mut tl.streaming_messages);
                    let (rebuilt_streaming_messages, should_schedule_frame) =
                        rebuild_streaming_messages_for_full_snapshot(
                            streaming_candidates_from_items(&initial_items),
                            Some(&previous_streaming_messages),
                        );

                    tl.items = initial_items;
                    tl.streaming_messages = rebuilt_streaming_messages;
                    refresh_stream_indices(
                        tl.items.iter().map(item_event_id),
                        &mut tl.streaming_messages,
                    );
                    if should_schedule_frame {
                        self.streaming_next_frame = cx.new_next_frame();
                    }
                    done_loading = true;
                }
                TimelineUpdate::NewItems { new_items, changed_indices, is_append, clear_cache } => {
                    if let Some(app_state) = app_state {
                        let discovered_bot_user_ids =
                            Self::discover_known_bot_user_ids_from_timeline_items(
                                app_state,
                                &new_items,
                            );
                        if !discovered_bot_user_ids.is_empty() {
                            Cx::post_action(AppStateAction::KnownBotUserIdsDiscovered {
                                bot_user_ids: discovered_bot_user_ids,
                            });
                        }
                    }
                    if new_items.is_empty() {
                        if !tl.items.is_empty() {
                            log!("process_timeline_updates(): timeline (had {} items) was cleared for room {}", tl.items.len(), tl.kind.room_id());
                            // For now, we paginate a cleared timeline in order to be able to show something at least.
                            // A proper solution would be what's described below, which would be to save a few event IDs
                            // and then either focus on them (if we're not close to the end of the timeline)
                            // or paginate backwards until we find them (only if we are close the end of the timeline).
                            should_continue_backwards_pagination = true;
                        }

                        // If the bottom of the timeline (the last event) is visible, then we should
                        // set the timeline to live mode.
                        // If the bottom of the timeline is *not* visible, then we should
                        // set the timeline to Focused mode.

                        // TODO: Save the event IDs of the top 3 items before we apply this update,
                        //       which indicates this timeline is in the process of being restored,
                        //       such that we can jump back to that position later after applying this update.

                        // TODO: here we need to re-build the timeline via TimelineBuilder
                        //       and set the TimelineFocus to one of the above-saved event IDs.

                        // TODO: the docs for `TimelineBuilder::with_focus()` claim that the timeline's focus mode
                        //       can be changed after creation, but I do not see any methods to actually do that.
                        //       <https://matrix-org.github.io/matrix-rust-sdk/matrix_sdk_ui/timeline/struct.TimelineBuilder.html#method.with_focus>
                        //
                        //       As such, we probably need to create a new async request enum variant
                        //       that tells the background async task to build a new timeline
                        //       (either in live mode or focused mode around one or more events)
                        //       and then replaces the existing timeline in ALL_ROOMS_INFO with the new one.
                    }

                    let prior_items_changed = clear_cache || changed_indices.start <= curr_first_id;

                    if new_items.len() == tl.items.len() {
                        // log!("process_timeline_updates(): no jump necessary for updated timeline of same length: {}", items.len());
                    }
                    else if curr_first_id > new_items.len() {
                        log!("process_timeline_updates(): jumping to bottom: curr_first_id {} is out of bounds for {} new items", curr_first_id, new_items.len());
                        portal_list.set_first_id_and_scroll(new_items.len().saturating_sub(1), 0.0);
                        portal_list.set_tail_range(true);
                        jump_to_bottom_button.update_visibility(cx, true);
                    }
                    // If the prior items changed, we need to find the new index of an item that was visible
                    // in the timeline viewport so that we can maintain the scroll position of that item,
                    // which ensures that the timeline doesn't jump around unexpectedly and ruin the user's experience.
                    else if let Some((curr_item_idx, new_item_idx, new_item_scroll, _event_id)) =
                        prior_items_changed.then(||
                            find_new_item_matching_current_item(cx, portal_list, curr_first_id, &tl.items, &new_items)
                        )
                        .flatten()
                    {
                        if curr_item_idx != new_item_idx {
                            log!("process_timeline_updates(): jumping view from event index {curr_item_idx} to new index {new_item_idx}, scroll {new_item_scroll}, event ID {_event_id}");
                            portal_list.set_first_id_and_scroll(new_item_idx, new_item_scroll);
                            tl.prev_first_index = Some(new_item_idx);
                            // Set scrolled_past_read_marker false when we jump to a new event
                            tl.scrolled_past_read_marker = false;
                            // Hide the tooltip when the timeline jumps, as a hover-out event won't occur.
                            cx.widget_action(ui,  RoomScreenTooltipActions::HoverOut);
                        }
                    }
                    //
                    // TODO: after an (un)ignore user event, all timelines are cleared. Handle that here.
                    //
                    else {
                        // warning!("!!! Couldn't find new event with matching ID for ANY event currently visible in the portal list");
                    }

                    // If new items were appended to the end of the timeline, show an unread messages badge on the jump to bottom button.
                    if is_append && !portal_list.is_at_end() {
                        // We only show unread message badges on the jump to bottom button for main room timelines,
                        // because the matrix SDK doesn't currently support querying unread message counts for threads.
                        if matches!(tl.kind, TimelineKind::MainRoom { .. }) {
                            // Immediately show the unread badge with no count while we fetch the actual count in the background.
                            jump_to_bottom_button.show_unread_message_badge(cx, UnreadMessageCount::Unknown);
                            submit_async_request(MatrixRequest::GetNumberUnreadMessages{
                                timeline_kind: tl.kind.clone(),
                            });
                        }
                    }

                    if !self.pending_invited_users.is_empty() {
                        let start = changed_indices.start.min(new_items.len());
                        let end = changed_indices.end.min(new_items.len());
                        let mut accepted_users: Vec<OwnedUserId> = Vec::new();
                        for idx in start..end {
                            let Some(new_item) = new_items.get(idx) else { continue };
                            let TimelineItemKind::Event(event_tl_item) = new_item.kind() else { continue };
                            let TimelineItemContent::MembershipChange(membership_change) = event_tl_item.content() else { continue };
                            let accepted = matches!(
                                membership_change.change(),
                                Some(MembershipChange::InvitationAccepted)
                                | Some(MembershipChange::Joined)
                            );
                            if accepted {
                                let invited_user_id = event_tl_item.sender().to_owned();
                                if self.pending_invited_users.contains(&invited_user_id) {
                                    accepted_users.push(invited_user_id);
                                }
                            }
                        }
                        for accepted_user in accepted_users {
                            self.pending_invited_users.remove(&accepted_user);
                            enqueue_popup_notification(
                                format!("{accepted_user} accepted the invite and joined."),
                                PopupKind::Success,
                                Some(4.0),
                            );
                        }
                    }

                    if prior_items_changed {
                        // If this RoomScreen is showing the loading pane and has an ongoing backwards pagination request,
                        // then we should update the status message in that loading pane
                        // and then continue paginating backwards until we find the target event.
                        // Note that we do this here because `clear_cache` will always be true if backwards pagination occurred.
                        let loading_pane = self.view.loading_pane(cx, ids!(loading_pane));
                        let mut loading_pane_state = loading_pane.take_state();
                        if let LoadingPaneState::BackwardsPaginateUntilEvent {
                            events_paginated, target_event_id, ..
                        } = &mut loading_pane_state {
                            *events_paginated += new_items.len().saturating_sub(tl.items.len());
                            log!("While finding target event {target_event_id}, we have now loaded {events_paginated} messages...");
                            // Here, we assume that we have not yet found the target event,
                            // so we need to continue paginating backwards.
                            // If the target event has already been found, it will be handled
                            // in the `TargetEventFound` match arm below, which will set
                            // `should_continue_backwards_pagination` to `false`.
                            // So either way, it's okay to set this to `true` here.
                            should_continue_backwards_pagination = true;
                        }
                        loading_pane.set_state(cx, loading_pane_state);
                    }

                    if clear_cache {
                        tl.content_drawn_since_last_update.clear();
                        tl.profile_drawn_since_last_update.clear();
                        tl.fully_paginated = false;
                    } else {
                        tl.content_drawn_since_last_update.remove(changed_indices.clone());
                        tl.profile_drawn_since_last_update.remove(changed_indices.clone());
                        // log!("process_timeline_updates(): changed_indices: {changed_indices:?}, items len: {}\ncontent drawn: {:#?}\nprofile drawn: {:#?}", items.len(), tl.content_drawn_since_last_update, tl.profile_drawn_since_last_update);
                    }

                    // --- MSC4357 streaming detection ---
                    if clear_cache {
                        let previous_streaming_messages = std::mem::take(&mut tl.streaming_messages);
                        let (rebuilt_streaming_messages, should_schedule_frame) =
                            rebuild_streaming_messages_for_full_snapshot(
                                streaming_candidates_from_items(&new_items),
                                Some(&previous_streaming_messages),
                            );
                        tl.streaming_messages = rebuilt_streaming_messages;
                        if should_schedule_frame {
                            self.streaming_next_frame = cx.new_next_frame();
                        }
                    } else if !new_items.is_empty() {
                        let mut should_schedule_frame = false;
                        let scan_range = streaming_scan_range(
                            clear_cache,
                            &changed_indices,
                            tl.items.len(),
                            new_items.len(),
                        );

                        let old_event_ids: HashSet<&EventId> = tl.items.iter()
                            .filter_map(|item| item_event_id(item))
                            .collect();

                        for idx in scan_range {
                            let Some(new_item) = new_items.get(idx) else { continue };
                            let TimelineItemKind::Event(new_evt) = new_item.kind() else { continue };
                            let Some(event_id) = new_evt.event_id().map(|id| id.to_owned()) else { continue };
                            let live = is_msc4357_live(new_evt);
                            let Some(new_text) = Self::extract_message_text(new_item) else { continue };
                            let render_full_target = should_render_streaming_full_snapshot(
                                &new_text,
                                new_evt.content()
                                    .as_message()
                                    .and_then(|message| match message.msgtype() {
                                        MessageType::Text(TextMessageEventContent { formatted, .. }) => formatted.as_ref(),
                                        MessageType::Notice(NoticeMessageEventContent { formatted, .. }) => formatted.as_ref(),
                                        _ => None,
                                    }),
                                is_timeline_sender_bot(
                                    new_evt.sender(),
                                    resolved_parent_bot_user_id.as_deref(),
                                    &room_bot_user_ids,
                                    &known_bot_user_ids,
                                ),
                            );

                            if let Some(state) = tl.streaming_messages.get_mut(&event_id) {
                                let should_invalidate_content = streaming_update_requires_content_invalidation(
                                    state,
                                    &new_text,
                                    live,
                                    render_full_target,
                                );
                                state.update_target(&new_text, live);
                                state.set_render_full_target(render_full_target);
                                if should_invalidate_content
                                    && let Some(idx) = state.timeline_index
                                {
                                    tl.content_drawn_since_last_update.remove(idx .. idx + 1);
                                }
                                // Schedule frame for animation OR for cleanup of just-completed state
                                should_schedule_frame |= state.needs_frame() || state.is_complete();
                                continue;
                            }

                            if live && !old_event_ids.contains(&*event_id) {
                                let mut state = StreamingAnimState::new(&new_text, true);
                                state.set_render_full_target(render_full_target);
                                should_schedule_frame |= state.needs_frame();
                                tl.streaming_messages.insert(event_id, state);
                            }
                        }

                        if should_schedule_frame {
                            self.streaming_next_frame = cx.new_next_frame();
                        }
                    }
                    // --- End streaming detection ---

                    tl.items = new_items;
                    refresh_stream_indices(
                        tl.items.iter().map(item_event_id),
                        &mut tl.streaming_messages,
                    );
                    done_loading = true;
                }
                TimelineUpdate::NewUnreadMessagesCount(unread_messages_count) => {
                    // We only show unread message badges on the jump to bottom button for main room timelines,
                    // because the matrix SDK doesn't currently support querying unread message counts for threads.
                    if matches!(tl.kind, TimelineKind::MainRoom { .. }) {
                        jump_to_bottom_button.show_unread_message_badge(cx, unread_messages_count);
                    }
                }
                TimelineUpdate::TargetEventFound { target_event_id, index } => {
                    // log!("Target event found in room {}: {target_event_id}, index: {index}", tl.kind.room_id());
                    tl.request_sender.send_if_modified(|requests| {
                        requests.retain(|r| &r.room_id != tl.kind.room_id());
                        // no need to notify/wake-up all receivers for a completed request
                        false
                    });

                    // sanity check: ensure the target event is in the timeline at the given `index`.
                    let item = tl.items.get(index);
                    let is_valid = item.is_some_and(|item|
                        item.as_event()
                            .is_some_and(|ev| ev.event_id() == Some(&target_event_id))
                    );
                    let loading_pane = self.view.loading_pane(cx, ids!(loading_pane));

                    // log!("TargetEventFound: is_valid? {is_valid}. room {}, event {target_event_id}, index {index} of {}\n  --> item: {item:?}", tl.kind.room_id(), tl.items.len());
                    if is_valid {
                        // We successfully found the target event, so we can close the loading pane,
                        // reset the loading panestate to `None`, and stop issuing backwards pagination requests.
                        loading_pane.set_status(cx, tr_key(self.app_language, "room_screen.loading.found_related_message"));
                        loading_pane.set_state(cx, LoadingPaneState::None);

                        // NOTE: this code was copied from the `MessageAction::JumpToRelated` handler;
                        //       we should deduplicate them at some point.
                        let speed = 50.0;
                        portal_list.smooth_scroll_to(cx, index, speed, None, 10.0);
                        // start highlight animation.
                        tl.message_highlight_animation_state = MessageHighlightAnimationState::Pending {
                            item_id: index
                        };
                    }
                    else {
                        // Here, the target event was not found in the current timeline,
                        // or we found it previously but it is no longer in the timeline (or has moved),
                        // which means we encountered an error and are unable to jump to the target event.
                        error!("Target event index {index} of {} is out of bounds for room {}", tl.items.len(), tl.kind.room_id());
                        // Show this error in the loading pane, which should already be open.
                        loading_pane.set_state(cx, LoadingPaneState::Error(
                            tr_key(self.app_language, "room_screen.loading.related_message_not_found").to_string()
                        ));
                    }

                    should_continue_backwards_pagination = false;

                    // redraw now before any other items get added to the timeline list.
                    self.view.redraw(cx);
                }
                TimelineUpdate::PaginationRunning(direction) => {
                    if direction == PaginationDirection::Backwards {
                        tl.backwards_pagination_in_flight = true;
                        top_space.set_visible(cx, true);
                        done_loading = false;
                    } else {
                        error!("Unexpected PaginationRunning update in the Forwards direction");
                    }
                }
                TimelineUpdate::PaginationError { error, direction } => {
                    if direction == PaginationDirection::Backwards {
                        tl.backwards_pagination_in_flight = false;
                    }
                    error!("Pagination error ({direction}) in {:?}: {error:?}", self.room_name_id);
                    let room_name = self.room_name_id.as_ref().map(|r| r.to_string());
                    enqueue_popup_notification(
                        utils::stringify_pagination_error(
                            &error,
                            room_name
                                .as_deref()
                                .unwrap_or(tr_key(self.app_language, "room_screen.fallback.unnamed_room")),
                        ),
                        PopupKind::Error,
                        Some(10.0),
                    );
                    done_loading = true;
                }
                TimelineUpdate::PaginationIdle { fully_paginated, direction } => {
                    if direction == PaginationDirection::Backwards {
                        tl.backwards_pagination_in_flight = false;
                        // Don't set `done_loading` to `true` here, because we want to keep the top space visible
                        // (with the "loading" message) until the corresponding `NewItems` update is received.
                        tl.fully_paginated = fully_paginated;
                        if fully_paginated {
                            done_loading = true;
                        }
                    } else {
                        error!("Unexpected PaginationIdle update in the Forwards direction");
                    }
                }
                TimelineUpdate::EventDetailsFetched {event_id, result } => {
                    if let Err(_e) = result {
                        error!("Failed to fetch details fetched for event {event_id} in room {}. Error: {_e:?}", tl.kind.room_id());
                    }
                    // Here, to be most efficient, we could redraw only the updated event,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::ThreadSummaryDetailsFetched {
                    thread_root_event_id,
                    timeline_item_index,
                    num_replies,
                    latest_reply_preview_text,
                } => {
                    tl.pending_thread_summary_fetches.remove(&thread_root_event_id);
                    tl.fetched_thread_summaries.insert(
                        thread_root_event_id.clone(),
                        FetchedThreadSummary {
                            num_replies,
                            latest_reply_preview_text,
                        },
                    );
                    let event_id_matches_at_index = tl.items
                        .get(timeline_item_index)
                        .and_then(|item| item.as_event())
                        .and_then(|ev| ev.event_id())
                        .is_some_and(|id| id == thread_root_event_id);
                    if event_id_matches_at_index {
                        tl.content_drawn_since_last_update
                            .remove(timeline_item_index .. timeline_item_index + 1);
                    } else {
                        tl.content_drawn_since_last_update.clear();
                    }
                }
                TimelineUpdate::RoomMembersSynced => {
                    tl.awaiting_post_sync_member_refresh = true;
                    submit_async_request(MatrixRequest::GetRoomMembers {
                        timeline_kind: tl.kind.clone(),
                        memberships: matrix_sdk::RoomMemberships::JOIN,
                        local_only: true,
                    });
                }
                TimelineUpdate::RoomMembersListFetched { members } => {
                    let members = Arc::new(members);
                    if tl.awaiting_post_sync_member_refresh {
                        tl.room_members_sync_pending = false;
                        tl.awaiting_post_sync_member_refresh = false;
                    }
                    // Invalidate old sort before replacing members to prevent
                    // stale sort + new members mismatch (index out of bounds).
                    tl.room_members_sort = None;
                    tl.room_members = Some(Arc::clone(&members));
                    // Compute new sort in background thread
                    crate::cpu_worker::spawn_cpu_job(cx, crate::cpu_worker::CpuJob::PrecomputeMemberSort(
                        crate::cpu_worker::PrecomputeMemberSortJob {
                            timeline_kind: tl.kind.clone(),
                            members,
                        }
                    ));
                },
                TimelineUpdate::MediaFetched(request) => {
                    log!("process_timeline_updates(): media fetched for room {}", tl.kind.room_id());
                    // Set Image to image viewer modal if the media is not a thumbnail.
                    if let (MediaFormat::File, media_source) = (request.format, request.source) {
                        populate_matrix_image_modal(cx, media_source, &mut tl.media_cache);
                    }
                    // Here, to be most efficient, we could redraw only the media items in the timeline,
                    // but for now we just fall through and let the final `redraw()` call re-draw the whole timeline view.
                }
                TimelineUpdate::MessageEdited { timeline_event_item_id: timeline_event_id, result } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .handle_edit_result(cx, timeline_event_id, result);
                }
                TimelineUpdate::PinResult { result, pin, .. } => {
                    let (message, auto_dismissal_duration, kind) = match &result {
                        Ok(true) => (
                            if pin {
                                tr_key(self.app_language, "room_screen.popup.pin.pinned_success").to_string()
                            } else {
                                tr_key(self.app_language, "room_screen.popup.pin.unpinned_success").to_string()
                            },
                            Some(4.0),
                            PopupKind::Success
                        ),
                        Ok(false) => (
                            if pin {
                                tr_key(self.app_language, "room_screen.popup.pin.already_pinned").to_string()
                            } else {
                                tr_key(self.app_language, "room_screen.popup.pin.already_unpinned").to_string()
                            },
                            Some(4.0),
                            PopupKind::Info
                        ),
                        Err(e) => (
                            tr_fmt(self.app_language, if pin {
                                "room_screen.popup.pin.pin_failed"
                            } else {
                                "room_screen.popup.pin.unpin_failed"
                            }, &[("error", &e.to_string())]),
                            None,
                            PopupKind::Error
                        ),
                    };
                    enqueue_popup_notification(message, kind, auto_dismissal_duration);
                }
                TimelineUpdate::TypingUsers { users } => {
                    // This update loop should be kept tight & fast, so all we do here is
                    // save the list of typing users for future use after the loop exits.
                    // Then, we "process" it later (by turning it into a string) after the
                    // update loop has completed, which avoids unnecessary expensive work
                    // if the list of typing users gets updated many times in a row.

                    typing_users = Some(users);
                }
                TimelineUpdate::PinnedEvents(pinned_events) => {
                    self.pinned_events = pinned_events;
                    // We need to redraw any events that might have been pinned or unpinned
                    // in order to have all events properly reflect their pinned state.
                    // However, it's intractable to find exactly which events in the timeline
                    // had a change in their pinned state, so we just clear all draw caches.
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                }
                TimelineUpdate::UserPowerLevels(user_power_levels) => {
                    tl.user_power = user_power_levels;
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .update_user_power_levels(cx, user_power_levels);
                    // Update the @room mention capability based on the user's power level
                    cx.action(MentionableTextInputAction::PowerLevelsUpdated {
                        room_id: tl.kind.room_id().clone(),
                        can_notify_room: user_power_levels.can_notify_room(),
                    });
                    // We need to redraw all events in order to reflect the new power levels,
                    // e.g., for the message context menu to be correctly populated.
                    tl.content_drawn_since_last_update.clear();
                    tl.profile_drawn_since_last_update.clear();
                }
                TimelineUpdate::OwnUserReadReceipt(receipt) => {
                    tl.latest_own_user_receipt = Some(receipt);
                }
                TimelineUpdate::Tombstoned(successor_room_details) => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .update_tombstone_footer(cx, tl.kind.room_id(), Some(&successor_room_details));
                    tl.tombstone_info = Some(successor_room_details);
                }
                TimelineUpdate::LinkPreviewFetched => {}
                TimelineUpdate::FileUploadConfirmed(file_data) => {
                    let room_input_bar = self.view.room_input_bar(cx, ids!(room_input_bar));
                    if let Some(replied_to) = room_input_bar.handle_file_upload_confirmed(cx, &file_data.name) {
                        submit_async_request(MatrixRequest::SendAttachment {
                            timeline_kind: tl.kind.clone(),
                            file_data,
                            replied_to,
                            #[cfg(feature = "tsp")]
                            sign_with_tsp: room_input_bar.is_tsp_signing_enabled(cx),
                        });
                    }
                }
                TimelineUpdate::FileUploadUpdate { current, total } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .set_upload_progress(cx, current, total);
                }
                TimelineUpdate::FileUploadAbortHandle(handle) => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .set_upload_abort_handle(handle);
                }
                TimelineUpdate::FileUploadError { error, file_data } => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .show_upload_error(cx, &error, file_data);
                }
                TimelineUpdate::FileUploadComplete => {
                    self.view.room_input_bar(cx, ids!(room_input_bar))
                        .hide_upload_progress(cx);
                }
            }
        }

        if should_continue_backwards_pagination {
            tl.backwards_pagination_in_flight = true;
            submit_async_request(MatrixRequest::PaginateTimeline {
                timeline_kind: tl.kind.clone(),
                num_events: VIEWPORT_FILL_PAGINATION_SIZE,
                direction: PaginationDirection::Backwards,
            });
        }

        if done_loading {
            top_space.set_visible(cx, false);
        }

        if let Some(users) = typing_users {
            self.view
                .typing_notice(cx, ids!(typing_notice))
                .show_or_hide(cx, &users);
        }

        if num_updates > 0 {
            self.schedule_stream_timeout(cx);
            // log!("Applied {} timeline updates for room {}, redrawing with {} items...", num_updates, tl.kind.room_id(), tl.items.len());
            self.redraw(cx);
        }
    }


    /// Handles a link being clicked in any child widgets of this RoomScreen.
    ///
    /// Returns `true` if the given `action` was handled as a link click.
    fn handle_link_clicked(
        &mut self,
        cx: &mut Cx,
        action: &Action,
        pane: &UserProfileSlidingPaneRef,
    ) -> bool {
        // A closure that handles both MatrixToUri and MatrixUri links,
        // and returns whether the link was handled.
        let mut handle_matrix_link = |id: &MatrixId, _via: &[OwnedServerName]| -> bool {
            match id {
                MatrixId::User(user_id) => {
                    let Some(room_name_id) = self.room_name_id.as_ref() else {
                        return false;
                    };
                    // There is no synchronous way to get the user's full profile info
                    // including the details of their room membership,
                    // so we fill in with the details we *do* know currently,
                    // show the UserProfileSlidingPane, and then after that,
                    // the UserProfileSlidingPane itself will fire off
                    // an async request to get the rest of the details.
                    self.show_user_profile(
                        cx,
                        pane,
                        UserProfilePaneInfo {
                            profile_and_room_id: UserProfileAndRoomId {
                                user_profile: UserProfile {
                                    user_id: user_id.to_owned(),
                                    username: None,
                                    avatar_state: AvatarState::Unknown,
                                },
                                room_id: room_name_id.room_id().clone(),
                            },
                            room_name: room_name_id.to_string(),
                            // TODO: use the extra `via` parameters
                            room_member: None,
                        },
                    );
                    true
                }
                MatrixId::Room(room_id) => {
                    if self.room_name_id.as_ref().is_some_and(|r| r.room_id() == room_id) {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.already_viewing_room"),
                            PopupKind::Info,
                            Some(4.0),
                        );
                        return true;
                    }
                    if let Some(room_name_id) = cx.get_global::<RoomsListRef>().get_room_name(room_id) {
                        cx.action(AppStateAction::NavigateToRoom {
                            room_to_close: None,
                            destination_room: BasicRoomDetails::Name(room_name_id),
                        });
                        return true;
                    } else {
                        log!("TODO: fetch and display room preview for room {}", room_id);
                    }
                    false
                }
                MatrixId::RoomAlias(room_alias) => {
                    log!("TODO: open room alias {}", room_alias);
                    // TODO: open a room loading screen that shows a spinner
                    //       while our background async task calls Client::resolve_room_alias()
                    //       and then either jumps to the room if known, or fetches and displays
                    //       a room preview for that room.
                    false
                }
                MatrixId::Event(room_id, event_id) => {
                    log!("TODO: open event {} in room {}", event_id, room_id);
                    // TODO: this requires the same first step as the `MatrixId::Room` case above,
                    //       but then we need to call Room::event_with_context() to get the event
                    //       and its context (surrounding events ?).
                    false
                }
                _ => false,
            }
        };

        if let HtmlLinkAction::Clicked { url, .. } = action.as_widget_action().cast() {
            // Handle mxc:// links (file downloads from Matrix media server)
            if url.starts_with("mxc://") {
                let mxc_uri = OwnedMxcUri::from(url.clone());
                self.handle_mxc_file_download(cx, mxc_uri);
                return true;
            }

            let mut link_was_handled = false;
            if let Ok(matrix_to_uri) = MatrixToUri::parse(&url) {
                link_was_handled |= handle_matrix_link(matrix_to_uri.id(), matrix_to_uri.via());
            }
            else if let Ok(matrix_uri) = MatrixUri::parse(&url) {
                link_was_handled |= handle_matrix_link(matrix_uri.id(), matrix_uri.via());
            }

            if !link_was_handled {
                log!("Opening URL \"{}\"", url);
                if let Err(e) = robius_open::Uri::new(&url).open() {
                    error!("Failed to open URL {:?}. Error: {:?}", url, e);
                    enqueue_popup_notification(
                        tr_fmt(self.app_language, "room_screen.popup.open_url_failed", &[("url", url.as_str())]),
                        PopupKind::Error,
                        Some(10.0),
                    );
                }
            }
            true
        }
        else if let RobrixHtmlLinkAction::ClickedMatrixLink { url, matrix_id, via, .. } = action.as_widget_action().cast() {
            let link_was_handled = handle_matrix_link(&matrix_id, &via);
            if !link_was_handled {
                log!("Opening URL \"{}\"", url);
                if let Err(e) = robius_open::Uri::new(&url).open() {
                    error!("Failed to open URL {:?}. Error: {:?}", url, e);
                    enqueue_popup_notification(
                        tr_fmt(self.app_language, "room_screen.popup.open_url_failed", &[("url", url.as_str())]),
                        PopupKind::Error,
                        Some(10.0),
                    );
                }
            }
            true
        }
        else {
            false
        }
    }

    /// Handles an mxc:// file download link click.
    /// Fetches the file from the Matrix media server, saves it with a unique name,
    /// and opens it with the system default application.
    fn handle_mxc_file_download(&mut self, _cx: &mut Cx, mxc_uri: OwnedMxcUri) {
        log!("handle_mxc_file_download: mxc_uri={mxc_uri}");

        enqueue_popup_notification(
            tr_key(self.app_language, "room_screen.file.downloading").to_string(),
            PopupKind::Info,
            Some(3.0),
        );

        // Download directly using the Matrix client (bypasses MediaCache to avoid
        // header parsing issues with non-ASCII Content-Disposition headers).
        let app_language = self.app_language;
        submit_async_request(MatrixRequest::DownloadAndSaveFile {
            mxc_uri,
            app_language,
        });
    }

    /// Handles image clicks in message content by opening the image viewer.
    fn handle_image_click(
        &mut self,
        cx: &mut Cx,
        mxc_uri: Option<MediaSource>,
        texture: Option<Texture>,
        item_id: usize,
    ) {
        let Some(media_source) = mxc_uri else {
            return;
        };
        let Some(tl_state) = self.tl_state.as_mut() else { return };
        let Some(event_tl_item) = tl_state.items.get(item_id).and_then(|item| item.as_event()) else { return };

        let timestamp_millis = event_tl_item.timestamp();
        let (image_name, image_file_size) = get_image_name_and_filesize(event_tl_item);
        cx.action(ImageViewerAction::Show(LoadState::Loading(
            texture.clone(),
            Some(ImageViewerMetaData {
                image_name,
                image_file_size,
                timestamp: unix_time_millis_to_datetime(timestamp_millis),
                avatar_parameter: Some((
                    tl_state.kind.clone(),
                    event_tl_item.clone(),
                )),
            }),
        )));

        populate_matrix_image_modal(cx, media_source, &mut tl_state.media_cache);
    }

    /// Looks up the event specified by the given message details in the given timeline.
    ///
    /// This will first try an instant index-based lookup via `details.item_id`,
    /// and then fall back to searching the timeline in reverse for the `details.event_id`
    /// if the index is "stale", meaning the timeline items have changed (e.g., due to pagination)
    /// since the message context menu was opened or the `MessageAction` was received by the `RoomScreen`.
    ///
    /// We search in reverse because it is far more likely that the user is interacting
    /// with an event that is close to the end of the timeline.
    fn find_event_in_timeline<'a>(
        items: &'a Vector<Arc<TimelineItem>>,
        details: &MessageDetails,
    ) -> Option<&'a EventTimelineItem> {
        let target_event_id = details.event_id()?;
        if let Some(event) = items.get(details.item_id)
            .and_then(|item| item.as_event())
            .filter(|ev| ev.event_id().is_some_and(|id| id == target_event_id))
        {
            return Some(event);
        }
        items.iter()
            .rev()
            .take(MAX_ITEMS_TO_SEARCH_THROUGH)
            .filter_map(|item| item.as_event())
            .find(|ev| ev.event_id().is_some_and(|id| id == target_event_id))
    }

    /// Handles any [`MessageAction`]s received by this RoomScreen.
    fn handle_message_actions(
        &mut self,
        cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
        loading_pane: &LoadingPaneRef,
    ) {
        if let Some(clicked_context) = self.octos_action_button_contexts
            .iter()
            .find_map(|(widget_uid, context)| {
                actions.find_widget_action(*widget_uid)
                    .and_then(|item| matches!(item.cast(), ButtonAction::Clicked(_)).then(|| context.clone()))
            })
        {
            if !are_action_buttons_disabled(
                &self.disabled_octos_action_source_event_ids,
                clicked_context.source_event_id.as_ref(),
            ) {
                let Some(tl) = self.tl_state.as_ref() else { return };
                let request = match &clicked_context.request {
                    OctosActionButtonRequest::Generic { action_id, label, .. } => build_octos_action_response_request(
                        &tl.kind,
                        label,
                        action_id,
                        clicked_context.source_event_id.as_ref(),
                        clicked_context.original_sender.as_ref(),
                    ),
                    OctosActionButtonRequest::Approval { request_id, title, decision, tool_args_digest, .. } => build_octos_approval_response_request(
                        &tl.kind,
                        title,
                        request_id,
                        decision,
                        tool_args_digest,
                        clicked_context.source_event_id.as_ref(),
                        clicked_context.original_sender.as_ref(),
                    ),
                };
                mark_action_buttons_disabled(
                    &mut self.disabled_octos_action_source_event_ids,
                    &clicked_context.source_event_id,
                );
                mark_selected_octos_action(
                    &mut self.selected_octos_action_by_source_event_id,
                    &clicked_context.source_event_id,
                    clicked_context.request.action_id(),
                    clicked_context.request.label(),
                    clicked_context.request.style(),
                );
                self.redraw_timeline_list(cx);
                submit_async_request(MatrixRequest::SendActionResponse {
                    timeline_kind: request.timeline_kind,
                    content: request.content,
                    target_user_id: request.target_user_id,
                    explicit_room: request.explicit_room,
                    source_event_id: request.source_event_id,
                });
            }
            return;
        }

        let room_screen_widget_uid = self.widget_uid();
        for action in actions {
            match action.as_widget_action().widget_uid_eq(room_screen_widget_uid).cast_ref() {
                MessageAction::React { details, reaction } => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    submit_async_request(MatrixRequest::ToggleReaction {
                        timeline_kind: tl.kind.clone(),
                        timeline_event_id: details.timeline_event_id.clone(),
                        reaction: reaction.clone(),
                    });
                }
                MessageAction::Reply(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details).cloned() {
                        let replied_to_info = EmbeddedEvent::from_timeline_item(&event_tl_item);
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_replying_to(cx, (event_tl_item, replied_to_info), &tl.kind);
                    }
                    else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.reply_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::Reply: couldn't find event [{}] {:?} to reply to in room {:?}",
                            details.item_id,
                            details.timeline_event_id,
                            self.room_id(),
                        );
                    }
                }
                MessageAction::Edit(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_editing_pane(
                                cx,
                                event_tl_item.clone(),
                                tl.kind.clone(),
                            );
                    }
                    else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.edit_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::Edit: couldn't find event [{}] {:?} to edit in room {:?}",
                            details.item_id,
                            details.timeline_event_id,
                            self.room_id(),
                        );
                    }
                }
                MessageAction::EditLatest => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(latest_sent_msg) = tl.items
                        .iter()
                        .rev()
                        .take(MAX_ITEMS_TO_SEARCH_THROUGH)
                        .find_map(|item| item.as_event().filter(|ev| ev.is_editable()).cloned())
                    {
                        self.view.room_input_bar(cx, ids!(room_input_bar))
                            .show_editing_pane(
                                cx,
                                latest_sent_msg,
                                tl.kind.clone(),
                            );
                    }
                    else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.no_recent_editable"),
                            PopupKind::Warning,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::MessageSubmittedLocally => {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    let last_item_idx = tl.items.len().saturating_sub(1);
                    portal_list.set_first_id_and_scroll(last_item_idx, 0.0);
                    portal_list.set_tail_range(true);
                    self.jump_to_bottom_button(cx, ids!(jump_to_bottom_button))
                        .update_visibility(cx, true);
                    self.redraw(cx);
                }
                MessageAction::Pin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        submit_async_request(MatrixRequest::PinEvent {
                            timeline_kind: tl.kind.clone(),
                            event_id: event_id.clone(),
                            pin: true,
                        });
                    } else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.cannot_pin"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::Unpin(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        submit_async_request(MatrixRequest::PinEvent {
                            timeline_kind: tl.kind.clone(),
                            event_id: event_id.clone(),
                            pin: false,
                        });
                    } else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.cannot_unpin"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                    }
                }
                MessageAction::CopyText(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
                        cx.copy_to_clipboard(&plaintext_body_of_timeline_item(event_tl_item));
                    }
                    else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.copy_text_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyText: couldn't find event [{}] {:?} to copy text from in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::CopyHtml(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    // The logic for getting the formatted body of a message is the same
                    // as the logic used in `populate_message_view()`.
                    let mut success = false;
                    if let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) {
                        if let Some(message) = event_tl_item.content().as_message() {
                            match message.msgtype() {
                                MessageType::Text(TextMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Notice(NoticeMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Emote(EmoteMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Image(ImageMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::File(FileMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Audio(AudioMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::Video(VideoMessageEventContent { formatted: Some(FormattedBody { body, .. }), .. })
                                | MessageType::VerificationRequest(KeyVerificationRequestEventContent { formatted: Some(FormattedBody { body, .. }), .. }) =>
                                {
                                    cx.copy_to_clipboard(body);
                                    success = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    if !success {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.copy_html_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyHtml: couldn't find event [{}] {:?} to copy HTML from in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::CopyLink(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    if let Some(event_id) = details.event_id() {
                        let matrix_to_uri = tl.kind.room_id().matrix_to_event_uri(event_id.clone());
                        cx.copy_to_clipboard(&matrix_to_uri.to_string());
                    } else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.copy_link_failed"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        error!("MessageAction::CopyLink: no `event_id`: [{}] {:?} in room {}",
                            details.item_id,
                            details.timeline_event_id,
                            tl.kind.room_id(),
                        );
                    }
                }
                MessageAction::ViewSource(details) => {
                    let Some(tl) = self.tl_state.as_ref() else { continue };
                    let Some(event_tl_item) = Self::find_event_in_timeline(&tl.items, details) else {
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.view_source_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        continue;
                    };
                    // Get the original JSON from the event and pretty-print it
                    let original_json: Option<String> = event_tl_item
                        .original_json()
                        .and_then(|raw_event| serde_json::to_value(raw_event).ok())
                        .and_then(|value| serde_json::to_string_pretty(&value).ok());

                    let event_id = event_tl_item.event_id().map(|e| e.to_owned());

                    cx.action(super::event_source_modal::EventSourceModalAction::Open {
                        room_id: tl.kind.room_id().clone(),
                        event_id,
                        original_json,
                    });
                }
                MessageAction::JumpToRelated(details) => {
                    let Some(related_event_id) = details.related_event_id.as_ref() else {
                        error!("BUG: MessageAction::JumpToRelated had no related event ID.\n{details:#?}");
                        enqueue_popup_notification(
                            tr_key(self.app_language, "room_screen.popup.message.related_not_found"),
                            PopupKind::Error,
                            Some(5.0),
                        );
                        continue;
                    };
                    self.jump_to_event(
                        cx,
                        related_event_id,
                        Some(details.item_id),
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::JumpToEvent(event_id) => {
                    self.jump_to_event(
                        cx,
                        event_id,
                        None,
                        portal_list,
                        loading_pane
                    );
                }
                MessageAction::OpenThread(thread_root_event_id) => {
                    let Some(room_name_id) = self.room_name_id.as_ref().cloned() else {
                        error!("### ERROR: MessageAction::OpenThread: thread_root_event_id: {thread_root_event_id}, but room_name_id was None!");
                        continue
                    };
                    cx.widget_action(
                        room_screen_widget_uid, 
                        RoomsListAction::Selected(SelectedRoom::Thread {
                            room_name_id,
                            thread_root_event_id: thread_root_event_id.clone(),
                        }),
                    );
                }
                MessageAction::ShowThreadsPane => {
                    self.show_threads_pane(cx);
                }
                MessageAction::ShowRoomInfoPane => {
                    self.show_room_info_pane(cx);
                }
                MessageAction::ToggleTranslationLangPopup { button_rect } => {
                    self.toggle_translation_lang_popup(cx, *button_rect);
                }
                MessageAction::Redact { details, reason } => {
                    let Some(tl) = self.tl_state.as_ref() else { return };
                    let timeline_event_id = details.timeline_event_id.clone();
                    let timeline_kind = tl.kind.clone();
                    let reason = reason.clone();
                    let app_language = self.app_language;
                    let content = ConfirmationModalContent {
                        title_text: tr_key(app_language, "room_screen.modal.delete_message.title").into(),
                        body_text: tr_key(app_language, "room_screen.modal.delete_message.body").into(),
                        accept_button_text: Some(tr_key(app_language, "room_screen.modal.delete_message.accept").into()),
                        on_accept_clicked: Some(Box::new(move |_cx| {
                            submit_async_request(MatrixRequest::RedactMessage {
                                timeline_kind,
                                timeline_event_id,
                                reason,
                            });
                        })),
                        ..Default::default()
                    };
                    cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
                }
                // MessageAction::Report(details) => {
                //     // TODO
                // }

                // This is handled within the Message widget itself.
                MessageAction::HighlightMessage(..) => { }
                // This is handled by the top-level App itself.
                MessageAction::OpenMessageContextMenu { .. } => { }
                // This isn't yet handled, as we need to completely redesign it.
                MessageAction::ActionBarOpen { .. } => { }
                // This isn't yet handled, as we need to completely redesign it.
                MessageAction::ActionBarClose => { }
                MessageAction::ToggleAppServiceActions => { }
                MessageAction::None => { }
            }
        }
    }

    fn toggle_translation_lang_popup(&mut self, cx: &mut Cx, button_rect: Rect) {
        let translation_lang_modal = self.view.modal(cx, ids!(translation_lang_modal));
        if translation_lang_modal.is_open() {
            translation_lang_modal.close(cx);
            return;
        }

        let room_screen_rect = self.view.area().clipped_rect(cx);
        let popup_abs_pos = compute_translation_lang_popup_abs_pos(button_rect, room_screen_rect);
        self.sync_translation_lang_popup(cx);
        log!(
            "Translation popup: button_rect={button_rect:?}, room_screen_rect={room_screen_rect:?}, popup_abs_pos={popup_abs_pos:?}"
        );
        if let Some(mut translation_lang_popup) = self
            .view
            .view(cx, ids!(translation_lang_modal.content.translation_lang_popup))
            .borrow_mut()
        {
            translation_lang_popup.walk.abs_pos = Some(popup_abs_pos);
            translation_lang_popup.walk.margin.left = 0.0;
            translation_lang_popup.walk.margin.top = 0.0;
            translation_lang_popup.walk.margin.right = 0.0;
            translation_lang_popup.walk.margin.bottom = 0.0;
        }
        translation_lang_modal.open(cx);
    }

    fn handle_translation_lang_popup_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let translation_lang_modal = self.view.modal(cx, ids!(translation_lang_modal));
        if !translation_lang_modal.is_open() {
            return;
        }

        let lang_ids: &[(&str, &[LiveId])] = &[
            ("en", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_en)]),
            ("zh", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_zh)]),
            ("zh-TW", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_zh_tw)]),
            ("ja", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_ja)]),
            ("ko", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_ko)]),
            ("es", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_es)]),
            ("fr", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_fr)]),
            ("de", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_de)]),
            ("ru", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_ru)]),
            ("pt", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_pt)]),
            ("ar", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_ar)]),
            ("vi", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_vi)]),
            ("th", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_th)]),
            ("id", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_id)]),
            ("ms", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_ms)]),
            ("tr", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_tr)]),
            ("hi", &[live_id!(translation_lang_modal), live_id!(content), live_id!(translation_lang_popup), live_id!(translation_lang_scroll), live_id!(lang_hi)]),
        ];
        for &(code, id_path) in lang_ids {
            if self.button(cx, id_path).clicked(actions) {
                self.view.room_input_bar(cx, ids!(room_input_bar)).activate_translation_language(cx, code);
                translation_lang_modal.close(cx);
                break;
            }
        }
    }

    /// Jumps to the target event ID in this timeline by smooth scrolling to it.
    ///
    /// This function searches backwards from the given `max_tl_idx` in the timeline
    /// for the given `event_id`. If found, it smooth-scrolls the portal list to that event.
    /// If not found, it displays the loading pane and starts a background search for the event.
    fn jump_to_event(
        &mut self,
        cx: &mut Cx,
        target_event_id: &OwnedEventId,
        max_tl_idx: Option<usize>,
        portal_list: &PortalListRef,
        loading_pane: &LoadingPaneRef,
    ) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        let max_tl_idx = max_tl_idx.unwrap_or_else(|| tl.items.len());

        // Attempt to find the index of replied-to message in the timeline.
        // Start from the current item's index (`tl_idx`) and search backwards,
        // since we know the related message must come before the current item.
        let mut num_items_searched = 0;
        let related_msg_tl_index = tl.items
            .focus()
            .narrow(..max_tl_idx)
            .into_iter()
            .rev()
            .take(MAX_ITEMS_TO_SEARCH_THROUGH)
            .position(|i| {
                num_items_searched += 1;
                i.as_event()
                    .and_then(|e| e.event_id())
                    .is_some_and(|ev_id| ev_id == target_event_id)
            })
            .map(|position| max_tl_idx.saturating_sub(position).saturating_sub(1));

        if let Some(index) = related_msg_tl_index {
            // log!("The related message {replied_to_event} was immediately found in room {}, scrolling to from index {reply_message_item_id} --> {index} (first ID {}).", tl.kind.room_id(), portal_list.first_id());
            let speed = 50.0;
            portal_list.smooth_scroll_to(cx, index, speed, None, 10.0);
            // start highlight animation.
            tl.message_highlight_animation_state = MessageHighlightAnimationState::Pending {
                item_id: index
            };
        } else {
            log!("The related event {target_event_id} wasn't immediately available in room {}, searching for it in the background...", tl.kind.room_id());
            // Here, we set the state of the loading pane and display it to the user.
            // The main logic will be handled in `process_timeline_updates()`, which is the only
            // place where we can receive updates to the timeline from the background tasks.
            loading_pane.set_state(
                cx,
                LoadingPaneState::BackwardsPaginateUntilEvent {
                    target_event_id: target_event_id.clone(),
                    events_paginated: 0,
                    request_sender: tl.request_sender.clone(),
                },
            );
            loading_pane.show(cx);

            tl.request_sender.send_if_modified(|requests| {
                if let Some(existing) = requests.iter_mut().find(|r| &r.room_id == tl.kind.room_id()) {
                    warning!("Unexpected: room {} already had an existing timeline request in progress, event: {:?}", tl.kind.room_id(), existing.target_event_id);
                    // We might as well re-use this existing request...
                    existing.target_event_id = target_event_id.clone();
                } else {
                    requests.push(BackwardsPaginateUntilEventRequest {
                        room_id: tl.kind.room_id().clone(),
                        target_event_id: target_event_id.clone(),
                        // avoid re-searching through items we already searched through.
                        starting_index: max_tl_idx.saturating_sub(num_items_searched),
                        current_tl_len: tl.items.len(),
                    });
                }
                true
            });

            // Don't unconditionally start backwards pagination here, because we want to give the
            // background `timeline_subscriber_handler` task a chance to process the request first
            // and search our locally-known timeline history for the replied-to message.
        }
        self.redraw(cx);
    }

    /// Shows the user profile sliding pane with the given avatar info.
    fn show_user_profile(
        &mut self,
        cx: &mut Cx,
        pane: &UserProfileSlidingPaneRef,
        info: UserProfilePaneInfo,
    ) {
        pane.set_info(cx, info);
        pane.show(cx);
        self.redraw(cx);
    }

    fn show_threads_pane(&mut self, cx: &mut Cx) {
        self.hide_room_info_pane(cx);
        self.ensure_threads_state_for_current_room();
        if !self.threads_pane_state.initialized && !self.threads_pane_state.is_loading {
            self.request_more_threads(cx, false);
        }
        self.refresh_threads_pane(cx);
        self.threads_sliding_pane(cx, ids!(threads_sliding_pane)).show(cx);
        self.redraw(cx);
    }

    fn refresh_threads_pane(&mut self, cx: &mut Cx) {
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        self.threads_sliding_pane(cx, ids!(threads_sliding_pane)).set_info(
            cx,
            ThreadsPaneInfo {
                room_name: room_name_id.to_string(),
                entries: self.threads_pane_state.entries.iter()
                    .map(|entry| ThreadsPaneEntryInfo {
                        thread_root_event_id: entry.thread_root_event_id.clone(),
                        title: entry.title.clone(),
                        subtitle: match entry.reply_count {
                            1 => String::from("1 reply"),
                            n => format!("{n} replies"),
                        },
                        time: utils::relative_format(entry.timestamp)
                            .unwrap_or_else(|| String::from("")),
                        preview: entry.latest_reply_preview.clone().unwrap_or_else(|| String::from("Tap to open thread")),
                    })
                    .collect(),
                status_text: self.threads_pane_state.status_text.clone(),
                show_entries: !self.threads_pane_state.entries.is_empty(),
                loading_text: if self.threads_pane_state.entries.is_empty() {
                    String::from("Loading threads...")
                } else {
                    String::from("Loading more threads...")
                },
                show_loading: self.threads_pane_state.is_loading,
            },
        );
    }

    fn hide_threads_pane(&mut self, cx: &mut Cx) {
        self.threads_sliding_pane(cx, ids!(threads_sliding_pane)).hide(cx);
    }

    fn refresh_room_info_pane(&mut self, cx: &mut Cx) {
        let Some(room_id) = self.room_id().cloned() else { return };
        let room_name = self.room_name_id.as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| room_id.to_string());
        let room_avatar_fallback_text = self.room_name_id.as_ref()
            .and_then(|room_name_id| room_name_id.name_for_avatar().map(ToOwned::to_owned))
            .unwrap_or_else(|| String::from("?"));
        let room_avatar_uri = self.room_avatar_url.clone();
        let (topic, visibility, encryption) = get_client()
            .and_then(|client| client.get_room(&room_id))
            .map(|room| {
                let topic = room.topic()
                    .unwrap_or_else(|| String::from("No topic"));
                let visibility = match room.is_public() {
                    Some(true) => String::from("Public room"),
                    Some(false) => String::from("Private room"),
                    None => String::from("Unknown"),
                };
                let encryption_state = room.encryption_state();
                let encryption = if encryption_state.is_unknown() {
                    String::from("Unknown")
                } else if encryption_state.is_encrypted() {
                    String::from("Encrypted")
                } else {
                    String::from("Unencrypted")
                };
                (topic, visibility, encryption)
            })
            .unwrap_or_else(|| (
                String::from("No topic"),
                String::from("Unknown"),
                String::from("Unknown"),
            ));

        let (people_entries, people_count_text, show_people_loading) = self.tl_state.as_ref()
            .map(|tl| {
                let Some(room_members) = tl.room_members.as_ref() else {
                    return (
                        Vec::new(),
                        String::from("People"),
                        true,
                    );
                };

                let mut people_entries: Vec<RoomInfoPeopleEntryInfo> = room_members.iter()
                    .map(|member| {
                        let display_name = member.display_name()
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| member.user_id().to_string());
                        let is_bot = is_likely_bot_member(member, None);
                        let level = match member.suggested_role_for_power_level() {
                            RoomMemberRole::Creator => String::from("Creator"),
                            RoomMemberRole::Administrator => String::from("Admin"),
                            RoomMemberRole::Moderator => String::from("Moderator"),
                            RoomMemberRole::User => String::new(),
                        };
                        let avatar_fallback_text = utils::user_name_first_letter(&display_name)
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| String::from("?"));
                        RoomInfoPeopleEntryInfo {
                            user_id: member.user_id().to_owned(),
                            display_name,
                            level,
                            is_bot,
                            avatar_uri: member.avatar_url().map(ToOwned::to_owned),
                            avatar_fallback_text,
                        }
                    })
                    .collect();

                let level_weight = |level: &str| -> u8 {
                    match level {
                        "Creator" => 0,
                        "Admin" => 1,
                        "Moderator" => 2,
                        _ => 3,
                    }
                };
                people_entries.sort_by(|a, b| {
                    level_weight(&a.level)
                        .cmp(&level_weight(&b.level))
                        .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
                });

                (
                    people_entries,
                    format!("{} Members", room_members.len()),
                    false,
                )
            })
            .unwrap_or_else(|| (
                Vec::new(),
                String::from("People"),
                true,
            ));

        self.room_info_sliding_pane(cx, ids!(room_info_sliding_pane)).set_info(
            cx,
            RoomInfoPaneInfo {
                room_name,
                room_id: room_id.to_string(),
                topic,
                visibility,
                encryption,
                room_avatar_uri,
                room_avatar_fallback_text,
                people_entries,
                people_count_text,
                show_people_loading,
            },
        );
    }

    fn show_room_info_pane(&mut self, cx: &mut Cx) {
        self.hide_threads_pane(cx);
        self.refresh_room_info_pane(cx);
        self.room_info_sliding_pane(cx, ids!(room_info_sliding_pane)).show(cx);
        self.redraw(cx);
    }

    fn hide_room_info_pane(&mut self, cx: &mut Cx) {
        self.room_info_sliding_pane(cx, ids!(room_info_sliding_pane)).hide(cx);
    }

    fn ensure_threads_state_for_current_room(&mut self) {
        let Some(room_id) = self.room_id().cloned() else { return };
        if self.threads_pane_state.room_id.as_ref().is_some_and(|current| current == &room_id) {
            return;
        }
        self.threads_pane_state = ThreadsPaneState {
            room_id: Some(room_id),
            status_text: String::from("Loading threads..."),
            ..Default::default()
        };
    }

    fn request_more_threads(&mut self, _cx: &mut Cx, load_more: bool) {
        self.ensure_threads_state_for_current_room();
        let Some(room_id) = self.threads_pane_state.room_id.clone() else { return };
        if self.threads_pane_state.is_loading {
            return;
        }
        let from = if load_more {
            let Some(from) = self.threads_pane_state.prev_batch_token.clone() else { return };
            Some(from)
        } else {
            None
        };
        self.threads_pane_state.is_loading = true;
        if !self.threads_pane_state.initialized {
            self.threads_pane_state.status_text = String::from("Loading threads...");
        }
        submit_async_request(MatrixRequest::ListRoomThreads {
            room_id,
            from,
        });
    }

    fn on_threads_loaded(
        &mut self,
        cx: &mut Cx,
        _from: Option<&String>,
        threads: &[FetchedRoomThread],
        prev_batch_token: Option<String>,
    ) {
        self.threads_pane_state.is_loading = false;
        self.threads_pane_state.initialized = true;
        self.threads_pane_state.prev_batch_token = prev_batch_token;
        self.threads_pane_state.entries.extend_from_slice(threads);
        self.threads_pane_state.entries.sort_by_key(|entry| u64::from(entry.timestamp.0));
        self.threads_pane_state.entries.dedup_by(|a, b| a.thread_root_event_id == b.thread_root_event_id);
        self.threads_pane_state.status_text = if self.threads_pane_state.entries.is_empty() {
            String::from("No threads yet.")
        } else {
            String::new()
        };
        self.refresh_threads_pane(cx);
        self.redraw(cx);
    }

    fn on_threads_failed(&mut self, cx: &mut Cx, error: &str) {
        self.threads_pane_state.is_loading = false;
        self.threads_pane_state.initialized = true;
        if self.threads_pane_state.entries.is_empty() {
            self.threads_pane_state.status_text = format!("Failed to load threads.\n\nError: {error}");
        } else {
            enqueue_popup_notification(
                format!("Failed to load more threads.\n\nError: {error}"),
                PopupKind::Error,
                Some(5.0),
            );
        }
        self.refresh_threads_pane(cx);
        self.redraw(cx);
    }

    /// Invoke this when this timeline is being shown,
    /// e.g., when the user navigates to this timeline.
    fn show_timeline(&mut self, cx: &mut Cx) {
        let kind = self.timeline_kind.clone()
            .expect("BUG: Timeline::show_timeline(): no timeline_kind was set.");
        let room_id = kind.room_id().clone();

        let state_opt = TIMELINE_STATES.with_borrow_mut(|ts| ts.remove(&kind));
        let (mut tl_state, mut is_first_time_being_loaded) = if let Some(existing) = state_opt {
            (existing, false)
        } else {
            let Some(timeline_endpoints) = take_timeline_endpoints(&kind) else {
                if let Some(thread_root_event_id) = kind.thread_root_event_id() {
                    submit_async_request(MatrixRequest::CreateThreadTimeline {
                        room_id: room_id.clone(),
                        thread_root_event_id: thread_root_event_id.clone(),
                    });
                    return;
                }
                if !self.is_loaded && self.all_rooms_loaded {
                    panic!("BUG: timeline {kind} is not loaded, but its RoomScreen \
                    was not waiting for its timeline to be loaded either.");
                }
                return;
            };
            let TimelineEndpoints {
                update_receiver,
                update_sender,
                request_sender,
                successor_room,
            } = timeline_endpoints;

            // Start with the basic tombstone info, and fetch the full details
            // if the room has been tombstoned.
            let tombstone_info = if let Some(sr) = successor_room {
                submit_async_request(MatrixRequest::GetSuccessorRoomDetails {
                    tombstoned_room_id: room_id.clone(),
                });
                Some(SuccessorRoomDetails::Basic(sr))
            } else {
                None
            };

            let tl_state = TimelineUiState {
                kind,
                // Initially, we assume the user has all power levels by default.
                // This avoids unexpectedly hiding any UI elements that should be visible to the user.
                // This doesn't mean that the user can actually perform all actions;
                // the power levels will be updated from the homeserver once the room is opened.
                user_power: UserPowerLevels::all(),
                // Room members start as None and get populated when fetched from the server
                room_members: None,
                room_members_sort: None,
                    room_members_sync_pending: false,
                awaiting_post_sync_member_refresh: false,
                // We assume timelines being viewed for the first time haven't been fully paginated.
                fully_paginated: false,
                backwards_pagination_in_flight: false,
                items: Vector::new(),
                content_drawn_since_last_update: RangeSet::new(),
                profile_drawn_since_last_update: RangeSet::new(),
                update_receiver,
                request_sender,
                media_cache: MediaCache::new(Some(update_sender.clone())),
                link_preview_cache: LinkPreviewCache::new(Some(update_sender)),
                fetched_thread_summaries: HashMap::new(),
                pending_thread_summary_fetches: HashSet::new(),
                saved_state: SavedState::default(),
                message_highlight_animation_state: MessageHighlightAnimationState::default(),
                streaming_messages: HashMap::new(),
                last_scrolled_index: usize::MAX,
                prev_first_index: None,
                scrolled_past_read_marker: false,
                latest_own_user_receipt: None,
                tombstone_info,
            };
            (tl_state, true)
        };

        // It is possible that this room has already been loaded (received from the server)
        // but that the RoomsList doesn't yet know about it.
        // In that case, `is_first_time_being_loaded` will already be `true` here,
        // so we can bypass checking the RoomsList to determine if a room is loaded.
        //
        // Note that we *do* still need to check the RoomsList to see whether this room is loaded
        // in order to handle the case when we're switching between rooms within
        // the same RoomScreen widget, as one room may be loaded while another is not.
        if is_first_time_being_loaded {
            self.is_loaded = true;
        } else if cx.has_global::<RoomsListRef>() {
            let rooms_list_ref = cx.get_global::<RoomsListRef>();
            let is_loaded_now = rooms_list_ref.is_room_loaded(&room_id);
            if is_loaded_now && !self.is_loaded {
                // log!("Detected that {}} is now loaded for the first time", tl_state.kind);
                is_first_time_being_loaded = true;
            }
            self.is_loaded = is_loaded_now;
        }

        self.view.restore_status_view(cx, ids!(restore_status_view)).set_visible(cx, !self.is_loaded);

        // Kick off a back pagination request if it's the first time loading this room,
        // because we want to show the user some messages as soon as possible
        // when they first open the room, and there might not be any messages yet.
        if is_first_time_being_loaded {
            if !tl_state.fully_paginated {
                tl_state.backwards_pagination_in_flight = true;
                log!("Sending a first-time backwards pagination request for {}", tl_state.kind);
                submit_async_request(MatrixRequest::PaginateTimeline {
                    timeline_kind: tl_state.kind.clone(),
                    num_events: VIEWPORT_FILL_PAGINATION_SIZE,
                    direction: PaginationDirection::Backwards,
                });
            }

            // Even though we specify that room member profiles should be lazy-loaded,
            // the matrix server still doesn't consistently send them to our client properly.
            // So we kick off a request to fetch the room members here upon first viewing the room.
            tl_state.room_members_sync_pending = true;
            tl_state.awaiting_post_sync_member_refresh = false;
            submit_async_request(MatrixRequest::SyncRoomMemberList {
                timeline_kind: tl_state.kind.clone(),
            });
        }

        // Hide the typing notice view initially.
        self.view(cx, ids!(typing_notice)).set_visible(cx, false);
        // If the room is loaded, we need to get a few key states:
        // 1. Get the current user's power levels for this room so that we can
        //    show/hide UI elements based on the user's permissions.
        // 2. Get the list of members in this room (from the SDK's local cache).
        // 3. Subscribe to our own user's read receipts so that we can update the
        //    read marker and properly send read receipts while scrolling through the timeline.
        // 4. Subscribe to typing notices again, now that the room is being shown.
        if self.is_loaded {
            submit_async_request(MatrixRequest::GetRoomPowerLevels {
                timeline_kind: tl_state.kind.clone(),
            });
            submit_async_request(MatrixRequest::GetRoomMembers {
                timeline_kind: tl_state.kind.clone(),
                memberships: matrix_sdk::RoomMemberships::JOIN,
                // Fetch from the local cache, as we already requested to sync
                // the room members from the homeserver above.
                local_only: true,
            });
            submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
                timeline_kind: tl_state.kind.clone(),
                subscribe: true,
            });
            // Only main room timelines can subscribe to typing notices and pinned events.
            if matches!(tl_state.kind, TimelineKind::MainRoom { .. }) {
                submit_async_request(MatrixRequest::SubscribeToTypingNotices {
                    room_id: room_id.clone(),
                    subscribe: true,
                });
                submit_async_request(MatrixRequest::SubscribeToPinnedEvents {
                    room_id: room_id.clone(),
                    subscribe: true,
                });
            }
        }

        // Now, restore the visual state of this timeline from its previously-saved state.
        self.restore_state(cx, &mut tl_state);

        // Store the tl_state for this room into this RoomScreen widget,
        // such that it can be accessed in future functions like event/draw handlers.
        self.tl_state = Some(tl_state);
        self.schedule_stream_timeout(cx);

        // Now that we have restored the TimelineUiState into this RoomScreen widget,
        // we can proceed to processing pending background updates.
        self.process_timeline_updates(cx, &self.portal_list(cx, ids!(list)), None);

        self.redraw(cx);
    }

    /// Invoke this when this RoomScreen/timeline is being hidden or no longer being shown.
    fn hide_timeline(&mut self) {
        let Some(timeline_kind) = self.timeline_kind.clone() else { return };
        self.streaming_timeout_timer = Timer::empty();

        self.save_state();

        // When closing a room view, we do the following with non-persistent states.
        // (This should be the inverse of what's done in `show_timeline()`.)
        // * Unsubscribe from typing notices, since we don't care about them
        //   when a given room isn't visible.
        // * Unsubscribe from updates to this room's pinned events, for the same reason.
        // * Unsubscribe from updates to our own user's read receipts, for the same reason.
        if matches!(timeline_kind, TimelineKind::MainRoom { .. }) {
            submit_async_request(MatrixRequest::SubscribeToTypingNotices {
                room_id: timeline_kind.room_id().clone(),
                subscribe: false,
            });
            submit_async_request(MatrixRequest::SubscribeToPinnedEvents {
                room_id: timeline_kind.room_id().clone(),
                subscribe: false,
            });
        }
        submit_async_request(MatrixRequest::SubscribeToOwnUserReadReceiptsChanged {
            timeline_kind,
            subscribe: false,
        });
        self.room_avatar_url = None;
        self.pending_invited_users.clear();
    }

    /// Removes the current room's visual UI state from this widget
    /// and saves it to the map of `TIMELINE_STATES` such that it can be restored later.
    ///
    /// Note: after calling this function, the widget's `tl_state` will be `None`.
    fn save_state(&mut self) {
        let Some(mut tl) = self.tl_state.take() else {
            error!("Timeline::save_state(): skipping due to missing state, room {:?}, {:?}", self.timeline_kind, self.room_name_id.as_ref().map(|r| r.display_name()));
            return;
        };

        let portal_list = self.child_by_path(ids!(timeline.list)).as_portal_list();
        let room_input_bar = self.child_by_path(ids!(room_input_bar)).as_room_input_bar();
        log!("Saving state for room {:?}\n\t{:?}\n\tfirst_id: {:?}, scroll: {}", self.room_name_id.as_ref().map(|r| r.display_name()), self.timeline_kind, portal_list.first_id(), portal_list.scroll_position());
        let state = SavedState {
            first_index_and_scroll: Some((portal_list.first_id(), portal_list.scroll_position())),
            room_input_bar_state: room_input_bar.save_state(),
        };
        tl.saved_state = state;
        // Clear room_members and precomputed sort to avoid wasting memory
        // (in case this room is never re-opened).
        tl.room_members = None;
        tl.room_members_sort = None;
        // Store this Timeline's `TimelineUiState` in the global map of states.
        TIMELINE_STATES.with_borrow_mut(|ts| ts.insert(tl.kind.clone(), tl));
    }

    /// Restores the previously-saved visual UI state of this room.
    ///
    /// Note: this accepts a direct reference to the timeline's UI state,
    /// so this function must not try to re-obtain it by accessing `self.tl_state`.
    fn restore_state(&mut self, cx: &mut Cx, tl_state: &mut TimelineUiState) {
        let SavedState {
            first_index_and_scroll,
            room_input_bar_state,
        } = &mut tl_state.saved_state;

        // 1. Restore the position of the timeline.
        let portal_list = self.portal_list(cx, ids!(timeline.list));
        if let Some((first_index, scroll_from_first_id)) = first_index_and_scroll {
            log!("Restoring state for room {:?}: first_id: {:?}, scroll: {}", self.room_name_id, first_index, scroll_from_first_id);
            portal_list.set_first_id_and_scroll(*first_index, *scroll_from_first_id);
            portal_list.set_tail_range(false);
        } else {
            // If the first index is not set, then the timeline has not yet been scrolled by the user,
            // so we reset the portal list's scroll position and set it to "tail" (track) the bottom.
            // The explicit reset is necessary when the same RoomScreen widget is reused for a
            // different room (e.g., via stack navigation view alternation), otherwise the portal list
            // would retain the previous room's scroll position which may be out of bounds.
            log!("Restoring state for room {:?}: first_id: None, scroll: None", self.room_name_id);
            portal_list.set_first_id_and_scroll(0, 0.0);
            portal_list.set_tail_range(true);
        }

        // 2. Restore the state of the room input bar.
        let room_input_bar = self.child_by_path(ids!(room_input_bar)).as_room_input_bar();
        let saved_room_input_bar_state = std::mem::take(room_input_bar_state);
        room_input_bar.restore_state(
            cx,
            tl_state.kind.clone(),
            saved_room_input_bar_state,
            tl_state.user_power,
            tl_state.tombstone_info.as_ref(),
        );

        refresh_stream_indices(
            tl_state.items.iter().map(item_event_id),
            &mut tl_state.streaming_messages,
        );

        // 3. If there are active streaming animations that can still reveal text,
        //    re-request the NextFrame event so the animation loop resumes.
        if tl_state.streaming_messages.values().any(|state| state.needs_frame()) {
            self.streaming_next_frame = cx.new_next_frame();
        }
    }

    /// Sets this `RoomScreen` widget to display the timeline for the given room.
    pub fn set_displayed_room(
        &mut self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        thread_root_event_id: Option<OwnedEventId>,
    ) {
        let timeline_kind = if let Some(thread_root_event_id) = thread_root_event_id {
            TimelineKind::Thread {
                room_id: room_name_id.room_id().clone(),
                thread_root_event_id,
            }
        } else {
            TimelineKind::MainRoom {
                room_id: room_name_id.room_id().clone(),
            }
        };

        // If this timeline is already displayed, we don't need to do anything major,
        // but we do need update the `room_name_id` in case it has changed, or it has been cleared.
        if self.timeline_kind.as_ref().is_some_and(|kind| kind == &timeline_kind) {
            self.room_name_id = Some(room_name_id.clone());
            self.room_avatar_url = get_client()
                .and_then(|client| client.get_room(room_name_id.room_id()))
                .and_then(|room| room.avatar_url());
            return;
        }

        self.hide_timeline();
        self.reset_app_service_ui(cx);
        self.hide_threads_pane(cx);
        self.hide_room_info_pane(cx);
        self.threads_pane_state = Default::default();
        // Reset the the state of the inner loading pane.
        self.loading_pane(cx, ids!(loading_pane)).take_state();

        self.room_name_id = Some(room_name_id.clone());
        self.room_avatar_url = get_client()
            .and_then(|client| client.get_room(room_name_id.room_id()))
            .and_then(|room| room.avatar_url());
        self.timeline_kind = Some(timeline_kind.clone());

        // We initially tell every MentionableTextInput widget that the current user
        // *does not* have privileges to notify the entire room;
        // this gets properly updated when room PowerLevels get fetched.
        cx.action(MentionableTextInputAction::PowerLevelsUpdated {
            room_id: timeline_kind.room_id().clone(),
            can_notify_room: false,
        });

        self.show_timeline(cx);
    }

    /// Sends read receipts based on the current scroll position of the timeline.
    fn send_user_read_receipts_based_on_scroll_pos(
        &mut self,
        _cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        //stopped scrolling
        if portal_list.scrolled(actions) {
            return;
        }
        let first_index = portal_list.first_id();
        let Some(tl_state) = self.tl_state.as_mut() else { return };

        if let Some(ref mut index) = tl_state.prev_first_index {
            // to detect change of scroll when scroll ends
            if *index != first_index {
                if first_index >= *index {
                    // Get event_id and timestamp for the last visible event
                    let Some((last_event_id, last_timestamp)) = tl_state
                        .items
                        .get(std::cmp::min(
                            first_index + portal_list.visible_items(),
                            tl_state.items.len().saturating_sub(1)
                        ))
                        .and_then(|f| f.as_event())
                        .and_then(|f| f.event_id().map(|e| (e, f.timestamp())))
                    else {
                        *index = first_index;
                        return;
                    };
                    submit_async_request(MatrixRequest::ReadReceipt {
                        timeline_kind: tl_state.kind.clone(),
                        event_id: last_event_id.to_owned(),
                        receipt_type: ReceiptType::Read,
                    });
                    if tl_state.scrolled_past_read_marker {
                        submit_async_request(MatrixRequest::ReadReceipt {
                            timeline_kind: tl_state.kind.clone(),
                            event_id: last_event_id.to_owned(),
                            receipt_type: ReceiptType::FullyRead,
                        });
                    } else {
                        if let Some(own_user_receipt_timestamp) = &tl_state.latest_own_user_receipt.clone()
                        .and_then(|receipt| receipt.ts) {
                            let Some((_first_event_id, first_timestamp)) = tl_state
                                .items
                                .get(first_index)
                                .and_then(|f| f.as_event())
                                .and_then(|f| f.event_id().map(|e| (e, f.timestamp())))
                                else {
                                    *index = first_index;
                                    return;
                                };
                            if own_user_receipt_timestamp >= &first_timestamp
                                && own_user_receipt_timestamp <= &last_timestamp
                            {
                                tl_state.scrolled_past_read_marker = true;
                                submit_async_request(MatrixRequest::ReadReceipt {
                                    timeline_kind: tl_state.kind.clone(),
                                    event_id: last_event_id.to_owned(),
                                    receipt_type: ReceiptType::FullyRead,
                                });
                            }

                        }
                    }
                }
                *index = first_index;
            }
        } else {
            tl_state.prev_first_index = Some(first_index);
        }
    }

    /// Sends a backwards pagination request if the user is scrolling up
    /// and is approaching the top of the timeline.
    fn send_pagination_request_based_on_scroll_pos(
        &mut self,
        _cx: &mut Cx,
        actions: &ActionsBuf,
        portal_list: &PortalListRef,
    ) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        if tl.fully_paginated { return };
        if !portal_list.scrolled(actions) { return };

        let first_index = portal_list.first_id();
        if first_index == 0 && tl.last_scrolled_index > 0 && !tl.backwards_pagination_in_flight {
            tl.backwards_pagination_in_flight = true;
            log!("Scrolled up from item {} --> 0, sending back pagination request for room {}",
                tl.last_scrolled_index, tl.kind,
            );
            submit_async_request(MatrixRequest::PaginateTimeline {
                timeline_kind: tl.kind.clone(),
                num_events: 50,
                direction: PaginationDirection::Backwards,
            });
        }
        tl.last_scrolled_index = first_index;
    }
}

impl RoomScreenRef {
    /// See [`RoomScreen::set_displayed_room()`].
    pub fn set_displayed_room(
        &self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        thread_root_event_id: Option<OwnedEventId>,
    ) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_displayed_room(cx, room_name_id, thread_root_event_id);
    }
}

/// Immutable RoomScreen states passed via Scope props
/// from a RoomScreen widget to its child widgets for event/draw handlers.
pub struct RoomScreenProps {
    pub room_screen_widget_uid: WidgetUid,
    pub room_name_id: RoomNameId,
    pub timeline_kind: TimelineKind,
    pub room_members: Option<Arc<Vec<RoomMember>>>,
    pub is_direct_room: bool,
    pub room_bot_user_ids: Vec<OwnedUserId>,
    pub room_members_sync_pending: bool,
    /// Pre-computed sort order for room members (for mention search optimization).
    pub room_members_sort: Option<Arc<crate::room::member_search::PrecomputedMemberSort>>,
    pub room_avatar_url: Option<OwnedMxcUri>,
    pub app_service_enabled: bool,
    pub app_service_room_bound: bool,
    pub has_persisted_management_binding: bool,
    pub bound_bot_user_id: Option<OwnedUserId>,
    pub resolved_parent_bot_user_id: Option<OwnedUserId>,
    pub known_bot_user_ids: Vec<OwnedUserId>,
}


/// Actions for the room screen's tooltip.
#[derive(Clone, Debug, Default)]
pub enum RoomScreenTooltipActions {
    /// Mouse over event when the mouse is over the read receipt.
    HoverInReadReceipt {
        /// The rect of the moused over widget
        widget_rect: Rect,
        /// Includes the list of users who have seen this event
        read_receipts: indexmap::IndexMap<matrix_sdk::ruma::OwnedUserId, Receipt>,
    },
    /// Mouse over event when the mouse is over the reaction button.
    HoverInReactionButton {
        /// The rectangle (bounds) of the hovered-over widget.
        widget_rect: Rect,
        /// Includes the list of users who have reacted to the emoji.
        reaction_data: ReactionData,
    },
    /// Mouse out event and clear tooltip.
    HoverOut,
    #[default]
    None,
}

/// A message that is sent from a background async task to a room's timeline view
/// for the purpose of update the Timeline UI contents or metadata.
pub enum TimelineUpdate {
    /// The very first update a given room's timeline receives.
    FirstUpdate {
        /// The initial list of timeline items (events) for a room.
        initial_items: Vector<Arc<TimelineItem>>,
    },
    /// The content of a room's timeline was updated in the background.
    NewItems {
        /// The entire list of timeline items (events) for a room.
        new_items: Vector<Arc<TimelineItem>>,
        /// The range of indices in the `items` list that have been changed in this update
        /// and thus must be removed from any caches of drawn items in the timeline.
        /// Any items outside of this range are assumed to be unchanged and need not be redrawn.
        changed_indices: Range<usize>,
        /// An optimization that informs the UI whether the changes to the timeline
        /// resulted in new items being *appended to the end* of the timeline.
        is_append: bool,
        /// Whether to clear the entire cache of drawn items in the timeline.
        /// This supersedes `index_of_first_change` and is used when the entire timeline is being redrawn.
        clear_cache: bool,
    },
    /// The updated number of unread messages in the room.
    NewUnreadMessagesCount(UnreadMessageCount),
    /// The target event ID was found at the given `index` in the timeline items vector.
    ///
    /// This means that the RoomScreen widget can scroll the timeline up to this event,
    /// and the background `timeline_subscriber_handler` async task can stop looking for this event.
    TargetEventFound {
        target_event_id: OwnedEventId,
        index: usize,
    },
    /// A notice that the background task doing pagination for this room is currently running
    /// a pagination request in the given direction, and is waiting for that request to complete.
    PaginationRunning(PaginationDirection),
    /// An error occurred while paginating the timeline for this room.
    PaginationError {
        error: timeline::Error,
        direction: PaginationDirection,
    },
    /// A notice that the background task doing pagination for this room has become idle,
    /// meaning that it has completed its recent pagination request(s).
    PaginationIdle {
        /// If `true`, the start of the timeline has been reached, meaning that
        /// there is no need to send further pagination requests.
        fully_paginated: bool,
        direction: PaginationDirection,
    },
    /// A notice that event details have been fetched from the server,
    /// including a `result` that indicates whether the request was successful.
    EventDetailsFetched {
        event_id: OwnedEventId,
        result: Result<(), matrix_sdk_ui::timeline::Error>,
    },
    /// A notice that fresh thread-summary details were fetched for a thread root.
    ThreadSummaryDetailsFetched {
        thread_root_event_id: OwnedEventId,
        timeline_item_index: usize,
        num_replies: u32,
        latest_reply_preview_text: Option<String>,
    },
    /// The result of a request to edit a message in this timeline.
    MessageEdited {
        timeline_event_item_id: TimelineEventItemId,
        result: Result<(), matrix_sdk_ui::timeline::Error>,
    },
    /// A notice that the room's members have been fetched from the server,
    /// though the success or failure of the request is not yet known until the client
    /// requests the member info via a timeline event's `sender_profile()` method.
    RoomMembersSynced,
    /// A notice that the room's full member list has been fetched from the server,
    /// includes a complete list of room members that can be shared across components.
    /// This is different from RoomMembersSynced which only indicates members were fetched
    /// but doesn't provide the actual data.
    RoomMembersListFetched {
        members: Vec<RoomMember>,
    },
    /// A notice with an option of Media Request Parameters that one or more requested media items (images, videos, etc.)
    /// that should be displayed in this timeline have now been fetched and are available.
    MediaFetched(MediaRequestParameters),
    /// A notice that one or more members of a this room are currently typing.
    TypingUsers {
        /// The list of display names of users who are currently typing in this room.
        users: Vec<String>,
    },
    /// The result of a pin/unpin request ([`MatrixRequest::PinEvent`]).
    PinResult {
        event_id: OwnedEventId,
        result: Result<bool, matrix_sdk::Error>,
        pin: bool,
    },
    /// An update containing the set of pinned events in this room.
    PinnedEvents(Vec<OwnedEventId>),
    /// An update containing the currently logged-in user's power levels for this room.
    UserPowerLevels(UserPowerLevels),
    /// An update to the currently logged-in user's own read receipt for this room.
    OwnUserReadReceipt(Receipt),
    /// A notice that the given room has been tombstoned (closed)
    /// and replaced by the given successor room.
    Tombstoned(SuccessorRoomDetails),
    /// A notice that link preview data for a URL has been fetched and is now available.
    LinkPreviewFetched,
    /// User confirmed a file upload via the file upload modal.
    FileUploadConfirmed(crate::shared::file_upload_modal::FileData),
    /// Progress update for an ongoing file upload.
    FileUploadUpdate {
        current: u64,
        total: u64,
    },
    /// The abort handle for an in-progress file upload.
    FileUploadAbortHandle(tokio::task::AbortHandle),
    /// An error occurred during file upload.
    FileUploadError {
        error: String,
        file_data: crate::shared::file_upload_modal::FileData,
    },
    /// File upload completed successfully.
    FileUploadComplete,
}

thread_local! {
    /// The global set of all timeline states, one entry per room.
    ///
    /// This is only useful when accessed from the main UI thread.
    static TIMELINE_STATES: RefCell<HashMap<TimelineKind, TimelineUiState>> = 
        RefCell::new(HashMap::new());
}

/// The UI-side state of a single room's timeline, which is only accessed/updated by the UI thread.
///
/// This struct should only include states that need to be persisted for a given room
/// across multiple `Hide`/`Show` cycles of that room's timeline within a RoomScreen.
/// If a state is more temporary and shouldn't be persisted when the timeline is hidden,
/// then it should be stored in the RoomScreen widget itself, not in this struct.
struct TimelineUiState {
    /// Info determining whether this is a main room timeline is a thread-focused timeline.
    kind: TimelineKind,

    /// The power levels of the currently logged-in user in this room.
    user_power: UserPowerLevels,

    /// The list of room members for this room.
    room_members: Option<Arc<Vec<RoomMember>>>,

    /// Pre-computed sort order for room members (for efficient mention search).
    room_members_sort: Option<Arc<crate::room::member_search::PrecomputedMemberSort>>,

    /// Whether the initial room-member sync is still in progress for this room.
    room_members_sync_pending: bool,

    /// Whether we're waiting for a refreshed local member snapshot after sync completion.
    awaiting_post_sync_member_refresh: bool,

    /// Whether this room's timeline has been fully paginated, which means
    /// that the oldest (first) event in the timeline is locally synced and available.
    /// When `true`, further backwards pagination requests will not be sent.
    ///
    /// This must be reset to `false` whenever the timeline is fully cleared.
    fully_paginated: bool,

    /// Whether a backwards pagination request has already been submitted
    /// and is still in flight.
    backwards_pagination_in_flight: bool,

    /// The list of items (events) in this room's timeline that our client currently knows about.
    items: Vector<Arc<TimelineItem>>,

    /// The range of items (indices in the above `items` list) whose event **contents** have been drawn
    /// since the last update and thus do not need to be re-populated on future draw events.
    ///
    /// This range is partially cleared on each background update (see below) to ensure that
    /// items modified during the update are properly redrawn. Thus, it is a conservative
    /// "cache tracker" that may not include all items that have already been drawn,
    /// but that's okay because big updates that clear out large parts of the rangeset
    /// only occur during back pagination, which is both rare and slow in and of itself.
    /// During typical usage, new events are appended to the end of the timeline,
    /// meaning that the range of already-drawn items doesn't need to be cleared.
    ///
    /// Upon a background update, only item indices greater than or equal to the
    /// `index_of_first_change` are removed from this set.
    content_drawn_since_last_update: RangeSet<usize>,

    /// Same as `content_drawn_since_last_update`, but for the event **profiles** (avatar, username).
    profile_drawn_since_last_update: RangeSet<usize>,

    /// The channel receiver for timeline updates for this room.
    ///
    /// Here we use a synchronous (non-async) channel because the receiver runs
    /// in a sync context and the sender runs in an async context,
    /// which is okay because a sender on an unbounded channel never needs to block.
    update_receiver: crossbeam_channel::Receiver<TimelineUpdate>,

    /// The sender for timeline requests from a RoomScreen showing this room
    /// to the background async task that handles this room's timeline updates.
    request_sender: TimelineRequestSender,

    /// The cache of media items (images, videos, etc.) that appear in this timeline.
    ///
    /// Currently this excludes avatars, as those are shared across multiple rooms.
    media_cache: MediaCache,

    /// Cache for link preview data indexed by URL to avoid redundant network requests.
    link_preview_cache: LinkPreviewCache,
    /// Cached fetched thread-summary details, keyed by thread-root event ID.
    fetched_thread_summaries: HashMap<OwnedEventId, FetchedThreadSummary>,
    /// Set of thread roots currently being fetched to avoid duplicate in-flight requests.
    pending_thread_summary_fetches: HashSet<OwnedEventId>,

    /// The states relevant to the UI display of this timeline that are saved upon
    /// a `Hide` action and restored upon a `Show` action.
    saved_state: SavedState,

    /// The state of the message highlight animation.
    ///
    /// We need to run the animation once the scrolling, triggered by the click of of a
    /// a reply preview, ends. so we keep a small state for it.
    /// By default, it starts in Off.
    /// Once the scrolling is started, the state becomes Pending.
    /// If the animation was triggered, the state goes back to Off.
    message_highlight_animation_state: MessageHighlightAnimationState,

    /// Active streaming animations, keyed by event ID.
    /// Stores the typewriter animation state for messages being streamed by bots.
    streaming_messages: HashMap<OwnedEventId, super::streaming_animation::StreamingAnimState>,

    /// The index of the timeline item that was most recently scrolled up past it.
    /// This is used to detect when the user has scrolled up past the second visible item (index 1)
    /// upwards to the first visible item (index 0), which is the top of the timeline,
    /// at which point we submit a backwards pagination request to fetch more events.
    last_scrolled_index: usize,

    /// The index of the first item shown in the timeline's PortalList from *before* the last "jump".
    ///
    /// This index is saved before the timeline undergoes any jumps, e.g.,
    /// receiving new items, major scroll changes, or other timeline view jumps.
    prev_first_index: Option<usize>,

    /// Whether the user has scrolled past their latest read marker.
    ///
    /// This is used to determine whether we should send a fully-read receipt
    /// after the user scrolls past their "read marker", i.e., their latest fully-read receipt.
    /// Its value is determined by comparing the fully-read event's timestamp with the
    /// first and last timestamp of displayed events in the timeline.
    /// When scrolling down, if the value is true, we send a fully-read receipt
    /// for the last visible event in the timeline.
    ///
    /// When new message come in, this value is reset to `false`.
    scrolled_past_read_marker: bool,
    latest_own_user_receipt: Option<Receipt>,

    /// If `Some`, this room has been tombstoned and the details of its successor room
    /// are contained within. If `None`, the room has not been tombstoned.
    tombstone_info: Option<SuccessorRoomDetails>,
}

#[derive(Default, Debug)]
enum MessageHighlightAnimationState {
    Pending { item_id: usize },
    #[default]
    Off,
}

/// States that are necessary to save in order to maintain a consistent UI display for a timeline.
///
/// These are saved when navigating away from a timeline (upon `Hide`)
/// and restored when navigating back to a timeline (upon `Show`).
#[derive(Default)]
struct SavedState {
    /// The index of the first item in the timeline's PortalList that is currently visible,
    /// and the scroll offset from the top of the list's viewport to the beginning of that item.
    /// If this is `None`, then the timeline has not yet been scrolled by the user
    /// and the portal list will be set to "tail" (track) the bottom of the list.
    first_index_and_scroll: Option<(usize, f64)>,
    /// The state of all UI elements in the `RoomInputBar`.
    room_input_bar_state: RoomInputBarState,
}

/// Returns info about the item in the list of `new_items` that matches the event ID
/// of a visible item in the given `curr_items` list.
///
/// This info includes a tuple of:
/// 1. the index of the item in the current items list,
/// 2. the index of the item in the new items list,
/// 3. the positional "scroll" offset of the corresponding current item in the portal list,
/// 4. the unique event ID of the item.
fn find_new_item_matching_current_item(
    cx: &mut Cx,
    portal_list: &PortalListRef,
    starting_at_curr_idx: usize,
    curr_items: &Vector<Arc<TimelineItem>>,
    new_items: &Vector<Arc<TimelineItem>>,
) -> Option<(usize, usize, f64, OwnedEventId)> {
    let mut curr_item_focus = curr_items.focus();
    let mut idx_curr = starting_at_curr_idx;
    let mut curr_items_with_ids: Vec<(usize, OwnedEventId)> = Vec::with_capacity(
        portal_list.visible_items()
    );

    // Find all items with real event IDs that are currently visible in the portal list.
    // TODO: if this is slow, we could limit it to 3-5 events at the most.
    if curr_items_with_ids.len() <= portal_list.visible_items() {
        while let Some(curr_item) = curr_item_focus.get(idx_curr) {
            if let Some(event_id) = curr_item.as_event().and_then(|ev| ev.event_id()) {
                curr_items_with_ids.push((idx_curr, event_id.to_owned()));
            }
            if curr_items_with_ids.len() >= portal_list.visible_items() {
                break;
            }
            idx_curr += 1;
        }
    }

    // Find a new item that has the same real event ID as any of the current items.
    for (idx_new, new_item) in new_items.iter().enumerate() {
        let Some(event_id) = new_item.as_event().and_then(|ev| ev.event_id()) else {
            continue;
        };
        if let Some((idx_curr, _)) = curr_items_with_ids
            .iter()
            .find(|(_, ev_id)| ev_id == event_id)
        {
            // Not all items in the portal list are guaranteed to have a position offset,
            // some may be zeroed-out, so we need to account for that possibility by only
            // using events that have a real non-zero area
            if let Some(pos_offset) = portal_list.position_of_item(cx, *idx_curr) {
                log!("Found matching event ID {event_id} at index {idx_new} in new items list, corresponding to current item index {idx_curr} at pos offset {pos_offset}");
                return Some((*idx_curr, idx_new, pos_offset, event_id.to_owned()));
            }
        }
    }

    None
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ItemDrawnStatus {
    /// Whether the profile info (avatar and displayable username) were drawn for this item.
    profile_drawn: bool,
    /// Whether the content of the item was drawn (e.g., the message text, image, video, sticker, etc).
    content_drawn: bool,
}

#[derive(Clone, Debug)]
struct FetchedThreadSummary {
    num_replies: u32,
    latest_reply_preview_text: Option<String>,
}

impl ItemDrawnStatus {
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `false`.
    const fn new() -> Self {
        Self {
            profile_drawn: false,
            content_drawn: false,
        }
    }
    /// Returns a new `ItemDrawnStatus` with both `profile_drawn` and `content_drawn` set to `true`.
    const fn both_drawn() -> Self {
        Self {
            profile_drawn: true,
            content_drawn: true,
        }
    }
}

/// Creates, populates, and adds a Message liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned `Message` widget is populated with data from a message
/// or sticker and its containing `EventTimelineItem`.
fn populate_message_view(
    cx: &mut Cx2d,
    list: &mut PortalList,
    item_id: usize,
    timeline_kind: &TimelineKind,
    app_language: AppLanguage,
    event_tl_item: &EventTimelineItem,
    msg_like_content: &MsgLikeContent,
    prev_event: Option<&Arc<TimelineItem>>,
    media_cache: &mut MediaCache,
    link_preview_cache: &mut LinkPreviewCache,
    fetched_thread_summaries: &HashMap<OwnedEventId, FetchedThreadSummary>,
    pending_thread_summary_fetches: &mut HashSet<OwnedEventId>,
    user_power_levels: &UserPowerLevels,
    pinned_events: &[OwnedEventId],
    item_drawn_status: ItemDrawnStatus,
    room_screen_widget_uid: WidgetUid,
    resolved_parent_bot_user_id: Option<&UserId>,
    room_bot_user_ids: &[OwnedUserId],
    known_bot_user_ids: &[OwnedUserId],
    streaming_messages: &mut HashMap<OwnedEventId, super::streaming_animation::StreamingAnimState>,
    action_button_contexts: &mut HashMap<WidgetUid, OctosActionButtonContext>,
    disabled_action_source_event_ids: &HashSet<OwnedEventId>,
    selected_actions: &HashMap<OwnedEventId, SelectedOctosActionState>,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let ts_millis = event_tl_item.timestamp();
    let sender_is_bot = is_timeline_sender_bot(
        event_tl_item.sender(),
        resolved_parent_bot_user_id,
        room_bot_user_ids,
        known_bot_user_ids,
    );

    let mut is_notice = false; // whether this message is a Notice (automated bot message)
    let mut is_server_notice = false; // whether this message is a Server Notice

    // Determine whether we can use a more compact UI view that hides the user's profile info
    // if the previous message (including stickers) was sent by the same user within 10 minutes.
    let use_compact_view = match prev_event.map(|p| p.kind()) {
        Some(TimelineItemKind::Event(prev_event_tl_item)) => match prev_event_tl_item.content() {
            TimelineItemContent::MsgLike(_msg_like_content) => {
                let prev_msg_sender = prev_event_tl_item.sender();
                prev_msg_sender == event_tl_item.sender()
                    && ts_millis.0
                        .checked_sub(prev_event_tl_item.timestamp().0)
                        .is_some_and(|d| d < uint!(600000)) // 10 mins in millis
            }
            _ => false,
        },
        _ => false,
    };

    let has_html_body: bool;

    // Sometimes we need to call this up-front, so we save the result in this variable
    // to avoid having to call it twice.
    let mut set_username_and_get_avatar_retval = None;
    let (item, used_cached_item) = match &msg_like_content.kind {
        MsgLikeKind::Message(msg) => {
            match msg.msgtype() {
                MessageType::Text(TextMessageEventContent { body, formatted, .. }) => {
                    has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        // Check if this message is being streamed
                        let is_streaming = event_tl_item.event_id()
                            .and_then(|eid| streaming_messages.get_mut(&eid.to_owned()));

                        if let Some(state) = is_streaming {
                            let render_full_snapshot = should_render_streaming_full_snapshot(
                                body,
                                formatted.as_ref(),
                                sender_is_bot,
                            );
                            state.set_render_full_target(render_full_snapshot);

                            // STREAMING MODE:
                            // - markdown-rich bot replies render the latest full snapshot directly
                            // - plain text keeps the local typewriter prefix with cursor
                            let mut link_preview_ref =
                                item.link_preview(cx, ids!(content.link_preview_view));
                            let (stream_body, stream_formatted) = if render_full_snapshot {
                                (body.as_str(), formatted.as_ref())
                            } else {
                                state.fill_display_buffer();
                                (state.display_buffer.as_str(), None)
                            };
                            let _ = populate_bot_text_message_content(
                                cx,
                                &item,
                                app_language,
                                stream_body,
                                stream_formatted,
                                Some(&mut link_preview_ref),
                                Some(media_cache),
                                Some(link_preview_cache),
                                sender_is_bot,
                            );
                            new_drawn_status.content_drawn = false; // force re-render
                        } else {
                            // Check for Splash card in custom event field
                            let splash_code = latest_effective_event_content_json(event_tl_item)
                                .and_then(|content|
                                    content
                                        .get("org.octos.splash_card")
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                );

                            if let Some(ref splash) = splash_code {
                                // SPLASH CARD MODE: render native Makepad card
                                item.view(cx, ids!(content.message)).set_visible(cx, false);
                                let splash_widget = item.splash(cx, ids!(content.splash_card));
                                splash_widget.set_visible(cx, true);
                                splash_widget.set_text(cx, splash);
                                new_drawn_status.content_drawn = true;
                            } else {
                                // NORMAL MODE: existing logic
                                let mut link_preview_ref =
                                    item.link_preview(cx, ids!(content.link_preview_view));
                                new_drawn_status.content_drawn = populate_bot_text_message_content(
                                    cx,
                                    &item,
                                    app_language,
                                    body,
                                    formatted.as_ref(),
                                    Some(&mut link_preview_ref),
                                    Some(media_cache),
                                    Some(link_preview_cache),
                                    sender_is_bot,
                                );
                            }
                        }
                        (item, false)
                    }
                }
                // A notice message is just a message sent by an automated bot,
                // so we treat it just like a message but use a different font color.
                MessageType::Notice(NoticeMessageEventContent{body, formatted, ..}) => {
                    is_notice = true;
                    has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        if !sender_is_bot {
                            let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                            // Apply gray color to all text styles for notice messages.
                            let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                            script_apply_eval!(cx, html_widget, {
                                font_color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT,
                                draw_block +: {
                                    quote_fg_color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT,
                                }
                            });
                        }
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        new_drawn_status.content_drawn = populate_bot_text_message_content(
                            cx,
                            &item,
                            app_language,
                            body,
                            formatted.as_ref(),
                            Some(&mut link_preview_ref),
                            Some(media_cache),
                            Some(link_preview_cache),
                            sender_is_bot,
                        );
                        (item, false)
                    }
                }
                MessageType::ServerNotice(sn) => {
                    is_server_notice = true;
                    has_html_body = false;
                    let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                        // Apply red color to all text styles for server notices.
                        let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                        script_apply_eval!(cx, html_widget, {
                            font_color: mod.widgets.COLOR_FG_DANGER_RED
                            draw_text +: { color: mod.widgets.COLOR_FG_DANGER_RED }
                            draw_block +: {
                                line_color: mod.widgets.COLOR_FG_DANGER_RED
                                quote_fg_color: mod.widgets.COLOR_FG_DANGER_RED
                            }
                        });
                        let formatted = format!(
                            "<b>{}</b> {}\n\n<i>{}</i>: {}{}{}",
                            tr_key(app_language, "room_screen.server_notice.title"),
                            sn.body,
                            tr_key(app_language, "room_screen.server_notice.notice_type"),
                            sn.server_notice_type.as_str(),
                            sn.limit_type.as_ref()
                                .map(|l| format!("\n<i>{}</i> {}", tr_key(app_language, "room_screen.server_notice.limit_type"), l.as_str()))
                                .unwrap_or_default(),
                            sn.admin_contact.as_ref()
                                .map(|c| format!("\n<i>{}</i> {}", tr_key(app_language, "room_screen.server_notice.admin_contact"), c))
                                .unwrap_or_default(),
                        );
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            &sn.body,
                            Some(&FormattedBody {
                                format: MessageFormat::Html,
                                body: formatted,
                            }),
                            Some(&mut link_preview_ref),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                // An emote is just like a message but is prepended with the user's name
                // to indicate that it's an "action" that the user is performing.
                MessageType::Emote(EmoteMessageEventContent { body, formatted, .. }) => {
                    has_html_body = formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        // Draw the profile up front here because we need the username for the emote body.
                        let (username, profile_drawn) = item.avatar(cx, ids!(profile.avatar)).set_avatar_and_get_username(
                            cx,
                            timeline_kind,
                            event_tl_item.sender(),
                            Some(event_tl_item.sender_profile()),
                            event_tl_item.event_id(),
                            true,
                        );

                        // Prepend a "* <username> " to the emote body, as suggested by the Matrix spec.
                        let (body, formatted) = if let Some(fb) = formatted.as_ref() {
                            (
                                Cow::from(&fb.body),
                                Some(FormattedBody {
                                    format: fb.format.clone(),
                                    body: format!("* {} {}", &username, &fb.body),
                                })
                            )
                        } else {
                            (Cow::from(format!("* {} {}", &username, body)), None)
                        };
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));
                        let link_previews_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            &body,
                            formatted.as_ref(),
                            Some(&mut link_preview_ref),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        set_username_and_get_avatar_retval = Some((username, profile_drawn));
                        new_drawn_status.content_drawn = link_previews_drawn;
                        (item, false)
                    }
                }
                MessageType::Image(image) => {
                    has_html_body = image.formatted.as_ref()
                        .is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedImageMessage)
                    } else {
                        id!(ImageMessage)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let image_info = image.info.clone();
                        let text_or_image_ref = item.text_or_image(cx, ids!(content.message));
                        let is_image_fully_drawn = populate_image_message_content(
                            cx,
                            &text_or_image_ref,
                            app_language,
                            image_info,
                            image.source.clone(),
                            msg.body(),
                            media_cache,
                        );
                        new_drawn_status.content_drawn = is_image_fully_drawn;
                        (item, false)
                    }
                }
                MessageType::Location(location) => {
                    has_html_body = false;
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let is_location_fully_drawn = populate_location_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            location,
                        );
                        new_drawn_status.content_drawn = is_location_fully_drawn;
                        (item, false)
                    }
                }
                MessageType::File(file_content) => {
                    has_html_body = file_content.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        new_drawn_status.content_drawn = populate_file_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            file_content,
                            media_cache,
                        );
                        (item, false)
                    }
                }
                MessageType::Audio(audio) => {
                    has_html_body = audio.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        new_drawn_status.content_drawn = populate_audio_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            audio,
                        );
                        (item, false)
                    }
                }
                MessageType::Video(video) => {
                    has_html_body = video.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = if use_compact_view {
                        id!(CondensedMessage)
                    } else {
                        id!(Message)
                    };
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        new_drawn_status.content_drawn = populate_video_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            video,
                        );
                        (item, false)
                    }
                }
                MessageType::VerificationRequest(verification) => {
                    has_html_body = verification.formatted.as_ref().is_some_and(|f| f.format == MessageFormat::Html);
                    let template = id!(Message);
                    let (item, existed) = list.item_with_existed(cx, item_id, template);
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        // Use `FormattedBody` to hold our custom summary of this verification request.
                        let formatted = FormattedBody {
                            format: MessageFormat::Html,
                            body: format!(
                                "<i>{}<b>{}</b>{}<br>({}: {})</i>",
                                tr_key(app_language, "room_screen.verification.sent_prefix"),
                                tr_key(app_language, "room_screen.verification.request"),
                                tr_fmt(app_language, "room_screen.verification.sent_to_suffix", &[("user_id", verification.to.as_str())]),
                                tr_key(app_language, "room_screen.verification.supported_methods"),
                                verification.methods
                                    .iter()
                                    .map(|m| m.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            ),
                        };
                        let html_or_plaintext_ref =
                            item.html_or_plaintext(cx, ids!(content.message));
                        let mut link_preview_ref =
                            item.link_preview(cx, ids!(content.link_preview_view));

                        new_drawn_status.content_drawn = populate_text_message_content(
                            cx,
                            &html_or_plaintext_ref,
                            app_language,
                            &verification.body,
                            Some(&formatted),
                            Some(&mut link_preview_ref),
                            Some(media_cache),
                            Some(link_preview_cache),
                        );
                        (item, false)
                    }
                }
                _ => {
                    has_html_body = false;
                    let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
                    if existed && item_drawn_status.content_drawn {
                        (item, true)
                    } else {
                        item.label(cx, ids!(content.message)).set_text(
                            cx,
                            &format!("{} {:?}", tr_key(app_language, "room_screen.unsupported.prefix"), msg_like_content.kind),
                        );
                        new_drawn_status.content_drawn = true;
                        (item, false)
                    }
                }
            }
        }
        // Handle sticker messages that are static images.
        MsgLikeKind::Sticker(sticker) => {
            has_html_body = false;
            let StickerEventContent { body, info, source, .. } = sticker.content();

            let template = if use_compact_view {
                id!(CondensedImageMessage)
            } else {
                id!(ImageMessage)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);

            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                if let StickerMediaSource::Plain(owned_mxc_url) = source {
                    let image_info = info;
                    let text_or_image_ref = item.text_or_image(cx, ids!(content.message));
                    let is_image_fully_drawn = populate_image_message_content(
                        cx,
                        &text_or_image_ref,
                        app_language,
                        Some(Box::new(image_info.clone())),
                        MediaSource::Plain(owned_mxc_url.clone()),
                        body,
                        media_cache,
                    );
                    new_drawn_status.content_drawn = is_image_fully_drawn;
                    (item, false)
                } else {
                    (item, true)
                }
            }
        } 
        // Handle messages that have been redacted (deleted).
        MsgLikeKind::Redacted => {
            has_html_body = false;
            let template = if use_compact_view {
                id!(CondensedMessage)
            } else {
                id!(Message)
            };
            let (item, existed) = list.item_with_existed(cx, item_id, template);
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                let html_or_plaintext_ref = item.html_or_plaintext(cx, ids!(content.message));
                // Apply a smaller font size for redacted messages.
                let mut html_widget = html_or_plaintext_ref.html(cx, ids!(html_view.html));
                script_apply_eval!(cx, html_widget, {
                    font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE
                    text_style_normal +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_italic +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_bold +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_bold_italic +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                    text_style_fixed +: { font_size: mod.widgets.REDACTED_MESSAGE_FONT_SIZE }
                });
                new_drawn_status.content_drawn = populate_redacted_message_content(
                    cx,
                    &html_or_plaintext_ref,
                    app_language,
                    event_tl_item,
                    timeline_kind.room_id(),
                );
                (item, false)
            }
        }
        other => {
            has_html_body = false;
            let (item, existed) = list.item_with_existed(cx, item_id, id!(Message));
            if existed && item_drawn_status.content_drawn {
                (item, true)
            } else {
                item.label(cx, ids!(content.message)).set_text(
                    cx,
                    &format!("{} {:?} ", tr_key(app_language, "room_screen.unsupported.prefix"), other),
                );
                new_drawn_status.content_drawn = true;
                (item, false)
            }
        }
    };

    let timeline_event_id = event_tl_item.identifier();

    // If we didn't use a cached item, we need to draw all other message content:
    // the reactions, the read receipts avatar row, the reply preview.
    if !used_cached_item {
        item.reaction_list(cx, ids!(content.reaction_list)).set_list(
            cx,
            event_tl_item.content().reactions(),
            timeline_kind.clone(),
            timeline_event_id.clone(),
            item_id,
        );
        populate_read_receipts(&item, cx, timeline_kind, event_tl_item);
        let is_reply_fully_drawn = draw_replied_to_message(
            cx,
            &item.view(cx, ids!(replied_to_message)),
            timeline_kind,
            app_language,
            msg_like_content.in_reply_to.as_ref(),
            event_tl_item.event_id(),
        );
        let is_thread_summary_fully_drawn = populate_thread_root_summary(
            cx,
            &item,
            item_id,
            timeline_kind,
            app_language,
            msg_like_content,
            event_tl_item,
            fetched_thread_summaries,
            pending_thread_summary_fetches,
        );

        // The content is only considered to be fully drawn if the logic above marked it as such
        // *and* if the reply preview was also fully drawn
        // *and* if the thread root summary (if applicable) was also fully drawn.
        new_drawn_status.content_drawn &= is_reply_fully_drawn;
        new_drawn_status.content_drawn &= is_thread_summary_fully_drawn;
    }


    // We must always re-set the message details, even when re-using a cached portallist item,
    // because the item type might be the same but for a different message entirely.
    let message_details = MessageDetails {
        thread_root_event_id: msg_like_content.thread_root.clone().or_else(|| {
            msg_like_content.thread_summary.as_ref()
                .and_then(|_| event_tl_item.event_id().map(|id| id.to_owned()))
        }),
        timeline_event_id,
        item_id,
        related_event_id: msg_like_content.in_reply_to.as_ref().map(|r| r.event_id.clone()),
        room_screen_widget_uid,
        is_thread_timeline: timeline_kind.thread_root_event_id().is_some(),
        abilities: MessageAbilities::from_user_power_and_event(
            user_power_levels,
            event_tl_item,
            msg_like_content,
            pinned_events,
            has_html_body,
        ),
        should_be_highlighted: event_tl_item.is_highlighted(),
    };
    item.as_message().set_data(message_details);


    // If `used_cached_item` is false, we should always redraw the profile, even if profile_drawn is true.
    let skip_draw_profile =
        use_compact_view || (used_cached_item && item_drawn_status.profile_drawn);
    if skip_draw_profile {
        // log!("\t --> populate_message_view(): SKIPPING profile draw for item_id: {item_id}");
        new_drawn_status.profile_drawn = true;
    } else {
        // log!("\t --> populate_message_view(): DRAWING  profile draw for item_id: {item_id}");
        let mut username_label = item.label(cx, ids!(content.username));

        if !is_server_notice { // the normal case
            let (username, profile_drawn) = set_username_and_get_avatar_retval.unwrap_or_else(||
                item.avatar(cx, ids!(profile.avatar)).set_avatar_and_get_username(
                    cx,
                    timeline_kind,
                    event_tl_item.sender(),
                    Some(event_tl_item.sender_profile()),
                    event_tl_item.event_id(),
                    true,
                )
            );
            if is_notice {
                script_apply_eval!(cx, username_label, {
                    draw_text +: {
                        color: mod.widgets.COLOR_MESSAGE_NOTICE_TEXT
                    }
                });
            }
            username_label.set_text(cx, &username);
            new_drawn_status.profile_drawn = profile_drawn;

            // Show/hide the bot badge based on sender's user ID
            item.view(cx, ids!(content.username_view.bot_badge)).set_visible(cx, sender_is_bot);
        }
        else {
            // Server notices are drawn with a red color avatar background and username.
            let avatar = item.avatar(cx, ids!(profile.avatar));
            avatar.show_text(cx, Some(COLOR_FG_DANGER_RED), None, "⚠");
            username_label.set_text(cx, tr_key(app_language, "room_screen.server_notice.username"));
            script_apply_eval!(cx, username_label, {
                draw_text +: {
                    color: (mod.widgets.COLOR_FG_DANGER_RED)
                }
            });
            item.view(cx, ids!(content.username_view.bot_badge)).set_visible(cx, false);
            new_drawn_status.profile_drawn = true;
        }
    }

    let action_button_content = latest_effective_event_content_json(event_tl_item);
    let original_action_button_content = original_event_content_json(event_tl_item);
    let source_event_id = event_tl_item.event_id().map(|event_id| event_id.to_owned());
    populate_octos_action_buttons(
        cx,
        &item,
        action_button_content.as_ref(),
        original_action_button_content.as_ref(),
        source_event_id.as_ref(),
        event_tl_item.sender(),
        action_button_contexts,
        disabled_action_source_event_ids,
        selected_actions,
    );

    // If we've previously drawn the item content, skip all other steps.
    if used_cached_item && item_drawn_status.content_drawn && item_drawn_status.profile_drawn {
        return (item, new_drawn_status);
    }

    // Set the timestamp.
    if let Some(dt) = unix_time_millis_to_datetime(ts_millis) {
        item.timestamp(cx, ids!(profile.timestamp)).set_date_time(cx, dt);
    }

    // Suppress "edited" indicator for actively streaming messages.
    let is_streaming = event_tl_item.event_id()
        .is_some_and(|eid| streaming_messages.contains_key(&eid.to_owned()));
    if msg_like_content.as_message().is_some_and(|m| m.is_edited()) && !is_streaming {
        item.edited_indicator(cx, ids!(profile.edited_indicator))
            .set_latest_edit(cx, event_tl_item);
    }

    #[cfg(feature = "tsp")] {
        use matrix_sdk::ruma::serde::Base64;
        use crate::tsp::{self, tsp_sign_indicator::{TspSignState, TspSignIndicatorWidgetRefExt}};

        if let Some(mut tsp_sig) = event_tl_item.latest_json()
            .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
            .flatten()
            .and_then(|content_obj| content_obj.get("org.robius.tsp_signature").cloned())
            .and_then(|tsp_sig_value| serde_json::from_value::<Base64>(tsp_sig_value).ok())
            .map(|b64| b64.into_inner())
        {
            log!("Found event {:?} with TSP signature.", event_tl_item.event_id());
            let tsp_sign_state = if let Some(sender_vid) = tsp::tsp_state_ref().lock().unwrap()
                .get_verified_vid_for(event_tl_item.sender())
            {
                log!("Found verified VID for sender {}: \"{}\"", event_tl_item.sender(), sender_vid.identifier());
                tsp_sdk::crypto::verify(&*sender_vid, &mut tsp_sig).map_or(
                    TspSignState::WrongSignature,
                    |(msg, msg_type)| {
                        log!("TSP signature verified successfully!\n    Msg type: {msg_type:?}\n    Message: {:?} ({msg:X?})", std::str::from_utf8(msg));
                        TspSignState::Verified
                    }
                )
            } else {
                TspSignState::Unknown
            };

            log!("TSP signature state for event {:?} is {:?}", event_tl_item.event_id(), tsp_sign_state);
            item.tsp_sign_indicator(cx, ids!(profile.tsp_sign_indicator))
                .show_with_state(cx, tsp_sign_state);
        }
    }

    (item, new_drawn_status)
}

/// Draws the Html or plaintext body of the given Text or Notice message into the `message_content_widget`.
/// Also populates link previews if a link_preview_ref is provided.
/// Returns whether the text items were fully drawn.
fn populate_text_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    body: &str,
    formatted_body: Option<&FormattedBody>,
    link_preview_ref: Option<&mut LinkPreviewRef>,
    media_cache: Option<&mut MediaCache>,
    link_preview_cache: Option<&mut LinkPreviewCache>,
) -> bool {
    // The message was HTML-formatted rich text.
    let mut links = Vec::new();
    if let Some(fb) = formatted_body.as_ref()
        .and_then(|fb| (fb.format == MessageFormat::Html).then_some(fb))
    {
        let linkified_html = utils::linkify_get_urls(
            utils::trim_start_html_whitespace(&fb.body),
            true,
            Some(&mut links),
        );
        message_content_widget.show_html(cx, linkified_html);
    }
    // The message was non-HTML plaintext.
    else {
        let linkified_html = utils::linkify_get_urls(body, false, Some(&mut links));
        match linkified_html {
            Cow::Owned(linkified_html) => message_content_widget.show_html(cx, &linkified_html),
            Cow::Borrowed(plaintext) => message_content_widget.show_plaintext(cx, plaintext),
        }
    };

    // Populate link previews if all required parameters are provided
    if let (Some(link_preview_ref), Some(media_cache), Some(link_preview_cache)) = 
        (link_preview_ref, media_cache, link_preview_cache)
    {
        link_preview_ref.populate_below_message(
            cx,
            &links,
            media_cache,
            link_preview_cache,
            &|cx, text_or_image_ref, image_info_source, original_source, body, media_cache| {
                populate_image_message_content(
                    cx,
                    text_or_image_ref,
                    app_language,
                    image_info_source,
                    original_source,
                    body,
                    media_cache,
                )
            },
        )
    } else {
        true
    }
}

fn populate_bot_text_message_content(
    cx: &mut Cx,
    item: &WidgetRef,
    app_language: AppLanguage,
    body: &str,
    formatted_body: Option<&FormattedBody>,
    link_preview_ref: Option<&mut LinkPreviewRef>,
    media_cache: Option<&mut MediaCache>,
    link_preview_cache: Option<&mut LinkPreviewCache>,
    is_bot_sender: bool,
) -> bool {
    let render_state = compute_bot_timeline_render_state(body, is_bot_sender);
    let bot_card_view = item.view(cx, ids!(content.bot_message_card));
    let message_view = item.html_or_plaintext(cx, ids!(content.message));

    bot_card_view.set_visible(cx, render_state.show_card);
    message_view.set_visible(cx, !render_state.show_card);

    if !render_state.show_card {
        return populate_text_message_content(
            cx,
            &message_view,
            app_language,
            body,
            formatted_body,
            link_preview_ref,
            media_cache,
            link_preview_cache,
        );
    }

    let status_strip = item.view(cx, ids!(content.bot_message_card.bot_status_strip));
    status_strip.set_visible(cx, render_state.show_status_strip);
    if let Some(status) = render_state.status.as_ref() {
        item.label(cx, ids!(content.bot_message_card.bot_status_strip.bot_status_label))
            .set_text(cx, status);
    }

    let provider_label = item.label(cx, ids!(content.bot_message_card.bot_metadata_footer.bot_provider_label));
    if let Some(provider) = render_state.provider.as_ref() {
        provider_label.set_text(cx, provider);
        provider_label.set_visible(cx, true);
    } else {
        provider_label.set_visible(cx, false);
    }

    let footer_label = item.label(cx, ids!(content.bot_message_card.bot_metadata_footer.bot_footer_label));
    if let Some(footer) = render_state.footer.as_ref() {
        footer_label.set_text(cx, display_bot_footer_text(footer));
        footer_label.set_visible(cx, true);
    } else {
        footer_label.set_visible(cx, false);
    }
    item.view(cx, ids!(content.bot_message_card.bot_metadata_footer))
        .set_visible(cx, render_state.show_metadata_footer);

    let body_card = item.view(cx, ids!(content.bot_message_card.bot_body_card));
    body_card.set_visible(cx, render_state.show_body_card);
    let body_widget = item.html_or_plaintext(cx, ids!(content.bot_message_card.bot_body_card.bot_card_body));
    let mut markdown_widget = item.markdown(cx, ids!(content.bot_message_card.bot_body_card.bot_card_markdown));
    let mut markdown_plain_widget = item.markdown(cx, ids!(content.bot_message_card.bot_body_card.bot_card_markdown_plain));
    let code_block_mode = bot_timeline_code_block_mode(&render_state);
    body_widget.set_visible(cx, code_block_mode == BotTimelineCodeBlockMode::None);
    markdown_widget.set_visible(cx, code_block_mode == BotTimelineCodeBlockMode::Highlighted);
    markdown_plain_widget.set_visible(cx, code_block_mode == BotTimelineCodeBlockMode::Plain);

    if render_state.show_body_card {
        if code_block_mode != BotTimelineCodeBlockMode::None {
            match code_block_mode {
                BotTimelineCodeBlockMode::Highlighted => markdown_widget.set_text(cx, &render_state.body),
                BotTimelineCodeBlockMode::Plain => markdown_plain_widget.set_text(cx, &render_state.body),
                BotTimelineCodeBlockMode::None => { }
            }

            if let (Some(link_preview_ref), Some(media_cache), Some(link_preview_cache)) =
                (link_preview_ref, media_cache, link_preview_cache)
            {
                let mut links = Vec::new();
                let _ = utils::linkify_get_urls(&render_state.body, false, Some(&mut links));
                link_preview_ref.populate_below_message(
                    cx,
                    &links,
                    media_cache,
                    link_preview_cache,
                    &|cx, text_or_image_ref, image_info_source, original_source, body, media_cache| {
                        populate_image_message_content(
                            cx,
                            text_or_image_ref,
                            app_language,
                            image_info_source,
                            original_source,
                            body,
                            media_cache,
                        )
                    },
                )
            } else {
                true
            }
        } else {
            let formatted_body_for_card =
                select_bot_timeline_body_formatted_body(&render_state, formatted_body);
            populate_text_message_content(
                cx,
                &body_widget,
                app_language,
                &render_state.body,
                formatted_body_for_card.as_ref(),
                link_preview_ref,
                media_cache,
                link_preview_cache,
            )
        }
    } else {
        true
    }
}

fn populate_octos_action_buttons(
    cx: &mut Cx,
    item: &WidgetRef,
    content: Option<&serde_json::Value>,
    original_content: Option<&serde_json::Value>,
    source_event_id: Option<&OwnedEventId>,
    original_sender: &UserId,
    action_button_contexts: &mut HashMap<WidgetUid, OctosActionButtonContext>,
    disabled_source_event_ids: &HashSet<OwnedEventId>,
    selected_actions: &HashMap<OwnedEventId, SelectedOctosActionState>,
) {
    let container = item.view(cx, ids!(content.action_buttons));
    let approval_request_view = item.view(cx, ids!(content.action_buttons.approval_request_view));
    let button_row = item.view(cx, ids!(content.action_buttons.action_button_row));
    let Some(source_event_id) = source_event_id else {
        container.set_visible(cx, false);
        return;
    };

    let parsed_payload = parse_octos_action_payload_for_render(content, original_content);

    if parsed_payload.malformed_approval_request {
        warning!("org.octos.approval_request: skipping malformed approval request");
    }

    let render_state = compute_action_button_render_state(
        &parsed_payload.actions,
        parsed_payload.approval_request.as_ref(),
        current_user_id().as_deref(),
    );
    let is_disabled = are_action_buttons_disabled(disabled_source_event_ids, source_event_id.as_ref())
        || !render_state.buttons_enabled;
    let selected_action = selected_actions.get(source_event_id);
    let visible_slots = action_button_render_slots_for_display(&render_state, selected_action);

    container.set_visible(cx, render_state.show_container);
    button_row.set_visible(cx, render_state.show_button_row && !visible_slots.is_empty());
    approval_request_view.set_visible(cx, render_state.approval_card.is_some());
    if let Some(approval_card) = render_state.approval_card.as_ref() {
        item.label(cx, ids!(content.action_buttons.approval_request_view.approval_title_label))
            .set_text(cx, &approval_card.title);
        item.label(cx, ids!(content.action_buttons.approval_request_view.approval_summary_label))
            .set_text(cx, &approval_card.summary);
    }

    for index in 0..MAX_OCTOS_ACTION_BUTTONS {
        let (slot_path, primary_path, secondary_path, danger_path) = octos_action_button_paths(index);
        item.view(cx, slot_path).set_visible(cx, false);

        let primary_button = item.button(cx, primary_path);
        action_button_contexts.remove(&primary_button.widget_uid());
        primary_button.set_visible(cx, false);
        primary_button.set_enabled(cx, !is_disabled);

        let secondary_button = item.button(cx, secondary_path);
        action_button_contexts.remove(&secondary_button.widget_uid());
        secondary_button.set_visible(cx, false);
        secondary_button.set_enabled(cx, !is_disabled);

        let danger_button = item.button(cx, danger_path);
        action_button_contexts.remove(&danger_button.widget_uid());
        danger_button.set_visible(cx, false);
        danger_button.set_enabled(cx, !is_disabled);

        let Some(render_slot) = visible_slots.get(index) else { continue };
        item.view(cx, slot_path).set_visible(cx, true);

        let active_button = match render_slot.style {
            OctosActionStyle::Primary => primary_button,
            OctosActionStyle::Secondary => secondary_button,
            OctosActionStyle::Danger => danger_button,
        };
        active_button.set_visible(cx, true);
        active_button.set_enabled(cx, !is_disabled);
        active_button.set_text(cx, &render_slot.label);

        if !is_disabled {
            let request = if let Some(approval_request) = parsed_payload.approval_request.as_ref() {
                OctosActionButtonRequest::Approval {
                    request_id: approval_request.request_id.clone(),
                    title: approval_request.title.clone(),
                    decision: render_slot.id.clone(),
                    label: render_slot.label.clone(),
                    tool_args_digest: approval_request.tool_args_digest.clone(),
                    style: render_slot.style,
                }
            } else {
                OctosActionButtonRequest::Generic {
                    action_id: render_slot.id.clone(),
                    label: render_slot.label.clone(),
                    style: render_slot.style,
                }
            };

            action_button_contexts.insert(active_button.widget_uid(), OctosActionButtonContext {
                source_event_id: source_event_id.clone(),
                original_sender: original_sender.to_owned(),
                request,
            });
        }
    }
}

/// Draws the given image message's content into the `message_content_widget`.
///
/// Returns whether the image message content was fully drawn.
fn populate_image_message_content(
    cx: &mut Cx,
    text_or_image_ref: &TextOrImageRef,
    app_language: AppLanguage,
    image_info_source: Option<Box<ImageInfo>>,
    original_source: MediaSource,
    body: &str,
    media_cache: &mut MediaCache,
) -> bool {
    // We don't use thumbnails, as their resolution is too low to be visually useful.
    // We also don't trust the provided mimetype, as it can be incorrect.
    let (mimetype, _width, _height) = image_info_source.as_ref()
        .map(|info| (info.mimetype.as_deref(), info.width, info.height))
        .unwrap_or_default();

    // If we have a known mimetype and it's not a static image,
    // then show a message about it being unsupported (e.g., for animated gifs).
    if let Some(mime) = mimetype.as_ref() {
        if ImageFormat::from_mimetype(mime).is_none() {
            text_or_image_ref.show_text(
                cx,
                tr_fmt(app_language, "room_screen.image.unsupported_type", &[("body", body), ("mime", mime)]),
            );
            return true; // consider this as fully drawn
        }
    }

    let mut fully_drawn = false;

    // A closure that fetches and shows the image from the given `mxc_uri`,
    // marking it as fully drawn if the image was available.
    let mut fetch_and_show_image_uri = |cx: &mut Cx, mxc_uri: OwnedMxcUri, image_info: Box<ImageInfo>| {
        match media_cache.try_get_media_or_fetch(&mxc_uri, MEDIA_THUMBNAIL_FORMAT.into()) {
            (MediaCacheEntry::Loaded(data), _media_format) => {
                let show_image_result = text_or_image_ref.show_image(cx, Some(MediaSource::Plain(mxc_uri)),|cx, img| {
                    utils::load_png_or_jpg(&img, cx, &data)
                        .map(|()| img.size_in_pixels(cx).unwrap_or_default())
                });
                if let Err(e) = show_image_result {
                    let err_str = tr_fmt(app_language, "room_screen.image.failed_to_display", &[("body", body), ("error", &format!("{e:?}"))]);
                    error!("{err_str}");
                    text_or_image_ref.show_text(cx, &err_str);
                }

                // We're done drawing the image, so mark it as fully drawn.
                fully_drawn = true;
            }
            (MediaCacheEntry::Requested, _media_format) => {
                // If the image is being fetched, we try to show its blurhash.
                if let (Some(ref blurhash), Some(width), Some(height)) = (image_info.blurhash.clone(), image_info.width, image_info.height) {
                    let show_image_result = text_or_image_ref.show_image(cx, Some(MediaSource::Plain(mxc_uri)), |cx, img| {
                        let (Ok(width), Ok(height)) = (width.try_into(), height.try_into()) else {
                            return Err(image_cache::ImageError::EmptyData)
                        };
                        let (width, height): (u32, u32) = (width, height);
                        if width == 0 || height == 0 {
                            warning!("Image had an invalid aspect ratio (width or height of 0).");
                            return Err(image_cache::ImageError::EmptyData);
                        }
                        let aspect_ratio: f32 = width as f32 / height as f32;
                        // Cap the blurhash to a max size of 500 pixels in each dimension
                        // because the `blurhash::decode()` function can be rather expensive.
                        let (mut capped_width, mut capped_height) = (width, height);
                        if capped_height > BLURHASH_IMAGE_MAX_SIZE {
                            capped_height = BLURHASH_IMAGE_MAX_SIZE;
                            capped_width = (capped_height as f32 * aspect_ratio).floor() as u32;
                        }
                        if capped_width > BLURHASH_IMAGE_MAX_SIZE {
                            capped_width = BLURHASH_IMAGE_MAX_SIZE;
                            capped_height = (capped_width as f32 / aspect_ratio).floor() as u32;
                        }

                        match blurhash::decode(blurhash, capped_width, capped_height, 1.0) {
                            Ok(data) => {
                                ImageBuffer::new(&data, capped_width as usize, capped_height as usize).map(|img_buff| {
                                    let texture = Some(img_buff.into_new_texture(cx));
                                    img.set_texture(cx, texture);
                                    img.size_in_pixels(cx).unwrap_or_default()
                                })
                            }
                            Err(e) => {
                                error!("Failed to decode blurhash {e:?}");
                                Err(image_cache::ImageError::EmptyData)
                            }   
                        }
                    });
                    if let Err(e) = show_image_result {
                        let err_str = tr_fmt(app_language, "room_screen.image.failed_to_display", &[("body", body), ("error", &format!("{e:?}"))]);
                        error!("{err_str}");
                        text_or_image_ref.show_text(cx, &err_str);
                    }
                }
                fully_drawn = false;
            }
            (MediaCacheEntry::Failed(_status_code), _media_format) => {
                if text_or_image_ref.view(cx, ids!(default_image_view)).visible() {
                    fully_drawn = true;
                    return;
                }
                text_or_image_ref
                    .show_text(cx, tr_fmt(app_language, "room_screen.image.failed_to_fetch", &[("body", body), ("mxc_uri", &format!("{mxc_uri:?}"))]));
                // For now, we consider this as being "complete". In the future, we could support
                // retrying to fetch thumbnail of the image on a user click/tap.
                fully_drawn = true;
            }
        }
    };

    let mut fetch_and_show_media_source = |cx: &mut Cx, media_source: MediaSource, image_info: Box<ImageInfo>| {
        match media_source {
            MediaSource::Encrypted(encrypted) => {
                // We consider this as "fully drawn" since we don't yet support encryption.
                text_or_image_ref.show_text(
                    cx,
                    tr_fmt(app_language, "room_screen.image.encrypted_todo", &[("body", body), ("url", &format!("{:?}", encrypted.url))])
                );
            },
            MediaSource::Plain(mxc_uri) => {
                fetch_and_show_image_uri(cx, mxc_uri, image_info)
            }
        }
    };

    match image_info_source {
        Some(image_info) => {
            // Use the provided thumbnail URI if it exists; otherwise use the original URI.
            let media_source = image_info.thumbnail_source.clone()
                .unwrap_or(original_source);
            fetch_and_show_media_source(cx, media_source, image_info);
        }
        None => {
            text_or_image_ref.show_text(cx, tr_fmt(app_language, "room_screen.image.no_source_url", &[("body", body)]));
            fully_drawn = true;
        }
    }

    fully_drawn
}


/// Draws a file message's content into the given `message_content_widget`.
///
/// Returns whether the file message content was fully drawn.
///
/// File download is NOT triggered automatically during rendering.
/// The user must click the `mxc://` link in the rendered HTML to initiate
/// the download via the existing `RobrixHtmlLinkAction` handler.
fn populate_file_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    file_content: &FileMessageEventContent,
    _media_cache: &mut MediaCache,
) -> bool {
    let filename = htmlize::escape_text(file_content.filename());
    let size = file_content
        .info
        .as_ref()
        .and_then(|info| info.size)
        .map(|bytes| format!("  ({})", ByteSize::b(bytes.into())))
        .unwrap_or_default();
    // Escape caption to prevent HTML injection from untrusted message content
    let caption = file_content.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", htmlize::escape_text(&fb.body)))
        .or_else(|| file_content.caption().map(|c| format!("<br><i>{}</i>", htmlize::escape_text(c))))
        .unwrap_or_default();

    // Build a clickable mxc:// link so the user can explicitly trigger download.
    // The link is handled by `RobrixHtmlLinkAction` / `robius_open` in the room screen.
    let download_link = match &file_content.source {
        MediaSource::Plain(mxc_uri) => {
            format!(
                "<br>→ <a href=\"{}\">{}</a>",
                htmlize::escape_text(mxc_uri.as_str()),
                tr_key(app_language, "room_screen.file.download"),
            )
        }
        MediaSource::Encrypted(_) => {
            format!("<br>→ <i>{}</i>", tr_key(app_language, "room_screen.file.encrypted_not_supported"))
        }
    };

    message_content_widget.show_html(
        cx,
        format!("<b>{filename}</b>{size}{caption}{download_link}"),
    );
    true
}

/// Draws an audio message's content into the given `message_content_widget`.
///
/// Returns whether the audio message content was fully drawn.
fn populate_audio_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    audio: &AudioMessageEventContent,
) -> bool {
    // Display the file name, human-readable size, caption, and a button to download it.
    let filename = htmlize::escape_text(audio.filename());
    let (duration, mime, size) = audio
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!("  {:.2} sec,", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!("  {m},"))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({}),", ByteSize::b(bytes.into())))
                .unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = audio.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", fb.body))
        .or_else(|| audio.caption().map(|c| format!("<br><i>{c}</i>")))
        .unwrap_or_default();

    // TODO: add an audio to play the audio file

    message_content_widget.show_html(
        cx,
        tr_fmt(app_language, "room_screen.audio.preview_html", &[
            ("filename", &filename),
            ("mime", mime.as_str()),
            ("duration", duration.as_str()),
            ("size", size.as_str()),
            ("caption", caption.as_str()),
        ]),
    );
    true
}


/// Draws a video message's content into the given `message_content_widget`.
///
/// Returns whether the video message content was fully drawn.
fn populate_video_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    video: &VideoMessageEventContent,
) -> bool {
    // Display the file name, human-readable size, caption, and a button to download it.
    let filename = htmlize::escape_text(video.filename());
    let (duration, mime, size, dimensions) = video
        .info
        .as_ref()
        .map(|info| (
            info.duration
                .map(|d| format!("  {:.2} sec,", d.as_secs_f64()))
                .unwrap_or_default(),
            info.mimetype
                .as_ref()
                .map(|m| format!("  {m},"))
                .unwrap_or_default(),
            info.size
                .map(|bytes| format!("  ({}),", ByteSize::b(bytes.into())))
                .unwrap_or_default(),
            info.width.and_then(|width|
                info.height.map(|height| format!("  {width}x{height},"))
            ).unwrap_or_default(),
        ))
        .unwrap_or_default();
    let caption = video.formatted_caption()
        .map(|fb| format!("<br><i>{}</i>", fb.body))
        .or_else(|| video.caption().map(|c| format!("<br><i>{c}</i>")))
        .unwrap_or_default();

    // TODO: add an video to play the video file

    message_content_widget.show_html(
        cx,
        tr_fmt(app_language, "room_screen.video.preview_html", &[
            ("filename", &filename),
            ("mime", mime.as_str()),
            ("duration", duration.as_str()),
            ("size", size.as_str()),
            ("dimensions", dimensions.as_str()),
            ("caption", caption.as_str()),
        ]),
    );
    true
}



/// Draws the given location message's content into the `message_content_widget`.
///
/// Returns whether the location message content was fully drawn.
fn populate_location_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    location: &LocationMessageEventContent,
) -> bool {
    let coords = location.geo_uri
        .get(utils::GEO_URI_SCHEME.len() ..)
        .and_then(|s| {
            let mut iter = s.split(',');
            if let (Some(lat), Some(long)) = (iter.next(), iter.next()) {
                Some((lat, long))
            } else {
                None
            }
        });
    if let Some((lat, long)) = coords {
        let short_lat = lat.find('.').and_then(|dot| lat.get(..dot + 7)).unwrap_or(lat);
        let short_long = long.find('.').and_then(|dot| long.get(..dot + 7)).unwrap_or(long);
        let safe_lat = htmlize::escape_attribute(lat);
        let safe_long = htmlize::escape_attribute(long);
        let safe_geo_uri = htmlize::escape_attribute(&location.geo_uri);
        let safe_short_lat = htmlize::escape_text(short_lat);
        let safe_short_long = htmlize::escape_text(short_long);
        let html_body = format!(
            "{} <a href=\"{}\">{safe_short_lat},{safe_short_long}</a><br>\
            <ul>\
            <li><a href=\"https://www.openstreetmap.org/?mlat={safe_lat}&amp;mlon={safe_long}#map=15/{safe_lat}/{safe_long}\">{}</a></li>\
            <li><a href=\"https://www.google.com/maps/search/?api=1&amp;query={safe_lat},{safe_long}\">{}</a></li>\
            <li><a href=\"https://maps.apple.com/?ll={safe_lat},{safe_long}&amp;q={safe_lat},{safe_long}\">{}</a></li>\
            </ul>",
            tr_key(app_language, "room_screen.location.label"),
            safe_geo_uri,
            tr_key(app_language, "room_screen.location.open_osm"),
            tr_key(app_language, "room_screen.location.open_google_maps"),
            tr_key(app_language, "room_screen.location.open_apple_maps"),
        );
        message_content_widget.show_html(cx, html_body);
    } else {
        let escaped_body = htmlize::escape_text(&location.body);
        message_content_widget.show_html(
            cx,
            tr_fmt(app_language, "room_screen.location.invalid_html", &[
                ("body", &escaped_body),
            ])
        );
    }

    // Currently we do not fetch location thumbnail previews, so we consider this as fully drawn.
    // In the future, when we do support this, we'll return false until the thumbnail is fetched,
    // at which point we can return true.
    true
}


/// Draws the given redacted message's content into the `message_content_widget`.
///
/// Returns whether the redacted message content was fully drawn.
fn populate_redacted_message_content(
    cx: &mut Cx,
    message_content_widget: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    event_tl_item: &EventTimelineItem,
    room_id: &OwnedRoomId,
) -> bool {
    let fully_drawn: bool;
    let mut redactor_id_and_reason = None;
    if let Some(redacted_msg) = event_tl_item.latest_json() {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            AnySyncMessageLikeEvent::RoomMessage(
                SyncMessageLikeEvent::Redacted(redaction)
            )
        )) = redacted_msg.deserialize() {
            if let Ok(redacted_because) = redaction.unsigned.redacted_because.deserialize() {
                redactor_id_and_reason = Some((
                    redacted_because.sender,
                    redacted_because.content.reason,
                ));
            }
        }
    }

    let html = if let Some((redactor, reason)) = redactor_id_and_reason {
        if redactor == event_tl_item.sender() {
            fully_drawn = true;
            match reason {
                Some(r) => {
                    let escaped_reason = htmlize::escape_text(r);
                    tr_fmt(app_language, "room_screen.redacted.self_with_reason", &[
                        ("reason", &escaped_reason),
                    ])
                }
                None => tr_key(app_language, "room_screen.redacted.self").to_string(),
            }
        } else {
            // Try to get the displayable name of the user who redacted this message.
            let redactor_name = user_profile_cache::get_user_display_name_for_room(
                cx,
                redactor.clone(),
                Some(room_id),
                true,
            );
            fully_drawn = redactor_name.was_found();
            let redactor_name_esc = htmlize::escape_text(redactor_name.as_deref().unwrap_or(redactor.as_str()));
            match reason {
                Some(r) => {
                    let escaped_reason = htmlize::escape_text(r);
                    tr_fmt(app_language, "room_screen.redacted.other_with_reason", &[
                        ("redactor", &redactor_name_esc),
                        ("reason", &escaped_reason),
                    ])
                }
                None => tr_fmt(app_language, "room_screen.redacted.other", &[
                    ("redactor", &redactor_name_esc),
                ]),
            }
        }
    } else {
        fully_drawn = true;
        tr_key(app_language, "room_screen.redacted.generic").to_string()
    };
    message_content_widget.show_html(cx, html);
    fully_drawn
}


/// Draws a ReplyPreview above a message if it was in-reply to another message.
///
/// ## Arguments
/// * `replied_to_message_view`: the destination `RepliedToMessage` view that will be populated.
/// * `timeline_kind`: the [`TimelineKind`] of the timeline that is being drawn.
/// * `in_reply_to`: if `Some`, the details that will be used to populate the `replied_to_message_view`.
///   If `None`, this function will mark it as non-visible and consider it fully drawn.
/// * `message_event_id`: the [`EventId`] of the message that is the reply itself (the response).
///   This is needed to fetch the details of the replied-to message (if not yet available).
///
/// Returns whether the in-reply-to information was available and fully drawn,
/// i.e., whether it can be considered cached and not needing to be redrawn later.
fn draw_replied_to_message(
    cx: &mut Cx2d,
    replied_to_message_view: &ViewRef,
    timeline_kind: &TimelineKind,
    app_language: AppLanguage,
    in_reply_to: Option<&InReplyToDetails>,
    message_event_id: Option<&EventId>,
) -> bool {
    let fully_drawn: bool;
    let show_reply: bool;

    if let Some(in_reply_to_details) = in_reply_to {
        show_reply = true;
        match &in_reply_to_details.event {
            TimelineDetails::Ready(replied_to_event) => {
                let (in_reply_to_username, is_avatar_fully_drawn) =
                    replied_to_message_view
                        .avatar(cx, ids!(replied_to_message_content.reply_preview_avatar))
                        .set_avatar_and_get_username(
                            cx,
                            timeline_kind,
                            &replied_to_event.sender,
                            Some(&replied_to_event.sender_profile),
                            Some(in_reply_to_details.event_id.as_ref()),
                            true,
                        );

                fully_drawn = is_avatar_fully_drawn;

                replied_to_message_view
                    .label(cx, ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, in_reply_to_username.as_str());
                let msg_body = replied_to_message_view.html_or_plaintext(cx, ids!(reply_preview_body));
                populate_preview_of_timeline_item(
                    cx,
                    &msg_body,
                    app_language,
                    &replied_to_event.content,
                    &replied_to_event.sender,
                    &in_reply_to_username,
                );
            }
            TimelineDetails::Error(_e) => {
                fully_drawn = true;
                replied_to_message_view
                    .label(cx, ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, tr_key(app_language, "room_screen.reply_preview.error_username"));
                replied_to_message_view
                    .avatar(cx, ids!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(cx, ids!(replied_to_message_content.reply_preview_body))
                    .show_plaintext(cx, tr_key(app_language, "room_screen.reply_preview.error_event"));
            }
            td @ TimelineDetails::Pending | td @ TimelineDetails::Unavailable => {
                // We don't have the replied-to message yet, so we can't fully draw the preview.
                fully_drawn = false;
                replied_to_message_view
                    .label(cx, ids!(replied_to_message_content.reply_preview_username))
                    .set_text(cx, tr_key(app_language, "room_screen.reply_preview.loading_username"));
                replied_to_message_view
                    .avatar(cx, ids!(replied_to_message_content.reply_preview_avatar))
                    .show_text(cx, None, None, "?");
                replied_to_message_view
                    .html_or_plaintext(cx, ids!(replied_to_message_content.reply_preview_body))
                    .show_plaintext(cx, tr_key(app_language, "room_screen.reply_preview.loading_event"));

                // Confusingly, we need to fetch the details of the `message` (the event that is the reply),
                // not the details of the original event that this `message` is replying to.
                if matches!(td, TimelineDetails::Unavailable) {
                    if let Some(event_id) = message_event_id {
                        submit_async_request(MatrixRequest::FetchDetailsForEvent {
                            timeline_kind: timeline_kind.clone(),
                            event_id: event_id.to_owned(),
                        });
                    }
                }
            }
        }
    } else {
        // This message was not in reply to another message, so we don't need to show a reply.
        show_reply = false;
        fully_drawn = true;
    }

    replied_to_message_view.set_visible(cx, show_reply);
    fully_drawn
}

/// Draws a one-line thread summary at the bottom of a message if it is the root of a thread.
///
/// Returns whether the thread summary information was available and fully drawn,
/// i.e., whether it can be considered cached and not needing to be redrawn later.
fn populate_thread_root_summary(
    cx: &mut Cx2d,
    item: &WidgetRef,
    timeline_item_index: usize,
    timeline_kind: &TimelineKind,
    app_language: AppLanguage,
    msg_like_content: &MsgLikeContent,
    event_tl_item: &EventTimelineItem,
    fetched_thread_summaries: &HashMap<OwnedEventId, FetchedThreadSummary>,
    pending_thread_summary_fetches: &mut HashSet<OwnedEventId>,
) -> bool {
    let thread_summary_view = item.view(cx, ids!(thread_root_summary));
    thread_summary_view.set_visible(cx, false); // hide by default
    let fully_drawn: bool;

    if matches!(timeline_kind, TimelineKind::Thread { .. }) {
        // If we're already drawing a message in a thread-focused timeline,
        // it doesn't make sense to show a redundant thread summary.
        fully_drawn = true;
        return fully_drawn;
    }

    let Some(thread_summary) = msg_like_content.thread_summary.as_ref() else {
        // consider this as fully drawn since there's no thread summary to show.
        fully_drawn = true;
        return fully_drawn;
    };

    // Here, we actually need to show the thread summary.
    thread_summary_view.set_visible(cx, true);
    let local_num_replies = thread_summary.num_replies;
    let thread_root_event_id = event_tl_item.event_id().map(|id| id.to_owned());
    let fetched_summary = thread_root_event_id
        .as_ref()
        .and_then(|root_id| fetched_thread_summaries.get(root_id));
    let replies_count = fetched_summary
        .map(|f| f.num_replies)
        .unwrap_or(local_num_replies);

    let latest_preview: Cow<str> = match &thread_summary.latest_event {
        TimelineDetails::Ready(embedded_event) => {
            fully_drawn = true;
            let sender_username = match &embedded_event.sender_profile {
                TimelineDetails::Ready(profile) => profile
                    .display_name
                    .as_deref()
                    .unwrap_or(embedded_event.sender.as_str()),
                _ => embedded_event.sender.as_str(),
            };
            let preview = text_preview_of_timeline_item(
                &embedded_event.content,
                &embedded_event.sender,
                sender_username,
            ).format_with(sender_username, true);
            match utils::replace_linebreaks_separators(&preview, true) {
                Cow::Borrowed(_) => Cow::Owned(preview),
                Cow::Owned(replaced) => Cow::Owned(replaced),
            }
        }
        td @ TimelineDetails::Pending | td @ TimelineDetails::Unavailable => {
            fully_drawn = true;
            if td.is_unavailable()
                && let Some(thread_root_event_id) = thread_root_event_id.clone()
            {
                let needs_refresh = fetched_summary
                    .is_none_or(|fs| fs.latest_reply_preview_text.is_none());
                if needs_refresh && pending_thread_summary_fetches.insert(thread_root_event_id.clone()) {
                    submit_async_request(MatrixRequest::FetchThreadSummaryDetails {
                        timeline_kind: timeline_kind.clone(),
                        thread_root_event_id,
                        timeline_item_index,
                    });
                }
            }
            fetched_summary.and_then(|fs| fs.latest_reply_preview_text.as_deref())
                .unwrap_or(tr_key(app_language, "room_screen.thread_summary.loading_latest_reply"))
                .into()
        }
        TimelineDetails::Error(_) => {
            fully_drawn = true; // consider this fully drawn since there's no point retrying.
            tr_key(app_language, "room_screen.thread_summary.error_latest_reply").into()
        }
    };

    let replies_count_text = match replies_count {
        1 => Cow::Borrowed(tr_key(app_language, "room_screen.thread_summary.one_reply")),
        n => Cow::Owned(tr_fmt(app_language, "room_screen.thread_summary.n_replies", &[("n", &n.to_string())]))
    };
    item.label(cx, ids!(thread_summary_count))
        .set_text(cx, &replies_count_text);
    item.html(cx, ids!(thread_summary_latest))
        .set_text(cx, &latest_preview);
    fully_drawn
}

/// Generates a rich HTML text preview of the given `timeline_item_content`
/// and populates the given `widget_out` with that content.
pub fn populate_preview_of_timeline_item(
    cx: &mut Cx,
    widget_out: &HtmlOrPlaintextRef,
    app_language: AppLanguage,
    timeline_item_content: &TimelineItemContent,
    sender_user_id: &UserId,
    sender_username: &str,
) {
    if let Some(m) = timeline_item_content.as_message() {
        match m.msgtype() {
            MessageType::Text(TextMessageEventContent { body, formatted, .. })
            | MessageType::Notice(NoticeMessageEventContent { body, formatted, .. }) => {
                let _ = populate_text_message_content(cx, widget_out, app_language, body, formatted.as_ref(), None, None, None);
                return;
            }
            _ => { } // fall through to the general case for all timeline items below.
        }
    }
    let html = text_preview_of_timeline_item(
        timeline_item_content,
        sender_user_id,
        sender_username,
    ).format_with(sender_username, true);
    widget_out.show_html(cx, html);
}


/// A trait for abstracting over the different types of timeline events
/// that can be displayed in a `SmallStateEvent` widget.
trait SmallStateEventContent {
    /// Populates the *content* (not the profile) of the given `item` with data from
    /// the given `event_tl_item` and `self` (the specific type of event content).
    ///
    /// ## Arguments
    /// * `item`: a `SmallStateEvent` widget that has already been added to
    ///   the given `PortalList` at the given `item_id`.
    ///   This function may either modify that item or completely replace it
    ///   with a different widget if needed.
    /// * `item_drawn_status`: the old (prior) drawn status of the item.
    /// * `new_drawn_status`: the new drawn status of the item, which may have already
    ///   been updated to reflect the item's profile having been drawn right before this function.
    ///
    /// ## Return
    /// Returns a tuple of the drawn `item` and its `new_drawn_status`.
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        event_tl_item: &EventTimelineItem,
        username: &str,
        item_drawn_status: ItemDrawnStatus,
        new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus);
}

// For unable to decrypt messages.
impl SmallStateEventContent for EncryptedMessage {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(cx, ids!(content)).set_text(
            cx,
            &text_preview_of_encrypted_message(self).format_with(username, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

// For other message-like content (custom message-like events).
impl SmallStateEventContent for LiveLocationState {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(cx, ids!(content)).set_text(
            cx,
            &format!("{username} shared a live location."),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for OtherMessageLike {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(cx, ids!(content)).set_text(
            cx,
            &text_preview_of_other_message_like(self).format_with(username, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

// TODO: once we properly display polls, we should remove this,
//       because Polls shouldn't be displayed using the SmallStateEvent widget.
impl SmallStateEventContent for PollState {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        _username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(cx, ids!(content)).set_text(
            cx,
            self.fallback_text().unwrap_or_else(|| self.results().question).as_str(),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for timeline::OtherState {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let item = if let Some(text_preview) = text_preview_of_other_state(self, false) {
            item.label(cx, ids!(content))
                .set_text(cx, &text_preview.format_with(username, false));
            new_drawn_status.content_drawn = true;
            item
        } else {
            let item = list.item(cx, item_id, id!(Empty));
            new_drawn_status = ItemDrawnStatus::new();
            item
        };
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for MemberProfileChange {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        _list: &mut PortalList,
        _item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        item.label(cx, ids!(content)).set_text(
            cx,
            &text_preview_of_member_profile_change(self, username, false)
                .format_with(username, false),
        );
        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

impl SmallStateEventContent for RoomMembershipChange {
    fn populate_item_content(
        &self,
        cx: &mut Cx,
        list: &mut PortalList,
        item_id: usize,
        item: WidgetRef,
        _event_tl_item: &EventTimelineItem,
        username: &str,
        _item_drawn_status: ItemDrawnStatus,
        mut new_drawn_status: ItemDrawnStatus,
    ) -> (WidgetRef, ItemDrawnStatus) {
        let Some(preview) = text_preview_of_room_membership_change(self, false) else {
            // Don't actually display anything for nonexistent/unimportant membership changes.
            return (
                list.item(cx, item_id, id!(Empty)),
                ItemDrawnStatus::new(),
            );
        };

        item.label(cx, ids!(content))
            .set_text(cx, &preview.format_with(username, false));

        // The invite_user_button is only used for "Knocked" membership change events.
        item.button(cx, ids!(invite_user_button)).set_visible(
            cx,
            matches!(self.change(), Some(MembershipChange::Knocked)),
        );

        new_drawn_status.content_drawn = true;
        (item, new_drawn_status)
    }
}

/// Creates, populates, and adds a SmallStateEvent liveview widget to the given `PortalList`
/// with the given `item_id`.
///
/// The content of the returned widget is populated with data from the
/// given room membership change and its parent `EventTimelineItem`.
fn populate_small_state_event(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    timeline_kind: &TimelineKind,
    app_language: AppLanguage,
    event_tl_item: &EventTimelineItem,
    event_content: &impl SmallStateEventContent,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    let (item, existed) = list.item_with_existed(cx, item_id, id!(SmallStateEvent));
    // The content of a small state event view may depend on the profile info,
    // so we can only mark the content as drawn after the profile has been fully drawn and cached.
    let skip_redrawing_profile = existed && item_drawn_status.profile_drawn;
    let skip_redrawing_content = skip_redrawing_profile && item_drawn_status.content_drawn;
    populate_read_receipts(&item, cx, timeline_kind, event_tl_item);
    if skip_redrawing_content {
        return (item, new_drawn_status);
    }

    // If the profile has been drawn, we can just quickly grab the user's display name
    // instead of having to call `set_avatar_and_get_username` again.
    let username_opt = skip_redrawing_profile
        .then(|| get_profile_display_name(event_tl_item))
        .flatten();

    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username` to get the user's display name.
        let avatar_ref = item.avatar(cx, ids!(avatar));

        let (username, profile_drawn) = avatar_ref.set_avatar_and_get_username(
            cx,
            timeline_kind,
            event_tl_item.sender(),
            Some(event_tl_item.sender_profile()),
            event_tl_item.event_id(),
            true,
        );
        // Draw the timestamp as part of the profile.
        if let Some(dt) = unix_time_millis_to_datetime(event_tl_item.timestamp()) {
            item.timestamp(cx, ids!(left_container.timestamp)).set_date_time(cx, dt);
        }
        new_drawn_status.profile_drawn = profile_drawn;
        username
    });

    // Proceed to draw the actual event content.
    let (item, new_drawn_status) = event_content.populate_item_content(
        cx,
        list,
        item_id,
        item,
        event_tl_item,
        &username,
        item_drawn_status,
        new_drawn_status,
    );

    item.button(cx, ids!(invite_user_button))
        .set_text(cx, tr_key(app_language, "room_screen.small_state.invite_to_room"));

    (item, new_drawn_status)
}


/// Returns the display name of the sender of the given `event_tl_item`, if available.
fn get_profile_display_name(event_tl_item: &EventTimelineItem) -> Option<String> {
    if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
        profile.display_name.clone()
    } else {
        None
    }
}


/// Actions related to invites within a room.
///
/// These are NOT widget actions, just regular actions.
#[derive(Debug)]
pub enum InviteAction {
    /// Show a confirmation modal for sending an invite.
    ///
    /// The content is wrapped in a `RefCell` to ensure that only one entity handles it
    /// and that that one entity can take ownership of the content object,
    /// which avoids having to clone it.
    ShowInviteConfirmationModal(RefCell<Option<ConfirmationModalContent>>),
}

/// The result of inviting a user to a room.
///
#[derive(Debug)]
pub enum InviteResultAction {
    /// The invite was sent successfully.
    ///
    /// This action is posted in response to the [`MatrixRequest::InviteUser`] request.
    Sent {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
    },
    /// The invite failed to be sent.
    ///
    /// This action is posted in response to the [`MatrixRequest::InviteUser`] request.
    Failed {
        room_id: OwnedRoomId,
        user_id: OwnedUserId,
        error: matrix_sdk::Error,
    },
}

/// The result of reporting a room.
#[derive(Debug)]
pub enum ReportRoomResultAction {
    Sent {
        room_id: OwnedRoomId,
    },
    Failed {
        room_id: OwnedRoomId,
        error: matrix_sdk::Error,
    },
}

#[derive(Debug)]
pub enum ActionResponseResultAction {
    Sent {
        room_id: OwnedRoomId,
        source_event_id: OwnedEventId,
    },
    Failed {
        room_id: OwnedRoomId,
        source_event_id: OwnedEventId,
        error: String,
    },
}

#[derive(Clone, Default, Debug)]
pub enum MessageAction {
    /// The user clicked the "react" button on a message
    /// and wants to send the given `reaction` to that message.
    React {
        details: MessageDetails,
        reaction: String,
    },
    /// The user clicked the "reply" button on a message.
    Reply(MessageDetails),
    /// The user clicked the "edit" button on a message.
    Edit(MessageDetails),
    /// The user requested to edit their latest message in this room.
    EditLatest,
    /// The user submitted a new local message and the timeline should follow the live tail.
    MessageSubmittedLocally,
    /// The user clicked the "pin" button on a message.
    Pin(MessageDetails),
    /// The user clicked the "unpin" button on a message.
    Unpin(MessageDetails),
    /// The user clicked the "copy text" button on a message.
    CopyText(MessageDetails),
    /// The user clicked the "copy HTML" button on a message.
    CopyHtml(MessageDetails),
    /// The user clicked the "copy link" button on a message.
    CopyLink(MessageDetails),
    /// The user clicked the "view source" button on a message.
    ViewSource(MessageDetails),
    /// The user clicked the "jump to related" button on a message,
    /// indicating that they want to auto-scroll back to the related message,
    /// e.g., a replied-to message.
    JumpToRelated(MessageDetails),
    /// The user clicked the thread summary on a thread-root message.
    OpenThread(OwnedEventId),
    /// The user requested to jump to a specific event in this room.
    JumpToEvent(OwnedEventId),
    /// The user clicked the "delete" button on a message.
    #[doc(alias("delete"))]
    Redact {
        details: MessageDetails,
        reason: Option<String>,
    },

    // /// The user clicked the "report" button on a message.
    // Report(MessageDetails),

    /// The message at the given item index in the timeline should be highlighted.
    HighlightMessage(usize),
    /// The user requested that we show a context menu with actions
    /// that can be performed on a given message.
    OpenMessageContextMenu {
        details: MessageDetails,
        /// The absolute position where we should show the context menu,
        /// in which the (0,0) origin coordinate is the top left corner of the app window.
        abs_pos: DVec2,
        opening_gesture: ContextMenuOpenGesture,
    },
    ToggleTranslationLangPopup {
        button_rect: Rect,
    },
    /// The user requested opening the message action bar
    ActionBarOpen {
        /// At the given timeline item index
        item_id: usize,
        /// The message rect, so the action bar can be positioned relative to it
        message_rect: Rect,
    },
    /// The user requested closing the message action bar
    ActionBarClose,
    /// The user requested toggling the in-room app service quick actions card.
    ToggleAppServiceActions,
    ShowThreadsPane,
    ShowRoomInfoPane,
    #[default]
    None,
}

impl ActionDefaultRef for MessageAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: MessageAction = MessageAction::None;
        &DEFAULT
    }
}

#[derive(Clone, Default, Debug)]
pub enum AppServicePanelAction {
    Dismiss,
    OpenCreateBotModal,
    OpenDeleteBotModal,
    SendListBots,
    SendBotHelp,
    ShowBoundBots,
    Unbind,
    #[default]
    None,
}

impl ActionDefaultRef for AppServicePanelAction {
    fn default_ref() -> &'static Self {
        static DEFAULT: AppServicePanelAction = AppServicePanelAction::None;
        &DEFAULT
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct AppServicePanel {
    #[deref] view: View,
    #[rust] app_language: AppLanguage,
    #[rust] app_language_initialized: bool,
}

impl Widget for AppServicePanel {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.handle_event(cx, event, scope);

        let room_screen_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("BUG: RoomScreenProps should be available in Scope::props for AppServicePanel");
        self.view
            .button(cx, ids!(keyboard.third_row.view_bound_button))
            .set_visible(cx, room_screen_props.app_service_enabled);
        self.view
            .button(cx, ids!(keyboard.third_row.unbind_button))
            .set_visible(cx, room_screen_props.app_service_room_bound);

        if let Event::Actions(actions) = event {
            if self
                .view
                .button(cx, ids!(bubble.header.dismiss_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::Dismiss,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.first_row.create_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::OpenCreateBotModal,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.first_row.list_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::SendListBots,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.second_row.delete_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::OpenDeleteBotModal,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.second_row.help_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::SendBotHelp,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.third_row.view_bound_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::ShowBoundBots,
                );
            }

            if self
                .view
                .button(cx, ids!(keyboard.third_row.unbind_button))
                .clicked(actions)
            {
                cx.widget_action(
                    room_screen_props.room_screen_widget_uid,
                    AppServicePanelAction::Unbind,
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_language = scope.data.get::<AppState>()
            .map(|app_state| app_state.app_language)
            .unwrap_or_default();
        if !self.app_language_initialized || self.app_language != app_language {
            self.set_app_language(cx, app_language);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AppServicePanel {
    fn set_app_language(&mut self, cx: &mut Cx, app_language: AppLanguage) {
        self.app_language = app_language;
        self.app_language_initialized = true;
        self.view
            .label(cx, ids!(sender_row.sender_name))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.sender_name"));
        self.view
            .label(cx, ids!(sender_row.sender_tag))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.sender_tag"));
        self.view
            .label(cx, ids!(bubble.header.title))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.title"));
        self.view
            .label(cx, ids!(bubble.subtitle))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.subtitle"));
        self.view
            .label(cx, ids!(bubble.footer.timestamp))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.timestamp_now"));
        self.view
            .button(cx, ids!(keyboard.first_row.create_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.create_bot"));
        self.view
            .button(cx, ids!(keyboard.first_row.list_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.list_bots"));
        self.view
            .button(cx, ids!(keyboard.second_row.delete_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.delete_bot"));
        self.view
            .button(cx, ids!(keyboard.second_row.help_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.bot_help"));
        self.view
            .button(cx, ids!(keyboard.third_row.view_bound_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.bots"));
        self.view
            .button(cx, ids!(keyboard.third_row.unbind_button))
            .set_text(cx, tr_key(self.app_language, "room_screen.app_service.button.unbind"));
        self.view.redraw(cx);
    }
}

/// A widget representing a single message of any kind within a room timeline.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct Message {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,

    #[rust] details: Option<MessageDetails>,
}

impl Widget for Message {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if !self.animator.is_track_animating(id!(highlight))
            && self.animator_in_state(cx, ids!(highlight.on))
        {
            self.animator_play(cx, ids!(highlight.off));
        }

        let Some(details) = self.details.clone() else { return };

        // We first handle a click on the replied-to message preview, if present,
        // because we don't want any widgets within the replied-to message to be
        // clickable or otherwise interactive.
        match event.hits(cx, self.view(cx, ids!(replied_to_message)).area()) {
            Hit::FingerDown(fe) if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) => {
                cx.widget_action(
                    details.room_screen_widget_uid,
                    MessageAction::OpenMessageContextMenu {
                        details: details.clone(),
                        abs_pos: fe.abs,
                        opening_gesture: ContextMenuOpenGesture::from_finger_down(&fe),
                    }
                );
            }
            Hit::FingerDown(_) => {}
            Hit::FingerLongPress(lp) => {
                cx.widget_action(
                    details.room_screen_widget_uid, 
                    MessageAction::OpenMessageContextMenu {
                        details: details.clone(),
                        abs_pos: lp.abs,
                        opening_gesture: ContextMenuOpenGesture::from_long_press(&lp),
                    }
                );
            }
            // If the hit occurred on the replied-to message preview, jump to it.
            Hit::FingerUp(fe) if fe.is_over && fe.is_primary_hit() && fe.was_tap() => {
                cx.widget_action(
                    details.room_screen_widget_uid, 
                    MessageAction::JumpToRelated(details.clone()),
                );
            }
            _ => { }
        }

        // Handle clicks on the thread summary shown beneath a thread-root message.
        if let Some(thread_root_event_id) = details.thread_root_event_id.as_ref() {
            let thread_root_summary = self.view(cx, ids!(thread_root_summary));
            let apply_hover = |cx: &mut Cx, bg_color: Vec4| {
                let mut thread_root_summary_ref = thread_root_summary.clone();
                script_apply_eval!(cx, thread_root_summary_ref, {
                    draw_bg.color: #(bg_color)
                });
            };
            match event.hits(cx, thread_root_summary.area()) {
                Hit::FingerDown(fe) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG_HOVER);
                    if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                        cx.widget_action(
                            details.room_screen_widget_uid, 
                            MessageAction::OpenMessageContextMenu {
                                details: details.clone(),
                                abs_pos: fe.abs,
                                opening_gesture: ContextMenuOpenGesture::from_finger_down(&fe),
                            }
                        );
                    }
                }
                Hit::FingerHoverIn(_) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG_HOVER);
                }
                Hit::FingerHoverOut(_) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG);
                }
                Hit::FingerLongPress(lp) => {
                    cx.widget_action(
                        details.room_screen_widget_uid, 
                        MessageAction::OpenMessageContextMenu {
                            details: details.clone(),
                            abs_pos: lp.abs,
                            opening_gesture: ContextMenuOpenGesture::from_long_press(&lp),
                        }
                    );
                }
                Hit::FingerUp(fe) => {
                    apply_hover(cx, COLOR_THREAD_SUMMARY_BG);
                    if fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                        cx.widget_action(
                            details.room_screen_widget_uid, 
                            MessageAction::OpenThread(thread_root_event_id.clone()),
                        );
                    }
                }
                _ => { }
            }
        }

        // Next, we forward the event to the child view such that it has the chance
        // to handle it before the Message widget handles it.
        // This ensures that events like right-clicking/long-pressing a reaction button
        // or a link within a message will be treated as an action upon that child view
        // rather than an action upon the message itself.
        self.view.handle_event(cx, event, scope);

        // Finally, handle any hits on the rest of the message body itself.
        let message_view_area = self.view.area();
        match event.hits(cx, message_view_area) {
            Hit::FingerDown(fe) => {
                cx.set_key_focus(message_view_area);
                // A right click means we should display the context menu.
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    cx.widget_action(
                        details.room_screen_widget_uid, 
                        MessageAction::OpenMessageContextMenu {
                            details: details.clone(),
                            abs_pos: fe.abs,
                            opening_gesture: ContextMenuOpenGesture::from_finger_down(&fe),
                        }
                    );
                }
            }
            Hit::FingerLongPress(lp) => {
                cx.widget_action(
                    details.room_screen_widget_uid, 
                    MessageAction::OpenMessageContextMenu {
                        details: details.clone(),
                        abs_pos: lp.abs,
                        opening_gesture: ContextMenuOpenGesture::from_long_press(&lp),
                    }
                );
            }
            Hit::FingerHoverIn(..) => {
                self.animator_play(cx, ids!(hover.on));
                // TODO: here, show the "action bar" buttons upon hover-in
            }
            Hit::FingerHoverOut(_fho) => {
                self.animator_play(cx, ids!(hover.off));
                // TODO: here, hide the "action bar" buttons upon hover-out
            }
            _ => { }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                match action.as_widget_action().widget_uid_eq(details.room_screen_widget_uid).cast_ref() {
                    MessageAction::HighlightMessage(id) if id == &details.item_id => {
                        self.animator_play(cx, ids!(highlight.on));
                        self.redraw(cx);
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.details.as_ref().is_some_and(|d| d.should_be_highlighted) {
            script_apply_eval!(cx, self, {
                draw_bg +: {
                    color: #ffffd1,
                    mentions_bar_color: #ffd54f
                }
            });
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

impl Message {
    fn set_data(&mut self, details: MessageDetails) {
        self.details = Some(details);
    }
}

impl MessageRef {
    fn set_data(&self, details: MessageDetails) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_data(details);
    }
}

/// Clears all UI-related timeline states for all known rooms.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread. 
pub fn clear_timeline_states(_cx: &mut Cx) {
    // Clear timeline states cache
    TIMELINE_STATES.with_borrow_mut(|states| {
        states.clear();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::streaming_animation::StreamingAnimState;
    use std::time::{Duration, Instant};

    fn make_state(text: &str) -> StreamingAnimState {
        StreamingAnimState::new(text, true)
    }

    #[test]
    fn test_streaming_scan_range() {
        // Incremental: clamp sentinel to new_len
        assert_eq!(streaming_scan_range(false, &(5..usize::MAX), 8, 9), 5..9);
        // Append: new item at end is scanned
        assert_eq!(streaming_scan_range(false, &(8..9), 8, 9), 8..9);
        // No changes: empty range
        assert_eq!(streaming_scan_range(false, &(8..8), 8, 8), 8..8);
        // Clear cache: full scan
        assert_eq!(streaming_scan_range(true, &(5..usize::MAX), 8, 9), 0..9);
    }

    #[test]
    fn test_refresh_stream_indices() {
        let event_id_a: OwnedEventId = "$event-a:example.com".try_into().unwrap();
        let event_id_b: OwnedEventId = "$event-b:example.com".try_into().unwrap();
        let missing_event_id: OwnedEventId = "$missing:example.com".try_into().unwrap();

        let mut streaming_messages = HashMap::new();
        streaming_messages.insert(event_id_a.clone(), make_state("alpha"));
        streaming_messages.insert(missing_event_id.clone(), make_state("missing"));

        let event_ids = vec![None, Some(event_id_a.as_ref()), Some(event_id_b.as_ref())];
        refresh_stream_indices(event_ids.into_iter(), &mut streaming_messages);

        assert_eq!(streaming_messages[&event_id_a].timeline_index, Some(1));
        assert_eq!(streaming_messages[&missing_event_id].timeline_index, None);
    }

    #[test]
    fn test_timeout_picks_earliest() {
        let mut live = make_state("alpha");
        live.last_update_time = Instant::now() - Duration::from_secs(40);
        let mut finished = make_state("beta");
        finished.is_live = false;
        finished.last_update_time = Instant::now() - Duration::from_secs(29);

        let timeout = next_stream_timeout([&live, &finished].into_iter()).unwrap();

        assert!(timeout <= Duration::from_secs(1));
    }

    #[test]
    fn test_full_snapshot_rebuild_drops_finished_cached_streams() {
        let event_id: OwnedEventId = "$event-live:example.com".try_into().unwrap();
        let mut previous = HashMap::new();
        let mut previous_state = make_state("hello live");
        previous_state.advance_displayed(4);
        previous.insert(event_id.clone(), previous_state);

        let (rebuilt, should_schedule_frame) = rebuild_streaming_messages_for_full_snapshot(
            [(event_id, String::from("hello final"), false)],
            Some(&previous),
        );

        assert!(rebuilt.is_empty());
        assert!(!should_schedule_frame);
    }

    #[test]
    fn test_full_snapshot_rebuild_restores_live_cached_streams() {
        let event_id: OwnedEventId = "$event-live:example.com".try_into().unwrap();
        let mut previous = HashMap::new();
        let mut previous_state = make_state("hello");
        previous_state.advance_displayed(3);
        previous.insert(event_id.clone(), previous_state);

        let (rebuilt, should_schedule_frame) = rebuild_streaming_messages_for_full_snapshot(
            [(event_id.clone(), String::from("hello world"), true)],
            Some(&previous),
        );

        let restored = rebuilt.get(&event_id).unwrap();
        assert_eq!(restored.displayed_char_count, 3);
        assert!(restored.is_live);
        assert!(should_schedule_frame);
    }

    #[test]
    fn test_full_snapshot_rebuild_skips_live_without_cached_state() {
        // Without previous state, full-snapshot rebuild must NOT create new
        // animations — the SDK may not have aggregated edits yet, so
        // completed messages can still appear as `live`.
        let event_id: OwnedEventId = "$event-live:example.com".try_into().unwrap();

        let (rebuilt, should_schedule_frame) = rebuild_streaming_messages_for_full_snapshot(
            [(event_id.clone(), String::from("hello world"), true)],
            None,
        );

        assert!(rebuilt.is_empty());
        assert!(!should_schedule_frame);
    }

    #[test]
    fn translation_lang_popup_abs_pos_prefers_above_button() {
        let button_rect = Rect {
            pos: dvec2(48.0, 680.0),
            size: dvec2(32.0, 32.0),
        };
        let container_rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(1280.0, 760.0),
        };

        let popup_pos = compute_translation_lang_popup_abs_pos(button_rect, container_rect);

        assert!(popup_pos.y < button_rect.pos.y);
        assert!(popup_pos.y >= TRANSLATION_LANG_POPUP_MARGIN);
        assert!(popup_pos.x >= TRANSLATION_LANG_POPUP_MARGIN);
    }

    #[test]
    fn translation_lang_popup_abs_pos_falls_below_when_top_space_is_insufficient() {
        let button_rect = Rect {
            pos: dvec2(48.0, 20.0),
            size: dvec2(32.0, 32.0),
        };
        let container_rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(1280.0, 760.0),
        };

        let popup_pos = compute_translation_lang_popup_abs_pos(button_rect, container_rect);

        assert!(popup_pos.y > button_rect.pos.y);
        assert!(popup_pos.y >= TRANSLATION_LANG_POPUP_MARGIN);
    }

    #[test]
    fn translation_lang_popup_abs_pos_clamps_to_room_screen_right_edge() {
        let button_rect = Rect {
            pos: dvec2(1240.0, 680.0),
            size: dvec2(32.0, 32.0),
        };
        let container_rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(1280.0, 760.0),
        };

        let popup_pos = compute_translation_lang_popup_abs_pos(button_rect, container_rect);

        assert_eq!(
            popup_pos.x + TRANSLATION_LANG_POPUP_WIDTH,
            container_rect.size.x - TRANSLATION_LANG_POPUP_MARGIN
        );
    }

    #[test]
    fn center_username_row_aligns_with_avatar_center() {
        assert_eq!(
            message_profile_avatar_center_y(),
            message_username_row_center_y(),
        );
    }

    #[test]
    fn center_bot_badge_aligns_with_username_row_center() {
        assert_eq!(
            message_username_row_center_y(),
            bot_badge_center_y_within_username_row(),
        );
    }

    #[test]
    fn bot_badge_text_is_centered_within_badge() {
        assert!(bot_badge_label_center_y() < (BOT_BADGE_HEIGHT * 0.5));
    }

    #[test]
    fn test_bot_detection_configured_parent() {
        let user_id: OwnedUserId = "@octosbot:127.0.0.1:8128".try_into().unwrap();
        let resolved_parent_bot_user_id = Some(user_id.clone());
        let known_bot_user_ids = Vec::new();

        assert!(is_known_or_likely_bot(
            user_id.as_ref(),
            resolved_parent_bot_user_id.as_deref(),
            &known_bot_user_ids,
        ));
    }

    #[test]
    fn test_bot_detection_heuristic_fallback() {
        let user_id: OwnedUserId = "@myservice_bot:other.server".try_into().unwrap();
        let known_bot_user_ids = Vec::new();

        assert!(is_known_or_likely_bot(
            user_id.as_ref(),
            None,
            &known_bot_user_ids,
        ));
    }

    #[test]
    fn test_bot_detection_child_bot() {
        let user_id: OwnedUserId = "@octosbot_weather:127.0.0.1:8128".try_into().unwrap();
        let known_bot_user_ids = vec![user_id.clone()];

        assert!(is_known_or_likely_bot(
            user_id.as_ref(),
            None,
            &known_bot_user_ids,
        ));
    }

    #[test]
    fn test_bot_detection_rejects_normal_user() {
        let user_id: OwnedUserId = "@alice:127.0.0.1:8128".try_into().unwrap();
        let known_bot_user_ids = Vec::new();

        assert!(!is_known_or_likely_bot(
            user_id.as_ref(),
            None,
            &known_bot_user_ids,
        ));
    }

    #[test]
    fn test_timeline_bot_detection_uses_room_bot_user_ids() {
        let user_id: OwnedUserId = "@octosbot_bob:127.0.0.1:8128".try_into().unwrap();
        let room_bot_user_ids = vec![user_id.clone()];
        let known_bot_user_ids = Vec::new();

        assert!(is_timeline_sender_bot(
            user_id.as_ref(),
            None,
            &room_bot_user_ids,
            &known_bot_user_ids,
        ));
    }

    #[test]
    fn test_parse_bot_timeline_layers_extracts_status_provider_body_and_footer() {
        let body = "施法中\nvia moonshot@api (kimi-k2.5)\n\n你好！我是 **Alex**\n\n_moonshot@api/kimi-k2.5 · 5.3K in · 330 out · 6s_";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers.status.as_deref(), Some("施法中"));
        assert_eq!(layers.provider.as_deref(), Some("via moonshot@api (kimi-k2.5)"));
        assert_eq!(layers.body, "你好！我是 **Alex**");
        assert_eq!(
            layers.footer.as_deref(),
            Some("_moonshot@api/kimi-k2.5 · 5.3K in · 330 out · 6s_"),
        );
    }

    #[test]
    fn test_parse_octos_actions_skips_malformed_entries() {
        let actions = parse_octos_actions_from_content(&serde_json::json!({
            "org.octos.actions": [
                { "id": "retry_pptx", "label": "Regenerate PPT", "style": "primary" },
                { "label": "Missing id" },
                { "id": "cancel", "label": "Cancel", "style": "secondary" }
            ]
        }));

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].id, "retry_pptx");
        assert_eq!(actions[1].id, "cancel");
    }

    #[test]
    fn test_parse_octos_actions_truncates_after_six() {
        let actions = parse_octos_actions_from_content(&serde_json::json!({
            "org.octos.actions": [
                { "id": "a1", "label": "A1" },
                { "id": "a2", "label": "A2" },
                { "id": "a3", "label": "A3" },
                { "id": "a4", "label": "A4" },
                { "id": "a5", "label": "A5" },
                { "id": "a6", "label": "A6" },
                { "id": "a7", "label": "A7" }
            ]
        }));

        assert_eq!(actions.len(), 6);
        assert_eq!(actions.last().map(|action| action.id.as_str()), Some("a6"));
    }

    #[test]
    fn test_parse_octos_actions_reads_m_new_content_wrapper() {
        let actions = parse_octos_actions_from_content(&serde_json::json!({
            "m.new_content": {
                "org.octos.actions": [
                    { "id": "confirm", "label": "确认", "style": "primary" },
                    { "id": "cancel", "label": "取消", "style": "secondary" }
                ]
            },
            "org.octos.actions": [
                { "id": "stale", "label": "旧按钮" }
            ]
        }));

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].id, "confirm");
        assert_eq!(actions[1].id, "cancel");
    }

    #[test]
    fn test_parse_octos_approval_request_from_content() {
        let approval = parse_octos_approval_request_from_content(&serde_json::json!({
            "org.octos.approval_request": {
                "request_id": "req_abc123",
                "tool_name": "shell",
                "tool_args_digest": "sha256:4bf5",
                "title": "Execute shell command",
                "summary": "rm -rf ~/tmp/cache",
                "risk_level": "critical",
                "authorized_approvers": ["@alice:example.org"],
                "expires_at": "2026-04-14T14:30:00Z",
                "on_timeout": "notify"
            }
        })).expect("approval request should parse");

        assert_eq!(approval.request_id, "req_abc123");
        assert_eq!(approval.tool_name, "shell");
        assert_eq!(approval.tool_args_digest, "sha256:4bf5");
        assert_eq!(approval.title, "Execute shell command");
        assert_eq!(approval.summary, "rm -rf ~/tmp/cache");
        assert_eq!(approval.risk_level, OctosApprovalRiskLevel::Critical);
        assert_eq!(approval.authorized_approvers, vec!["@alice:example.org"]);
        assert_eq!(approval.on_timeout, OctosApprovalTimeoutBehavior::Notify);
    }

    #[test]
    fn test_parse_octos_approval_request_ignores_m_new_content_wrapper() {
        let approval = parse_octos_approval_request_from_content(&serde_json::json!({
            "org.octos.approval_request": {
                "request_id": "req_original",
                "tool_name": "shell",
                "tool_args_digest": "sha256:4bf5",
                "title": "Original request",
                "summary": "rm -rf ~/tmp/cache",
                "risk_level": "critical",
                "authorized_approvers": ["@alice:example.org"],
                "expires_at": "2026-04-14T14:30:00Z",
                "on_timeout": "notify"
            },
            "m.new_content": {
                "org.octos.approval_request": {
                    "request_id": "req_edited",
                    "tool_name": "shell",
                    "tool_args_digest": "sha256:mallory",
                    "title": "Edited request",
                    "summary": "whoami",
                    "risk_level": "normal",
                    "authorized_approvers": ["@mallory:example.org"],
                    "expires_at": "2026-04-14T14:30:00Z",
                    "on_timeout": "notify"
                }
            }
        })).expect("approval request should parse from original content");

        assert_eq!(approval.request_id, "req_original");
        assert_eq!(approval.authorized_approvers, vec!["@alice:example.org"]);
        assert_eq!(approval.risk_level, OctosApprovalRiskLevel::Critical);
    }

    #[test]
    fn test_parse_octos_approval_request_rejects_empty_authorized_approvers() {
        assert!(parse_octos_approval_request_from_content(&serde_json::json!({
            "org.octos.approval_request": {
                "request_id": "req_abc123",
                "tool_name": "shell",
                "tool_args_digest": "sha256:4bf5",
                "title": "Execute shell command",
                "summary": "rm -rf ~/tmp/cache",
                "risk_level": "critical",
                "authorized_approvers": [],
                "expires_at": "2026-04-14T14:30:00Z",
                "on_timeout": "notify"
            }
        })).is_none());
    }

    #[test]
    fn test_build_approval_response_request_targets_original_sender() {
        let timeline_kind = TimelineKind::MainRoom {
            room_id: "!room:127.0.0.1:8128".try_into().unwrap(),
        };
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let original_sender: OwnedUserId = "@octosbot:127.0.0.1:8128".try_into().unwrap();

        let request = build_octos_approval_response_request(
            &timeline_kind,
            "Execute shell command",
            "req_abc123",
            "approve",
            "sha256:4bf5",
            source_event_id.as_ref(),
            original_sender.as_ref(),
        );

        assert_eq!(request.timeline_kind, timeline_kind);
        assert_eq!(request.target_user_id, original_sender);
        assert!(!request.explicit_room);
        assert_eq!(request.content["org.octos.approval_response"]["request_id"], "req_abc123");
        assert_eq!(request.content["org.octos.approval_response"]["decision"], "approve");
        assert_eq!(request.content["org.octos.approval_response"]["tool_args_digest"], "sha256:4bf5");
    }

    #[test]
    fn test_action_buttons_render_state_hidden_without_actions() {
        let state = compute_action_button_render_state(&[], None, None);

        assert!(!state.show_container);
        assert!(state.visible_slots.is_empty());
    }

    #[test]
    fn test_action_buttons_render_state_with_primary_secondary_danger() {
        let state = compute_action_button_render_state(&[
            OctosActionButton {
                id: "retry".into(),
                label: "Regenerate PPT".into(),
                style: OctosActionStyle::Primary,
            },
            OctosActionButton {
                id: "cancel".into(),
                label: "Cancel".into(),
                style: OctosActionStyle::Secondary,
            },
            OctosActionButton {
                id: "delete".into(),
                label: "Delete".into(),
                style: OctosActionStyle::Danger,
            },
        ], None, None);

        assert!(state.show_container);
        assert!(state.show_button_row);
        assert!(state.buttons_enabled);
        assert!(state.approval_card.is_none());
        assert_eq!(state.visible_slots.len(), 3);
        assert_eq!(state.visible_slots[0].style, OctosActionStyle::Primary);
        assert_eq!(state.visible_slots[1].style, OctosActionStyle::Secondary);
        assert_eq!(state.visible_slots[2].style, OctosActionStyle::Danger);
    }

    #[test]
    fn test_approval_buttons_disabled_for_unauthorized_user() {
        let approval_request = OctosApprovalRequest {
            request_id: "req_abc123".into(),
            tool_name: "shell".into(),
            tool_args_digest: "sha256:4bf5".into(),
            title: "Execute shell command".into(),
            summary: "rm -rf ~/tmp/cache".into(),
            risk_level: OctosApprovalRiskLevel::Critical,
            authorized_approvers: vec!["@alice:example.org".into()],
            expires_at: "2026-04-14T14:30:00Z".into(),
            on_timeout: OctosApprovalTimeoutBehavior::Notify,
        };
        let current_user_id = UserId::parse("@mallory:example.org").unwrap();
        let state = compute_action_button_render_state(&[
            OctosActionButton {
                id: "approve".into(),
                label: "Approve".into(),
                style: OctosActionStyle::Primary,
            },
            OctosActionButton {
                id: "deny".into(),
                label: "Deny".into(),
                style: OctosActionStyle::Danger,
            },
        ], Some(&approval_request), Some(current_user_id.as_ref()));

        assert!(state.show_container);
        assert!(state.show_button_row);
        assert!(!state.buttons_enabled);
        assert_eq!(
            state.approval_card.as_ref().map(|card| card.title.as_str()),
            Some("Execute shell command"),
        );
        assert_eq!(
            state.approval_card.as_ref().map(|card| card.summary.as_str()),
            Some("rm -rf ~/tmp/cache"),
        );
    }

    #[test]
    fn test_selected_action_reduces_visible_slots_to_clicked_button() {
        let render_state = compute_action_button_render_state(&[
            OctosActionButton {
                id: "approve".into(),
                label: "Approve".into(),
                style: OctosActionStyle::Primary,
            },
            OctosActionButton {
                id: "deny".into(),
                label: "Deny".into(),
                style: OctosActionStyle::Danger,
            },
        ], None, None);

        let visible_slots = action_button_render_slots_for_display(&render_state, Some(&SelectedOctosActionState {
            id: "deny".into(),
            label: "Deny".into(),
            style: OctosActionStyle::Danger,
        }));

        assert_eq!(visible_slots.len(), 1);
        assert_eq!(visible_slots[0].id, "deny");
        assert_eq!(visible_slots[0].label, "✓ Deny");
        assert_eq!(visible_slots[0].style, OctosActionStyle::Danger);
    }

    #[test]
    fn test_generic_actions_without_approval_request_remain_supported() {
        let payload = parse_octos_action_payload_for_render(
            Some(&serde_json::json!({
                "org.octos.actions": [
                    { "id": "retry_pptx", "label": "Regenerate PPT", "style": "primary" }
                ]
            })),
            None,
        );

        assert!(payload.approval_request.is_none());
        assert!(!payload.malformed_approval_request);
        assert_eq!(payload.actions.len(), 1);
        assert_eq!(payload.actions[0].id, "retry_pptx");
    }

    #[test]
    fn test_malformed_approval_request_hides_buttons() {
        let payload = parse_octos_action_payload_for_render(
            Some(&serde_json::json!({
                "org.octos.actions": [
                    { "id": "approve", "label": "Approve", "style": "primary" },
                    { "id": "deny", "label": "Deny", "style": "danger" }
                ]
            })),
            Some(&serde_json::json!({
                "org.octos.approval_request": {
                    "request_id": "req_abc123"
                },
                "org.octos.actions": [
                    { "id": "approve", "label": "Approve", "style": "primary" },
                    { "id": "deny", "label": "Deny", "style": "danger" }
                ]
            })),
        );
        let state = compute_action_button_render_state(
            &payload.actions,
            payload.approval_request.as_ref(),
            None,
        );

        assert!(payload.malformed_approval_request);
        assert!(!state.show_container);
        assert!(state.visible_slots.is_empty());
    }

    #[test]
    fn test_approval_request_ignores_m_replace_edits() {
        let payload = parse_octos_action_payload_for_render(
            Some(&serde_json::json!({
                "m.new_content": {
                    "org.octos.approval_request": {
                        "request_id": "req_replaced",
                        "tool_name": "shell",
                        "tool_args_digest": "sha256:replaced",
                        "title": "Replaced request",
                        "summary": "echo hacked",
                        "risk_level": "normal",
                        "authorized_approvers": ["@mallory:example.org"],
                        "expires_at": "2026-04-14T14:35:00Z",
                        "on_timeout": "notify"
                    },
                    "org.octos.actions": [
                        { "id": "approve", "label": "Approve", "style": "primary" },
                        { "id": "deny", "label": "Deny", "style": "danger" }
                    ]
                }
            })),
            Some(&serde_json::json!({
                "org.octos.approval_request": {
                    "request_id": "req_original",
                    "tool_name": "shell",
                    "tool_args_digest": "sha256:original",
                    "title": "Original request",
                    "summary": "rm -rf ~/tmp/cache",
                    "risk_level": "critical",
                    "authorized_approvers": ["@alice:example.org"],
                    "expires_at": "2026-04-14T14:30:00Z",
                    "on_timeout": "notify"
                },
                "org.octos.actions": [
                    { "id": "approve", "label": "Approve", "style": "primary" },
                    { "id": "deny", "label": "Deny", "style": "danger" }
                ]
            })),
        );
        let current_user_id = UserId::parse("@alice:example.org").unwrap();
        let state = compute_action_button_render_state(
            &payload.actions,
            payload.approval_request.as_ref(),
            Some(current_user_id.as_ref()),
        );

        assert_eq!(
            payload.approval_request.as_ref().map(|approval| approval.request_id.as_str()),
            Some("req_original"),
        );
        assert!(state.buttons_enabled);
        assert_eq!(
            state.approval_card.as_ref().map(|card| card.title.as_str()),
            Some("Original request"),
        );
    }

    #[test]
    fn test_build_action_response_request_targets_original_sender() {
        let timeline_kind = TimelineKind::MainRoom {
            room_id: "!room:127.0.0.1:8128".try_into().unwrap(),
        };
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let original_sender: OwnedUserId = "@octosbot_weather:127.0.0.1:8128".try_into().unwrap();

        let request = build_octos_action_response_request(
            &timeline_kind,
            "Regenerate PPT",
            "retry_pptx",
            source_event_id.as_ref(),
            original_sender.as_ref(),
        );

        assert_eq!(request.timeline_kind, timeline_kind);
        assert_eq!(request.target_user_id, original_sender);
        assert!(!request.explicit_room);
    }

    #[test]
    fn test_build_action_response_request_preserves_reply_relation() {
        let timeline_kind = TimelineKind::MainRoom {
            room_id: "!room:127.0.0.1:8128".try_into().unwrap(),
        };
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let original_sender: OwnedUserId = "@octosbot_weather:127.0.0.1:8128".try_into().unwrap();

        let request = build_octos_action_response_request(
            &timeline_kind,
            "Regenerate PPT",
            "retry_pptx",
            source_event_id.as_ref(),
            original_sender.as_ref(),
        );

        let action_response = &request.content["org.octos.action_response"];
        assert_eq!(request.content["body"], "[Action: Regenerate PPT]");
        assert_eq!(action_response["action_id"], "retry_pptx");
        assert_eq!(action_response["source_event_id"], "$orig123");
        assert_eq!(request.content["m.relates_to"]["m.in_reply_to"]["event_id"], "$orig123");
    }

    #[test]
    fn test_disable_action_buttons_marks_source_event_disabled() {
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let mut disabled = HashSet::new();

        mark_action_buttons_disabled(&mut disabled, &source_event_id);

        assert!(are_action_buttons_disabled(&disabled, source_event_id.as_ref()));
    }

    #[test]
    fn test_reenable_action_buttons_clears_disabled_state() {
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let mut disabled = HashSet::new();
        mark_action_buttons_disabled(&mut disabled, &source_event_id);

        clear_action_buttons_disabled(&mut disabled, source_event_id.as_ref());

        assert!(!are_action_buttons_disabled(&disabled, source_event_id.as_ref()));
    }

    #[test]
    fn test_selected_action_state_marks_and_clears_by_source_event_id() {
        let source_event_id: OwnedEventId = "$orig123".try_into().unwrap();
        let mut selected_actions = HashMap::new();

        mark_selected_octos_action(
            &mut selected_actions,
            &source_event_id,
            "approve",
            "Approve",
            OctosActionStyle::Primary,
        );
        assert_eq!(
            selected_actions.get(&source_event_id).map(|state| state.label.as_str()),
            Some("Approve"),
        );

        clear_selected_octos_action(&mut selected_actions, source_event_id.as_ref());
        assert!(!selected_actions.contains_key(&source_event_id));
    }

    #[test]
    fn test_parse_bot_timeline_layers_extracts_footer_without_provider_prefix() {
        let body = "PPT 已经生成并发送了！\n\n你应该已经收到了文件。\n\n_moonshot@api/kimi-k2.5 · 11.0K in · 279 out · 9s_";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers.status, None);
        assert_eq!(layers.provider, None);
        assert_eq!(layers.body, "PPT 已经生成并发送了！\n\n你应该已经收到了文件。");
        assert_eq!(
            layers.footer.as_deref(),
            Some("_moonshot@api/kimi-k2.5 · 11.0K in · 279 out · 9s_"),
        );
    }

    #[test]
    fn test_parse_bot_timeline_layers_falls_back_for_unmatched_bot_text() {
        let body = "你好！我是 Alex。\n今天可以帮你查天气。";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers, BotTimelineLayers::plain(body));
    }

    #[test]
    fn test_parse_bot_timeline_layers_ignores_regular_user_messages() {
        let body = "via moonshot@api (kimi-k2.5)\n\n这不是 bot 消息。";

        let layers = parse_bot_timeline_layers(body, false);

        assert_eq!(layers, BotTimelineLayers::plain(body));
    }

    #[test]
    fn test_parse_bot_timeline_layers_prefers_safe_fallback_for_malformed_metadata() {
        let body = "施法中\n这个不是 provider 行\n\n你好，我还在。";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers, BotTimelineLayers::plain(body));
    }

    #[test]
    fn test_parse_bot_timeline_layers_invalid_metadata_does_not_panic() {
        let body = "施法中\nvia moonshot@api (kimi-k2.5)\n\n_\n";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers, BotTimelineLayers::plain(body));
    }

    #[test]
    fn test_parse_bot_timeline_layers_tolerates_streaming_cursor_in_footer() {
        let body = "via moonshot@api (kimi-k2.5)\n\n你好！我是 **Alex**\n\n_moonshot@api/kimi-k2.5 · 5.3K in · 330 out · 6s_ ●";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers.body, "你好！我是 **Alex**");
        assert_eq!(
            layers.footer.as_deref(),
            Some("_moonshot@api/kimi-k2.5 · 5.3K in · 330 out · 6s_"),
        );
    }

    #[test]
    fn test_parse_bot_timeline_layers_promotes_metrics_only_body_to_footer() {
        let body = "疯狂输出中\nvia moonshot@api (kimi-k2.5)\n4s";

        let layers = parse_bot_timeline_layers(body, true);

        assert_eq!(layers.status.as_deref(), Some("疯狂输出中"));
        assert_eq!(layers.provider.as_deref(), Some("via moonshot@api (kimi-k2.5)"));
        assert!(layers.body.is_empty());
        assert_eq!(layers.footer.as_deref(), Some("4s"));
    }

    #[test]
    fn test_rich_markdown_streaming_prefers_full_snapshot_rendering() {
        let formatted = FormattedBody::html("<p><strong>OpenClaw</strong></p>");
        assert!(should_render_streaming_full_snapshot(
            "根据搜索结果， **OpenClaw** 有两个不同的项目。",
            Some(&formatted),
            true,
        ));
    }

    #[test]
    fn test_plain_text_streaming_keeps_typewriter_path() {
        assert!(!should_render_streaming_full_snapshot(
            "你好，我是 Octos。",
            None,
            true,
        ));
    }

    #[test]
    fn test_bot_timeline_card_visible_for_bot_text_message() {
        let state = compute_bot_timeline_render_state(
            "施法中\nvia moonshot@api (kimi-k2.5)\n\n你好！我是 Alex。\n\n_moonshot@api/kimi-k2.5 · 1.2K in · 88 out · 2s_",
            true,
        );

        assert!(state.show_card);
        assert_eq!(state.body, "你好！我是 Alex。");
    }

    #[test]
    fn test_bot_timeline_card_hidden_for_regular_user_message() {
        let state = compute_bot_timeline_render_state("你好", false);

        assert!(!state.show_card);
    }

    #[test]
    fn test_bot_status_strip_renders_above_body_and_not_inside_body() {
        let state = compute_bot_timeline_render_state(
            "施法中\nvia moonshot@api (kimi-k2.5)\n\n你好！我是 Alex。",
            true,
        );

        assert_eq!(state.status.as_deref(), Some("施法中"));
        assert!(state.show_status_strip);
        assert!(!state.body.starts_with("施法中"));
    }

    #[test]
    fn test_bot_metadata_footer_renders_below_body() {
        let state = compute_bot_timeline_render_state(
            "via moonshot@api (kimi-k2.5)\n\n你好！我是 Alex。\n\n_moonshot@api/kimi-k2.5 · 1.2K in · 88 out · 2s_",
            true,
        );

        assert!(state.show_metadata_footer);
        assert_eq!(state.provider.as_deref(), Some("via moonshot@api (kimi-k2.5)"));
        assert_eq!(
            state.footer.as_deref(),
            Some("_moonshot@api/kimi-k2.5 · 1.2K in · 88 out · 2s_"),
        );
    }

    #[test]
    fn test_bot_progress_message_hides_body_card_when_only_metrics_remain() {
        let state = compute_bot_timeline_render_state(
            "疯狂输出中\nvia moonshot@api (kimi-k2.5)\n4s",
            true,
        );

        assert!(state.show_card);
        assert!(!state.show_body_card);
        assert!(state.show_status_strip);
        assert!(state.show_metadata_footer);
        assert_eq!(state.footer.as_deref(), Some("4s"));
    }

    #[test]
    fn test_bot_timeline_card_body_uses_html_or_plaintext_rendering() {
        let state = compute_bot_timeline_render_state(
            "施法中\nvia moonshot@api (kimi-k2.5)\n\n你好！我是 **Alex**",
            true,
        );

        let formatted = select_bot_timeline_body_formatted_body(&state, None)
            .expect("structured bot body should still produce formatted content");

        assert_eq!(formatted.format, MessageFormat::Html);
        assert!(formatted.body.contains("<strong>Alex</strong>"));
    }

    #[test]
    fn test_bot_plain_markdown_body_without_formatted_html_still_renders_as_markdown() {
        let state = compute_bot_timeline_render_state(
            "## 标题\n\n```rust\n// 中文注释\nlet answer = 42;\n```",
            true,
        );

        let formatted = select_bot_timeline_body_formatted_body(&state, None)
            .expect("rich markdown bot body should synthesize HTML during streaming");

        assert_eq!(formatted.format, MessageFormat::Html);
        assert!(formatted.body.contains("<h2>标题</h2>"));
        assert!(formatted.body.contains("中文注释"));
    }

    #[test]
    fn test_bot_timeline_body_prefers_markdown_widget_for_fenced_code_blocks() {
        let state = compute_bot_timeline_render_state(
            "## 标题\n\n```rust\nlet answer = 42;\n```\n\n这里是中文说明。",
            true,
        );

        assert!(should_render_bot_timeline_body_with_markdown_widget(&state));
        assert_eq!(
            bot_timeline_code_block_mode(&state),
            BotTimelineCodeBlockMode::Highlighted,
        );
    }

    #[test]
    fn test_bot_timeline_body_keeps_html_widget_for_non_code_markdown() {
        let state = compute_bot_timeline_render_state(
            "## 标题\n\n这里有 **加粗**，但没有代码块。",
            true,
        );

        assert!(!should_render_bot_timeline_body_with_markdown_widget(&state));
        assert_eq!(
            bot_timeline_code_block_mode(&state),
            BotTimelineCodeBlockMode::None,
        );
    }

    #[test]
    fn test_bot_timeline_body_uses_plain_markdown_code_block_for_cjk_code() {
        let state = compute_bot_timeline_render_state(
            "```rust\n// 中文注释\nprintln!(\"你好\");\n```",
            true,
        );

        assert_eq!(
            bot_timeline_code_block_mode(&state),
            BotTimelineCodeBlockMode::Plain,
        );
    }

    #[test]
    fn test_fenced_code_blocks_ignore_cjk_outside_code_block() {
        let body = "## 标题\n\n```rust\nlet answer = 42;\n```\n\n这里是中文总结。";

        assert!(!fenced_code_blocks_contain_cjk(body));
    }

    #[test]
    fn test_streaming_update_requires_content_invalidation_for_new_full_snapshot_text() {
        let state = StreamingAnimState::new("你好", true);

        assert!(streaming_update_requires_content_invalidation(
            &state,
            "## 标题\n\n内容",
            true,
            true,
        ));
    }

    #[test]
    fn test_streaming_update_skips_invalidation_when_target_and_mode_are_unchanged() {
        let mut state = StreamingAnimState::new("## 标题\n\n内容", true);
        state.set_render_full_target(true);

        assert!(!streaming_update_requires_content_invalidation(
            &state,
            "## 标题\n\n内容",
            true,
            true,
        ));
    }

    #[test]
    fn test_bot_timeline_card_preserves_reply_preview_and_condensed_layout() {
        let reply_state = compute_bot_timeline_render_state(
            "via moonshot@api (kimi-k2.5)\n\n第一条回复",
            true,
        );
        let condensed_state = compute_bot_timeline_render_state(
            "via moonshot@api (kimi-k2.5)\n\n第二条回复",
            true,
        );

        assert!(reply_state.show_card);
        assert!(condensed_state.show_card);
        assert!(reply_state.show_metadata_footer);
        assert!(condensed_state.show_metadata_footer);
    }
}
