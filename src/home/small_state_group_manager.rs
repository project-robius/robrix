use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedUserId, UserId};
use matrix_sdk_ui::timeline::{
    AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, MsgLikeContent, MsgLikeKind, TimelineItem, TimelineItemContent, TimelineItemKind
};
use rangemap::RangeSet;
use std::{collections::HashMap, ops::Range, sync::Arc};
use indexmap::IndexMap;

use crate::home::room_read_receipt::{AvatarRowWidgetRefExt, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT};

// Minimum number of sequential small state events to collapse 
const MIN_GROUP_SIZE_FOR_COLLAPSE: usize = 3;
// Maximum number of user names to display before coalescing
const SUMMARY_LENGTH: usize = 4;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::room_read_receipt::*;
    pub SmallStateHeader = <View> {
        width: Fill,
        height: Fit
        visible: false
        padding: { left: 7.0, top: 2.0, bottom: 2.0 }
        flow: Right,
        align: { y: 0.5 }
        spacing: 7.0

        user_event_avatar_row = <AvatarRow> {
            margin: { left: 10.0 },
        }
        
        summary_text = <Label> {
            width: Fill, height: Fit
            flow: RightWrap,
            padding: 0,
            draw_text: {
                wrap: Word,
                text_style: <THEME_FONT_REGULAR>{
                    font_size: (SMALL_STATE_FONT_SIZE),
                },
                color: (SMALL_STATE_TEXT_COLOR)
            }
        }

        // Collapsible button for small state event groups
        // Shows on the first item of each group to toggle expansion
        // Text is dynamically updated: ▶ (collapsed) or ▼ (expanded)
        collapsible_button = <Button> {
            width: Fit,
            height: Fit,
            margin: { left: 5, right: 5 }
            padding: { left: 4, right: 4, top: 2, bottom: 2 }
            text: "▼"  // Default to collapsed state
            draw_text: {
                text_style: <SMALL_STATE_TEXT_STYLE> {},
                color: #666
            }
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }
        }
    }
}

/// Represents a group of adjacent small state events that can be collapsed/expanded in the UI.
///
/// This struct encapsulates the grouping logic for small state events (membership changes,
/// profile changes, etc.) that appear together in the timeline. Groups can be toggled
/// between expanded (showing all individual events) and collapsed (showing a summary).
///
/// The group is identified by a range of timeline item indices, and contains a mapping
/// of user events within that range for generating summaries.
#[derive(Debug, Clone, Default)]
pub struct SmallStateGroup {
    /// The range of timeline item indices that belong to this group.
    ///
    /// The range is inclusive of the start index and exclusive of the end index.
    pub range: std::ops::Range<usize>,

    /// Whether this group is currently expanded (true) or collapsed (false).
    ///
    /// When collapsed, only the first item is shown with a summary of the group.
    /// When expanded, all items in the group are displayed individually.
    pub opened: bool,

    /// Mapping of user IDs to their events within this group.
    ///
    /// Key: The user ID who performed the state changes (OwnedUserId).
    /// Value: Vector of UserEvent containing details about the specific state changes
    /// for that user within this group.
    ///
    /// This simplified structure allows efficient lookup and summary generation.
    /// Event ordering is preserved within each user's Vec<UserEvent> by their index field.
    pub user_events_map: HashMap<OwnedUserId, Vec<UserEvent>>,

    /// Cached summary text to avoid recomputation during rendering.
    /// This is computed once when the group is created or modified.
    pub cached_summary: Option<String>,

    /// Cached list of user IDs for avatar display, pre-sorted and limited.
    /// This avoids expensive sorting and extraction during rendering.
    pub cached_avatar_user_ids: Option<Vec<OwnedUserId>>,
}

/// Combined state for managing small state groups and room creation information
#[derive(Default, Debug)]
pub struct SmallStateGroupManager {
    /// Optional room creation info (creator user ID and event ID)
    pub room_creation: Option<(OwnedUserId, OwnedEventId)>,
    /// Creation collapsible list state
    pub creation_collapsible_list: CreationCollapsibleList,
    /// List of small state groups
    pub small_state_groups: Vec<SmallStateGroup>,
}

impl SmallStateGroupManager {
    /// Analyzes a timeline item and determines its grouping behavior.
    /// 
    /// Returns computed group state result without performing UI operations.
    pub fn compute_group_state(
        &mut self,
        username: String,
        current_item: &UserEvent,
        previous_item_is_small_state: bool,
        next_item_is_small_state: bool,
    ) -> GroupStateResult {
        if !previous_item_is_small_state && !next_item_is_small_state {
            return GroupStateResult {
                show: true,
                collapsible_button: CollapsibleButton::None,
                summary_text: None,
                avatar_user_ids: None,
            };
        }

        // Handle room creation events as a special case
        if let Some((show, collapsible_button)) = process_room_creation_event(
            current_item, previous_item_is_small_state, self
        ) {
            let summary_text = if collapsible_button != CollapsibleButton::None {
                Some(format!(
                    "{} created and configured the room",
                    self.creation_collapsible_list.username
                ))
            } else {
                None
            };
            let avatar_user_ids = if let Some((creator_id, _)) = &self.room_creation {
                Some(vec![creator_id.clone()])
            } else {
                None
            };
            
            return GroupStateResult {
                show: show || !previous_item_is_small_state,
                collapsible_button,
                summary_text,
                avatar_user_ids,
            };
        }

        // Set username for creation collapsible list and create user event
        self.creation_collapsible_list.username = username.clone();

        // Handle creation collapsible list logic
        if let Some((show, collapsible_button)) = process_room_setup_events(
            current_item, self
        ) {
            return GroupStateResult {
                show,
                collapsible_button,
                summary_text: None,
                avatar_user_ids: None,
            };
        }

        // Try to find and update existing groups
        if let Some((show, collapsible_button)) = find_and_update_existing_group(
            current_item, self, previous_item_is_small_state
        ) {
            // Get cached data for the group if it's the start of a group
            let (summary_text, avatar_user_ids) = if collapsible_button != CollapsibleButton::None {
                // Find the group and get its cached data
                if let Some(group) = self.small_state_groups.iter_mut().find(|g| g.range.start == current_item.index) {
                    let summary = Some(group.get_summary().to_string());
                    let avatars = Some(group.get_avatar_user_ids().to_vec());
                    (summary, avatars)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            
            return GroupStateResult {
                show,
                collapsible_button,
                summary_text,
                avatar_user_ids,
            };
        }

        // Create new group if needed
        if let Some((show, collapsible_button)) = create_new_group_if_needed(
        current_item, previous_item_is_small_state,next_item_is_small_state, self
        ) {
            return GroupStateResult {
                show,
                collapsible_button,
                summary_text: None,
                avatar_user_ids: None,
            };
        }
        // Default return - collapsed state, no collapsible button
        GroupStateResult {
            show: true,
            collapsible_button: CollapsibleButton::None,
            summary_text: None,
            avatar_user_ids: None,
        }
    }

    /// Handles the rendering logic for small state events based on their group state.
    /// This function manages visibility, collapsible button states, and summary text display
    /// for timeline items that are part of collapsible groups.
    ///
    /// # Arguments
    /// * `cx` - Makepad context for UI operations
    /// * `item` - The widget reference for the timeline item
    /// * `item_id` - The index of this item in the timeline
    /// * `opened` - Whether this individual item should be rendered (based on group state)
    /// * `collapsible_button` - Collapsible button state for this item
    /// * `room_id` - The room ID for avatar fetching
    pub fn render_collapsible_button_and_body(
        &mut self,
        cx: &mut Cx,
        item: &WidgetRef,
        item_id: usize,
        opened: bool,
        collapsible_button: CollapsibleButton,
        room_id: &matrix_sdk::ruma::RoomId,
    ) {
        // Render logic based on group state
        if opened {
            // This item should be visible - set appropriate button text if this is a group leader
            if collapsible_button != CollapsibleButton::None {
                // Update button text to show current group state:
                // ▶ = group is expanded (click to collapse)
                // ▼ = group is collapsed (click to expand)
                let button_text = if collapsible_button == CollapsibleButton::Expanded { "▼" } else { "▶" };
                item.button(ids!(collapsible_button))
                    .set_text(cx, button_text);
                if self.room_creation.is_some()
                    && self.creation_collapsible_list.range.start == item_id
                {
                    let summary_text = format!(
                        "{} created and configured the room",
                        self.creation_collapsible_list.username
                    );
                    item.view(ids!(small_state_header)).set_visible(cx, true);
                    
                    // For creation groups, show only the creator's avatar
                    if let Some((creator_id, _)) = &self.room_creation {
                        let creator_ids = vec![creator_id.clone()];
                        populate_avatar_row_from_user_ids(cx, item, room_id, &creator_ids);
                    }
                    
                    item.label(ids!(small_state_header.summary_text))
                        .set_text(cx, &summary_text);
                    item.view(ids!(body)).set_visible(cx, false);
                    return;
                }
                // Find the group and use cached data for rendering
                for group in &mut self.small_state_groups {
                    if group.range.start == item_id {
                        item.view(ids!(small_state_header)).set_visible(cx, true);
                        
                        // Use cached summary text (compute if not cached)
                        let summary_text = group.get_summary();
                        item.label(ids!(small_state_header.summary_text))
                            .set_text(cx, summary_text);
                        
                        // Use cached avatar user IDs for lightweight avatar population
                        let avatar_user_ids = group.get_avatar_user_ids();
                        populate_avatar_row_from_user_ids(cx, item, room_id, avatar_user_ids);
                        
                        item.view(ids!(body)).set_visible(cx, false);
                        break;
                    }
                }
                item.view(ids!(body)).set_visible(cx, collapsible_button == CollapsibleButton::Expanded);
            } else {
                item.view(ids!(small_state_header)).set_visible(cx, false);
                item.view(ids!(body)).set_visible(cx, true);
            }
        } else {
            item.view(ids!(small_state_header)).set_visible(cx, false);
            item.view(ids!(body)).set_visible(cx, false);
        }
    }
}
/// Represent Small state type.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SmallStateType {
    CreateRoom,
    ConfigureRoom,
    Joined,
    Left,
    JoinedAndLeft,
    LeftAndJoined,
    InvitationRejected,
    InvitationRevoked,
    Invited,
    Banned,
    Unbanned,
    Kicked,
    ChangedName,
    ChangedAvatar,
    #[default]
    NoChange,
    ServerAcl,
    ChangedPins,
    MessageRemoved,
    UnableToDecrypt,
    HiddenEvent,
}

impl From<MembershipChange> for SmallStateType {
    fn from(change: MembershipChange) -> Self {
        use matrix_sdk_ui::timeline::MembershipChange;
        match change {
            MembershipChange::Joined => SmallStateType::Joined,
            MembershipChange::Left => SmallStateType::Left,
            MembershipChange::Banned => SmallStateType::Banned,
            MembershipChange::Unbanned => SmallStateType::Unbanned,
            MembershipChange::Kicked => SmallStateType::Kicked,
            MembershipChange::Invited => SmallStateType::Invited,
            MembershipChange::KickedAndBanned => SmallStateType::Banned,
            MembershipChange::InvitationAccepted => SmallStateType::Joined,
            MembershipChange::InvitationRejected => SmallStateType::InvitationRejected,
            MembershipChange::InvitationRevoked => SmallStateType::InvitationRevoked,
            MembershipChange::Knocked => SmallStateType::HiddenEvent,
            MembershipChange::KnockAccepted => SmallStateType::Joined,
            MembershipChange::KnockRetracted => SmallStateType::HiddenEvent,
            MembershipChange::None => SmallStateType::NoChange,
            MembershipChange::Error => SmallStateType::NoChange,
            MembershipChange::NotImplemented => SmallStateType::NoChange,
            MembershipChange::KnockDenied => SmallStateType::HiddenEvent,
        }
    }
}

impl From<&AnyOtherFullStateEventContent> for SmallStateType {
    fn from(content: &AnyOtherFullStateEventContent) -> Self {
        match content {
            AnyOtherFullStateEventContent::RoomServerAcl(_) => SmallStateType::ServerAcl,
            AnyOtherFullStateEventContent::RoomCreate(_) => SmallStateType::CreateRoom,
            AnyOtherFullStateEventContent::_Custom { .. } => SmallStateType::HiddenEvent,
            // All other room state events are configuration changes
            _ => SmallStateType::ConfigureRoom,
        }
    }
}

impl From<&TimelineItemContent> for SmallStateType {
    fn from(content: &TimelineItemContent) -> Self {
        use matrix_sdk_ui::timeline::TimelineItemContent;
        match content {
            TimelineItemContent::MembershipChange(change) => {
                change.change()
                    .map(SmallStateType::from)
                    .unwrap_or_default()
            }
            TimelineItemContent::ProfileChange(change) => {
                match (change.avatar_url_change(), change.displayname_change()) {
                    (Some(_), _) => SmallStateType::ChangedAvatar,
                    (None, Some(_)) => SmallStateType::ChangedName,
                    _ => SmallStateType::NoChange,
                }
            }
            TimelineItemContent::OtherState(other_state) => {
                SmallStateType::from(other_state.content())
            }
            TimelineItemContent::MsgLike(MsgLikeContent { kind, .. }) => {
                match kind {
                    MsgLikeKind::Poll(_) => SmallStateType::NoChange,
                    MsgLikeKind::Redacted => SmallStateType::MessageRemoved,
                    MsgLikeKind::UnableToDecrypt(_) => SmallStateType::UnableToDecrypt,
                    _ => SmallStateType::NoChange,
                }
            }
            _ => SmallStateType::NoChange,
        }
    }
}

/// Condensed version of a `EventTimelineItem`, used for generating summaries.
#[derive(Debug, Clone, Default)]
pub struct UserEvent {
    /// The transition type
    pub transition: SmallStateType,
    /// The index of the event in the timeline
    pub index: usize,
    /// The sender of the event
    pub sender: Option<OwnedUserId>,
    /// The display name of the sender
    pub display_name: String,
    /// The state key of the event
    pub state_key: Option<String>,
    /// The ID of the event
    pub event_id: Option<OwnedEventId>,
}

/// State for the creation collapsible list
#[derive(Debug)]
pub struct CreationCollapsibleList {
    pub range: std::ops::Range<usize>,
    pub opened: bool,
    pub username: String,
}

impl Default for CreationCollapsibleList {
    fn default() -> Self {
        CreationCollapsibleList {
            range: 0..0,
            opened: true,
            username: String::new(),
        }
    }
}

impl SmallStateGroup {
    /// Computes and caches the summary text and avatar user IDs for this group.
    /// 
    /// This should be called whenever the group's user_events_map is modified.
    pub fn update_cached_data(&mut self) {
        // Cache the summary text
        self.cached_summary = Some(generate_summary(&self.user_events_map, SUMMARY_LENGTH));
        
        // Cache the avatar user IDs
        self.cached_avatar_user_ids = Some(extract_avatar_user_ids(&self.user_events_map, MAX_VISIBLE_AVATARS_IN_READ_RECEIPT));
    }

    /// Gets the cached summary text, computing it if not available.
    pub fn get_summary(&mut self) -> &str {
        if self.cached_summary.is_none() {
            self.update_cached_data();
        }
        self.cached_summary.as_ref().unwrap()
    }

    /// Gets the cached avatar user IDs, computing them if not available.
    pub fn get_avatar_user_ids(&mut self) -> &[OwnedUserId] {
        if self.cached_avatar_user_ids.is_none() {
            self.update_cached_data();
        }
        self.cached_avatar_user_ids.as_ref().unwrap()
    }
}


/// Result of group state computation, containing all necessary data for rendering.
#[derive(Debug)]
pub struct GroupStateResult {
    /// Whether to show this item in the timeline
    /// 
    /// This is always `true` for item that is first in a collapsible group.
    /// This is `false` for items under collapsed groups. 
    pub show: bool,
    /// Whether to show the collapsible button
    /// 
    /// This is always `false` for items that are not the first in a collapsible group.
    pub collapsible_button: CollapsibleButton,
    pub summary_text: Option<String>,
    pub avatar_user_ids: Option<Vec<OwnedUserId>>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum CollapsibleButton {
    Expanded,
    Collapsed,
    #[default]
    None
}
/// Combines neighboring transitions into compound actions, e.g. [Joined, Left] -> [JoinedAndLeft]
fn merge_adjacent_transitions(transitions: &[SmallStateType]) -> Vec<SmallStateType> {
    let mut res = Vec::new();
    let mut i = 0;
    while i < transitions.len() {
        let t = transitions[i];
        if i + 1 < transitions.len() {
            let t2 = transitions[i + 1];
            match (t, t2) {
                (SmallStateType::Joined, SmallStateType::Left) => {
                    res.push(SmallStateType::JoinedAndLeft);
                    i += 2;
                    continue;
                }
                (SmallStateType::Left, SmallStateType::Joined) => {
                    res.push(SmallStateType::LeftAndJoined);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        res.push(t);
        i += 1;
    }
    res
}

/// Groups consecutive identical transitions with their count (e.g. [JoinedAndLeft, JoinedAndLeft] -> (JoinedAndLeft, 2))
fn group_repeated_transitions(transitions: &[SmallStateType]) -> Vec<(SmallStateType, usize)> {
    let mut res = Vec::new();
    for &t in transitions {
        if let Some((last, count)) = res.last_mut() {
            if *last == t {
                *count += 1;
                continue;
            }
        }
        res.push((t, 1));
    }
    res
}

/// Creates human-readable text for a transition type with user count and repetition count
fn format_transition_text(
    transition: SmallStateType,
    user_count: usize,
    repeat_count: usize,
) -> String {
    let is_plural = user_count > 1;
    match transition {
        SmallStateType::CreateRoom => "created and configured the room".to_string(),
        SmallStateType::ConfigureRoom => "".to_string(),
        SmallStateType::Joined => {
            if repeat_count > 1 {
                if is_plural {
                    format!("joined (×{})", repeat_count)
                } else {
                    format!("joined the room (×{})", repeat_count)
                }
            } else {
                if is_plural {
                    "joined".to_string()
                } else {
                    "joined the room".to_string()
                }
            }
        }
        SmallStateType::Left => {
            if repeat_count > 1 {
                if is_plural {
                    format!("left (×{})", repeat_count)
                } else {
                    format!("left the room (×{})", repeat_count)
                }
            } else {
                if is_plural {
                    "left".to_string()
                } else {
                    "left the room".to_string()
                }
            }
        }
        SmallStateType::JoinedAndLeft => {
            if repeat_count > 1 {
                format!("joined and left (×{})", repeat_count)
            } else {
                "joined and left".to_string()
            }
        }
        SmallStateType::LeftAndJoined => {
            if repeat_count > 1 {
                format!("left and rejoined (×{})", repeat_count)
            } else {
                "left and rejoined".to_string()
            }
        }
        SmallStateType::ChangedName => {
            if repeat_count > 1 {
                format!("changed their name (×{})", repeat_count)
            } else {
                "changed their name".to_string()
            }
        }
        SmallStateType::ChangedAvatar => {
            if repeat_count > 1 {
                format!("changed their profile picture (×{})", repeat_count)
            } else {
                "changed their profile picture".to_string()
            }
        }
        SmallStateType::Invited => {
            if repeat_count > 1 {
                format!("was invited (×{})", repeat_count)
            } else {
                "was invited".to_string()
            }
        }
        SmallStateType::Banned => {
            if repeat_count > 1 {
                format!("was banned (×{})", repeat_count)
            } else {
                "was banned".to_string()
            }
        }
        SmallStateType::Unbanned => {
            if repeat_count > 1 {
                format!("was unbanned (×{})", repeat_count)
            } else {
                "was unbanned".to_string()
            }
        }
        SmallStateType::InvitationRejected => {
            if repeat_count > 1 {
                format!("rejected invite (×{})", repeat_count)
            } else {
                "rejected invite".to_string()
            }
        }
        SmallStateType::InvitationRevoked => {
            if repeat_count > 1 {
                format!("invite withdrawn (×{})", repeat_count)
            } else {
                "invite withdrawn".to_string()
            }
        }
        SmallStateType::Kicked => {
            if repeat_count > 1 {
                format!("was kicked (×{})", repeat_count)
            } else {
                "was kicked".to_string()
            }
        }
        SmallStateType::ServerAcl => {
            if repeat_count > 1 {
                format!("updated server ACLs (×{})", repeat_count)
            } else {
                "updated server ACLs".to_string()
            }
        }
        SmallStateType::ChangedPins => {
            if repeat_count > 1 {
                format!("changed pinned messages (×{})", repeat_count)
            } else {
                "changed pinned messages".to_string()
            }
        }
        SmallStateType::MessageRemoved => {
            if repeat_count > 1 {
                format!("removed a message (×{})", repeat_count)
            } else {
                "removed a message".to_string()
            }
        }
        SmallStateType::HiddenEvent => {
            if repeat_count > 1 {
                format!("did a hidden event (×{})", repeat_count)
            } else {
                "did a hidden event".to_string()
            }
        }
        SmallStateType::NoChange => {
            if repeat_count > 1 {
                format!("made no changes (×{})", repeat_count)
            } else {
                "made no changes".to_string()
            }
        }
        SmallStateType::UnableToDecrypt => {
            if repeat_count > 1 {
                format!("decryption failed (×{})", repeat_count)
            } else {
                "decryption failed".to_string()
            }
        }
    }
}

/// Produce an English-readable name list, with "and N others"
fn format_user_list(user_names: &[String], max_display_count: usize) -> String {
    match user_names.len() {
        0 => "".into(),
        1 => user_names[0].clone(),
        2 => format!("{} and {}", user_names[0], user_names[1]),
        3 => format!("{}, {}, and {}", user_names[0], user_names[1], user_names[2]),
        n if n <= max_display_count => {
            let all_but_last = &user_names[..n-1];
            let last = &user_names[n-1];
            format!("{}, and {}", all_but_last.join(", "), last)
        },
        n => format!(
            "{}, and {} others",
            user_names[..max_display_count].join(", "),
            n - max_display_count
        ),
    }
}

/// Convert an event timeline item to a user event
pub fn convert_event_tl_item_to_user_event(
    event_item: &EventTimelineItem,
    index: usize,
) -> UserEvent {
    use matrix_sdk_ui::timeline::TimelineItemContent;
    let state_key = match event_item.content() {
        TimelineItemContent::MembershipChange(change) => {
            Some(change.user_id().to_string())
        }
        TimelineItemContent::OtherState(other) => Some(other.state_key().to_string()),
        _ => None,
    };
    let transition_type: SmallStateType = event_item.content().into();

    UserEvent {
        index,
        display_name: event_item.sender().to_string(),
        transition: transition_type,
        sender: Some(event_item.sender().into()),
        state_key,
        event_id: event_item.event_id().map(|e|e.to_owned()),
    }
}



/// Gets the effective user ID for a user event, preferring state_key over sender.
/// This handles cases where the state_key represents the actual user being affected.
fn get_effective_user_id(user_event: &UserEvent) -> Option<OwnedUserId> {
    match user_event.state_key.as_ref().map(UserId::parse) {
        Some(Ok(user_id)) => Some(user_id),
        _ => user_event.sender.clone(),
    }
}

/// Appends a new user event to the given HashMap of user events.
/// 
/// The HashMap structure maps: user_id -> Vec<UserEvent>
/// This simplified structure groups events by user ID, avoiding the complexity
/// of maintaining timeline indices as keys.
fn append_user_event_to_map(
    user_event: UserEvent,
    user_events: &mut HashMap<OwnedUserId, Vec<UserEvent>>,
) {
    let Some(effective_user_id) = get_effective_user_id(&user_event) else { return };
    // Get or create the user's event list
    let user_events_vec = user_events.entry(effective_user_id).or_default();
    
    // Add the event if it's not already present (avoid duplicates)
    if !user_events_vec.iter().any(|event| event.index == user_event.index) {
        user_events_vec.push(user_event);
    }
}

/// Appends a user event to a group and invalidates its cache.
fn append_user_event_to_group(
    user_event: UserEvent,
    group: &mut SmallStateGroup,
) {
    append_user_event_to_map(user_event, &mut group.user_events_map);
    // Invalidate cache since the group has been modified
    group.cached_summary = None;
    group.cached_avatar_user_ids = None;
}

/// Checks if a timeline item represents a small state change (membership change, profile change, poll, redacted, or unable to decrypt).
///
/// # Arguments
/// * `timeline_item` - The timeline item to check
///
/// # Returns
/// * `bool` - Whether this is a small state event
pub fn is_small_state(timeline_item: Option<&Arc<TimelineItem>>) -> bool {
    timeline_item
        .and_then(|timeline_item| match timeline_item.kind() {
            TimelineItemKind::Event(event_tl_item) => Some(event_tl_item),
            _ => None,
        })
        .map(|e| {
            matches!(e.content(), TimelineItemContent::MembershipChange(_) | TimelineItemContent::ProfileChange(_) |
                TimelineItemContent::OtherState(_) | TimelineItemContent::MsgLike(MsgLikeContent { kind: MsgLikeKind::Poll(_) | 
                MsgLikeKind::Redacted | MsgLikeKind::UnableToDecrypt(_), .. }))
        })
        .unwrap_or(false)
}


/// Generates a summary string from user events.
///
/// # Arguments
/// * `user_events` - A HashMap mapping user IDs to their list of UserEvents
/// * `summary_length` - Maximum number of user names to display before coalescing
/// # Returns
/// * `String` - The generated summary string
fn generate_summary(
    user_events: &HashMap<OwnedUserId, Vec<UserEvent>>,
    summary_length: usize,
) -> String {
    // Aggregate by transition sequence
    let mut aggregates: Vec<(Vec<SmallStateType>, Vec<String>)> = Vec::new();

    for (user_id, events) in user_events {
        // Create sorted indices instead of cloning the entire events vector
        let mut sorted_indices: Vec<usize> = (0..events.len()).collect();
        sorted_indices.sort_by_key(|&i| events[i].index);
        let mut transitions: Vec<_> = sorted_indices.iter().map(|&i| events[i].transition).collect();

        // Filter out Joined transitions for room creators
        if transitions.contains(&SmallStateType::CreateRoom) {
            transitions.retain(|&t| t != SmallStateType::Joined);
        }
        
        let canonical = merge_adjacent_transitions(&transitions);
        let name = if let Some(&first_idx) = sorted_indices.first() {
            &events[first_idx].display_name
        } else {
            &user_id.to_string()
        };
            
        if let Some((_, names)) = aggregates.iter_mut().find(|(seq, _)| seq == &canonical) {
            names.push(name.to_string());
        } else {
            aggregates.push((canonical, vec![name.to_string()]));
        }
    }
    
    // Build text
    let mut summary_parts = Vec::new();
    for (canonical, names) in aggregates {
        let coalesced = group_repeated_transitions(&canonical);

        let descs: Vec<String> = coalesced
            .into_iter()
            .map(|(transition, repeat_count)| {
                format_transition_text(transition, names.len(), repeat_count)
            })
            .collect();
        let name_list = format_user_list(&names, summary_length);
        let transition_text = descs.join(", ");
        summary_parts.push(format!("{} {}", name_list, transition_text));
    }

    summary_parts.join(", ")
}

/// Extracts and sorts user IDs from user events map for avatar display.
/// 
/// This is the expensive computation part that should be cached.
fn extract_avatar_user_ids(
    user_events_map: &HashMap<OwnedUserId, Vec<UserEvent>>,
    max_avatars: usize,
) -> Vec<OwnedUserId> {
    // Extract user IDs from the map, sorted by their first event index for consistency
    let mut user_data: Vec<(OwnedUserId, usize)> = user_events_map
        .iter()
        .map(|(user_id, events)| {
            let first_index = events.iter().map(|e| e.index).min().unwrap_or(0);
            (user_id.clone(), first_index)
        })
        .collect();
    
    // Sort by first event index to maintain chronological order
    user_data.sort_by_key(|(_, first_index)| *first_index);
    
    // Extract just the user IDs and limit to max_avatars
    user_data
        .into_iter()
        .take(max_avatars)
        .map(|(user_id, _)| user_id)
        .collect()
}

/// Populates the avatar row with user avatars from pre-computed user IDs.
fn populate_avatar_row_from_user_ids(
    cx: &mut Cx,
    avatar_row: &WidgetRef,
    room_id: &matrix_sdk::ruma::RoomId,
    user_ids: &[OwnedUserId],
) {
    // Reuse read receipts logic to populate the avatar row.
    let receipts_map: IndexMap<OwnedUserId, matrix_sdk::ruma::events::receipt::Receipt> = user_ids
        .iter()
        .map(|user_id| {
            let receipt = matrix_sdk::ruma::events::receipt::Receipt::new(
                matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
            );
            (user_id.clone(), receipt)
        })
        .collect();
    
    avatar_row.avatar_row(ids!(user_event_avatar_row)).set_avatar_row(cx, room_id, None, &receipts_map);
}

/// Processes room creation events for special grouping treatment.
/// 
/// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
fn process_room_creation_event(
    current_item: &UserEvent,
    previous_item_is_small_state: bool,
    group_manager: &mut SmallStateGroupManager,
) -> Option<(bool, CollapsibleButton)> {
    if group_manager.room_creation.is_some() {
        return None;
    }
    let item_id = current_item.index;
    if current_item.transition == SmallStateType::CreateRoom {
        if let (Some(creator_id), Some(event_id)) = (current_item.sender.clone(), current_item.event_id.clone()) {
            group_manager.room_creation = Some((creator_id, event_id.to_owned()));
            group_manager.creation_collapsible_list.range.start = item_id;
            if group_manager.creation_collapsible_list.range.end <= item_id {
                group_manager.creation_collapsible_list.range.end = item_id + 1;
            }
            return Some((
                true,
                if previous_item_is_small_state {
                    CollapsibleButton::None
                } else if group_manager.creation_collapsible_list.opened {
                    CollapsibleButton::Expanded
                } else {
                    CollapsibleButton::Collapsed
                }
            ));
        }
    }
    None
}

/// Manages room setup events in the creation collapsible list.
/// 
/// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
fn process_room_setup_events(
    user_event: &UserEvent,
    group_manager: &mut SmallStateGroupManager,
) -> Option<(bool, CollapsibleButton)> {
    let item_id = user_event.index;
    // Check if this is the start of the creation collapsible list
    if item_id == group_manager.creation_collapsible_list.range.start
        && !group_manager.creation_collapsible_list.range.is_empty()
    {
        return Some((
            true,
            if group_manager.creation_collapsible_list.opened {
                CollapsibleButton::Expanded
            } else {
                CollapsibleButton::Collapsed
            },
        ));
    }

    // Handle configure room and joined events in creation list
    if matches!(user_event.transition, SmallStateType::ConfigureRoom | SmallStateType::Joined) {
        if group_manager.creation_collapsible_list.range.is_empty() {
            return None;
        }
        
        if group_manager.creation_collapsible_list.range.end == item_id {
            group_manager.creation_collapsible_list.range.end = item_id + 1;
            return Some((
                group_manager.creation_collapsible_list.opened,
                CollapsibleButton::None,
            ));
        }
        
        if group_manager.creation_collapsible_list.range.contains(&item_id) {
            return Some((
                group_manager.creation_collapsible_list.opened,
                CollapsibleButton::None,
            ));
        }
    }
    None
}

/// Finds and updates existing small state groups.
/// 
/// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
fn find_and_update_existing_group(
    user_event: &UserEvent,
    group_manager: &mut SmallStateGroupManager,
    previous_item_is_small_state: bool,
) -> Option<(bool, CollapsibleButton)> {
    let item_id = user_event.index;
    // First check for direct matches or containment
    for group in group_manager.small_state_groups.iter_mut() {
        if group.range.start == item_id {
            append_user_event_to_group(user_event.clone(), group);
            let collapsible_button = if group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || previous_item_is_small_state {
                CollapsibleButton::None
            } else if group.opened {
                CollapsibleButton::Expanded
            } else {
                CollapsibleButton::Collapsed
            };
            return Some((true, collapsible_button));
        }
        
        if group.range.contains(&item_id) {
            append_user_event_to_group(user_event.clone(), group);
            return Some((
                group.opened || group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE,
                CollapsibleButton::None,
            ));
        }
    }
    
    // Check for backward extension (need separate loop to avoid borrow conflicts)
    let group_ranges: Vec<Range<usize>> = group_manager
        .small_state_groups
        .iter()
        .map(|g| g.range.clone())
        .collect();
    
    for (idx, group) in group_manager.small_state_groups.iter_mut().enumerate() {
        if group.range.start == item_id + 1 {
            // Check if item_id is already covered by another group
            let mut conflict = false;
            for (other_idx, other_range) in group_ranges.iter().enumerate() {
                if other_idx != idx && other_range.contains(&item_id) {
                    conflict = true;
                    break;
                }
            }
            
            if !conflict {
                // Extend this group backwards to include current item
                let old_end = group.range.end;
                group.range = item_id..old_end;
                append_user_event_to_group(user_event.clone(), group);
                return Some((
                    group.opened || group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || !previous_item_is_small_state,
                    if previous_item_is_small_state {
                        CollapsibleButton::None
                    } else {
                        CollapsibleButton::Collapsed
                    },
                ));
            }
        }
    }
    
    None
}

/// Creates a new group if the next item is also a small state event.
/// 
/// Returns Some(result) if a new group was created, None otherwise.
fn create_new_group_if_needed(
    user_event: &UserEvent,
    previous_item_is_small_state: bool,
    next_item_is_small_state: bool,
    group_manager: &mut SmallStateGroupManager,
) -> Option<(bool, CollapsibleButton)> {
    if next_item_is_small_state {
        let mut user_events_map = HashMap::new();
        append_user_event_to_map(user_event.clone(), &mut user_events_map);
        let item_id = user_event.index;
        let mut new_group = SmallStateGroup {
            range: item_id..(item_id + 2), // Plus 2 to include the next item into the group
            opened: false,
            user_events_map,
            cached_summary: None,
            cached_avatar_user_ids: None,
        };
        
        // Pre-compute cache for the new group
        new_group.update_cached_data();
        
        group_manager.small_state_groups.push(new_group.clone());
        let collapsible_button = if new_group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || previous_item_is_small_state {
            CollapsibleButton::None
        } else {
            CollapsibleButton::Collapsed
        };
        return Some((true, collapsible_button));
    }
    None
}

/// Handles item_id changes during backward pagination by shifting indices in small SmallStateGroupManager
///
/// # Arguments
/// * `old_len` - The length of the timeline before adding new items
/// * `new_len` - The length of the timeline after adding new items  
/// * `group_manager` - Mutable reference to the small state group manager to update
pub fn handle_backward_pagination_index_shift(
    shift: i32,
    group_manager: &mut SmallStateGroupManager,
) {
    // Apply the shift to the small_state_groups
    for SmallStateGroup {
        range,
        user_events_map,
        opened: _,
        ..
    } in &mut group_manager.small_state_groups
    {
        let new_start = (range.start as i32 + shift).max(0) as usize;
        let new_end = (range.end as i32 + shift).max(0) as usize;
        *range = new_start..new_end;

        // Update the user events map indices
        for user_events in user_events_map.values_mut() {
            for user_event in user_events.iter_mut() {
                user_event.index = (user_event.index as i32 + shift).max(0) as usize;
            }
        }
    }

    // Apply the shift to the creation collapsible list
    if !group_manager.creation_collapsible_list.range.is_empty() {
        let new_start =
            (group_manager.creation_collapsible_list.range.start as i32 + shift).max(0) as usize;
        let new_end =
            (group_manager.creation_collapsible_list.range.end as i32 + shift).max(0) as usize;
        group_manager.creation_collapsible_list.range = new_start..new_end;
    }
}


/// Handles collapsible button click events for small state event groups.
/// 
/// This function manages toggling the open/closed state of groups and updates
/// the UI accordingly including button text and clearing cached drawn status.
///
/// # Arguments
/// * `cx` - Makepad context for UI operations
/// * `wr` - Widget reference for UI updates
/// * `item_id` - The index of the clicked item in the timeline
/// * `portal_list` - Portal list widget for scrolling operations
/// * `group_manager` - Mutable reference to the small state group manager
/// * `content_drawn_since_last_update` - Mutable reference to content drawn tracking
/// * `profile_drawn_since_last_update` - Mutable reference to profile drawn tracking
/// * `items_len` - Length of the timeline items list
pub fn handle_collapsible_button_click(
    cx: &mut Cx,
    wr: &WidgetRef,
    item_id: usize,
    portal_list: &makepad_widgets::PortalListRef,
    group_manager: &mut SmallStateGroupManager,
    content_drawn_since_last_update: &mut RangeSet<usize>,
    profile_drawn_since_last_update: &mut RangeSet<usize>,
    items_len: usize,
) {
    let mut is_creation_group = false;                        
    if group_manager.creation_collapsible_list.range.start == item_id {
        let open = &mut group_manager.creation_collapsible_list.opened;
        // Toggle the group's open/closed state
        *open = !*open;
        let range = &group_manager.creation_collapsible_list.range;
        let to_redraw = range.start..range.end + 1;
        // Force redraw of all items in this group by clearing their cached drawn status
        content_drawn_since_last_update.remove(to_redraw.clone());
        profile_drawn_since_last_update.remove(to_redraw);
        
        // Update button text to reflect new state:
        // ▼ (down arrow) = expanded/open - items are visible
        // ▶(right arrow) = collapsed/closed - items are hidden
        let button_text = if *open { "▼" } else { "▶" };
        wr.button(ids!(collapsible_button)).set_text(cx, button_text);
        // If the last item is a group of small state events, scroll to the end when it is expanded.
        if group_manager.creation_collapsible_list.range.end == items_len && *open {
            portal_list.smooth_scroll_to_end(cx, 90.0, None);
        }
        is_creation_group = true;
    }

    if !is_creation_group {
        for SmallStateGroup { range, opened, .. } in &mut group_manager.small_state_groups {
            if range.start == item_id {
                // Toggle the group's open/closed state
                *opened = !*opened;
                // Force redraw of all items in this group by clearing their cached drawn status
                let range_to_remove = range.clone(); // Only clone when actually needed
                content_drawn_since_last_update.remove(range_to_remove.clone());
                profile_drawn_since_last_update.remove(range_to_remove);

                // Update button text to reflect new state:
                // ▼ (down arrow) = expanded/open - items are visible
                // ▶ (right arrow) = collapsed/closed - items are hidden
                let button_text = if *opened { "▼" } else { "▶" };
                wr.button(ids!(collapsible_button)).set_text(cx, button_text);
                // If the last item is a group of small state events, scroll to the end when it is expanded.
                if range.end == items_len && *opened {
                    portal_list.smooth_scroll_to_end(cx, 90.0, None);
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::ruma::UserId;
    use ruma::EventId;

    fn create_test_user_event(
        index: usize,
        transition_type: SmallStateType,
        is_previous_small_state: bool,
        is_next_small_state: bool,
    ) -> (UserEvent, bool, bool) {
        let sender = "@alice:example.com";
        (UserEvent {
            index,
            transition: transition_type,
            display_name: sender.to_string(),
            state_key: None,
            event_id: EventId::parse("$bY-3JMD1c4gGBiGVAey0s-ndY_5NPRwYtMoXImd0LaA").ok(),
            sender: UserId::parse(sender).ok(),
        }, is_previous_small_state, is_next_small_state)
    }

    #[test]
    fn test_compute_group_state() {
        let mut group_manager = SmallStateGroupManager::default();
        let user_events = vec![
            create_test_user_event(5, SmallStateType::Left, true, true),
            create_test_user_event(4, SmallStateType::Left, true, true),
            create_test_user_event(3, SmallStateType::Left, true, true), 
            create_test_user_event(2, SmallStateType::Left, false, true)
        ];
        let mut results = HashMap::new();
        for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
            let result = group_manager.compute_group_state(
                "Alice".to_string(),
                &user_event,
                previous_item_is_small_state, // previous not small state
                next_item_is_small_state, // next not small state
            );
            results.insert(user_event.index, result);
        }
        for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
            let result = group_manager.compute_group_state(
                "Alice".to_string(),
                &user_event,
                previous_item_is_small_state, // previous not small state
                next_item_is_small_state, // next not small state
            );
            results.insert(user_event.index, result);
        }
        assert!(results.get(&2).unwrap().show);
        assert!(results.get(&2).unwrap().collapsible_button != CollapsibleButton::None);

    }

    #[test]
    fn test_compute_group_state_2_items() {
        let mut group_manager = SmallStateGroupManager::default();
        let user_events = vec![
            create_test_user_event(14, SmallStateType::ChangedName, true, false),
            create_test_user_event(13, SmallStateType::ChangedName, false, true),
        ];
        let mut results = HashMap::new();
        for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
            let result = group_manager.compute_group_state(
                "Alice".to_string(),
                &user_event,
                previous_item_is_small_state, // previous not small state
                next_item_is_small_state, // next not small state
            );
            results.insert(user_event.index, result);
        }
        for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
            let result = group_manager.compute_group_state(
                "Alice".to_string(),
                &user_event,
                previous_item_is_small_state, // previous not small state
                next_item_is_small_state, // next not small state
            );
            results.insert(user_event.index, result);
        }
        assert!(results.get(&13).unwrap().show);
        assert!(results.get(&13).unwrap().collapsible_button == CollapsibleButton::None);
    }

    #[test]
    fn test_compute_group_state_joined_items_out_creation() {
        let mut group_manager = SmallStateGroupManager::default();
        let user_events = vec![
            create_test_user_event(16, SmallStateType::Joined, true, false),
            create_test_user_event(15, SmallStateType::Joined, true, true),
            create_test_user_event(14, SmallStateType::Joined, true, true),
            create_test_user_event(13, SmallStateType::Joined, false, true),
            create_test_user_event(3, SmallStateType::Joined, true, false),
            create_test_user_event(2, SmallStateType::CreateRoom, false, true),
        ];
        let mut results = HashMap::new();
        for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
            let result = group_manager.compute_group_state(
                "Alice".to_string(),
                &user_event,
                previous_item_is_small_state, // previous not small state
                next_item_is_small_state, // next not small state
            );
            results.insert(user_event.index, result);
        }
        assert!(results.get(&13).unwrap().show);
        assert!(results.get(&13).unwrap().collapsible_button != CollapsibleButton::None);
    }
}
