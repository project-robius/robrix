use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedEventId, OwnedUserId, UserId};
use matrix_sdk_ui::timeline::{
    AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, MsgLikeContent, MsgLikeKind, TimelineItem, TimelineItemContent, TimelineItemKind
};
use makepad_widgets::{Cx, WidgetRef};
use rangemap::{RangeMap, RangeSet};
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
    pub SmallStateHeader = <View> {
        width: Fill,
        height: Fit
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmallStateGroup {
    pub is_room_creation: bool,
    pub room_creator: Option<OwnedUserId>,
    /// The range of timeline item indices that belong to this group.
    ///
    /// The range is inclusive of the start index and exclusive of the end index.
    //pub range: std::ops::Range<usize>,

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

/// Combined state for managing small state groups
#[derive(Default, Debug)]
pub struct SmallStateGroupManager {
    /// Map of small state groups by their index ranges, storing the group header's event_id
    pub small_state_groups: RangeMap<usize, OwnedEventId>,
    /// Map from event_id to actual SmallStateGroup data
    pub groups_by_event_id: HashMap<OwnedEventId, SmallStateGroup>,
}

impl SmallStateGroupManager {
    // COMMENTED OUT: This method uses the old Vec interface (iter_mut, push, etc.)
    // /// Analyzes a timeline item and determines its grouping behavior.
    // /// 
    // /// Returns computed group state result without performing UI operations.
    // pub fn compute_group_state(
    //     &mut self,
    //     username: String,
    //     current_item: &UserEvent,
    //     previous_item_is_small_state: bool,
    //     next_item_is_small_state: bool,
    // ) -> GroupStateResult {
    //     if !previous_item_is_small_state && !next_item_is_small_state {
    //         return GroupStateResult {
    //             show: true,
    //             collapsible_button: CollapsibleButton::None,
    //             summary_text: None,
    //             avatar_user_ids: None,
    //         };
    //     }

    //     // Handle room creation events as a special case
    //     if let Some((show, collapsible_button)) = process_room_creation_event(
    //         current_item, previous_item_is_small_state, self
    //     ) {
    //         let summary_text = if collapsible_button != CollapsibleButton::None {
    //             Some(format!(
    //                 "{} created and configured the room",
    //                 // self.creation_collapsible_list.username
    //                 "Unknown"
    //             ))
    //         } else {
    //             None
    //         };
    //         let avatar_user_ids = if let Some((creator_id, _)) = &self.room_creation {
    //             // Some(vec![creator_id.clone()])
    //             None
    //         } else {
    //             None
    //         };
            
    //         return GroupStateResult {
    //             show: show || !previous_item_is_small_state,
    //             collapsible_button,
    //             summary_text,
    //             avatar_user_ids,
    //         };
    //     }

    //     // Set username for creation collapsible list and create user event
    //     // self.creation_collapsible_list.username = username.clone();

    //     // Handle creation collapsible list logic
    //     if let Some((show, collapsible_button)) = process_room_setup_events(
    //         current_item, self
    //     ) {
    //         return GroupStateResult {
    //             show,
    //             collapsible_button,
    //             summary_text: None,
    //             avatar_user_ids: None,
    //         };
    //     }

    //     // Try to find and update existing groups
    //     if let Some((show, collapsible_button)) = find_and_update_existing_group(
    //         current_item, self, previous_item_is_small_state
    //     ) {
    //         // Get cached data for the group if it's the start of a group
    //         let (summary_text, avatar_user_ids) = if collapsible_button != CollapsibleButton::None {
    //             // Find the group and get its cached data
    //             if let Some(group) = self.small_state_groups.iter_mut().find(|g| g.range.start == current_item.index) {
    //                 let summary = Some(group.get_summary().to_string());
    //                 let avatars = Some(group.get_avatar_user_ids().to_vec());
    //                 (summary, avatars)
    //             } else {
    //                 (None, None)
    //             }
    //         } else {
    //             (None, None)
    //         };
            
    //         return GroupStateResult {
    //             show,
    //             collapsible_button,
    //             summary_text,
    //             avatar_user_ids,
    //         };
    //     }

    //     // Create new group if needed
    //     if let Some((show, collapsible_button)) = create_new_group_if_needed(
    //     current_item, previous_item_is_small_state,next_item_is_small_state, self
    //     ) {
    //         return GroupStateResult {
    //             show,
    //             collapsible_button,
    //             summary_text: None,
    //             avatar_user_ids: None,
    //         };
    //     }
    //     // Default return - collapsed state, no collapsible button
    //     GroupStateResult {
    //         show: true,
    //         collapsible_button: CollapsibleButton::None,
    //         summary_text: None,
    //         avatar_user_ids: None,
    //     }
    // }

    pub fn compute_group_state_2(&mut self, small_state_events: Vec<UserEvent>) {
        if small_state_events.is_empty() {
            return;
        }

        // Track room creation state locally
        let mut creation_events = Vec::new();
        let mut creation_end_index = None;
        let mut room_creator = None;
        let mut regular_events = Vec::new();
        
        for event in &small_state_events {
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

    /// Creates a room creation SmallStateGroup from creation events
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
        
        let room_creator = events.first()
            .and_then(|e| e.sender.clone());
        
        let mut new_group = SmallStateGroup {
            is_room_creation: true,
            room_creator,
            opened: false,
            user_events_map,
            cached_summary: None,
            cached_avatar_user_ids: None,
        };
        
        // Pre-compute cache for the new group
        new_group.update_cached_data();
        
        // Use the first event's event_id as the group header event_id
        if let Some(header_event_id) = events.first().and_then(|e| e.event_id.clone()) {
            self.small_state_groups.insert(start_index..end_index, header_event_id.clone());
            self.groups_by_event_id.insert(header_event_id, new_group);
        }
    }

    /// Creates a SmallStateGroup from a collection of consecutive UserEvents
    fn create_group_from_events(&mut self, events: &[UserEvent]) {
        if events.is_empty() {
            return;
        }
        let mut user_events_map = HashMap::new();
        for event in events {
            // Direct implementation since append_user_event_to_map is commented out
            if let Some(effective_user_id) = get_effective_user_id(event) {
                let user_events_vec: &mut Vec<UserEvent> = user_events_map.entry(effective_user_id).or_default();
                if !user_events_vec.iter().any(|e| e.index == event.index) {
                    user_events_vec.push(event.clone());
                }
            }
        }
        
        let start_index = events.first().unwrap().index;
        let end_index = events.last().unwrap().index + 1;
        
        let mut new_group = SmallStateGroup {
            is_room_creation: false,
            room_creator: None,
            opened: false,
            user_events_map,
            cached_summary: None,
            cached_avatar_user_ids: None,
        };
        
        // Pre-compute cache for the new group
        new_group.update_cached_data();
        
        // Use the first event's event_id as the group header event_id
        if let Some(header_event_id) = events.first().and_then(|e| e.event_id.clone()) {
            self.small_state_groups.insert(start_index..end_index, header_event_id.clone());
            self.groups_by_event_id.insert(header_event_id, new_group);
        }
    }


    // COMMENTED OUT: This method uses the old Vec interface (iter_mut)
    // Temporary stub method to maintain compilation
    /// Handles the rendering logic for small state events based on their group state.
    /// 
    /// STUB: This is a temporary stub to allow testing of compute_group_state_2.
    pub fn render_collapsible_button_and_body(
        &mut self,
        _cx: &mut Cx,
        _item: &WidgetRef,
        _item_id: usize,
        _opened: bool,
        _collapsible_button: CollapsibleButton,
        _room_id: &matrix_sdk::ruma::RoomId,
    ) {
        // Stub implementation - does nothing
    }

    // COMMENTED OUT: This method uses the old Vec interface (iter_mut, push, etc.)
    // Temporary stub method to maintain compilation
    /// Analyzes a timeline item and determines its grouping behavior.
    /// 
    /// STUB: This is a temporary stub to allow testing of compute_group_state_2.
    pub fn compute_group_state(
        &mut self,
        _username: String,
        _current_item: &UserEvent,
        _previous_item_is_small_state: bool,
        _next_item_is_small_state: bool,
    ) -> GroupStateResult {
        // Stub implementation - returns default
        GroupStateResult {
            show: true,
            collapsible_button: CollapsibleButton::None,
            summary_text: None,
            avatar_user_ids: None,
        }
    }

    /// Checks if an item is a group header and returns the count of groups before it.
    ///
    /// # Arguments
    /// * `item_id` - The index of the timeline item to check
    ///
    /// # Returns
    /// * `(bool, usize)` - A tuple containing:
    ///   - `bool`: Whether this item is a group header (first item in a group)
    ///   - `usize`: The count of groups that start before or at this item_id
    pub fn check_group_header_status(&self, item_id: usize) -> (bool, usize) {
        let groups_before = self.small_state_groups
            .iter()
            .filter(|(range, _)| range.start <= item_id)
            .count();
            
        let mut is_header = false;
        for (range, _group_id) in self.small_state_groups.iter() {
            if item_id == range.start + groups_before {
                // This is the first item in a small state group, draw collapsible button instead of message
                is_header = true;
                break;
            }
        }
        
        (is_header, groups_before)
    }

    /// Populates a SmallStateHeader widget for a group header item.
    ///
    /// # Arguments
    /// * `cx` - The drawing context
    /// * `list` - The portal list reference
    /// * `item_id` - The index of the timeline item
    /// * `room_id` - The room ID for avatar population
    /// * `item_drawn_status` - The current drawing status of the item
    ///
    /// # Returns
    /// * `(WidgetRef, ItemDrawnStatus)` - A tuple containing:
    ///   - `WidgetRef`: The SmallStateHeader widget reference
    ///   - `ItemDrawnStatus`: The updated drawing status after population
    pub fn populate_small_state_header(
        &self,
        cx: &mut Cx,
        list: &mut makepad_widgets::PortalList,
        item_id: usize,
        groups_before: usize,
        room_id: &matrix_sdk::ruma::RoomId,
    ) -> (WidgetRef, bool) {
        let (item, existed) = list.item_with_existed(cx, item_id, live_id!(SmallStateHeader));
        println!("header: {}, existed: {}", item_id, existed);
        // Find the corresponding SmallStateGroup and populate summary text and avatars
        if let Some((_range, group_event_id)) = self.small_state_groups.iter().find(|(range, _)| range.start + groups_before == item_id) {
            if let Some(group) = self.groups_by_event_id.get(group_event_id) {
                // Set the summary text
                if let Some(summary_text) = &group.cached_summary {
                    let summary_label = item.label(ids!(summary_text));
                    summary_label.set_text(cx, summary_text);
                }
                
                // Populate the avatar row
                if let Some(user_ids) = &group.cached_avatar_user_ids {
                    let avatar_row = &item;
                    populate_avatar_row_from_user_ids(cx, avatar_row, room_id, user_ids);
                }
            }
        }
        
        (item, existed)
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

// COMMENTED OUT: This function uses the old Vec interface (push, iter, any)
// /// Appends a new user event to the given HashMap of user events.
// /// 
// /// The HashMap structure maps: user_id -> Vec<UserEvent>
// /// This simplified structure groups events by user ID, avoiding the complexity
// /// of maintaining timeline indices as keys.
// fn append_user_event_to_map(
//     user_event: UserEvent,
//     user_events: &mut HashMap<OwnedUserId, Vec<UserEvent>>,
// ) {
//     let Some(effective_user_id) = get_effective_user_id(&user_event) else { return };
//     // Get or create the user's event list
//     let user_events_vec = user_events.entry(effective_user_id).or_default();
    
//     // Add the event if it's not already present (avoid duplicates)
//     if !user_events_vec.iter().any(|event| event.index == user_event.index) {
//         user_events_vec.push(user_event);
//     }
// }

// COMMENTED OUT: This function uses the old Vec interface (push via append_user_event_to_map)
// /// Appends a user event to a group and invalidates its cache.
// fn append_user_event_to_group(
//     user_event: UserEvent,
//     group: &mut SmallStateGroup,
// ) {
//     append_user_event_to_map(user_event, &mut group.user_events_map);
//     // Invalidate cache since the group has been modified
//     group.cached_summary = None;
//     group.cached_avatar_user_ids = None;
// }

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
                    Some(convert_event_tl_item_to_user_event(event_tl_item, index))
                } else {
                    None
                }
            } else {
                None
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

// COMMENTED OUT: This function references removed fields
// /// Processes room creation events for special grouping treatment.
// /// 
// /// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
// fn process_room_creation_event(
//     current_item: &UserEvent,
//     previous_item_is_small_state: bool,
//     group_manager: &mut SmallStateGroupManager,
// ) -> Option<(bool, CollapsibleButton)> {
//     if group_manager.room_creation.is_some() {
//         return None;
//     }
//     let item_id = current_item.index;
//     if current_item.transition == SmallStateType::CreateRoom {
//         if let (Some(creator_id), Some(event_id)) = (current_item.sender.clone(), current_item.event_id.clone()) {
//             group_manager.room_creation = Some((creator_id, event_id.to_owned()));
//             group_manager.creation_collapsible_list.range.start = item_id;
//             if group_manager.creation_collapsible_list.range.end <= item_id {
//                 group_manager.creation_collapsible_list.range.end = item_id + 1;
//             }
//             return Some((
//                 true,
//                 if previous_item_is_small_state {
//                     CollapsibleButton::None
//                 } else if group_manager.creation_collapsible_list.opened {
//                     CollapsibleButton::Expanded
//                 } else {
//                     CollapsibleButton::Collapsed
//                 }
//             ));
//         }
//     }
//     None
// }

// COMMENTED OUT: This function references removed fields
// /// Manages room setup events in the creation collapsible list.
// /// 
// /// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
// fn process_room_setup_events(
//     user_event: &UserEvent,
//     group_manager: &mut SmallStateGroupManager,
// ) -> Option<(bool, CollapsibleButton)> {
//     let item_id = user_event.index;
//     // Check if this is the start of the creation collapsible list
//     if item_id == group_manager.creation_collapsible_list.range.start
//         && !group_manager.creation_collapsible_list.range.is_empty()
//     {
//         return Some((
//             true,
//             if group_manager.creation_collapsible_list.opened {
//                 CollapsibleButton::Expanded
//             } else {
//                 CollapsibleButton::Collapsed
//             },
//         ));
//     }
//
//     // Handle configure room and joined events in creation list
//     if matches!(user_event.transition, SmallStateType::ConfigureRoom | SmallStateType::Joined) {
//         if group_manager.creation_collapsible_list.range.is_empty() {
//             return None;
//         }
//         
//         if group_manager.creation_collapsible_list.range.end == item_id {
//             group_manager.creation_collapsible_list.range.end = item_id + 1;
//             return Some((
//                 group_manager.creation_collapsible_list.opened,
//                 CollapsibleButton::None,
//             ));
//         }
//         
//         if group_manager.creation_collapsible_list.range.contains(&item_id) {
//             return Some((
//                 group_manager.creation_collapsible_list.opened,
//                 CollapsibleButton::None,
//             ));
//         }
//     }
//     None
// }

// COMMENTED OUT: This function uses the old Vec interface (iter_mut, iter)
// /// Finds and updates existing small state groups.
// /// 
// /// Returns an optional tuple containing a boolean flag (whether to display it) and a collapsible button state.
// fn find_and_update_existing_group(
//     user_event: &UserEvent,
//     group_manager: &mut SmallStateGroupManager,
//     previous_item_is_small_state: bool,
// ) -> Option<(bool, CollapsibleButton)> {
//     let item_id = user_event.index;
//     // First check for direct matches or containment
//     for group in group_manager.small_state_groups.iter_mut() {
//         if group.range.start == item_id {
//             append_user_event_to_group(user_event.clone(), group);
//             let collapsible_button = if group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || previous_item_is_small_state {
//                 CollapsibleButton::None
//             } else if group.opened {
//                 CollapsibleButton::Expanded
//             } else {
//                 CollapsibleButton::Collapsed
//             };
//             return Some((true, collapsible_button));
//         }
        
//         if group.range.contains(&item_id) {
//             append_user_event_to_group(user_event.clone(), group);
//             return Some((
//                 group.opened || group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE,
//                 CollapsibleButton::None,
//             ));
//         }
//     }
    
//     // Check for backward extension (need separate loop to avoid borrow conflicts)
//     let group_ranges: Vec<Range<usize>> = group_manager
//         .small_state_groups
//         .iter()
//         .map(|g| g.range.clone())
//         .collect();
    
//     for (idx, group) in group_manager.small_state_groups.iter_mut().enumerate() {
//         if group.range.start == item_id + 1 {
//             // Check if item_id is already covered by another group
//             let mut conflict = false;
//             for (other_idx, other_range) in group_ranges.iter().enumerate() {
//                 if other_idx != idx && other_range.contains(&item_id) {
//                     conflict = true;
//                     break;
//                 }
//             }
            
//             if !conflict {
//                 // Extend this group backwards to include current item
//                 let old_end = group.range.end;
//                 group.range = item_id..old_end;
//                 append_user_event_to_group(user_event.clone(), group);
//                 return Some((
//                     group.opened || group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || !previous_item_is_small_state,
//                     if previous_item_is_small_state {
//                         CollapsibleButton::None
//                     } else {
//                         CollapsibleButton::Collapsed
//                     },
//                 ));
//             }
//         }
//     }
    
//     None
// }

// COMMENTED OUT: This function uses the old Vec interface (push)
// /// Creates a new group if the next item is also a small state event.
// /// 
// /// Returns Some(result) if a new group was created, None otherwise.
// fn create_new_group_if_needed(
//     user_event: &UserEvent,
//     previous_item_is_small_state: bool,
//     next_item_is_small_state: bool,
//     group_manager: &mut SmallStateGroupManager,
// ) -> Option<(bool, CollapsibleButton)> {
//     if next_item_is_small_state {
//         let mut user_events_map = HashMap::new();
//         append_user_event_to_map(user_event.clone(), &mut user_events_map);
//         let item_id = user_event.index;
//         let mut new_group = SmallStateGroup {
//             range: item_id..(item_id + 2), // Plus 2 to include the next item into the group
//             opened: false,
//             user_events_map,
//             cached_summary: None,
//             cached_avatar_user_ids: None,
//         };
        
//         // Pre-compute cache for the new group
//         new_group.update_cached_data();
        
//         group_manager.small_state_groups.push(new_group.clone());
//         let collapsible_button = if new_group.range.len() <= MIN_GROUP_SIZE_FOR_COLLAPSE || previous_item_is_small_state {
//             CollapsibleButton::None
//         } else {
//             CollapsibleButton::Collapsed
//         };
//         return Some((true, collapsible_button));
//     }
//     None
// }

// COMMENTED OUT: This function uses the old Vec interface (iter_mut, values_mut, iter_mut)
// Temporary stub function to maintain compilation
/// Handles item_id changes during backward pagination by shifting indices in small SmallStateGroupManager
///
/// When backward pagination occurs, new items are inserted at the beginning of the timeline,
/// which shifts all existing indices. This function updates the rangemap and user event indices
/// to maintain consistency with the shifted timeline.
///
/// # Arguments
/// * `shift` - The number of positions to shift indices (positive values shift forward)
/// * `group_manager` - The group manager to update
pub fn handle_backward_pagination_index_shift(
    shift: i32,
    group_manager: &mut SmallStateGroupManager,
) {
    if shift == 0 {
        return;
    }

    // Create new rangemap with shifted ranges
    let mut new_rangemap = RangeMap::new();
    
    for (range, event_id) in group_manager.small_state_groups.iter() {
        let new_start = (range.start as i32 + shift).max(0) as usize;
        let new_end = (range.end as i32 + shift).max(0) as usize;
        let new_range = new_start..new_end;
        new_rangemap.insert(new_range, event_id.clone());
    }
    
    group_manager.small_state_groups = new_rangemap;

    // Update user event indices in all groups
    for group in group_manager.groups_by_event_id.values_mut() {
        for user_events in group.user_events_map.values_mut() {
            for user_event in user_events.iter_mut() {
                user_event.index = (user_event.index as i32 + shift).max(0) as usize;
            }
        }
        
        // Invalidate cached data since indices have changed
        group.cached_summary = None;
        group.cached_avatar_user_ids = None;
    }
}


// COMMENTED OUT: This function uses the old Vec interface (iter_mut)
// Temporary stub function to maintain compilation
/// Handles collapsible button click events for small state event groups.
/// 
/// STUB: This is a temporary stub to allow testing of compute_group_state_2.
pub fn handle_collapsible_button_click(
    _cx: &mut Cx,
    _wr: &WidgetRef,
    _item_id: usize,
    _portal_list: &makepad_widgets::PortalListRef,
    _group_manager: &mut SmallStateGroupManager,
    _content_drawn_since_last_update: &mut RangeSet<usize>,
    _profile_drawn_since_last_update: &mut RangeSet<usize>,
    _items_len: usize,
) {
    // Stub implementation - does nothing
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
            event_id: EventId::parse("$bY-3JMD1c4gGBiGVAey0s-nv_5NPRwYtMoXImd0LaA").ok(),
            sender: UserId::parse(sender).ok(),
        }, is_previous_small_state, is_next_small_state)
    }
    fn create_test_user_event2(
        index: usize,
        transition_type: SmallStateType,
    ) -> UserEvent {
        let sender = "@alice:example.com";
        UserEvent {
            index,
            transition: transition_type,
            display_name: sender.to_string(),
            state_key: None,
            event_id: EventId::parse(&format!("$event-{}-{:?}", index, transition_type)).ok(),
            sender: UserId::parse(sender).ok(),
        }
    }
    // COMMENTED OUT: This test uses the old Vec interface methods
    // #[test]
    // fn test_compute_group_state() {
    //     let mut group_manager = SmallStateGroupManager::default();
    //     let user_events = vec![
    //         create_test_user_event(5, SmallStateType::Left, true, true),
    //         create_test_user_event(4, SmallStateType::Left, true, true),
    //         create_test_user_event(3, SmallStateType::Left, true, true), 
    //         create_test_user_event(2, SmallStateType::Left, false, true)
    //     ];
    //     let mut results = HashMap::new();
    //     for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
    //         let result = group_manager.compute_group_state(
    //             "Alice".to_string(),
    //             &user_event,
    //             previous_item_is_small_state, // previous not small state
    //             next_item_is_small_state, // next not small state
    //         );
    //         results.insert(user_event.index, result);
    //     }
    //     for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
    //         let result = group_manager.compute_group_state(
    //             "Alice".to_string(),
    //             &user_event,
    //             previous_item_is_small_state, // previous not small state
    //             next_item_is_small_state, // next not small state
    //         );
    //         results.insert(user_event.index, result);
    //     }
    //     assert!(results.get(&2).unwrap().show);
    //     assert!(results.get(&2).unwrap().collapsible_button != CollapsibleButton::None);

    // }

    // COMMENTED OUT: This test uses the old Vec interface methods
    // #[test]
    // fn test_compute_group_state_2_items() {
    //     let mut group_manager = SmallStateGroupManager::default();
    //     let user_events = vec![
    //         create_test_user_event(14, SmallStateType::ChangedName, true, false),
    //         create_test_user_event(13, SmallStateType::ChangedName, false, true),
    //     ];
    //     let mut results = HashMap::new();
    //     for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
    //         let result = group_manager.compute_group_state(
    //             "Alice".to_string(),
    //             &user_event,
    //             previous_item_is_small_state, // previous not small state
    //             next_item_is_small_state, // next not small state
    //         );
    //         results.insert(user_event.index, result);
    //     }
    //     for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
    //         let result = group_manager.compute_group_state(
    //             "Alice".to_string(),
    //             &user_event,
    //             previous_item_is_small_state, // previous not small state
    //             next_item_is_small_state, // next not small state
    //         );
    //         results.insert(user_event.index, result);
    //     }
    //     assert!(results.get(&13).unwrap().show);
    //     assert!(results.get(&13).unwrap().collapsible_button == CollapsibleButton::None);
    // }

    // COMMENTED OUT: This test uses the old Vec interface methods
    // #[test]
    // fn test_compute_group_state_joined_items_out_creation() {
    //     let mut group_manager = SmallStateGroupManager::default();
    //     let user_events = vec![
    //         create_test_user_event(16, SmallStateType::Joined, true, false),
    //         create_test_user_event(15, SmallStateType::Joined, true, true),
    //         create_test_user_event(14, SmallStateType::Joined, true, true),
    //         create_test_user_event(13, SmallStateType::Joined, false, true),
    //         create_test_user_event(3, SmallStateType::Joined, true, false),
    //         create_test_user_event(2, SmallStateType::CreateRoom, false, true),
    //     ];
    //     let mut results = HashMap::new();
    //     for (user_event, previous_item_is_small_state, next_item_is_small_state)  in user_events.clone() {
    //         let result = group_manager.compute_group_state(
    //             "Alice".to_string(),
    //             &user_event,
    //             previous_item_is_small_state, // previous not small state
    //             next_item_is_small_state, // next not small state
    //         );
    //         results.insert(user_event.index, result);
    //     }
    //     assert!(results.get(&13).unwrap().show);
    //     assert!(results.get(&13).unwrap().collapsible_button != CollapsibleButton::None);
    // }
    #[test]
    fn test_new_compute_group_state_2_items() {
        let mut group_manager = SmallStateGroupManager::default();
        let small_state_events = vec![
            create_test_user_event2(2, SmallStateType::CreateRoom),
            create_test_user_event2(3, SmallStateType::Joined),
            create_test_user_event2(13, SmallStateType::Joined),
            create_test_user_event2(14, SmallStateType::Joined),
            create_test_user_event2(15, SmallStateType::Joined),
            create_test_user_event2(16, SmallStateType::Joined),
        ];
        group_manager.compute_group_state_2(small_state_events);
        println!("group_manager: {:?}", group_manager);
    }

    #[test]
    fn test_new_compute_group_state_in_room() {
        let mut group_manager = SmallStateGroupManager::default();
        let small_state_events = vec![
            UserEvent {
                transition: SmallStateType::CreateRoom,
                index: 2,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$5iVFVZRVAV_t2lQxpEA84-SHibJ_0RW6FiEHt3rF-6o").ok(),
            },
            UserEvent {
                transition: SmallStateType::Joined,
                index: 3,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("@ruitoalpha:matrix.org".to_string()),
                event_id: EventId::parse("$RiNNqcH4xjUvBpMy-9GN5QUXGBfUn0wNFzr6AXhoij8").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 4,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$qKZko9pxnm0G8_kdudNaeALlkQc35asEdFC0KoQaqFA").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 5,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$A2aaV1IgBT5Y0PY8hoUpbPurR7mejeEnhSunHcNJxC4").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 6,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$JRMNs1NLOjr2GwFDKILJ7upQ9Jj2lBPdnLG21U4ByqI").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 7,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$HYY_-sH5Ek4UDPuuegRz5K13Aq7839uI8F67BX3rkls").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 8,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$8yqHIZd-kjJkAXB0KTJHA2kWrmcYHhCae1WNvgGbyvg").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 9,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$IlYc4t03j8ILo3SWNIEruulcMgsyDIXj7VPhhbusEY4").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 10,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$298PuV38_98MW4W-SQRrZ7h_klfRqQF6zGt5Yd6XzIY").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 11,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$5GclF_FSgBvjgOFKtNJbxcED7Wv6t-juExuzvjypsuw").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 12,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$JmpSXEwW2xw19IsdtG5iEv8WsArErTnfNqiaGTxYqso").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 13,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$GYKDh99yuEQ_zyVUf3UxHmoyCPLXuQsNU4xTwGKKv68").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 14,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$Ad9SVGo-pTuJDCP3kcFUTd_G3wZmZB_mllGaX_0sCx8").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 15,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$L5B6xnrT_qVO4cmBTc291c8OAOmo6Soys1_eYt7g5cU").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 17,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$82Y0r4Gr0mPDShQ1UUo8clRjziL6Hb481LCv7AQ1Vd0").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 18,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$u03fCRDsrKdAxzN9d96Wp2302XV3LfHJAq7MeYg8Cj4").ok(),
            },
            UserEvent {
                transition: SmallStateType::ChangedName,
                index: 20,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$XrpaH-WD3SfJ9tZty4f9jnPYnPZR1RggjKsBulY8SB4").ok(),
            },
            UserEvent {
                transition: SmallStateType::ChangedName,
                index: 21,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$-jOWJ53Gpqs0IzMmpzzuSIxmEJ-nzAnKz5mG5f7PhYs").ok(),
            },
            UserEvent {
                transition: SmallStateType::ChangedName,
                index: 23,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$joGXHDoxL4oEAHSL7tJHY-vnigQAP12qhAMDWIbawqo").ok(),
            },
        ];
        
        group_manager.compute_group_state_2(small_state_events);
        
        // Test with item_id 1 (not a header)
        let item_id = 1;
        let (is_header, groups_before) = group_manager.check_group_header_status(item_id);
        println!("item_id {}: is_header: {}, groups_before: {}", item_id, is_header, groups_before);
        
        // Test with item_id 2 (is a header)
        let item_id = 2;
        let (is_header, groups_before) = group_manager.check_group_header_status(item_id);
        println!("item_id {}: is_header: {}, groups_before: {}", item_id, is_header, groups_before);
        
        // Test with item_id 10 (is a header)
        let item_id = 10;
        let (is_header, groups_before) = group_manager.check_group_header_status(item_id);
        println!("item_id {}: is_header: {}, groups_before: {}", item_id, is_header, groups_before);
        //println!("group_manager: {:?}", group_manager);

    }

    #[test]
    fn test_new_compute_group_state_in_room_shift() {
        let mut group_manager = SmallStateGroupManager::default();
        let small_state_events = vec![
            UserEvent {
                transition: SmallStateType::Joined,
                index: 3,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("@ruitoalpha:matrix.org".to_string()),
                event_id: EventId::parse("$RiNNqcH4xjUvBpMy-9GN5QUXGBfUn0wNFzr6AXhoij8").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 4,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$qKZko9pxnm0G8_kdudNaeALlkQc35asEdFC0KoQaqFA").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 5,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$qKZko9pxnm0G8_kdudNaeALlkQc35asEdFC0KoQaqFA").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 10,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$298PuV38_98MW4W-SQRrZ7h_klfRqQF6zGt5Yd6XzIY").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 11,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$298PuV38_98MW4W-SQRrZ7h_klfRqQF6zGt5Yd6XzIY").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 12,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$298PuV38_98MW4W-SQRrZ7h_klfRqQF6zGt5Yd6XzIY").ok(),
            },
        ];
        
        group_manager.compute_group_state_2(small_state_events);
        println!("group_manager: {:?}", group_manager);

        let small_state_events = vec![
            UserEvent {
                transition: SmallStateType::CreateRoom,
                index: 1,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$5iVFVZRVAV_t2lQxpEA84-SHibJ_0RW6FiEHt3rF-6o").ok(),
            },
            UserEvent {
                transition: SmallStateType::Joined,
                index: 2,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("@ruitoalpha:matrix.org".to_string()),
                event_id: EventId::parse("$RiNNqcH4xjUvBpMy-9GN5QUXGBfUn0wNFzr6AXhoij8").ok(),
            },
            UserEvent {
                transition: SmallStateType::ConfigureRoom,
                index: 3,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: Some("".to_string()),
                event_id: EventId::parse("$qKZko9pxnm0G8_kdudNaeALlkQc35asEdFC0KoQaqFA").ok(),
            },
            UserEvent {
                transition: SmallStateType::UnableToDecrypt,
                index: 9,
                sender: Some("@ruitoalpha:matrix.org".to_owned().try_into().unwrap()),
                display_name: "@ruitoalpha:matrix.org".to_string(),
                state_key: None,
                event_id: EventId::parse("$298PuV38_98MW4W-SQRrZ7h_klfRqQF6zGt5Yd6XzIY").ok(),
            },
        ];
        handle_backward_pagination_index_shift(
            1,
            &mut group_manager,
        );
        group_manager.compute_group_state_2(small_state_events);
    }

}
