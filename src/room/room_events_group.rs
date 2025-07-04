use std::collections::BTreeMap;
use std::collections::HashMap;

use makepad_widgets::*;
use matrix_sdk_ui::timeline::{TimelineItem, TimelineItemKind, TimelineItemContent};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;

    pub GroupedSmallStateEvent = {{GroupedSmallStateEvent}} {
        width: Fill,
        height: Fit,
        flow: Right,
        padding: { left: 7.0, right: 7.0 }
        margin: { left: 9.5 }

        summary_text = <Label> {
            width: Fill,
            height: Fit,
            align: { y: 0.5 },
            draw_text: {
                wrap: Word,
                text_style: <SMALL_STATE_TEXT_STYLE> {},
                color: (SMALL_STATE_TEXT_COLOR)
            }
            text: ""
        }

        collapse_button = <Button> {
            width: Fit,
            height: Fit,
            margin: 0,

            draw_bg: {
                instance color: (COLOR_PRIMARY)
                instance color_hover: #A
                instance border_size: 0.0
                instance border_color: #D0D5DD
                instance border_radius: 3.0

                fn get_color(self) -> vec4 {
                    return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.border_size,
                        self.border_size,
                        self.rect_size.x - (self.border_size * 2.0),
                        self.rect_size.y - (self.border_size * 2.0),
                        max(1.0, self.border_radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_size > 0.0 {
                        sdf.stroke(self.border_color, self.border_size)
                    }
                    return sdf.result;
                }
            }

            text: "expand",
            draw_text: {
                text_style: <REGULAR_TEXT>{font_size: 10},
                color: #000
                fn get_color(self) -> vec4 {
                    return self.color;
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct GroupedSmallStateEvent {
    #[deref] view: View,
    #[rust] group_id: Option<String>,
}

impl Widget for GroupedSmallStateEvent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle events AFTER the view has processed them
        self.view.handle_event(cx, event, scope);
        
        // Then check for button clicks
        
        if let Event::Actions(actions) = event {
            // Check for button click using ButtonAction
            if self.view.button(id!(collapse_button)).clicked(actions) {
                log!("Button clicked in GroupedSmallStateEvent!");
                
                // Send action with widget UID so the handler can look up the actual group_id
                // Don't change button text here - let the state management handle it during redraw
                log!("Sending ToggleExpanded action for widget UID: {:?}", self.widget_uid());
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    GroupedSmallStateEventAction::ToggleExpanded { 
                        group_id: format!("widget_{}", self.widget_uid().0),
                        expanded: true // This parameter isn't actually used in the handler
                    },
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl GroupedSmallStateEvent {
    pub fn set_group_id(&mut self, group_id: String) {
        self.group_id = Some(group_id);
    }
}

pub struct EventsGroupedState {
    active_groups: BTreeMap<usize, GroupedEventsState>,
    pub current_grouping_group: Option<GroupedEventsState>,
    pub config: GroupingConfig,
    expanded_groups: std::collections::HashSet<String>,
    widget_to_group_map: std::collections::HashMap<u64, String>,
    // Cache for summary texts to prevent constant regeneration
    summary_cache: std::collections::HashMap<String, String>,
}

impl EventsGroupedState {
    pub fn new() -> Self {
        log!("Creating new EventsGroupedState");
        Self {
            active_groups: BTreeMap::new(),
            current_grouping_group: None,
            config: GroupingConfig::default(),
            expanded_groups: std::collections::HashSet::new(),
            widget_to_group_map: std::collections::HashMap::new(),
            summary_cache: std::collections::HashMap::new(),
        }
    }

    pub fn try_add_event_to_group(
        &mut self,
        index: usize,
        group_type: EventGroupType,
    ) -> GroupingAction {
        if !self.config.enabled {
            return GroupingAction::None
        }

        if let Some(existing_group) = self.find_group_containing(index) {
            return GroupingAction::ExtendGroup(existing_group.clone())
        }

        match &mut self.current_grouping_group {
            Some(ref mut current_group) => {
                if current_group.can_extend_with(index, &group_type, &self.config) {
                    current_group.extend_to(index);
                    GroupingAction::ExtendGroup(current_group.clone())
                } else {
                    let finalized_group = self.finish_current_grouping();
                    self.start_new_group(index, group_type);
                    GroupingAction::FinishedAndStartNewGroup {
                        previous_group: finalized_group,
                        new_group: self.current_grouping_group.clone(),
                    }
                }
            },
            None => {
               self.start_new_group(index, group_type);
               GroupingAction::StartNewGroup(self.current_grouping_group.clone())
            },
        }
    }

    fn start_new_group(
        &mut self,
        index: usize,
        group_type: EventGroupType
    ) {
        let group_id = format!("group_{}_{:?}", index, group_type);
        self.current_grouping_group = Some(GroupedEventsState {
            group_id,
            start_index: index,
            end_index: index,
            group_type,
            items_count: 1,
            is_finalized: false,
        });
    }

    pub fn finish_current_grouping(&mut self) -> Option<GroupedEventsState> {
        if let Some(mut group) = self.current_grouping_group.take() {
            group.is_finalized = true;
            group.items_count = group.get_items_count();

            if group.meets_minimum_size(&self.config) {
                let start_index = group.start_index;
                // Invalidate cache when group is finalized
                self.invalidate_summary_cache(&group.group_id);
                self.active_groups.insert(start_index, group.clone());
                Some(group)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn find_group_containing(&self, index: usize) -> Option<&GroupedEventsState> {
        if let Some(ref group) = self.current_grouping_group {
            if group.contains_index(index) {
                return Some(group);
            }
        }

        for group in self.active_groups.values() {
            if group.contains_index(index) {
                return Some(group);
            }
        }
        None
    }

    pub fn toggle_group_expanded(&mut self, group_id: &str) {
        if self.expanded_groups.contains(group_id) {
            self.expanded_groups.remove(group_id);
        } else {
            self.expanded_groups.insert(group_id.to_string());
        }
    }

    pub fn is_group_expanded(&self, group_id: &str) -> bool {
        let is_expanded = self.expanded_groups.contains(group_id);
        is_expanded
    }

    pub fn find_group_by_start_index(&self, start_index: usize) -> Option<&GroupedEventsState> {
        self.active_groups.get(&start_index)
    }

    pub fn get_all_group_ids(&self) -> Vec<String> {
        self.active_groups.values()
            .map(|g| g.get_group_id().to_string())
            .collect()
    }

    pub fn register_widget_to_group(&mut self, widget_uid: u64, group_id: String) {
        self.widget_to_group_map.insert(widget_uid, group_id);
    }

    pub fn get_group_id_for_widget(&self, widget_uid: u64) -> Option<&String> {
        self.widget_to_group_map.get(&widget_uid)
    }

    pub fn get_cached_summary(&self, group_id: &str) -> Option<&String> {
        self.summary_cache.get(group_id)
    }

    pub fn cache_summary(&mut self, group_id: String, summary: String) {
        self.summary_cache.insert(group_id, summary);
    }

    pub fn invalidate_summary_cache(&mut self, group_id: &str) {
        self.summary_cache.remove(group_id);
    }

    pub fn clear_summary_cache(&mut self) {
        self.summary_cache.clear();
    }
}

impl Default for EventsGroupedState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct GroupedEventsState {
    group_id: String,
    pub start_index: usize,
    pub end_index: usize,
    group_type: EventGroupType,
    pub items_count: usize,
    is_finalized: bool,
}

impl GroupedEventsState {
    pub fn contains_index(&self, index: usize) -> bool {
        index >= self.start_index && index <= self.end_index
    }

    pub fn extend_to(&mut self, index: usize) {
        if index > self.end_index {
            self.end_index = index;
            // Don't update items_count here - it will be calculated during finalization
        }
    }

    pub fn get_cache_key(&self) -> String {
        format!("{}_{}__{}", self.group_id, self.start_index, self.end_index)
    }

    pub fn can_extend_with(
        &self,
        index: usize,
        group_type: &EventGroupType,
        _config: &GroupingConfig,
    ) -> bool {
        if self.group_type != *group_type {
            return false;
        }

        if self.is_finalized {
            return false;
        }

        if index != self.end_index + 1 {
            return false;
        }

        index == self.end_index + 1
    }

    pub fn get_items_count(&self) -> usize {
        self.end_index - self.start_index + 1
    }

    pub fn meets_minimum_size(&self, config: &GroupingConfig) -> bool {
        let group_size = self.end_index - self.start_index + 1;
        group_size >= config.min_small_state_events_group_size
    }

    pub fn get_position_info(&self, index: usize) -> Option<GroupPositionInfo> {
        if !self.contains_index(index) {
            return None;
        }

        Some(GroupPositionInfo {
            is_first: index == self.start_index,
            is_last: index == self.end_index,
            position_in_group: index - self.start_index,
            total_items: self.items_count,
        })
    }

    pub fn is_finalized(&self) -> bool {
        self.is_finalized
    }

    pub fn get_group_id(&self) -> &str {
        &self.group_id
    }

    pub fn create_summary(&self, timeline_items: &imbl::Vector<std::sync::Arc<TimelineItem>>) -> String {
        self.create_summary_with_fallback(timeline_items, true)
    }

    pub fn create_summary_with_fallback(&self, timeline_items: &imbl::Vector<std::sync::Arc<TimelineItem>>, use_fallback: bool) -> String {
        let mut user_actions: HashMap<String, Vec<String>> = HashMap::new();
        
        // Analyze events within this group's range
        for i in self.start_index..=self.end_index {
            if let Some(timeline_item) = timeline_items.get(i) {
                if let Some((user, action)) = self.analyze_timeline_item(timeline_item) {
                    user_actions.entry(user).or_insert_with(Vec::new).push(action);
                }
            }
        }
        
        // Generate summary from actual events
        let mut summary_parts = Vec::new();
        for (user, actions) in user_actions {
            if actions.len() == 1 {
                summary_parts.push(format!("{} {}", user, actions[0]));
            } else {
                // Group similar actions
                let actions_text = actions.join(" and ");
                summary_parts.push(format!("{} {}", user, actions_text));
            }
        }
        
        if summary_parts.is_empty() {
            if use_fallback {
                format!("{} state events", self.items_count)
            } else {
                "Loading...".to_string()
            }
        } else if summary_parts.len() <= 2 {
            summary_parts.join(", ")
        } else {
            format!("{}, and {} others had activity", summary_parts[0], summary_parts.len() - 1)
        }
    }
    
    fn analyze_timeline_item(&self, timeline_item: &TimelineItem) -> Option<(String, String)> {
        match timeline_item.kind() {
            TimelineItemKind::Event(event_item) => {
                let user = self.get_display_name_from_event(event_item);
                let action = match event_item.content() {
                    TimelineItemContent::MembershipChange(membership) => {
                        use matrix_sdk_ui::timeline::MembershipChange;
                        match membership.change() {
                            Some(MembershipChange::Joined) => Some("joined the room".to_string()),
                            Some(MembershipChange::Left) => Some("left the room".to_string()),
                            Some(MembershipChange::Banned) => Some("was banned".to_string()),
                            Some(MembershipChange::Unbanned) => Some("was unbanned".to_string()),
                            Some(MembershipChange::Kicked) => Some("was kicked".to_string()),
                            Some(MembershipChange::Invited) => Some("was invited".to_string()),
                            Some(MembershipChange::InvitationAccepted) => Some("accepted invitation".to_string()),
                            Some(MembershipChange::InvitationRejected) => Some("rejected invitation".to_string()),
                            _ => None,
                        }
                    },
                    TimelineItemContent::ProfileChange(profile) => {
                        let mut changes = Vec::new();
                        if profile.displayname_change().is_some() {
                            changes.push("changed their display name");
                        }
                        if profile.avatar_url_change().is_some() {
                            changes.push("changed their profile picture");
                        }
                        if !changes.is_empty() {
                            Some(changes.join(" and "))
                        } else {
                            None
                        }
                    },
                    TimelineItemContent::OtherState(_) => {
                        Some("changed room settings".to_string())
                    },
                    _ => None,
                };
                action.map(|a| (user, a))
            },
            _ => None,
        }
    }
    
    fn get_display_name_from_event(&self, event_item: &matrix_sdk_ui::timeline::EventTimelineItem) -> String {
        let sender = event_item.sender();
        let profile = event_item.sender_profile();
        
        // Try to get display name from profile
        match profile {
            matrix_sdk_ui::timeline::TimelineDetails::Ready(profile_data) => {
                if let Some(display_name) = profile_data.display_name.as_ref() {
                    return display_name.clone();
                }
            },
            _ => {}
        }
        
        // Fall back to extracting username from user ID
        let user_id_str = sender.as_str();
        if let Some(localpart) = user_id_str.strip_prefix('@').and_then(|s| s.split(':').next()) {
            return localpart.to_string();
        }
        
        user_id_str.to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventGroupType {
    SmallStateEvent,
}

#[derive(Debug, Clone)]
pub struct GroupingConfig {
    pub enabled: bool,
    pub min_small_state_events_group_size: usize,
}

impl Default for GroupingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_small_state_events_group_size: 3,
        }
    }
}

#[derive(Debug, Clone, DefaultNone)]
pub enum GroupingAction {
    StartNewGroup(Option<GroupedEventsState>),
    ExtendGroup(GroupedEventsState),
    FinishedAndStartNewGroup {
        previous_group: Option<GroupedEventsState>,
        new_group: Option<GroupedEventsState>,
    },
    None,
}

#[derive(Debug, Clone)]
pub struct GroupPositionInfo {
    pub is_first: bool,
    pub is_last: bool,
    pub position_in_group: usize,
    pub total_items: usize,
}

#[derive(Debug, Clone, DefaultNone)]
pub enum GroupedSmallStateEventAction {
    CollapseButtonClicked,
    ToggleExpanded { group_id: String, expanded: bool },
    None,
}