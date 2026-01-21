use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedUserId, UserId};
use matrix_sdk_ui::timeline::{
    AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, MsgLikeContent, MsgLikeKind, TimelineItem, TimelineItemContent, TimelineItemKind
};
use rangemap::RangeMap;
use std::{collections::HashMap, sync::Arc};
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
    use crate::shared::fold_button_with_text::FoldButtonWithText;
    use crate::shared::view_list::ViewList;

    // FoldHeader for grouped small state events
    // Follows the pattern from portal_list_auto_grouping skills doc
    pub SmallStateGroupHeader = <FoldHeader> {
        // Header: Always visible, shows summary and fold button
        header: <View> {
            width: Fill,
            height: Fit
            padding: { left: 7.0, top: 2.0, bottom: 2.0 }
            flow: Down,
            spacing: 7.0

            <View> {
                width: Fill,
                height: Fit
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
            }

            <View> {
                width: Fill, height: Fit,
                flow: Right,
                align: {x: 0.5, y: 0.5},
                padding: {top: 4},
                fold_button = <FoldButtonWithText> {
                    open_text: "Show More"
                    close_text: "Show Less"
                }
            }
        }

        body: <View> {}
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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SmallStateGroup {
    pub room_creator: Option<OwnedUserId>,
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

/// Combined state for managing small state groups
#[derive(Default, Debug)]
pub struct SmallStateGroupManager {
    /// Map of small state groups by their index ranges, storing the group header's event_id
    pub small_state_groups: RangeMap<usize, OwnedEventId>,
    /// Map from event_id to actual SmallStateGroup data
    pub groups_by_event_id: HashMap<OwnedEventId, SmallStateGroup>,
}

impl SmallStateGroupManager {
    /// Recomputes the grouped small state view from a flat list of `UserEvent`s.
    ///
    /// This method:
    /// - Clears and repopulates `small_state_groups` with fresh group ranges.
    /// - Detects and builds a dedicated room-creation group for the creator's early events.
    /// - Builds groups of consecutive small state events for other transitions.
    pub fn compute_group_state(&mut self, small_state_events: Vec<UserEvent>) {
        if small_state_events.is_empty() {
            self.small_state_groups.clear();
            self.groups_by_event_id.clear();
            return;
        }
        self.small_state_groups.clear();
        self.groups_by_event_id.clear();
        // Track room creation state locally
        let mut creation_events = Vec::new();
        let mut creation_end_index = None;
        let mut room_creator = None;
        let mut regular_events = Vec::new();

        for event in &small_state_events {
            // Skip VirtualTimelineItem events - they should not be grouped
            if event.transition == SmallStateType::VirtualTimelineItem {
                continue;
            }

            if event.transition == SmallStateType::CreateRoom {
                // Start tracking room creation
                room_creator = event.sender.clone();
                creation_end_index = Some(event.index);
                creation_events.push(event.clone());
            } else if let Some(last_creation_index) = creation_end_index {
                if (event.transition == SmallStateType::Joined || event.transition == SmallStateType::ConfigureRoom) &&
                   event.sender.as_ref() == room_creator.as_ref() &&
                   event.index == last_creation_index + 1 {
                    // Include consecutive room creator events in room creation group
                    creation_end_index = Some(event.index);
                    creation_events.push(event.clone());
                } else {
                    // End room creation group and process it
                    if !creation_events.is_empty() {
                        self.create_room_creation_group(&creation_events);
                        creation_events.clear();
                    }
                    creation_end_index = None;
                    room_creator = None;
                    // Add current event to regular events
                    regular_events.push(event.clone());
                }
            } else {
                // Add to regular events
                regular_events.push(event.clone());
            }
        }

        // Process any remaining room creation group
        if !creation_events.is_empty() {
            self.create_room_creation_group(&creation_events);
        }

        // Group consecutive regular events
        if regular_events.is_empty() {
            return;
        }
        let mut current_group_events = vec![regular_events[0].clone()];
        for i in 1..regular_events.len() {
            let prev_index = regular_events[i - 1].index;
            let current_index = regular_events[i].index;

            // Check if current event is consecutive to the previous one
            if current_index == prev_index + 1 {
                // Add to current group
                current_group_events.push(regular_events[i].clone());
            } else {
                // Process current group if it has enough events
                if current_group_events.len() >= MIN_GROUP_SIZE_FOR_COLLAPSE {
                    self.create_group_from_events(&current_group_events);
                }

                // Start a new group
                current_group_events = vec![regular_events[i].clone()];
            }
        }

        // Process the final group
        if current_group_events.len() >= MIN_GROUP_SIZE_FOR_COLLAPSE {
            self.create_group_from_events(&current_group_events);
        }
    }

    /// Creates or updates a room-creation `SmallStateGroup` from the given creation events.
    ///
    /// The resulting group:
    /// - Covers the contiguous range spanned by `events`.
    /// - Is marked as `is_room_creation` and stores the room creator.
    /// - Is registered in both `small_state_groups` and `groups_by_event_id`.
    fn create_room_creation_group(&mut self, events: &[UserEvent]) {
        if events.is_empty() {
            return;
        }

        let start_index = events.first().unwrap().index;
        let end_index = events.last().unwrap().index + 1;

        let mut user_events_map = HashMap::new();
        for event in events {
            if let Some(effective_user_id) = get_effective_user_id(event) {
                let user_events_vec: &mut Vec<UserEvent> = user_events_map.entry(effective_user_id).or_default();
                if !user_events_vec.iter().any(|e| e.index == event.index) {
                    user_events_vec.push(event.clone());
                }
            }
        }

        // Get or create a synthetic event_id if none exists
        let header_event_id = events.first()
            .and_then(|e| e.event_id.clone())
            .unwrap_or_else(|| {
                // Create synthetic event ID based on range
                OwnedEventId::try_from(format!("$synthetic_{}_{}", start_index, end_index).as_str())
                    .expect("Failed to create synthetic event ID")
            });

        self.small_state_groups.insert(start_index..end_index, header_event_id.clone());

        use std::collections::hash_map::Entry;
        match self.groups_by_event_id.entry(header_event_id) {
            Entry::Occupied(mut entry) => {
                let group = entry.get_mut();
                group.room_creator = events.first().and_then(|e| e.sender.clone());
                group.user_events_map = user_events_map;
                group.cached_summary = None;
                group.cached_avatar_user_ids = None;
                group.update_cached_data();
            }
            Entry::Vacant(entry) => {
                let mut group = SmallStateGroup {
                    room_creator: events.first().and_then(|e| e.sender.clone()),
                    user_events_map,
                    cached_summary: None,
                    cached_avatar_user_ids: None,
                };
                group.update_cached_data();
                entry.insert(group);
            }
        }
    }

    /// Creates or updates a regular `SmallStateGroup` from consecutive `UserEvent`s.
    ///
    /// The resulting group:
    /// - Covers the contiguous range spanned by `events`.
    /// - Is registered in `small_state_groups` and `groups_by_event_id`.
    /// - Is not marked as a room-creation group.
    fn create_group_from_events(&mut self, events: &[UserEvent]) {
        if events.is_empty() {
            return;
        }

        let start_index = events.first().unwrap().index;
        let end_index = events.last().unwrap().index + 1;

        let mut user_events_map = HashMap::new();
        for event in events {
            if let Some(effective_user_id) = get_effective_user_id(event) {
                let user_events_vec: &mut Vec<UserEvent> = user_events_map.entry(effective_user_id).or_default();
                if !user_events_vec.iter().any(|e| e.index == event.index) {
                    user_events_vec.push(event.clone());
                }
            }
        }

        // Get or create a synthetic event_id if none exists
        let header_event_id = events.first()
            .and_then(|e| e.event_id.clone())
            .unwrap_or_else(|| {
                // Create synthetic event ID based on range
                OwnedEventId::try_from(format!("$synthetic_{}_{}", start_index, end_index).as_str())
                    .expect("Failed to create synthetic event ID")
            });

        self.small_state_groups.insert(start_index..end_index, header_event_id.clone());

        use std::collections::hash_map::Entry;
        match self.groups_by_event_id.entry(header_event_id) {
            Entry::Occupied(mut entry) => {
                let group = entry.get_mut();
                group.room_creator = None;
                group.user_events_map = user_events_map;
                group.cached_summary = None;
                group.cached_avatar_user_ids = None;
                group.update_cached_data();
            }
            Entry::Vacant(entry) => {
                let mut group = SmallStateGroup {
                    room_creator: None,
                    user_events_map,
                    cached_summary: None,
                    cached_avatar_user_ids: None,
                };
                group.update_cached_data();
                entry.insert(group);
            }
        }
    }

    /// Checks whether the portal-list item at `item_id` is part of a group.
    ///
    /// Returns `Some(Range)` if the item is within a group range, `None` otherwise.
    /// The range indicates the full extent of the group in the timeline.
    pub fn check_group_range(&self, item_id: usize) -> Option<std::ops::Range<usize>> {
        self.small_state_groups.get_key_value(&item_id)
            .map(|(range, _)| range.clone())
    }

    /// Gets the group metadata for a group starting at the given item_id.
    ///
    /// Returns `None` if there's no group starting at this item_id.
    pub fn get_group_at_item_id(&self, item_id: usize) -> Option<&SmallStateGroup> {
        self.small_state_groups
            .iter()
            .find(|(range, _)| range.start == item_id)
            .and_then(|(_, event_id)| self.groups_by_event_id.get(event_id))
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
    VirtualTimelineItem
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        _ => "".to_string()
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
fn convert_event_timeline_item_to_user_event(
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

/// Checks if a timeline item represents a small state change (membership change, profile change, poll, redacted, or unable to decrypt).
///
/// # Arguments
/// * `timeline_item` - The timeline item to check
///
/// # Returns
/// * `bool` - Whether this is a small state event
fn is_small_state(timeline_item: Option<&Arc<TimelineItem>>) -> bool {
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

/// Extracts small state events from timeline items and converts them to UserEvents.
///
/// # Arguments
/// * `timeline_items` - An iterator over timeline items to process
///
/// # Returns
/// * `Vec<UserEvent>` - A vector of converted UserEvent objects from small state timeline items
pub fn extract_small_state_events<I>(timeline_items: I) -> Vec<UserEvent>
where
    I: IntoIterator<Item = std::sync::Arc<matrix_sdk_ui::timeline::TimelineItem>>,
    I::IntoIter: ExactSizeIterator,
{
    timeline_items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if is_small_state(Some(&item)) {
                if let matrix_sdk_ui::timeline::TimelineItemKind::Event(event_tl_item) = item.kind() {
                    Some(convert_event_timeline_item_to_user_event(event_tl_item, index))
                } else {
                    None
                }
            } else {
                if let matrix_sdk_ui::timeline::TimelineItemKind::Virtual(_) = item.kind() {
                    Some(UserEvent {
                            index,
                            display_name: "".to_string(),
                            transition: SmallStateType::VirtualTimelineItem,
                            sender: None,
                            state_key: None,
                            event_id: None,
                        })
                } else {
                    None
                }
            }
        })
        .collect()
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
pub fn populate_avatar_row_from_user_ids(
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
