//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::typing_animation::TypingAnimationWidgetRefExt;
use crate::shared::styles::COLOR_UNKNOWN_ROOM_AVATAR;
use crate::utils;

use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::ruma::{events::room::message::RoomMessageEventContent, events::Mentions, OwnedRoomId, OwnedUserId};
use matrix_sdk::room::RoomMember;
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;
use crate::home::room_screen::RoomScreenProps;
use crate::sliding_sync::get_client;



live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;
    use crate::shared::typing_animation::TypingAnimation;

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
        spacing: 8.0

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
        flow: Right
        spacing: 8.0
        align: {y: 0.5}

        // Replace Icon with an Avatar to display room avatar
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
            text: "@room"
        }
    }

    // Template for loading indicator when members are being fetched
    LoadingIndicator = <View> {
        width: Fill,
        height: 56,
        margin: {left: 4, right: 4}
        padding: {left: 16, right: 16, top: 16, bottom: 16},
        flow: Right,
        spacing: 8.0,
        align: {x: 0., y: 0.5}
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

        loading_animation = <TypingAnimation> {
            width: 60,
            height: 24,
            draw_bg: {
                color: (COLOR_ROBRIX_PURPLE),
                dot_radius: 2.0,
            }
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
                height: Fill
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
    }
}

// /// A special string used to denote the start of a mention within
// /// the actual text being edited.
// /// This is used to help easily locate and distinguish actual mentions
// /// from normal `@` characters.
// const MENTION_START_STRING: &str = "\u{8288}@\u{8288}";


#[derive(Clone, DefaultNone, Debug)]
pub enum MentionableTextInputAction {
    /// Notifies the MentionableTextInput about updated power levels for the room.
    PowerLevelsUpdated(OwnedRoomId, bool),
    /// Notifies that room members have been loaded/updated
    RoomMembersLoaded(OwnedRoomId),
    None,
}

/// Widget that extends CommandTextInput with @mention capabilities
#[derive(Live, LiveHook, Widget)]
pub struct MentionableTextInput {
    /// Base command text input
    #[deref] cmd_text_input: CommandTextInput,
    /// Template for user list items
    #[live] user_list_item: Option<LivePtr>,
    /// Template for the @room mention list item
    #[live] room_mention_list_item: Option<LivePtr>,
    /// Template for loading indicator
    #[live] loading_indicator: Option<LivePtr>,
    /// Position where the @ mention starts
    #[rust] current_mention_start_index: Option<usize>,
    /// The set of users that were mentioned (at one point) in this text input.
    /// Due to characters being deleted/removed, this list is a *superset*
    /// of possible users who may have been mentioned.
    /// All of these mentions may not exist in the final text input content;
    /// this is just a list of users to search the final sent message for
    /// when adding in new mentions.
    #[rust] possible_mentions: BTreeMap<OwnedUserId, String>,
    /// Indicates if the `@room` option was explicitly selected.
    #[rust] possible_room_mention: bool,
    /// Indicates if currently in mention search mode
    #[rust] is_searching: bool,
    /// Whether the current user can notify everyone in the room (@room mention)
    #[rust] can_notify_room: bool,
    /// Whether the room members are currently being loaded
    #[rust] members_loading: bool,
}


impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.cmd_text_input.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {

            let text_input_ref = self.cmd_text_input.text_input_ref(); // Get reference to the inner TextInput
            let text_input_uid = text_input_ref.widget_uid();
            // --- Begin modification ---
            // Get the Area of the inner TextInput
            let text_input_area = text_input_ref.area();
            // Check if this Area has keyboard focus using cx
            let has_focus = cx.has_key_focus(text_input_area);
            // --- End modification ---


            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                // return;
            }

            if self.cmd_text_input.should_build_items(actions) {
                // Only build the list when this instance's TextInput has focus
                if has_focus {
                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                    // update_user_list already includes a check for Scope room_id
                    self.update_user_list(cx, &search_text, scope);
                } else {
                    // If no focus but received a build request (possibly from a previous state), ensure popup is closed
                    if self.cmd_text_input.view(id!(popup)).visible() {
                        self.close_mention_popup(cx);
                    }
                }
            }

            if let Some(action) =
                actions.find_widget_action(self.cmd_text_input.text_input_ref().widget_uid())
            {
                if let TextInputAction::Changed(text) = action.cast() {
                    // First check if there are any mention markers
                    // If no "[" or "](", the user may have deleted all user mentions
                    if !text.contains('[') || !text.contains("](") {
                        // Clear all possible user mentions
                        self.possible_mentions.clear();

                        if !text.contains("@room") {
                            self.possible_room_mention = false;
                        }
                    }

                    self.handle_text_change(cx, scope, text);
                }
            }

            for action in actions {
                // Check if it's a TextInputAction
                if let Some(widget_action) = action.as_widget_action() {
                    // Ensure the Action comes from our own TextInput
                    if widget_action.widget_uid == text_input_uid {
                        // Removed special backspace key detection, instead detect deletion in text change

                        if let TextInputAction::Changed(text) = widget_action.cast() {
                            // Only process text changes when this instance's TextInput has focus
                            if has_focus {
                                // handle_text_change internally calls update_user_list,
                                // update_user_list has internal Scope room_id check
                                self.handle_text_change(cx, scope, text.to_owned());
                            }
                            // Found the corresponding Change Action, can break out of inner loop
                            break;
                        }
                    }
                }

                // Check for MentionableTextInputAction actions
                if let Some(action_ref) = action.downcast_ref::<MentionableTextInputAction>() {
                    match action_ref {
                        MentionableTextInputAction::PowerLevelsUpdated(room_id, can_notify_room) => {
                            // Best practice: Always check Scope first to get current context
                            // Scope represents the current widget context as passed down from parents
                            let scope_room_id = &scope.props.get::<RoomScreenProps>()
                                .expect("RoomScreenProps should be available in scope for MentionableTextInput")
                                .room_id;

                            // If Scope room_id doesn't match action's room_id, this action may be for another room
                            // This is important to avoid applying actions to the wrong room context
                            if scope_room_id != room_id {
                                continue; // Skip this action
                            }

                            // Only update and possibly redraw when can_notify_room state actually changes
                            if self.can_notify_room != *can_notify_room {
                                self.can_notify_room = *can_notify_room;
                                // If currently searching, may need to immediately update list to show/hide @room
                                if self.is_searching && has_focus { // Only update list when has focus
                                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                                    // Pass scope to update_user_list to ensure consistent context
                                    self.update_user_list(cx, &search_text, scope);
                                } else {
                                    self.redraw(cx);
                                }
                            }
                        },
                        MentionableTextInputAction::RoomMembersLoaded(room_id) => {
                            // Get current room context from scope
                            let scope_room_id = &scope.props.get::<RoomScreenProps>()
                                .expect("RoomScreenProps should be available in scope for MentionableTextInput")
                                .room_id;

                            // Only process if this action is for the current room
                            if scope_room_id == room_id {

                                // If we were showing loading, hide it and refresh the list
                                if self.members_loading {
                                    self.members_loading = false;

                                    // If currently searching, refresh the user list
                                    if self.is_searching && has_focus {
                                        let search_text = self.cmd_text_input.search_text().to_lowercase();
                                        self.update_user_list(cx, &search_text, scope);
                                    }
                                }
                            }
                        },
                        _ => {},
                    }
                }
            }

            if !has_focus && self.cmd_text_input.view(id!(popup)).visible() {
                    self.close_mention_popup(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.cmd_text_input.draw_walk(cx, scope, walk)
    }
}


impl MentionableTextInput {

    // Handles item selection from mention popup (either user or @room)
    fn on_user_selected(&mut self, cx: &mut Cx, _scope: &mut Scope, selected: WidgetRef) {
        // Note: We receive scope as parameter but don't use it in this method
        // This is good practice to maintain signature consistency with other methods
        // and allow for future scope-based enhancements

        let text_input_ref = self.cmd_text_input.text_input_ref();
        let current_text = text_input_ref.text();
        let head = text_input_ref.borrow().map_or(0, |p| p.cursor().index);

        if let Some(start_idx) = self.current_mention_start_index {
            let room_mention_label = selected.label(id!(room_mention));
            let room_mention_text = room_mention_label.text();
            let user_id_text = selected.label(id!(user_id)).text();

            let is_room_mention = { room_mention_text == "@room" && user_id_text.is_empty() };

            let mention_to_insert = if is_room_mention {
                // Always set to true, don't reset previously selected @room mentions
                self.possible_room_mention = true;
                "@room ".to_string()
            } else {
                // User selected a specific user
                let username = selected.label(id!(user_info.username)).text();
                let user_id_str = selected.label(id!(user_id)).text();
                let Ok(user_id): Result<OwnedUserId, _> = user_id_str.clone().try_into() else {
                    log!("Failed to parse user_id: {}", user_id_str);
                    return;
                };
                self.possible_mentions.insert(user_id.clone(), username.clone());

                // Currently, we directly insert the markdown link for user mentions
                // instead of the user's display name, because we don't yet have a way
                // to track mentioned display names and replace them later.
                format!(
                    "[{username}]({}) ",
                    user_id.matrix_to_uri(),
                )
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
            text_input_ref.set_cursor(cx, Cursor { index: new_pos, prefer_next_row: false }, false);
        }

        self.is_searching = false;
        self.current_mention_start_index = None;
        self.close_mention_popup(cx);
    }

    // Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, scope: &mut Scope, text: String) {
        // Check if text is empty or contains only whitespace
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            self.possible_mentions.clear();
            self.possible_room_mention = false;
            if self.is_searching {
                self.close_mention_popup(cx);
            }
            return;
        }

        let cursor_pos = self.cmd_text_input.text_input_ref().borrow().map_or(0, |p| p.cursor().index);

        // Continue with the remaining parts of text change handling
        // Look for trigger position for @ menu
        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            // Ensure @ is preceded by whitespace or is at text start, so consecutive @mentions work properly
            let is_valid_mention = if trigger_pos > 0 {
                let pre_char = &text[trigger_pos-1..trigger_pos];
                // Valid @ symbol: at text start or preceded by space
                pre_char == " " || trigger_pos == 0
            } else {
                true
            };

            if !is_valid_mention {
                if self.is_searching {
                    self.close_mention_popup(cx);
                }
                return;
            }

            self.current_mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            let search_text = utils::safe_substring_by_byte_indices(
                &text,
                trigger_pos + 1,
                cursor_pos
            ).to_lowercase();

            // Ensure header view is visible to prevent header disappearing during consecutive @mentions
            let popup = self.cmd_text_input.view(id!(popup));
            let header_view = self.cmd_text_input.view(id!(popup.header_view));
            header_view.set_visible(cx, true);

            self.update_user_list(cx, &search_text, scope);
            popup.set_visible(cx, true);
        } else if self.is_searching {
            self.close_mention_popup(cx);
        }
    }

    // Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // 1. Get Props from Scope
        let room_props = scope.props.get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

        // Use room_id from scope - it's always current and correct
        let room_id = &room_props.room_id;

        // Always use room_members provided in current scope
        // These member lists should come from TimelineUiState.room_members_map and are already the correct list for current room
        let Some(room_members) = &room_props.room_members else {
            self.members_loading = true;
            self.show_loading_indicator(cx);
            return;
        };

        // 4. Check if members are loaded or still loading
        let members_are_empty = room_members.is_empty();

        if members_are_empty && !self.members_loading {
            // Members list is empty and we're not already showing loading - start loading state
            self.members_loading = true;
            self.show_loading_indicator(cx);
            return;
        } else if !members_are_empty && self.members_loading {
            // Members have been loaded, stop loading state
            self.members_loading = false;
        } else if members_are_empty && self.members_loading {
            // Still loading and members are empty - keep showing loading indicator
            return;
        }

        // Clear old list items, prepare to populate new list
        self.cmd_text_input.clear_items();

        if self.is_searching {
            let is_desktop = cx.display_context.is_desktop();

            if self.can_notify_room && ("@room".contains(search_text) || search_text.is_empty()) {
                let room_mention_item = match self.room_mention_list_item {
                    Some(ptr) => WidgetRef::new_from_ptr(cx, Some(ptr)),
                    None => {
                        return;
                    }
                };
                let mut room_avatar_shown = false;

                // Set up room avatar
                let avatar_ref = room_mention_item.avatar(id!(room_avatar));

                // Get room avatar from current room Props
                if let Some(client) = get_client() {
                    if let Some(room) = client.get_room(room_id) {
                        if let Some(avatar_url) = room.avatar_url() {

                            match get_or_fetch_avatar(cx, avatar_url.to_owned()) {
                                AvatarCacheEntry::Loaded(avatar_data) => {
                                    // Display room avatar
                                    let result = avatar_ref.show_image(cx, None, |cx, img| {
                                        utils::load_png_or_jpg(&img, cx, &avatar_data)
                                    });
                                    if result.is_ok() {
                                        room_avatar_shown = true;
                                    } else {
                                        log!("Failed to show @room avatar with room avatar image");
                                    }
                                },
                                AvatarCacheEntry::Requested => {
                                    avatar_ref.show_text(cx, Some(COLOR_UNKNOWN_ROOM_AVATAR), None, "R");
                                    room_avatar_shown = true;
                                },
                                AvatarCacheEntry::Failed => {
                                    log!("Failed to load room avatar for @room");
                                }
                            }
                        } else {
                            log!("Room has no avatar URL for @room");
                        }
                    } else {
                        log!("Could not find room for @room avatar with room_id: {}", room_id);
                    }
                } else {
                    log!("Could not get client for @room avatar");
                }

                // If unable to display room avatar, show letter R with red background
                if !room_avatar_shown {
                    avatar_ref.show_text(cx, Some(COLOR_UNKNOWN_ROOM_AVATAR), None, "R");
                }


                self.cmd_text_input.add_item(room_mention_item);
            }

            // Improved search: match both display names and Matrix IDs, then sort by priority
            const MAX_MATCHED_MEMBERS: usize = MAX_VISIBLE_ITEMS * 2;  // Buffer for better UX

            // Collect all matching members with their priority scores
            let mut prioritized_members = Vec::new();

            for member in room_members.iter() {
                if prioritized_members.len() >= MAX_MATCHED_MEMBERS {
                    break;
                }

                // Check if this member matches the search text (including Matrix ID)
                if self.user_matches_search(member, search_text) {
                    let display_name = member
                        .display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| member.user_id().to_string());

                    let priority = self.get_match_priority(member, search_text);
                    prioritized_members.push((priority, display_name, member));
                }
            }

            // Sort by priority (lower number = higher priority)
            prioritized_members.sort_by_key(|(priority, _, _)| *priority);

            // Convert to the format expected by the rest of the code
            let matched_members: Vec<(String, &RoomMember)> = prioritized_members
                .into_iter()
                .map(|(_, display_name, member)| (display_name, member))
                .collect();

            let member_count = matched_members.len();

            // Performance issue: When a room has more than 2,000 users, rendering the mention popup causes the app to lag and show a loading spinner.
            //
            // Performance Bottlenecks:
            // 1. Unrestricted iteration over all 2,000+ members
            // 2. Widget instances are created for all matching members, even those not visible
            // 3. No virtualization or limiting mechanism

            // Solution:
            // 1. Early Termination: Limit the number of matching members to MAX_VISIBLE_ITEMS * 2 (30 items)
            // 2. Smart Search: Two-stage search—first match by prefix, then by substring—to provide a better user experience
            // 3. Virtualization: Only create Widget instances for actually visible items (‎⁠take(MAX_VISIBLE_ITEMS)⁠)

            // Performance Improvements:
            // 1. Reduces processing from over 2,000 items to a maximum of 30 items
            // 2. Reduces Widget creation from over 2,000 to at most 15 visible Widgets
            // 3. Significantly decreases string operations and Widget creation overhead
            const MAX_VISIBLE_ITEMS: usize = 15;
            let popup = self.cmd_text_input.view(id!(popup));

            // Adjust height calculation to include the potential @room item
            let total_items_in_list = member_count + if "@room".contains(search_text) { 1 } else { 0 };

            if total_items_in_list == 0 {
                // If there are no matching items, just hide the entire popup and clear search state
                popup.apply_over(cx, live! { height: Fit });
                self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
                // Clear search state
                self.is_searching = false;
                self.current_mention_start_index = None;
                return;
            }

            let header_view = self.cmd_text_input.view(id!(popup.header_view));

            let header_height = if header_view.area().rect(cx).size.y > 0.0 {
                header_view.area().rect(cx).size.y
            } else {
                // Fallback
                let estimated_padding = 24.0;
                let text_height = 16.0;
                estimated_padding + text_height
            };

            // Get spacing between header and list
            let estimated_spacing = 4.0;

            if total_items_in_list <= MAX_VISIBLE_ITEMS {
                let single_item_height = if is_desktop { 32.0 } else { 64.0 };
                let total_height =
                    (total_items_in_list as f64 * single_item_height) + header_height + estimated_spacing;
                popup.apply_over(cx, live! { height: (total_height) });
            } else {
                let max_height = if is_desktop { 400.0 } else { 480.0 };
                popup.apply_over(cx, live! { height: (max_height) });
            }

            // Only create widgets for items that will actually be visible
            for (index, (display_name, member)) in matched_members.into_iter().take(MAX_VISIBLE_ITEMS).enumerate() {
                let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                item.label(id!(user_info.username)).set_text(cx, &display_name);

                // Use the full user ID string
                let user_id_str = member.user_id().as_str();
                item.label(id!(user_id)).set_text(cx, user_id_str);

                // item.apply_over(cx, live! {
                //     show_bg: true,
                //     cursor: Hand,
                //     padding: {left: 8., right: 8., top: 4., bottom: 4.}
                // });

                if is_desktop {
                    item.apply_over(
                        cx,
                        live!(
                            flow: Right,
                            height: 32.0,
                            align: {y: 0.5}
                        ),
                    );
                    item.view(id!(user_info.filler)).set_visible(cx, true);
                } else {
                    item.apply_over(
                        cx,
                        live!(
                            flow: Down,
                            height: 64.0,
                            spacing: 4.0
                        ),
                    );
                    item.view(id!(user_info.filler)).set_visible(cx, false);
                }

                let avatar = item.avatar(id!(user_info.avatar));
                if let Some(mxc_uri) = member.avatar_url() {
                    if let Some(avatar_data) = get_avatar(cx, mxc_uri) {
                        let _ = avatar.show_image(cx, None, |cx, img| {
                            utils::load_png_or_jpg(&img, cx, &avatar_data)
                        });
                    } else {
                        avatar.show_text(cx, None, None, &display_name);
                    }
                } else {
                    avatar.show_text(cx, None, None, "Room");
                }

                self.cmd_text_input.add_item(item.clone());

                // Set keyboard focus to the first item (either @room or the first user)
                if index == 0 && !"@room".contains(search_text) { // If @room was added, it's the first item
                    self.cmd_text_input.set_keyboard_focus_index(1); // Focus the first user if @room is index 0
                } else if index == 0 && "@room".contains(search_text) {
                    self.cmd_text_input.set_keyboard_focus_index(0); // Focus @room if it's the first item
                }
            }

            self.cmd_text_input.view(id!(popup)).set_visible(cx, true);
            if self.is_searching {
                self.cmd_text_input.text_input_ref().set_key_focus(cx);
            }
        }
    }

    // Detects valid mention trigger positions in text
    fn find_mention_trigger_position(&self, text: &str, cursor_pos: usize) -> Option<usize> {
        if cursor_pos == 0 {
            return None;
        }

        // Use utility function to convert byte position to grapheme index
        let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(text, cursor_pos);
        let text_graphemes: Vec<&str> = text.graphemes(true).collect();

        // Build byte position mapping to facilitate conversion back to byte positions
        let byte_positions = utils::build_grapheme_byte_positions(text);

        // Simple logic: trigger when cursor is immediately after @ symbol
        // Only trigger if @ is preceded by whitespace or beginning of text
        if cursor_grapheme_idx > 0 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1 ||
                (cursor_grapheme_idx > 1 && text_graphemes[cursor_grapheme_idx - 2].trim().is_empty());
            if is_preceded_by_whitespace_or_start && cursor_grapheme_idx - 1 < byte_positions.len() {
                return Some(byte_positions[cursor_grapheme_idx - 1]);
            }
        }

        // Find the last @ symbol before the cursor for search continuation
        let last_at_pos = text_graphemes[..cursor_grapheme_idx]
            .iter()
            .enumerate()
            .filter(|(_, g)| **g == "@")
            .map(|(i, _)| i)
            .next_back();

        if let Some(at_idx) = last_at_pos {
            // Extract the text after the @ symbol up to the cursor position
            let mention_text = &text_graphemes[at_idx + 1..cursor_grapheme_idx];

            // Only trigger if this looks like an ongoing mention (contains only alphanumeric and basic chars)
            if self.is_valid_mention_text(mention_text) {
                // Ensure at_idx is within bounds of byte_positions
                if at_idx < byte_positions.len() {
                    return Some(byte_positions[at_idx]);
                }
            }
        }

        None
    }

    // Check if the cursor is inside a markdown link

    // Simple validation for mention text
    fn is_valid_mention_text(&self, graphemes: &[&str]) -> bool {
        // Allow empty text (for @)
        if graphemes.is_empty() {
            return true;
        }

        // Check if it contains newline characters
        !graphemes.iter().any(|g| g.contains('\n'))
    }

    // Helper function to check if a user matches the search text
    // Checks both display name and Matrix ID for matching
    fn user_matches_search(&self, member: &RoomMember, search_text: &str) -> bool {
        let search_text_lower = search_text.to_lowercase();

        // Check display name
        let display_name = member
            .display_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| member.user_id().to_string());

        let display_name_lower = display_name.to_lowercase();
        if display_name_lower.contains(&search_text_lower) {
            return true;
        }

        // Check Matrix ID (user ID)
        let user_id_str = member.user_id().to_string();
        let user_id_lower = user_id_str.to_lowercase();

        // Match against the full user ID (e.g., "@mihran:matrix.org")
        if user_id_lower.contains(&search_text_lower) {
            return true;
        }

        // Also match against just the localpart (e.g., "mihran" from "@mihran:matrix.org")
        let localpart = member.user_id().localpart();
        let localpart_lower = localpart.to_lowercase();
        if localpart_lower.contains(&search_text_lower) {
            return true;
        }

        false
    }

    // Helper function to determine match priority for sorting
    // Lower values = higher priority (better matches shown first)
    fn get_match_priority(&self, member: &RoomMember, search_text: &str) -> u8 {
        let search_text_lower = search_text.to_lowercase();

        let display_name = member
            .display_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| member.user_id().to_string());

        let display_name_lower = display_name.to_lowercase();

        // Priority 1: Display name starts with search text
        if display_name_lower.starts_with(&search_text_lower) {
            return 1;
        }

        // Priority 2: Localpart starts with search text
        let localpart = member.user_id().localpart();
        let localpart_lower = localpart.to_lowercase();
        if localpart_lower.starts_with(&search_text_lower) {
            return 2;
        }

        // Priority 3: Display name contains search text
        if display_name_lower.contains(&search_text_lower) {
            return 3;
        }

        // Priority 4: Full user ID contains search text
        let user_id_str = member.user_id().to_string();
        let user_id_lower = user_id_str.to_lowercase();
        if user_id_lower.contains(&search_text_lower) {
            return 4;
        }

        // Priority 5: Localpart contains search text
        if localpart_lower.contains(&search_text_lower) {
            return 5;
        }

        // Should not reach here if user_matches_search returned true
        u8::MAX
    }

    // Shows the loading indicator when members are being fetched
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create loading indicator widget
        let loading_item = match self.loading_indicator {
            Some(ptr) => {
                WidgetRef::new_from_ptr(cx, Some(ptr))
            },
            None => {
                return;
            }
        };

        // Start the loading animation
        loading_item.typing_animation(id!(loading_animation)).start_animation(cx);

        // Add the loading indicator to the popup
        self.cmd_text_input.add_item(loading_item);

        // Setup popup dimensions for loading state
        let popup = self.cmd_text_input.view(id!(popup));
        let header_view = self.cmd_text_input.view(id!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Set appropriate height for loading indicator
        let is_desktop = cx.display_context.is_desktop();

        // Calculate header height
        let header_height = if header_view.area().rect(cx).size.y > 0.0 {
            header_view.area().rect(cx).size.y
        } else {
            // Fallback estimate for header
            let estimated_padding = 24.0;
            let text_height = 16.0;
            estimated_padding + text_height
        };

        // Loading indicator needs: animation height (24) + padding (16+16) + text height (~16) = ~56px minimum
        let loading_content_height = if is_desktop { 56.0 } else { 64.0 };
        let estimated_spacing = 4.0;
        let loading_height = header_height + loading_content_height + estimated_spacing;

        popup.apply_over(cx, live! { height: (loading_height) });
        popup.set_visible(cx, true);

        // Maintain text input focus
        if self.is_searching {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    // Cleanup helper for closing mention popup
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        self.current_mention_start_index = None;
        self.is_searching = false;
        self.members_loading = false; // Reset loading state when closing popup

        // Clear list items to avoid keeping old content when popup is shown again
        self.cmd_text_input.clear_items();

        // Get popup and header view references
        let popup = self.cmd_text_input.view(id!(popup));
        let header_view = self.cmd_text_input.view(id!(popup.header_view));

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
        self.redraw(cx);
    }

    /// Returns the current text content
    pub fn text(&self) -> String {
        self.cmd_text_input.text_input_ref().text()
    }

    /// Sets the text content
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.cmd_text_input.text_input_ref().set_text(cx, text);
        self.redraw(cx);
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

    /// Returns the mentions analysis for the given html message content.
    /// Returns (user_mentions, has_room_mention)
    fn get_real_mentions_in_html_text(&self, html: &str) -> (BTreeSet<OwnedUserId>, bool) {
        let Some(inner) = self.borrow() else {
            return (BTreeSet::new(), false);
        };

        let mut user_mentions = BTreeSet::new();
        for (user_id, username) in &inner.possible_mentions {
            if html.contains(&format!(
                "<a href=\"{}\">{}</a>",
                user_id.matrix_to_uri(),
                username,
            )) {
                user_mentions.insert(user_id.clone());
            }
        }

        // Check for @room mention in HTML content
        let has_room_mention = inner.possible_room_mention && html.contains("@room");

        (user_mentions, has_room_mention)
    }

    /// Returns the mentions analysis for the given markdown message content.
    /// Returns (user_mentions, has_room_mention)
    fn get_real_mentions_in_markdown_text(&self, markdown: &str) -> (BTreeSet<OwnedUserId>, bool) {
        let Some(inner) = self.borrow() else {
            return (BTreeSet::new(), false);
        };

        let mut user_mentions = BTreeSet::new();
        for (user_id, username) in &inner.possible_mentions {
            if markdown.contains(&format!(
                "[{}]({})",
                username,
                user_id.matrix_to_uri(),
            )) {
                user_mentions.insert(user_id.clone());
            }
        }

        // Check for @room mention in markdown content
        let has_room_mention = inner.possible_room_mention && markdown.contains("@room");

        (user_mentions, has_room_mention)
    }

    /// Processes entered text and creates a message with mentions based on detected message type.
    /// This method handles /html, /plain prefixes and defaults to markdown.
    pub fn create_message_with_mentions(&self, entered_text: &str) -> RoomMessageEventContent {
        if let Some(html_text) = entered_text.strip_prefix("/html") {
            let message = RoomMessageEventContent::text_html(html_text, html_text);
            let (user_mentions, has_room_mention) = self.get_real_mentions_in_html_text(html_text);

            if !user_mentions.is_empty() || has_room_mention {
                let mut matrix_mentions = Mentions::with_user_ids(user_mentions);
                matrix_mentions.room = has_room_mention;
                message.add_mentions(matrix_mentions)
            } else {
                message
            }
        } else if let Some(plain_text) = entered_text.strip_prefix("/plain") {
            // Plain text messages don't support mentions
            RoomMessageEventContent::text_plain(plain_text)
        } else {
            let message = RoomMessageEventContent::text_markdown(entered_text);
            let (user_mentions, has_room_mention) = self.get_real_mentions_in_markdown_text(entered_text);

            if !user_mentions.is_empty() || has_room_mention {
                let mut matrix_mentions = Mentions::with_user_ids(user_mentions);
                matrix_mentions.room = has_room_mention;
                message.add_mentions(matrix_mentions)
            } else {
                message
            }
        }
    }

}
