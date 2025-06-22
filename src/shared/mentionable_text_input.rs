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

// Constants for mention popup height calculations
const DESKTOP_ITEM_HEIGHT: f64 = 32.0;
const MOBILE_ITEM_HEIGHT: f64 = 64.0;
const MOBILE_USERNAME_SPACING: f64 = 0.5;

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

        // Best practice: Always check Scope first to get current context
        // Scope represents the current widget context as passed down from parents
        let scope_room_id = scope.props.get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput")
            .room_id.clone();

        if let Event::Actions(actions) = event {

            let text_input_ref = self.cmd_text_input.text_input_ref(); // Get reference to the inner TextInput
            let text_input_uid = text_input_ref.widget_uid();
            let text_input_area = text_input_ref.area();
            let has_focus = cx.has_key_focus(text_input_area);


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
                            // If Scope room_id doesn't match action's room_id, this action may be for another room
                            // This is important to avoid applying actions to the wrong room context
                            if &scope_room_id != room_id {
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
                            // Only process if this action is for the current room
                            if &scope_room_id == room_id {

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
            let room_mention_label = selected.label(id!(user_info.room_mention));
            let room_mention_text = room_mention_label.text();
            let room_user_id_text = selected.label(id!(room_user_id)).text();

            let is_room_mention = { room_mention_text == "Notify the entire room" && room_user_id_text == "@room" };

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

        // Check if we're currently searching and the @ symbol was deleted
        if self.is_searching {
            if let Some(start_pos) = self.current_mention_start_index {
                // Check if the @ symbol at the start position still exists
                if start_pos >= text.len() || &text[start_pos..start_pos+1] != "@" {
                    // The @ symbol was deleted, stop searching
                    self.close_mention_popup(cx);
                    return;
                }
            }
        }

        // Look for trigger position for @ menu
        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
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
            // Reset popup height to ensure proper calculation for user list
            let popup = self.cmd_text_input.view(id!(popup));
            popup.apply_over(cx, live! { height: Fit });
        } else if members_are_empty && self.members_loading {
            // Still loading and members are empty - keep showing loading indicator
            return;
        }

        // Clear old list items, prepare to populate new list
        self.cmd_text_input.clear_items();

        if self.is_searching {
            let is_desktop = cx.display_context.is_desktop();
            let max_visible_items = if is_desktop { 10 } else { 5 };

            if self.can_notify_room && ("@room".contains(search_text) || search_text.is_empty()) {
                let room_mention_item = match self.room_mention_list_item {
                    Some(ptr) => WidgetRef::new_from_ptr(cx, Some(ptr)),
                    None => {
                        return;
                    }
                };
                let mut room_avatar_shown = false;

                let avatar_ref = room_mention_item.avatar(id!(user_info.room_avatar));

                // Get room avatar fallback text from room display name
                let room_name_first_char = get_client()
                    .and_then(|client| client.get_room(room_id))
                    .and_then(|room| room.cached_display_name().map(|name| name.to_string()))
                    .and_then(|name| name.graphemes(true).next().map(|s| s.to_uppercase()))
                    .filter(|s| s != "@" && s.chars().all(|c| c.is_alphabetic()))
                    .unwrap_or_else(|| "R".to_string());

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
                                    avatar_ref.show_text(cx, Some(COLOR_UNKNOWN_ROOM_AVATAR), None, &room_name_first_char);
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

                // If unable to display room avatar, show first character of room name
                if !room_avatar_shown {
                    avatar_ref.show_text(cx, Some(COLOR_UNKNOWN_ROOM_AVATAR), None, &room_name_first_char);
                }

                // The room_user_id already has "@room" text set in the template
                // No need to set it again

                // Apply layout and height styling based on device type
                let new_height = if is_desktop { DESKTOP_ITEM_HEIGHT } else { MOBILE_ITEM_HEIGHT };
                if is_desktop {
                    room_mention_item.apply_over(cx, live! {
                        height: (new_height),
                        flow: Right,
                    });
                } else {
                    room_mention_item.apply_over(cx, live! {
                        height: (new_height),
                        flow: Down,
                    });
                }

                self.cmd_text_input.add_item(room_mention_item);
            }

            // Improved search: match both display names and Matrix IDs, then sort by priority
            let max_matched_members = max_visible_items * 2;  // Buffer for better UX

            // Collect all matching members with their priority scores
            let mut prioritized_members = Vec::new();

            // Get current user ID to filter out self-mentions
            let current_user_id = crate::sliding_sync::current_user_id();

            for member in room_members.iter() {
                if prioritized_members.len() >= max_matched_members {
                    break;
                }

                // Skip the current user - users should not be able to mention themselves
                if let Some(ref current_id) = current_user_id {
                    if member.user_id() == current_id {
                        continue;
                    }
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
            // 1. Reduces processing from over 2,000 items to a reasonable amount
            // 2. Reduces Widget creation to visible items only
            // 3. Significantly decreases string operations and Widget creation overhead
            let popup = self.cmd_text_input.view(id!(popup));

            // Adjust height calculation to include the potential @room item
            // Use the same condition as when actually adding the @room item
            let has_room_item = self.can_notify_room && ("@room".contains(search_text) || search_text.is_empty());
            let total_items_in_list = member_count + has_room_item as usize;

            if total_items_in_list == 0 {
                // If there are no matching items, just hide the entire popup and clear search state
                popup.apply_over(cx, live! { height: Fit });
                self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
                // Clear search state
                self.is_searching = false;
                self.current_mention_start_index = None;
                return;
            }

            // Only create widgets for items that will actually be visible
            // If @room exists, reserve one slot for it
            let user_items_limit = max_visible_items.saturating_sub(has_room_item as usize);
            for (index, (display_name, member)) in matched_members.into_iter().take(user_items_limit).enumerate() {
                let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                item.label(id!(user_info.username)).set_text(cx, &display_name);

                // Use the full user ID string
                let user_id_str = member.user_id().as_str();
                item.label(id!(user_id)).set_text(cx, user_id_str);

                if is_desktop {
                    item.apply_over(
                        cx,
                        live!(
                            flow: Right,
                            height: (DESKTOP_ITEM_HEIGHT),
                            align: {y: 0.5}
                        ),
                    );
                    item.view(id!(user_info.filler)).set_visible(cx, true);
                } else {
                    item.apply_over(
                        cx,
                        live!(
                            flow: Down,
                            height: (MOBILE_ITEM_HEIGHT),
                            spacing: (MOBILE_USERNAME_SPACING)
                        ),
                    );
                    item.view(id!(user_info.filler)).set_visible(cx, false);
                }

                let avatar = item.avatar(id!(user_info.avatar));
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

                // Set keyboard focus to the first item
                if index == 0 {
                    // If @room exists, it's index 0, otherwise first user is index 0
                    self.cmd_text_input.set_keyboard_focus_index(0);
                }
            }

            let popup = self.cmd_text_input.view(id!(popup));
            popup.set_visible(cx, true);
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
        // Only continue if we're already in search mode
        if self.is_searching {
            let last_at_pos = text_graphemes[..cursor_grapheme_idx]
                .iter()
                .enumerate()
                .filter(|(_, g)| **g == "@")
                .map(|(i, _)| i)
                .next_back();

            if let Some(at_idx) = last_at_pos {
                // Get the byte position of this @ symbol
                let at_byte_pos = if at_idx < byte_positions.len() {
                    byte_positions[at_idx]
                } else {
                    return None;
                };

                // Extract the text after the @ symbol up to the cursor position
                let mention_text = &text_graphemes[at_idx + 1..cursor_grapheme_idx];

                // Only trigger if this looks like an ongoing mention (contains only alphanumeric and basic chars)
                if self.is_valid_mention_text(mention_text) {
                    return Some(at_byte_pos);
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

        // Only match against the localpart (e.g., "mihran" from "@mihran:matrix.org")
        // Don't match against the homeserver part to avoid false matches
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
        let localpart = member.user_id().localpart();
        let localpart_lower = localpart.to_lowercase();

        // Priority 0: Exact case-sensitive match (highest priority)
        if display_name == search_text || localpart == search_text {
            return 0;
        }

        // Priority 1: Exact match (case-insensitive)
        if display_name_lower == search_text_lower || localpart_lower == search_text_lower {
            return 1;
        }

        // Priority 2: Case-sensitive prefix match
        if display_name.starts_with(search_text) || localpart.starts_with(search_text) {
            return 2;
        }

        // Priority 3: Display name starts with search text (case-insensitive)
        if display_name_lower.starts_with(&search_text_lower) {
            return 3;
        }

        // Priority 4: Localpart starts with search text (case-insensitive)
        if localpart_lower.starts_with(&search_text_lower) {
            return 4;
        }

        // Priority 5: Display name contains search text at word boundary
        if let Some(pos) = display_name_lower.find(&search_text_lower) {
            // Check if it's at the start of a word (preceded by space or at start)
            if pos == 0 || display_name_lower.chars().nth(pos - 1) == Some(' ') {
                return 5;
            }
        }

        // Priority 6: Localpart contains search text at word boundary
        if let Some(pos) = localpart_lower.find(&search_text_lower) {
            // Check if it's at the start of a word (preceded by non-alphanumeric or at start)
            if pos == 0 || !localpart_lower.chars().nth(pos - 1).unwrap_or('a').is_alphanumeric() {
                return 6;
            }
        }

        // Priority 7: Display name contains search text (anywhere)
        if display_name_lower.contains(&search_text_lower) {
            return 7;
        }

        // Priority 8: Localpart contains search text (anywhere)
        if localpart_lower.contains(&search_text_lower) {
            return 8;
        }

        // Should not reach here if user_matches_search returned true
        u8::MAX
    }

    // Shows the loading indicator when members are being fetched
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create loading indicator widget
        let Some(ptr) = self.loading_indicator else { return };
        let loading_item = WidgetRef::new_from_ptr(cx, Some(ptr));

        // Start the loading animation
        loading_item.typing_animation(id!(loading_animation)).start_animation(cx);

        // Add the loading indicator to the popup
        self.cmd_text_input.add_item(loading_item);

        // Setup popup dimensions for loading state
        let popup = self.cmd_text_input.view(id!(popup));
        let header_view = self.cmd_text_input.view(id!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Don't manually set popup height for loading - let it auto-size based on content
        // This avoids conflicts with list = { height: Fill }
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

    /// Extracts existing @room and user mentions from the current text and optionally from the original event
    /// and populates possible_mentions and possible_room_mention for editing.
    ///
    /// The method searches for mentions from two sources:
    /// 1. Original Matrix event data (preferred): extracts from event.content.mentions if available
    /// 2. Text analysis fallback: searches for markdown patterns in the text
    ///    - @room mentions: literal "@room" text
    ///    - User mentions: markdown links in format [displayname](matrix:u/@userid:server.com)
    ///
    /// This allows the text input to properly track mentions when editing existing messages.
    pub fn extract_existing_mentions(&mut self, cx: &mut Cx) {
        self.extract_existing_mentions_with_event(cx, None);
    }

    /// Internal method that extracts mentions from both the original event and text analysis.
    /// This is called by both extract_existing_mentions() and the editing flow.
    fn extract_existing_mentions_with_event(&mut self, cx: &mut Cx, original_event_item: Option<&matrix_sdk_ui::timeline::EventTimelineItem>) {
        // First, try to extract mentions from the original Matrix event data (most reliable)
        let mut mentions_from_event = false;
        if let Some(event_item) = original_event_item {
            if let Some(original_event) = event_item.latest_json() {
                if let Ok(matrix_sdk::ruma::events::AnySyncTimelineEvent::MessageLike(
                    matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                )) = original_event.deserialize() {
                    if let Some(mentions) = sync_event.as_original().and_then(|evt| evt.content.mentions.as_ref()) {
                        // Convert BTreeSet<OwnedUserId> to BTreeMap<OwnedUserId, String> for possible_mentions
                        self.possible_mentions = mentions.user_ids.iter()
                            .map(|user_id| (user_id.clone(), user_id.to_string()))
                            .collect();

                        self.possible_room_mention = mentions.room;
                        mentions_from_event = true;

                        // If we have mentions but the current text doesn't contain proper markdown format,
                        // reconstruct the text with proper mention markdown to preserve formatting
                        self.reconstruct_text_with_mentions(cx);
                    }
                }
            }
        }

        // If we couldn't extract mentions from the event, fall back to text analysis
        if !mentions_from_event {
            self.extract_mentions_from_text();
        }
    }

    /// Reconstructs the text content to include proper mention markdown when the text
    /// has been stripped of markdown by other clients (like Element) but we still have
    /// the mention information from the original event.
    fn reconstruct_text_with_mentions(&mut self, cx: &mut Cx) {
        let current_text = self.text();

        log!("Reconstructing text with mentions {:#?}", current_text);

        // Check if the text already contains markdown mentions - if so, don't modify it
        if current_text.contains("](matrix:u/") {
            return;
        }

        let mut reconstructed_text = current_text.clone();

        // First, try to convert HTML format mentions to Markdown
        if current_text.contains("<a href=") {
            reconstructed_text = self.convert_html_mentions_to_markdown(&current_text);
        }

        // If we still don't have markdown mentions, try to reconstruct from plain text
        if !reconstructed_text.contains("](matrix:u/") {
            // Reconstruct user mentions: replace plain text usernames with markdown links
            for user_id in self.possible_mentions.clone().keys() {
                // Try to find the user's display name or localpart in the text
                let localpart = user_id.localpart();

                // Look for patterns like the user's localpart or display name that should be linked
                // This is a heuristic approach - we replace text that looks like it should be a mention
                let patterns_to_try = vec![
                    localpart.to_string(),
                    format!("@{}", localpart),
                    user_id.to_string(),
                ];

                for pattern in patterns_to_try {
                    if reconstructed_text.contains(&pattern) {
                        // Create the markdown mention link
                        let mention_markdown = format!("[{}]({})", pattern.trim_start_matches('@'), user_id.matrix_to_uri());

                        // Replace the first occurrence to avoid replacing the same mention multiple times
                        if let Some(pos) = reconstructed_text.find(&pattern) {
                            // Check if this text is at word boundary (not part of another word)
                            let is_word_boundary = (pos == 0 || !reconstructed_text.chars().nth(pos - 1).unwrap_or(' ').is_alphanumeric()) &&
                                (pos + pattern.len() >= reconstructed_text.len() || !reconstructed_text.chars().nth(pos + pattern.len()).unwrap_or(' ').is_alphanumeric());

                            if is_word_boundary {
                                reconstructed_text.replace_range(pos..pos + pattern.len(), &mention_markdown);
                                break; // Only replace the first occurrence of this user
                            }
                        }
                    }
                }
            }
        }

        // Update the text if we made changes
        if reconstructed_text != current_text {
            self.cmd_text_input.text_input_ref().set_text(cx, &reconstructed_text);
        }
    }

    /// Converts HTML format mentions to Markdown format.
    /// Handles Matrix HTML mentions with any valid Matrix URI format.
    /// Converts them to Markdown format while preserving the original URI.
    fn convert_html_mentions_to_markdown(&self, html_text: &str) -> String {
        let mut markdown_text = html_text.to_string();

        // Pattern to match HTML links: <a href="...">DisplayName</a>
        let mut pos = 0;
        while let Some(start_pos) = markdown_text[pos..].find("<a href=\"") {
            let absolute_start = pos + start_pos;

            // Find the end of the href attribute
            if let Some(href_end) = markdown_text[absolute_start..].find("\">") {
                let href_end_absolute = absolute_start + href_end;

                // Extract the full URL from the href
                let href_start = absolute_start + "<a href=\"".len();
                let full_url = &markdown_text[href_start..href_end_absolute];

                // Check if this is a Matrix user mention by looking for @user:domain pattern in the URL
                let is_matrix_user_mention = self.is_matrix_user_mention_url(full_url);

                if is_matrix_user_mention {
                    // Find the display name (text between > and </a>)
                    let display_name_start = href_end_absolute + 2; // Skip ">
                    if let Some(link_end) = markdown_text[display_name_start..].find("</a>") {
                        let link_end_absolute = display_name_start + link_end;
                        let display_name = &markdown_text[display_name_start..link_end_absolute];

                        // Create the Markdown mention, preserving the original URL
                        let markdown_mention = format!("[{}]({})", display_name, full_url);

                        // Replace the entire HTML mention with Markdown
                        let full_link_end = link_end_absolute + 4; // Include "</a>"
                        markdown_text.replace_range(absolute_start..full_link_end, &markdown_mention);

                        // Update position to continue searching after the replacement
                        pos = absolute_start + markdown_mention.len();
                    } else {
                        // Malformed HTML, skip
                        pos = href_end_absolute + 2;
                    }
                } else {
                    // Not a Matrix user mention, skip this link
                    pos = href_end_absolute + 2;
                }
            } else {
                // Malformed HTML, skip
                pos = absolute_start + 1;
            }
        }

        markdown_text
    }

    /// Checks if a URL is a Matrix user mention.
    /// This method looks for @user:domain patterns in the URL, regardless of the URL format.
    fn is_matrix_user_mention_url(&self, url: &str) -> bool {
        // Look for Matrix user ID pattern: @username:domain
        // Use simple string parsing to avoid regex dependency
        if let Some(at_pos) = url.find('@') {
            // Find the colon after the @
            if let Some(colon_pos) = url[at_pos..].find(':') {
                let colon_abs_pos = at_pos + colon_pos;

                // Check that there's content after the colon (domain part)
                if colon_abs_pos + 1 < url.len() {
                    // Extract potential username and domain
                    let username_part = &url[at_pos + 1..colon_abs_pos];
                    let remaining = &url[colon_abs_pos + 1..];

                    // Find where domain part ends (could be end of string, or next special char)
                    let domain_end = remaining.find(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                        .unwrap_or(remaining.len());
                    let domain_part = &remaining[..domain_end];

                    // Basic validation: username and domain should not be empty and contain valid chars
                    return !username_part.is_empty()
                        && !domain_part.is_empty()
                        && username_part.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '=' || c == '-')
                        && domain_part.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
                        && domain_part.contains('.');
                }
            }
        }

        false
    }

    /// Fallback method that extracts mentions by analyzing the text content.
    /// This is used when the original event data is not available.
    fn extract_mentions_from_text(&mut self) {
        let text = self.text();

        // === Part 1: Find all @room mentions ===
        // Search pattern: "@room"
        // Example: "Hello @room everyone" -> finds @room at position 6
        let mut pos = 0;
        while let Some(found_pos) = text[pos..].find("@room") {
            let absolute_pos = pos + found_pos;

            // Validate: @room must be at text start OR preceded by whitespace
            // Valid: "@room", " @room", "\n@room"
            // Invalid: "fake@room", "test@room"
            let is_valid_room_mention = absolute_pos == 0 ||
                text.chars().nth(absolute_pos - 1).is_some_and(|c| c.is_whitespace());

            if is_valid_room_mention {
                // Enable room mentions for this text input
                self.possible_room_mention = true;

            }

            // Continue searching from next character to find multiple @room mentions
            pos = absolute_pos + 1;
        }

        // === Part 2: Find all user mention patterns ===
        // Search pattern: "](matrix:u/" which is part of [displayname](matrix:u/@userid:server.com)
        // Example: "Hello [Alice](matrix:u/@alice:example.com) there"
        //          Structure: [Alice](matrix:u/@alice:example.com)
        //                     ^     ^              ^               ^
        //                     |     |              |               |
        //               bracket_start  link_pattern_start    user_id_end
        pos = 0;
        while let Some(found_pos) = text[pos..].find("](matrix:u/") {
            let link_end = pos + found_pos; // Position right before "]"

            // Find the corresponding opening bracket by searching backwards
            // This handles cases where there might be multiple '[' characters
            if let Some(bracket_start) = text[..link_end].rfind('[') {

                // Validate: mention must be at text start OR preceded by whitespace
                // This prevents matching partial mentions like "fake[user](matrix:u/...)"
                let is_valid_user_mention = bracket_start == 0 ||
                    text.chars().nth(bracket_start - 1).is_some_and(|c| c.is_whitespace());

                if is_valid_user_mention {
                    // Extract user ID from the URL part: matrix:u/@userid:server.com)
                    // Search for '@' after "](matrix:u/" and extract until ')'
                    if let Some(user_id_start) = text[link_end..].find("@") {
                        if let Some(user_id_end) = text[link_end + user_id_start..].find(")") {
                            // Extract the full user ID string (e.g., "@alice:example.com")
                            let user_id_str = &text[link_end + user_id_start..link_end + user_id_start + user_id_end];

                            // Try to parse as a valid Matrix user ID
                            if let Ok(user_id) = user_id_str.try_into() {
                                // Store the user ID for mention detection during final message creation
                                // Key: OwnedUserId, Value: display string (currently just the user ID)
                                self.possible_mentions.insert(user_id, user_id_str.to_string());
                            }
                        }
                    }

                }
            }

            // Continue searching from after the current pattern
            pos = link_end + 1;
        }
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

    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    /// Extracts existing @room and user mentions from the current text
    pub fn extract_existing_mentions(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.extract_existing_mentions(cx);
        }
    }

    /// Extracts existing mentions from both the original Matrix event data and text analysis.
    /// This method should be used when editing a message to preserve the original mentions.
    ///
    /// Priority:
    /// 1. First tries to extract from the original Matrix event data (most reliable)
    /// 2. Falls back to text analysis if event data is unavailable
    pub fn extract_existing_mentions_from_event(&self, cx: &mut Cx, original_event_item: &matrix_sdk_ui::timeline::EventTimelineItem) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.extract_existing_mentions_with_event(cx, Some(original_event_item));
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

    /// Returns the mentions analysis for the given markdown message content.
    /// Returns (user_mentions, has_room_mention)
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

#[cfg(test)]
mod tests_html_to_markdown_conversion {
    #[test]
    fn tests_convert_single_html_mention_to_markdown() {
        // Create a mock MentionableTextInput for testing
        // We can't easily create a full widget instance in tests,
        // so we'll test the conversion logic directly
        let html_input = r#"Hello <a href="https://matrix.to/#/@alice:example.com">Alice</a> how are you?"#;
        let expected_markdown = r#"Hello [Alice](https://matrix.to/#/@alice:example.com) how are you?"#;

        // Test the conversion logic
        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_convert_multiple_html_mentions_to_markdown() {
        let html_input = r#"@room @room  @room  <a href="https://matrix.to/#/@blackanger:matrix.org">Alex</a>  <a href="https://matrix.to/#/@feeds:integrations.ems.host">Feeds</a> @room  <a href="https://matrix.to/#/@blackanger:matrix.org">Alex</a>"#;
        let expected_markdown = r#"@room @room  @room  [Alex](https://matrix.to/#/@blackanger:matrix.org)  [Feeds](https://matrix.to/#/@feeds:integrations.ems.host) @room  [Alex](https://matrix.to/#/@blackanger:matrix.org)"#;

        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_convert_mixed_content_with_html_mentions() {
        let html_input = r#"Hi <a href="https://matrix.to/#/@user:server.com">User</a>, let's discuss this with <a href="https://matrix.to/#/@admin:server.com">Admin</a>."#;
        let expected_markdown = r#"Hi [User](https://matrix.to/#/@user:server.com), let's discuss this with [Admin](https://matrix.to/#/@admin:server.com)."#;

        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_no_conversion_when_no_html_mentions() {
        let plain_text = "Hello @room this is a test message";
        let result = convert_html_mention_to_markdown_test_helper(plain_text);
        assert_eq!(result, plain_text);
    }

    #[test]
    fn tests_convert_alternative_matrix_url_formats() {
        // Test with different Matrix URL formats
        let html_input = r#"Hi <a href="matrix:u/@user:example.com">User</a> and <a href="https://example.com/matrix/#/@admin:test.org">Admin</a>!"#;
        let expected_markdown = r#"Hi [User](matrix:u/@user:example.com) and [Admin](https://example.com/matrix/#/@admin:test.org)!"#;

        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_ignore_non_matrix_links() {
        // Test that non-Matrix links are left unchanged
        let html_input = r#"Check out <a href="https://example.com">this website</a> and <a href="mailto:test@example.com">send email</a>."#;
        let expected_markdown = html_input; // Should remain unchanged

        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_mixed_matrix_and_regular_links() {
        // Test mixing Matrix mentions with regular links
        let html_input = r#"Visit <a href="https://example.com">our site</a> or contact <a href="https://matrix.to/#/@support:example.com">Support</a>."#;
        let expected_markdown = r#"Visit <a href="https://example.com">our site</a> or contact [Support](https://matrix.to/#/@support:example.com)."#;

        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    #[test]
    fn tests_debug_problematic_input() {
        // Test the exact problematic input to debug the issue
        let problematic_input = r#"@room [Feeds](https://matrix.to/#/@feeds:integrations.ems.host)"#;
        let result = convert_html_mention_to_markdown_test_helper(problematic_input);
        // This input is already markdown, should remain unchanged
        assert_eq!(result, problematic_input);

        // Test what the actual HTML input might look like
        let html_input = r#"@room <a href="https://matrix.to/#/@feeds:integrations.ems.host">Feeds</a>"#;
        let expected_markdown = r#"@room [Feeds](https://matrix.to/#/@feeds:integrations.ems.host)"#;
        let result = convert_html_mention_to_markdown_test_helper(html_input);
        assert_eq!(result, expected_markdown);
    }

    // Helper function to test the HTML to Markdown conversion logic
    fn convert_html_mention_to_markdown_test_helper(html_text: &str) -> String {
        let mut markdown_text = html_text.to_string();

        // This is the same logic as in convert_html_mentions_to_markdown
        let mut pos = 0;
        while let Some(start_pos) = markdown_text[pos..].find("<a href=\"") {
            let absolute_start = pos + start_pos;

            if let Some(href_end) = markdown_text[absolute_start..].find("\">") {
                let href_end_absolute = absolute_start + href_end;

                let href_start = absolute_start + "<a href=\"".len();
                let full_url = &markdown_text[href_start..href_end_absolute];

                // Check if this is a Matrix user mention
                let is_matrix_user_mention = is_matrix_user_mention_url_test_helper(full_url);

                if is_matrix_user_mention {
                    let display_name_start = href_end_absolute + 2;
                    if let Some(link_end) = markdown_text[display_name_start..].find("</a>") {
                        let link_end_absolute = display_name_start + link_end;
                        let display_name = &markdown_text[display_name_start..link_end_absolute];

                        let markdown_mention = format!("[{}]({})", display_name, full_url);

                        let full_link_end = link_end_absolute + 4;
                        markdown_text.replace_range(absolute_start..full_link_end, &markdown_mention);

                        pos = absolute_start + markdown_mention.len();
                    } else {
                        pos = href_end_absolute + 2;
                    }
                } else {
                    pos = href_end_absolute + 2;
                }
            } else {
                pos = absolute_start + 1;
            }
        }

        markdown_text
    }

    // Helper function for testing Matrix user mention URL detection
    fn is_matrix_user_mention_url_test_helper(url: &str) -> bool {
        if let Some(at_pos) = url.find('@') {
            if let Some(colon_pos) = url[at_pos..].find(':') {
                let colon_abs_pos = at_pos + colon_pos;

                if colon_abs_pos + 1 < url.len() {
                    let username_part = &url[at_pos + 1..colon_abs_pos];
                    let remaining = &url[colon_abs_pos + 1..];

                    let domain_end = remaining.find(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
                        .unwrap_or(remaining.len());
                    let domain_part = &remaining[..domain_end];

                    return !username_part.is_empty()
                        && !domain_part.is_empty()
                        && username_part.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '=' || c == '-')
                        && domain_part.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
                        && domain_part.contains('.');
                }
            }
        }

        false
    }
}
