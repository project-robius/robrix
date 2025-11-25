//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::bouncing_dots::BouncingDotsWidgetRefExt;
use crate::shared::styles::COLOR_UNKNOWN_ROOM_AVATAR;
use crate::utils;
use crate::cpu_worker::{self, CpuJob, SearchRoomMembersJob};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::ruma::{
    events::{room::message::RoomMessageEventContent, Mentions},
    OwnedRoomId, OwnedUserId,
};
use matrix_sdk::RoomMemberships;
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;
use crate::home::room_screen::RoomScreenProps;

// Channel types for member search communication
use std::sync::{mpsc::Receiver, Arc};
use std::sync::atomic::{AtomicBool, Ordering};

/// Result type for member search channel communication
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub search_id: u64,
    pub results: Vec<usize>, // indices in members vec
    pub is_complete: bool,
    pub search_text: Arc<String>,
}

/// State machine for mention search functionality
#[derive(Debug, Default)]
enum MentionSearchState {
    /// Not in search mode
    #[default]
    Idle,

    /// Waiting for room members data to be loaded
    WaitingForMembers {
        trigger_position: usize,
        pending_search_text: String,
    },

    /// Actively searching with background task
    Searching {
        trigger_position: usize,
        search_text: String,
        receiver: Receiver<SearchResult>,
        accumulated_results: Vec<usize>,
        search_id: u64,
        cancel_token: Arc<std::sync::atomic::AtomicBool>,
    },

    /// Search was just cancelled (prevents immediate re-trigger)
    JustCancelled,
}

// Default is derived above; Idle is marked as the default variant

// Constants for mention popup height calculations
const DESKTOP_ITEM_HEIGHT: f64 = 32.0;
const MOBILE_ITEM_HEIGHT: f64 = 64.0;
const MOBILE_USERNAME_SPACING: f64 = 0.5;

// Constants for search behavior
const DESKTOP_MAX_VISIBLE_ITEMS: usize = 10;
const MOBILE_MAX_VISIBLE_ITEMS: usize = 5;
const SEARCH_BUFFER_MULTIPLIER: usize = 2;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;
    use crate::shared::bouncing_dots::BouncingDots;

    pub FOCUS_HOVER_COLOR = #C
    pub KEYBOARD_FOCUS_OR_COLOR_HOVER = #1C274C

    // Template for user list items in the mention dropdown
    UserListItem = <View> {
        width: Fill,
        height: Fit,
        margin: {left: 4, right: 4}
        padding: {left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: (COLOR_PRIMARY),
            uniform border_radius: 4.0,
            instance hover: 0.0,
            instance selected: 0.0,

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                // Draw rounded rectangle with configurable radius
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);

                if self.selected > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_COLOR_HOVER)
                } else if self.hover > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_COLOR_HOVER)
                } else {
                    // Default state
                    sdf.fill(self.color)
                }
                return sdf.result
            }
        }
        flow: Down
        spacing: 2.0

        user_info = <View> {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 8.0
            align: {y: 0.5}

            avatar = <Avatar> {
                width: 24,
                height: 24,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 12.0 }
                }}}
            }

            username = <Label> {
                height: Fit,
                draw_text: {
                    color: #000,
                    text_style: {font_size: 14.0}
                }
            }

            filler = <FillerX> {}
        }

        user_id = <Label> {
            height: Fit,
            draw_text: {
                color: #666,
                text_style: {font_size: 12.0}
            }
        }
    }

    // Template for the @room mention list item
    RoomMentionListItem = <View> {
        width: Fill,
        height: Fit,
        margin: {left: 4, right: 4}
        padding: {left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: (COLOR_PRIMARY),
            uniform border_radius: 4.0,
            instance hover: 0.0,
            instance selected: 0.0,

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);

                if self.selected > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_COLOR_HOVER)
                } else if self.hover > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_COLOR_HOVER)
                } else {
                    sdf.fill(self.color)
                }
                return sdf.result
            }
        }
        flow: Down
        spacing: 2.0
        align: {y: 0.5}

        user_info = <View> {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 8.0
            align: {y: 0.5}

            room_avatar = <Avatar> {
                width: 24,
                height: 24,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 12.0 }
                }}}
            }

            room_mention = <Label> {
                height: Fit,
                draw_text: {
                    color: #000,
                    text_style: {font_size: 14.0}
                }
                text: "Notify the entire room"
            }

            filler = <FillerX> {}
        }

        room_user_id = <Label> {
            height: Fit,
            align: {y: 0.5},
            draw_text: {
                color: #666,
                text_style: {font_size: 12.0}
            }
            text: "@room"
        }
    }

    // Template for loading indicator when members are being fetched
    LoadingIndicator = <View> {
        width: Fill,
        height: 48,
        margin: {left: 4, right: 4}
        padding: {left: 8, right: 8, top: 8, bottom: 8},
        flow: Right,
        spacing: 8.0,
        align: {x: 0.0, y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY),
        }

        loading_text = <Label> {
            height: Fit,
            draw_text: {
                color: #666,
                text_style: {font_size: 14.0}
            }
            text: "Loading members"
        }

        loading_animation = <BouncingDots> {
            width: 60,
            height: 24,
            draw_bg: {
                color: (COLOR_ROBRIX_PURPLE),
                dot_radius: 2.0,
            }
        }
    }

    // Template for no matches indicator when no users match the search
    NoMatchesIndicator = <View> {
        width: Fill,
        height: 48,
        margin: {left: 4, right: 4}
        padding: {left: 8, right: 8, top: 8, bottom: 8},
        flow: Right,
        spacing: 8.0,
        align: {x: 0.0, y: 0.5}
        draw_bg: {
            color: (COLOR_PRIMARY),
        }

        no_matches_text = <Label> {
            height: Fit,
            draw_text: {
                color: #666,
                text_style: {font_size: 14.0}
            }
            text: "No matching users found"
        }
    }

    pub MentionableTextInput = {{MentionableTextInput}}<CommandTextInput> {
        width: Fill,
        height: Fit
        trigger: "@"
        inline_search: true

        color_focus: (FOCUS_HOVER_COLOR),
        color_hover: (FOCUS_HOVER_COLOR),

        popup = {
            spacing: 0.0
            padding: 0.0

            draw_bg: {
                color: (COLOR_SECONDARY),
            }
            header_view = {
                margin: {left: 4, right: 4}
                draw_bg: {
                    color: (COLOR_ROBRIX_PURPLE),
                }
                header_label = {
                    draw_text: {
                        color: (COLOR_PRIMARY_DARKER),
                    }
                    text: "Users in this Room"
                }
            }

            list = {
                height: Fit
                clip_y: true
                spacing: 0.0
                padding: 0.0
            }
        }

        persistent = {
            top = { height: 0 }
            bottom = { height: 0 }
            center = {
                text_input = <RobrixTextInput> {
                    empty_text: "Start typing..."
                }
            }
        }

        // Template for user list items in the mention popup
        user_list_item: <UserListItem> {}
        room_mention_list_item: <RoomMentionListItem> {}
        loading_indicator: <LoadingIndicator> {}
        no_matches_indicator: <NoMatchesIndicator> {}
    }
}

// /// A special string used to denote the start of a mention within
// /// the actual text being edited.
// /// This is used to help easily locate and distinguish actual mentions
// /// from normal `@` characters.
// const MENTION_START_STRING: &str = "\u{8288}@\u{8288}";

#[derive(Debug)]
pub enum MentionableTextInputAction {
    /// Notifies the MentionableTextInput about updated power levels for the room.
    PowerLevelsUpdated {
        room_id: OwnedRoomId,
        can_notify_room: bool,
    },
    /// Notifies the MentionableTextInput that room members have been loaded.
    RoomMembersLoaded {
        room_id: OwnedRoomId,
        /// Whether member sync is still in progress
        sync_in_progress: bool,
        /// Whether we currently have cached members
        has_members: bool,
    },
}

/// Widget that extends CommandTextInput with @mention capabilities
#[derive(Live, LiveHook, Widget)]
pub struct MentionableTextInput {
    /// Base command text input
    #[deref]
    cmd_text_input: CommandTextInput,
    /// Template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    /// Template for the @room mention list item
    #[live]
    room_mention_list_item: Option<LivePtr>,
    /// Template for loading indicator
    #[live]
    loading_indicator: Option<LivePtr>,
    /// Template for no matches indicator
    #[live]
    no_matches_indicator: Option<LivePtr>,
    /// The set of users that were mentioned (at one point) in this text input.
    /// Due to characters being deleted/removed, this list is a *superset*
    /// of possible users who may have been mentioned.
    /// All of these mentions may not exist in the final text input content;
    /// this is just a list of users to search the final sent message for
    /// when adding in new mentions.
    #[rust]
    possible_mentions: BTreeMap<OwnedUserId, String>,
    /// Indicates if the `@room` option was explicitly selected.
    #[rust]
    possible_room_mention: bool,
    /// Whether the current user can notify everyone in the room (@room mention)
    #[rust]
    can_notify_room: bool,
    /// Current state of the mention search functionality
    #[rust]
    search_state: MentionSearchState,
    /// Last search text to avoid duplicate searches
    #[rust]
    last_search_text: Option<String>,
    /// Next identifier for submitted search jobs
    #[rust]
    next_search_id: u64,
    /// Whether the background search task has pending results
    #[rust]
    search_results_pending: bool,
    /// Active loading indicator widget while we wait for members/results
    #[rust]
    loading_indicator_ref: Option<WidgetRef>,
    /// Cached text analysis to avoid repeated grapheme parsing
    /// Format: (text, graphemes_as_strings, byte_positions)
    #[rust]
    cached_text_analysis: Option<(String, Vec<String>, Vec<usize>)>,
    /// Last known member count - used ONLY for change detection (not rendering)
    /// Rendering always uses props as source of truth
    #[rust]
    last_member_count: usize,
    /// Last known sync pending state - used ONLY for change detection (not rendering)
    #[rust]
    last_sync_pending: bool,
    /// Whether a deferred popup cleanup is pending after focus loss
    #[rust]
    pending_popup_cleanup: bool,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle ESC key early before passing to child widgets
        if self.is_searching() {
            if let Event::KeyUp(key_event) = event {
                if key_event.key_code == KeyCode::Escape {
                    self.cancel_active_search();
                    self.search_state = MentionSearchState::JustCancelled;
                    self.close_mention_popup(cx);
                    self.redraw(cx);
                    return; // Don't process other events
                }
            }
        }

        self.cmd_text_input.handle_event(cx, event, scope);

        // Best practice: Always check Scope first to get current context
        // Scope represents the current widget context as passed down from parents
        let (scope_room_id, scope_member_count) = {
            let room_props = scope
                .props
                .get::<RoomScreenProps>()
                .expect("RoomScreenProps should be available in scope for MentionableTextInput");
            let member_count = room_props
                .room_members
                .as_ref()
                .map(|members| members.len())
                .unwrap_or(0);
            (room_props.room_id.clone(), member_count)
        };

        // Check search channel on every frame if we're searching
        if let MentionSearchState::Searching { .. } = &self.search_state {
            if let Event::NextFrame(_) = event {
                // Only continue requesting frames if we're still waiting for results
                if self.check_search_channel(cx, scope) {
                    cx.new_next_frame();
                }
            }
        }
        // Handle deferred cleanup after focus loss
        if let Event::NextFrame(_) = event {
            if self.pending_popup_cleanup {
                self.pending_popup_cleanup = false;

                // Only close if input still doesn't have focus and we're not actively searching
                let text_input_ref = self.cmd_text_input.text_input_ref();
                let text_input_area = text_input_ref.area();
                let has_focus = cx.has_key_focus(text_input_area);

                // If user refocused or is actively typing/searching, don't cleanup
                if !has_focus && !self.is_searching() {
                    self.close_mention_popup(cx);
                }
            }
        }

        if let Event::Actions(actions) = event {
            let text_input_ref = self.cmd_text_input.text_input_ref();
            let text_input_uid = text_input_ref.widget_uid();
            let text_input_area = text_input_ref.area();
            let has_focus = cx.has_key_focus(text_input_area);

            // ESC key is now handled in the main event handler using KeyUp event
            // This avoids conflicts with escaped() method being consumed by other components

            // Handle item selection from mention popup
            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
            }

            // Handle build items request
            if self.cmd_text_input.should_build_items(actions) {
                if has_focus {
                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                    self.update_user_list(cx, &search_text, scope);
                } else if self.cmd_text_input.view(ids!(popup)).visible() {
                    self.close_mention_popup(cx);
                }
            }

            // Process all actions
            for action in actions {
                // Handle TextInput changes
                if let Some(widget_action) = action.as_widget_action() {
                    if widget_action.widget_uid == text_input_uid {
                        if let TextInputAction::Changed(text) = widget_action.cast() {
                            if has_focus {
                                self.handle_text_change(cx, scope, text.to_owned());
                            }
                            continue; // Continue processing other actions
                        }
                    }
                }

                // Handle MentionableTextInputAction actions
                if let Some(action) = action.downcast_ref::<MentionableTextInputAction>() {
                    match action {
                        MentionableTextInputAction::PowerLevelsUpdated {
                            room_id,
                            can_notify_room,
                        } => {
                            if &scope_room_id != room_id {
                                continue;
                            }

                            if self.can_notify_room != *can_notify_room {
                                self.can_notify_room = *can_notify_room;
                                if self.is_searching() && has_focus {
                                    let search_text =
                                        self.cmd_text_input.search_text().to_lowercase();
                                    self.update_user_list(cx, &search_text, scope);
                                } else {
                                    self.cmd_text_input.redraw(cx);
                                }
                            }
                        }
                        MentionableTextInputAction::RoomMembersLoaded {
                            room_id,
                            sync_in_progress,
                            has_members,
                        } => {
                            if &scope_room_id != room_id {
                                continue;
                            }

                            log!(
                                "MentionableTextInput: RoomMembersLoaded room={} sync_in_progress={} has_members={} is_searching={}",
                                room_id,
                                sync_in_progress,
                                has_members,
                                self.is_searching()
                            );

                            // CRITICAL: Use locally stored previous state for change detection
                            // (not from props, which is already the new state in the same frame)
                            let previous_member_count = self.last_member_count;
                            let was_sync_pending = self.last_sync_pending;

                            // Current state: read fresh props to avoid stale snapshot from handle_event entry
                            let current_member_count = scope
                                .props
                                .get::<RoomScreenProps>()
                                .map(|p| p.room_members.as_ref().map(|m| m.len()).unwrap_or(0))
                                .unwrap_or(scope_member_count);
                            let current_sync_pending = *sync_in_progress;

                            // Detect actual changes
                            let member_count_changed = current_member_count != previous_member_count
                                && current_member_count > 0
                                && previous_member_count > 0;
                            let sync_just_completed = !current_sync_pending && was_sync_pending;

                            // Update local state for next comparison
                            self.last_member_count = current_member_count;
                            self.last_sync_pending = current_sync_pending;

                            if *has_members {
                                // CRITICAL FIX: Use saved state instead of reading from text input
                                // Reading from text input causes race condition (text may be empty when members arrive)
                                // Extract needed values first to avoid borrow checker issues
                                let action = match &self.search_state {
                                    MentionSearchState::WaitingForMembers {
                                        pending_search_text,
                                        ..
                                    } => Some((true, pending_search_text.clone())),
                                    MentionSearchState::Searching { search_text, .. } => {
                                        Some((false, search_text.clone()))
                                    }
                                    _ => None,
                                };

                                if let Some((is_waiting, search_text)) = action {
                                    let member_set_updated = member_count_changed
                                        && matches!(self.search_state, MentionSearchState::Searching { .. });

                                    if is_waiting {
                                        log!("  → Members loaded, resuming search with saved text='{}'", search_text);
                                        self.last_search_text = None;
                                        self.update_user_list(cx, &search_text, scope);
                                    } else {
                                        // Already in Searching state
                                        // Check if remote sync just completed or member set changed - need to re-search with full member list
                                        if member_set_updated {
                                            log!(
                                                "  → Member list changed ({} -> {}), restarting search with text='{}'",
                                                previous_member_count,
                                                current_member_count,
                                                search_text
                                            );
                                            self.last_search_text = None;
                                            self.update_user_list(cx, &search_text, scope);
                                        } else if sync_just_completed {
                                            log!("  → Remote sync completed while searching, re-searching with full member list, text='{}'", search_text);
                                            self.last_search_text = None;
                                            self.update_user_list(cx, &search_text, scope);
                                        } else {
                                            log!("  → Already searching, updating UI with search_text='{}'", search_text);
                                            self.update_ui_with_results(cx, scope, &search_text);
                                        }
                                    }
                                } else {
                                    // Not in WaitingForMembers or Searching state
                                    // Check if remote sync just completed - if so, refresh UI if there's an active mention trigger
                                    if sync_just_completed {
                                        log!("  → Remote sync just completed while not searching, checking for active mention trigger");
                                        // Check if there's currently an active mention trigger in the text
                                        let text = self.cmd_text_input.text_input_ref().text();
                                        let cursor_pos = self.cmd_text_input.text_input_ref()
                                            .borrow()
                                            .map_or(0, |p| p.cursor().index);

                                        if let Some(_trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
                                            // Extract search text and refresh UI
                                            let search_text = self.cmd_text_input.search_text().to_lowercase();
                                            log!("  → Found active mention trigger, refreshing with text='{}'", search_text);
                                            self.last_search_text = None;
                                            self.update_user_list(cx, &search_text, scope);
                                        } else {
                                            log!("  → No active mention trigger found, sync complete");
                                        }
                                    } else {
                                        log!("  → Not in searching state, ignoring member load");
                                    }
                                }
                            } else if self.is_searching() {
                                // Still no members returned yet; keep showing loading indicator.
                                self.cmd_text_input.clear_items();
                                self.show_loading_indicator(cx);
                                let popup = self.cmd_text_input.view(ids!(popup));
                                popup.set_visible(cx, true);
                                // Only restore focus if input currently has focus
                                let text_input_area = self.cmd_text_input.text_input_ref().area();
                                if cx.has_key_focus(text_input_area) {
                                    self.cmd_text_input.text_input_ref().set_key_focus(cx);
                                }
                            }
                        }
                    }
                }
            }

            // Close popup and clean up search state if focus is lost while searching
            // This prevents background search tasks from continuing when user is no longer interested
            if !has_focus && self.is_searching() {
                let popup = self.cmd_text_input.view(ids!(popup));
                popup.set_visible(cx, false);
                self.pending_popup_cleanup = true;
                // Guarantee cleanup executes even if search completes and stops requesting frames
                cx.new_next_frame();
            }
        }

        // Check if we were waiting for members and they're now available
        // When members arrive, always update regardless of focus state
        // update_user_list will handle popup visibility based on current focus
        if let MentionSearchState::WaitingForMembers {
            trigger_position: _,
            pending_search_text,
        } = &self.search_state
        {
            let room_props = scope
                .props
                .get::<RoomScreenProps>()
                .expect("RoomScreenProps should be available in scope");

            if let Some(room_members) = &room_props.room_members {
                if !room_members.is_empty() {
                    let search_text = pending_search_text.clone();
                    self.update_user_list(cx, &search_text, scope);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.cmd_text_input.draw_walk(cx, scope, walk)
    }
}

impl MentionableTextInput {
    /// Check if currently in any form of search mode
    fn is_searching(&self) -> bool {
        matches!(
            self.search_state,
            MentionSearchState::WaitingForMembers { .. } | MentionSearchState::Searching { .. }
        )
    }

    /// Generate the next unique identifier for a background search job.
    fn allocate_search_id(&mut self) -> u64 {
        if self.next_search_id == 0 {
            self.next_search_id = 1;
        }
        let id = self.next_search_id;
        self.next_search_id = self.next_search_id.wrapping_add(1);
        if self.next_search_id == 0 {
            self.next_search_id = 1;
        }
        id
    }

    /// Get the current trigger position if in search mode
    fn get_trigger_position(&self) -> Option<usize> {
        match &self.search_state {
            MentionSearchState::WaitingForMembers {
                trigger_position, ..
            }
            | MentionSearchState::Searching {
                trigger_position, ..
            } => Some(*trigger_position),
            _ => None,
        }
    }

    /// Check if search was just cancelled
    fn is_just_cancelled(&self) -> bool {
        matches!(self.search_state, MentionSearchState::JustCancelled)
    }

    /// Tries to add the `@room` mention item to the list of selectable popup mentions.
    ///
    /// Returns true if @room item was added to the list and will be displayed in the popup.
    fn try_add_room_mention_item(
        &mut self,
        cx: &mut Cx,
        search_text: &str,
        room_props: &RoomScreenProps,
        is_desktop: bool,
    ) -> bool {
        // Don't show @room option in direct messages
        if room_props.is_direct_room {
            return false;
        }
        if !self.can_notify_room || !("@room".contains(search_text) || search_text.is_empty()) {
            return false;
        }

        let Some(ptr) = self.room_mention_list_item else {
            return false;
        };
        let room_mention_item = WidgetRef::new_from_ptr(cx, Some(ptr));
        let mut room_avatar_shown = false;

        let avatar_ref = room_mention_item.avatar(ids!(user_info.room_avatar));

        // Get room avatar fallback text from room display name
        let room_name_first_char = room_props
            .room_display_name
            .as_ref()
            .and_then(|name| name.graphemes(true).next().map(|s| s.to_uppercase()))
            .filter(|s| s != "@" && s.chars().all(|c| c.is_alphabetic()))
            .unwrap_or_else(|| "R".to_string());

        if let Some(avatar_url) = &room_props.room_avatar_url {
            match get_or_fetch_avatar(cx, avatar_url.to_owned()) {
                AvatarCacheEntry::Loaded(avatar_data) => {
                    // Display room avatar
                    let result = avatar_ref.show_image(cx, None, |cx, img| {
                        utils::load_png_or_jpg(&img, cx, &avatar_data)
                    });
                    if result.is_ok() {
                        room_avatar_shown = true;
                    }
                }
                AvatarCacheEntry::Requested => {
                    avatar_ref.show_text(
                        cx,
                        Some(COLOR_UNKNOWN_ROOM_AVATAR),
                        None,
                        &room_name_first_char,
                    );
                    room_avatar_shown = true;
                }
                AvatarCacheEntry::Failed => {
                    // Failed to load room avatar - will use fallback text
                }
            }
        }

        // If unable to display room avatar, show first character of room name
        if !room_avatar_shown {
            avatar_ref.show_text(
                cx,
                Some(COLOR_UNKNOWN_ROOM_AVATAR),
                None,
                &room_name_first_char,
            );
        }

        // Apply layout and height styling based on device type
        let new_height = if is_desktop {
            DESKTOP_ITEM_HEIGHT
        } else {
            MOBILE_ITEM_HEIGHT
        };
        if is_desktop {
            room_mention_item.apply_over(
                cx,
                live! {
                    height: (new_height),
                    flow: Right,
                },
            );
        } else {
            room_mention_item.apply_over(
                cx,
                live! {
                    height: (new_height),
                    flow: Down,
                },
            );
        }

        self.cmd_text_input.add_item(room_mention_item);
        true
    }

    /// Add user mention items to the list from search results
    /// Returns the number of items added
    fn add_user_mention_items_from_results(
        &mut self,
        cx: &mut Cx,
        results: &[usize],
        user_items_limit: usize,
        is_desktop: bool,
        room_props: &RoomScreenProps,
    ) -> usize {
        let mut items_added = 0;

        // Get the actual members vec from room_props
        let Some(members) = &room_props.room_members else {
            return 0;
        };

        for (index, &member_idx) in results.iter().take(user_items_limit).enumerate() {
            // Get the actual member from the index
            let Some(member) = members.get(member_idx) else {
                continue;
            };

            // Get display name from member, with better fallback
            // Trim whitespace and filter out empty/whitespace-only names
            let display_name = member.display_name()
                .map(|name| name.trim())  // Remove leading/trailing whitespace
                .filter(|name| !name.is_empty())  // Filter out empty or whitespace-only names
                .unwrap_or_else(|| member.user_id().localpart())
                .to_owned();

            // Log warning for extreme cases where we still have no displayable text
            #[cfg(debug_assertions)]
            if display_name.is_empty() {
                log!(
                    "Warning: Member {} has no displayable name (empty display_name and localpart)",
                    member.user_id()
                );
            }

            let Some(user_list_item_ptr) = self.user_list_item else {
                // user_list_item_ptr is None
                continue;
            };
            let item = WidgetRef::new_from_ptr(cx, Some(user_list_item_ptr));

            item.label(ids!(user_info.username)).set_text(cx, &display_name);

            // Use the full user ID string
            let user_id_str = member.user_id().as_str();
            item.label(ids!(user_id)).set_text(cx, user_id_str);

            if is_desktop {
                item.apply_over(
                    cx,
                    live!(
                        flow: Right,
                        height: (DESKTOP_ITEM_HEIGHT),
                        align: {y: 0.5}
                    ),
                );
                item.view(ids!(user_info.filler)).set_visible(cx, true);
            } else {
                item.apply_over(
                    cx,
                    live!(
                        flow: Down,
                        height: (MOBILE_ITEM_HEIGHT),
                        spacing: (MOBILE_USERNAME_SPACING)
                    ),
                );
                item.view(ids!(user_info.filler)).set_visible(cx, false);
            }

            let avatar = item.avatar(ids!(user_info.avatar));
            if let Some(mxc_uri) = member.avatar_url() {
                match get_or_fetch_avatar(cx, mxc_uri.to_owned()) {
                    AvatarCacheEntry::Loaded(avatar_data) => {
                        let _ = avatar.show_image(cx, None, |cx, img| {
                            utils::load_png_or_jpg(&img, cx, &avatar_data)
                        });
                    }
                    AvatarCacheEntry::Requested | AvatarCacheEntry::Failed => {
                        avatar.show_text(cx, None, None, &display_name);
                    }
                }
            } else {
                avatar.show_text(cx, None, None, &display_name);
            }

            self.cmd_text_input.add_item(item.clone());
            items_added += 1;

            // Set keyboard focus to the first item
            if index == 0 {
                // If @room exists, it's index 0, otherwise first user is index 0
                self.cmd_text_input.set_keyboard_focus_index(0);
            }
        }

        items_added
    }

    /// Update popup visibility and layout based on current state
    fn update_popup_visibility(&mut self, cx: &mut Cx, scope: &mut Scope, has_items: bool) {
        let popup = self.cmd_text_input.view(ids!(popup));

        // Get current state from props
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope");
        let members_sync_pending = room_props.room_members_sync_pending;
        let members_available = room_props
            .room_members
            .as_ref()
            .is_some_and(|m| !m.is_empty());

        match &self.search_state {
            MentionSearchState::Idle | MentionSearchState::JustCancelled => {
                // Not in search mode, hide popup
                popup.apply_over(cx, live! { height: Fit });
                popup.set_visible(cx, false);
            }
            MentionSearchState::WaitingForMembers { .. } => {
                // Waiting for room members to be loaded
                self.show_loading_indicator(cx);
                popup.set_visible(cx, true);
                // Only restore focus if input currently has focus
                let text_input_area = self.cmd_text_input.text_input_ref().area();
                if cx.has_key_focus(text_input_area) {
                    self.cmd_text_input.text_input_ref().set_key_focus(cx);
                }
            }
            MentionSearchState::Searching {
                accumulated_results,
                ..
            } => {
                if has_items {
                    // We have search results to display
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                } else if accumulated_results.is_empty() {
                    if members_sync_pending || self.search_results_pending {
                        // Still fetching either member list or background search results.
                        self.show_loading_indicator(cx);
                    } else if members_available {
                        // Search completed with no results even though we have members.
                        self.show_no_matches_indicator(cx);
                    } else {
                        // No members available yet.
                        self.show_loading_indicator(cx);
                    }
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                } else {
                    // Has accumulated results but no items (should not happen)
                    popup.set_visible(cx, true);
                    // Only restore focus if input currently has focus
                    let text_input_area = self.cmd_text_input.text_input_ref().area();
                    if cx.has_key_focus(text_input_area) {
                        self.cmd_text_input.text_input_ref().set_key_focus(cx);
                    }
                }
            }
        }
    }

    /// Handles item selection from mention popup (either user or @room)
    fn on_user_selected(&mut self, cx: &mut Cx, _scope: &mut Scope, selected: WidgetRef) {
        // Note: We receive scope as parameter but don't use it in this method
        // This is good practice to maintain signature consistency with other methods
        // and allow for future scope-based enhancements

        let text_input_ref = self.cmd_text_input.text_input_ref();
        let current_text = text_input_ref.text();
        let head = text_input_ref.borrow().map_or(0, |p| p.cursor().index);

        if let Some(start_idx) = self.get_trigger_position() {
            let room_mention_label = selected.label(ids!(user_info.room_mention));
            let room_mention_text = room_mention_label.text();
            let room_user_id_text = selected.label(ids!(room_user_id)).text();

            let is_room_mention =
                { room_mention_text == "Notify the entire room" && room_user_id_text == "@room" };

            let mention_to_insert = if is_room_mention {
                // Always set to true, don't reset previously selected @room mentions
                self.possible_room_mention = true;
                "@room ".to_string()
            } else {
                // User selected a specific user
                let username = selected.label(ids!(user_info.username)).text();
                let user_id_str = selected.label(ids!(user_id)).text();
                let Ok(user_id): Result<OwnedUserId, _> = user_id_str.clone().try_into() else {
                    // Invalid user ID format - skip selection
                    return;
                };
                self.possible_mentions
                    .insert(user_id.clone(), username.clone());

                // Currently, we directly insert the markdown link for user mentions
                // instead of the user's display name, because we don't yet have a way
                // to track mentioned display names and replace them later.
                format!("[{username}]({}) ", user_id.matrix_to_uri(),)
            };

            // Use utility function to safely replace text
            let new_text = utils::safe_replace_by_byte_indices(
                &current_text,
                start_idx,
                head,
                &mention_to_insert,
            );

            self.cmd_text_input.set_text(cx, &new_text);
            // Calculate new cursor position
            let new_pos = start_idx + mention_to_insert.len();
            text_input_ref.set_cursor(
                cx,
                Cursor {
                    index: new_pos,
                    prefer_next_row: false,
                },
                false,
            );
        }

        self.cancel_active_search();
        self.search_state = MentionSearchState::JustCancelled;
        self.close_mention_popup(cx);
    }

    /// Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, scope: &mut Scope, text: String) {
        log!("handle_text_change: text='{}' search_state={:?}", text, self.search_state);

        // If search was just cancelled, clear the flag and don't re-trigger search
        if self.is_just_cancelled() {
            self.search_state = MentionSearchState::Idle;
            return;
        }

        // Check if text is empty or contains only whitespace
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            self.possible_mentions.clear();
            self.possible_room_mention = false;
            if self.is_searching() {
                self.close_mention_popup(cx);
            }
            return;
        }

        let cursor_pos = self
            .cmd_text_input
            .text_input_ref()
            .borrow()
            .map_or(0, |p| p.cursor().index);

        // Check if we're currently searching and the @ symbol was deleted
        if let Some(start_pos) = self.get_trigger_position() {
            // Check if the @ symbol at the start position still exists
            if start_pos >= text.len()
                || text.get(start_pos..start_pos + 1).is_some_and(|c| c != "@")
            {
                // The @ symbol was deleted, stop searching
                self.close_mention_popup(cx);
                return;
            }
        }

        // Look for trigger position for @ menu
        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            let search_text =
                utils::safe_substring_by_byte_indices(&text, trigger_pos + 1, cursor_pos);

            // Check if this is a continuation of existing search or a new one
            let is_new_search = self.get_trigger_position() != Some(trigger_pos);

            if is_new_search {
                // This is a new @ mention, reset everything
                self.last_search_text = None;
            } else {
                // User is editing existing mention, don't reset search state
                // This allows smooth deletion/modification of search text
                // But clear last_search_text if the new text is different to trigger search
                if self.last_search_text.as_ref() != Some(&search_text) {
                    self.last_search_text = None;
                }
            }

            // Ensure header view is visible to prevent header disappearing during consecutive @mentions
            let popup = self.cmd_text_input.view(ids!(popup));
            let header_view = self.cmd_text_input.view(ids!(popup.header_view));
            header_view.set_visible(cx, true);

            // Transition to appropriate state and update user list
            // update_user_list will handle state transition properly
            self.update_user_list(cx, &search_text, scope);

            popup.set_visible(cx, true);

            // Immediately check for results instead of waiting for next frame
            self.check_search_channel(cx, scope);

            // Redraw to ensure UI updates are visible
            cx.redraw_all();
        } else if self.is_searching() {
            self.close_mention_popup(cx);
        }
    }

    /// Check the search channel for new results
    /// Returns true if we should continue checking for more results
    fn check_search_channel(&mut self, cx: &mut Cx, scope: &mut Scope) -> bool {
        // Only check if we're in Searching state
        let mut is_complete = false;
        let mut search_text: Option<Arc<String>> = None;
        let mut any_results = false;
        let mut should_update_ui = false;
        let mut new_results = Vec::new();

        // Process all available results from the channel
        if let MentionSearchState::Searching {
            receiver,
            accumulated_results,
            search_id,
            ..
        } = &mut self.search_state
        {
            while let Ok(result) = receiver.try_recv() {
                if result.search_id != *search_id {
                    continue;
                }

                any_results = true;
                search_text = Some(result.search_text.clone());
                is_complete = result.is_complete;

                // Collect results
                if !result.results.is_empty() {
                    new_results.extend(result.results);
                    should_update_ui = true;
                }
            }

            if !new_results.is_empty() {
                accumulated_results.extend(new_results);
            }
        } else {
            return false;
        }

        // Update UI immediately if we got new results
        if should_update_ui {
            // Get accumulated results from state for UI update
            let results_for_ui = if let MentionSearchState::Searching {
                accumulated_results,
                ..
            } = &self.search_state
            {
                accumulated_results.clone()
            } else {
                Vec::new()
            };

            if !results_for_ui.is_empty() {
                // Results are already sorted in member_search.rs and indices are unique
                let query = search_text
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                self.update_ui_with_results(cx, scope, query);
            }
        }

        // Handle completion
        if is_complete {
            self.search_results_pending = false;
            // Search is complete - get results for final UI update
            let final_results = if let MentionSearchState::Searching {
                accumulated_results,
                ..
            } = &self.search_state
            {
                accumulated_results.clone()
            } else {
                Vec::new()
            };

            if final_results.is_empty() {
                // No user results, but still update UI (may show @room)
                let query = search_text
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                self.update_ui_with_results(cx, scope, query);
            }

            // Don't change state here - let update_ui_with_results handle it
        } else if !any_results {
            // No results received yet - check if channel is still open
            let disconnected =
                if let MentionSearchState::Searching { receiver, .. } = &self.search_state {
                    matches!(
                        receiver.try_recv(),
                        Err(std::sync::mpsc::TryRecvError::Disconnected)
                    )
                } else {
                    false
                };

            if disconnected {
                // Channel was closed - search completed or failed
                self.search_results_pending = false;
                self.handle_search_channel_closed(cx, scope);
            }
        }

        // Return whether we should continue checking for results
        !is_complete && matches!(self.search_state, MentionSearchState::Searching { .. })
    }

    /// Common UI update logic for both streaming and non-streaming results
    fn update_ui_with_results(&mut self, cx: &mut Cx, scope: &mut Scope, search_text: &str) {
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        // If we're in Searching state, we have local data - always show results
        // Don't wait for remote sync to complete
        // Remote sync will trigger update when it completes (if data changed)
        self.cmd_text_input.clear_items();
        self.loading_indicator_ref = None;

        let is_desktop = cx.display_context.is_desktop();
        let max_visible_items: usize = if is_desktop {
            DESKTOP_MAX_VISIBLE_ITEMS
        } else {
            MOBILE_MAX_VISIBLE_ITEMS
        };
        let mut items_added = 0;

        // 4. Try to add @room mention item
        let has_room_item = self.try_add_room_mention_item(cx, search_text, room_props, is_desktop);
        if has_room_item {
            items_added += 1;
        }

        // Get accumulated results from current state
        let results_to_display = if let MentionSearchState::Searching {
            accumulated_results,
            ..
        } = &self.search_state
        {
            accumulated_results.clone()
        } else {
            Vec::new()
        };

        // Add user mention items using the results
        if !results_to_display.is_empty() {
            let user_items_limit = max_visible_items.saturating_sub(has_room_item as usize);
            let user_items_added = self.add_user_mention_items_from_results(
                cx,
                &results_to_display,
                user_items_limit,
                is_desktop,
                room_props,
            );
            items_added += user_items_added;
        }

        // If remote sync is still in progress, add loading indicator after results
        // This gives visual feedback that more members may be loading
        // IMPORTANT: Don't call show_loading_indicator here as it calls clear_items()
        // which would remove the user list we just added
        if room_props.room_members_sync_pending {
            log!("Remote sync still pending, adding loading indicator after partial results");

            // Add loading indicator widget without clearing existing items
            if let Some(ptr) = self.loading_indicator {
                let loading_item = WidgetRef::new_from_ptr(cx, Some(ptr));
                self.cmd_text_input.add_item(loading_item.clone());
                self.loading_indicator_ref = Some(loading_item.clone());

                // Start the loading animation
                loading_item
                    .bouncing_dots(ids!(loading_animation))
                    .start_animation(cx);
                cx.new_next_frame();

                items_added += 1;
            }
        }

        // Update popup visibility based on whether we have items
        self.update_popup_visibility(cx, scope, items_added > 0);

        // Force immediate redraw to ensure UI updates are visible
        cx.redraw_all();
    }

    /// Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // Get room_props to read real-time member state from props (single source of truth)
        let room_props = scope
            .props
            .get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        let has_members_in_props = room_props.room_members
            .as_ref()
            .is_some_and(|m| !m.is_empty());

        log!("update_user_list: search_text='{}' has_members_in_props={} search_state={:?}",
             search_text, has_members_in_props, self.search_state);

        // Get trigger position from current state (if in searching mode)
        let trigger_pos = match &self.search_state {
            MentionSearchState::WaitingForMembers {
                trigger_position, ..
            }
            | MentionSearchState::Searching {
                trigger_position, ..
            } => *trigger_position,
            _ => {
                // Not in searching mode, need to determine trigger position
                if let Some(pos) = self.find_mention_trigger_position(
                    &self.cmd_text_input.text_input_ref().text(),
                    self.cmd_text_input
                        .text_input_ref()
                        .borrow()
                        .map_or(0, |p| p.cursor().index),
                ) {
                    pos
                } else {
                    return;
                }
            }
        };

        // Skip if search text hasn't changed AND we're already in Searching state
        // Don't skip if we're in WaitingForMembers - need to transition to Searching
        if self.last_search_text.as_deref() == Some(search_text) {
            if matches!(self.search_state, MentionSearchState::Searching { .. }) {
                return; // Already searching with same text, skip
            }
            // In WaitingForMembers with same text -> need to start search now that members arrived
        }

        self.last_search_text = Some(search_text.to_string());

        let is_desktop = cx.display_context.is_desktop();
        let max_visible_items = if is_desktop {
            DESKTOP_MAX_VISIBLE_ITEMS
        } else {
            MOBILE_MAX_VISIBLE_ITEMS
        };

        let cached_members = match &room_props.room_members {
            Some(members) if !members.is_empty() => {
                // Members available, continue to search
                members.clone()
            }
            _ => {
                let already_waiting = matches!(
                    self.search_state,
                    MentionSearchState::WaitingForMembers { .. }
                );

                self.cancel_active_search();

                if !already_waiting {
                    submit_async_request(MatrixRequest::GetRoomMembers {
                        room_id: room_props.room_id.clone(),
                        memberships: RoomMemberships::JOIN,
                        local_only: true,
                    });
                }

                self.search_state = MentionSearchState::WaitingForMembers {
                    trigger_position: trigger_pos,
                    pending_search_text: search_text.to_string(),
                };

                // Clear old items before showing loading indicator
                self.cmd_text_input.clear_items();
                self.show_loading_indicator(cx);
                // Request next frame to check when members are loaded
                cx.new_next_frame();
                return; // Don't submit search request yet
            }
        };

        // We have cached members, ensure popup is visible
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));
        header_view.set_visible(cx, true);
        popup.set_visible(cx, true);
        // Only restore focus if input currently has focus
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }

        // Create a new channel for this search
        let (sender, receiver) = std::sync::mpsc::channel();

        // Prepare background search job parameters
        let search_text_clone = search_text.to_string();
        let max_results = max_visible_items * SEARCH_BUFFER_MULTIPLIER;
        let search_id = self.allocate_search_id();

        // Transition to Searching state with new receiver
        self.cancel_active_search();
        let cancel_token = Arc::new(AtomicBool::new(false));
        self.search_state = MentionSearchState::Searching {
            trigger_position: trigger_pos,
            search_text: search_text.to_string(),
            receiver,
            accumulated_results: Vec::new(),
            search_id,
            cancel_token: cancel_token.clone(),
        };
        self.search_results_pending = true;

        let precomputed_sort = room_props.room_members_sort.clone();
        let cancel_token_for_job = cancel_token.clone();
        cpu_worker::spawn_cpu_job(cx, CpuJob::SearchRoomMembers(SearchRoomMembersJob {
            members: cached_members,
            search_text: search_text_clone,
            max_results,
            sender,
            search_id,
            precomputed_sort,
            cancel_token: Some(cancel_token_for_job),
        }));

        // Request next frame to check the channel
        cx.new_next_frame();

        // Try to check immediately for faster response
        self.check_search_channel(cx, scope);
    }

    /// Detects valid mention trigger positions in text
    fn find_mention_trigger_position(&mut self, text: &str, cursor_pos: usize) -> Option<usize> {
        if cursor_pos == 0 {
            return None;
        }

        // Check cache and rebuild if text changed (performance optimization)
        let (text_graphemes_owned, byte_positions) = if let Some((cached_text, cached_graphemes, cached_positions)) = &self.cached_text_analysis {
            if cached_text == text {
                // Cache hit - use cached data
                (cached_graphemes.clone(), cached_positions.clone())
            } else {
                // Cache miss - rebuild and update cache
                let graphemes_owned: Vec<String> = text.graphemes(true).map(|s| s.to_string()).collect();
                let positions = utils::build_grapheme_byte_positions(text);
                self.cached_text_analysis = Some((text.to_string(), graphemes_owned.clone(), positions.clone()));
                (graphemes_owned, positions)
            }
        } else {
            // No cache - build and cache
            let graphemes_owned: Vec<String> = text.graphemes(true).map(|s| s.to_string()).collect();
            let positions = utils::build_grapheme_byte_positions(text);
            self.cached_text_analysis = Some((text.to_string(), graphemes_owned.clone(), positions.clone()));
            (graphemes_owned, positions)
        };

        // Convert owned strings to slices for processing
        let text_graphemes: Vec<&str> = text_graphemes_owned.iter().map(|s| s.as_str()).collect();

        // Use utility function to convert byte position to grapheme index
        let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(text, cursor_pos);

        // Simple logic: trigger when cursor is immediately after @ symbol
        // Only trigger if @ is preceded by whitespace or beginning of text
        if cursor_grapheme_idx > 0 && text_graphemes.get(cursor_grapheme_idx - 1) == Some(&"@") {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1
                || (cursor_grapheme_idx > 1
                    && text_graphemes
                        .get(cursor_grapheme_idx - 2)
                        .is_some_and(|g| g.trim().is_empty()));
            if is_preceded_by_whitespace_or_start {
                if let Some(&byte_pos) = byte_positions.get(cursor_grapheme_idx - 1) {
                    return Some(byte_pos);
                }
            }
        }

        // Find the last @ symbol before the cursor for search continuation
        // Only continue if we're already in search mode
        if self.is_searching() {
            let last_at_pos = text_graphemes.get(..cursor_grapheme_idx).and_then(|slice| {
                slice
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| **g == "@")
                    .map(|(i, _)| i)
                    .next_back()
            });

            if let Some(at_idx) = last_at_pos {
                // Get the byte position of this @ symbol
                let &at_byte_pos = byte_positions.get(at_idx)?;

                // Extract the text after the @ symbol up to the cursor position
                let mention_text = text_graphemes
                    .get(at_idx + 1..cursor_grapheme_idx)
                    .unwrap_or(&[]);

                // Only trigger if this looks like an ongoing mention (contains only alphanumeric and basic chars)
                if self.is_valid_mention_text(mention_text) {
                    return Some(at_byte_pos);
                }
            }
        }

        None
    }

    /// Simple validation for mention text
    fn is_valid_mention_text(&self, graphemes: &[&str]) -> bool {
        // Allow empty text (for @)
        if graphemes.is_empty() {
            return true;
        }

        // Check if it contains newline characters
        !graphemes.iter().any(|g| g.contains('\n'))
    }

    /// Shows the loading indicator when waiting for initial members to be loaded
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        // Check if we already have a loading indicator displayed
        // Avoid recreating it on every call, which would prevent animation from playing
        if let Some(ref existing_indicator) = self.loading_indicator_ref {
            // Already showing, just ensure animation is running
            existing_indicator
                .bouncing_dots(ids!(loading_animation))
                .start_animation(cx);
            cx.new_next_frame();
            return;
        }

        // Clear old items before creating new loading indicator
        self.cmd_text_input.clear_items();

        // Create fresh loading indicator widget
        let Some(ptr) = self.loading_indicator else {
            return;
        };
        let loading_item = WidgetRef::new_from_ptr(cx, Some(ptr));

        // Start the loading animation
        loading_item.bouncing_dots(ids!(loading_animation)).start_animation(cx);

        // Now that the widget is in the UI tree, start the loading animation
        loading_item
            .bouncing_dots(ids!(loading_animation))
            .start_animation(cx);
        cx.new_next_frame();

        // Setup popup dimensions for loading state
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Don't manually set popup height for loading - let it auto-size based on content
        // This avoids conflicts with list = { height: Fill }
        popup.set_visible(cx, true);

        // Maintain text input focus only if it currently has focus
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if self.is_searching() && cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    /// Shows the no matches indicator when no users match the search
    fn show_no_matches_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create no matches indicator widget
        let Some(ptr) = self.no_matches_indicator else {
            return;
        };
        let no_matches_item = WidgetRef::new_from_ptr(cx, Some(ptr));

        // Add the no matches indicator to the popup
        self.cmd_text_input.add_item(no_matches_item);
        self.loading_indicator_ref = None;

        // Setup popup dimensions for no matches state
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Let popup auto-size based on content
        popup.apply_over(cx, live! { height: Fit });

        // Maintain text input focus so user can continue typing, but only if currently focused
        let text_input_area = self.cmd_text_input.text_input_ref().area();
        if self.is_searching() && cx.has_key_focus(text_input_area) {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    /// Check if mention search is currently active
    pub fn is_mention_searching(&self) -> bool {
        self.is_searching()
    }

    /// Check if ESC was handled by mention popup
    pub fn handled_escape(&self) -> bool {
        self.is_just_cancelled()
    }

    /// Handle search channel closed event
    fn handle_search_channel_closed(&mut self, cx: &mut Cx, scope: &mut Scope) {
        // Get accumulated results before changing state
        let has_results = if let MentionSearchState::Searching {
            accumulated_results,
            ..
        } = &self.search_state
        {
            !accumulated_results.is_empty()
        } else {
            false
        };

        // If no results were shown, show empty state
        if !has_results {
            self.update_ui_with_results(cx, scope, "");
        }

        // Keep searching state but mark search as complete
        // The state will be reset when user types or closes popup
    }

    fn cancel_active_search(&mut self) {
        if let MentionSearchState::Searching { cancel_token, .. } = &self.search_state {
            cancel_token.store(true, Ordering::Relaxed);
        }
        self.search_results_pending = false;
    }

    /// Reset all search-related state
    fn reset_search_state(&mut self) {
        self.cancel_active_search();

        // Reset to idle state
        self.search_state = MentionSearchState::Idle;

        // Reset last search text to allow new searches
        self.last_search_text = None;
        self.search_results_pending = false;
        self.loading_indicator_ref = None;

        // Reset change detection state
        self.last_member_count = 0;
        self.last_sync_pending = false;
        self.pending_popup_cleanup = false;

        // Clear list items
        self.cmd_text_input.clear_items();
    }

    /// Cleanup helper for closing mention popup
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        // Reset all search-related state
        self.reset_search_state();

        // Get popup and header view references
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));

        // Force hide header view - necessary when handling deletion operations
        // When backspace-deleting mentions, we want to completely hide the header
        header_view.set_visible(cx, false);

        // Hide the entire popup
        popup.set_visible(cx, false);

        // Reset popup height
        popup.apply_over(cx, live! { height: Fit });

        // Ensure header view is reset to visible next time it's triggered
        // This will happen before update_user_list is called in handle_text_change

        self.cmd_text_input.request_text_input_focus();
        self.cmd_text_input.redraw(cx);
    }

    /// Returns the current text content
    pub fn text(&self) -> String {
        self.cmd_text_input.text_input_ref().text()
    }

    /// Sets the text content
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.cmd_text_input.text_input_ref().set_text(cx, text);
        self.cmd_text_input.redraw(cx);
    }

    /// Sets whether the current user can notify the entire room (@room mention)
    pub fn set_can_notify_room(&mut self, can_notify: bool) {
        self.can_notify_room = can_notify;
    }

    /// Gets whether the current user can notify the entire room (@room mention)
    pub fn can_notify_room(&self) -> bool {
        self.can_notify_room
    }
}

impl MentionableTextInputRef {
    pub fn text(&self) -> String {
        self.borrow().map_or_else(String::new, |inner| inner.text())
    }

    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map(|inner| inner.cmd_text_input.text_input_ref())
            .unwrap_or_default()
    }

    /// Check if mention search is currently active
    pub fn is_mention_searching(&self) -> bool {
        self.borrow()
            .is_some_and(|inner| inner.is_mention_searching())
    }

    /// Check if ESC was handled by mention popup
    pub fn handled_escape(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.handled_escape())
    }

    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    /// Sets whether the current user can notify the entire room (@room mention)
    pub fn set_can_notify_room(&self, can_notify: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_can_notify_room(can_notify);
        }
    }

    /// Gets whether the current user can notify the entire room (@room mention)
    pub fn can_notify_room(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.can_notify_room())
    }

    /// Returns the mentions actually present in the given html message content.
    fn get_real_mentions_in_html_text(&self, html: &str) -> Mentions {
        let mut mentions = Mentions::new();

        let Some(inner) = self.borrow() else {
            return mentions;
        };

        let mut user_ids = BTreeSet::new();

        for (user_id, username) in &inner.possible_mentions {
            if html.contains(&format!(
                "<a href=\"{}\">{}</a>",
                user_id.matrix_to_uri(),
                username,
            )) {
                user_ids.insert(user_id.clone());
            }
        }

        mentions.user_ids = user_ids;
        // Check for @room mention in HTML content
        mentions.room = inner.possible_room_mention && html.contains("@room");
        mentions
    }

    /// Returns the mentions actually present in the given markdown message content.
    fn get_real_mentions_in_markdown_text(&self, markdown: &str) -> Mentions {
        let mut mentions = Mentions::new();

        let Some(inner) = self.borrow() else {
            return mentions;
        };

        let mut user_ids = BTreeSet::new();
        for (user_id, username) in &inner.possible_mentions {
            // Check both username format and user_id format for flexibility
            let username_pattern = format!("[{}]({})", username, user_id.matrix_to_uri());
            let userid_pattern = format!("[{}]({})", user_id, user_id.matrix_to_uri());

            if markdown.contains(&username_pattern) || markdown.contains(&userid_pattern) {
                user_ids.insert(user_id.clone());
            }
        }

        mentions.user_ids = user_ids;
        // Check for @room mention in markdown content
        mentions.room = inner.possible_room_mention && markdown.contains("@room");
        mentions
    }

    /// Processes entered text and creates a message with mentions based on detected message type.
    /// This method handles /html, /plain prefixes and defaults to markdown.
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        if let Some(html_text) = entered_text.strip_prefix("/html") {
            let message = RoomMessageEventContent::text_html(html_text, html_text);
            message.add_mentions(self.get_real_mentions_in_html_text(html_text))
        } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
            // Plain text messages don't support mentions
            RoomMessageEventContent::text_plain(plain_text)
        } else {
            let message = RoomMessageEventContent::text_markdown(entered_text);
            message.add_mentions(self.get_real_mentions_in_markdown_text(entered_text))
        }
    }
}
