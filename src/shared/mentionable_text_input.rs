//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::typing_animation::TypingAnimationWidgetRefExt;
use crate::shared::styles::COLOR_UNKNOWN_ROOM_AVATAR;
use crate::utils;

mod mention_utils;

use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::ruma::{events::{room::message::RoomMessageEventContent, Mentions, AnySyncTimelineEvent, AnySyncMessageLikeEvent}, OwnedRoomId, OwnedUserId};
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
            let text_input_ref = self.cmd_text_input.text_input_ref();
            let text_input_uid = text_input_ref.widget_uid();
            let text_input_area = text_input_ref.area();
            let has_focus = cx.has_key_focus(text_input_area);

            // Handle item selection from mention popup
            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
            }

            // Handle build items request
            if self.cmd_text_input.should_build_items(actions) {
                if has_focus {
                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                    self.update_user_list(cx, &search_text, scope);
                } else if self.cmd_text_input.view(id!(popup)).visible() {
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
                            break; // Found our text change action, no need to continue
                        }
                    }
                }

                // Handle MentionableTextInputAction actions
                if let Some(action_ref) = action.downcast_ref::<MentionableTextInputAction>() {
                    match action_ref {
                        MentionableTextInputAction::PowerLevelsUpdated(room_id, can_notify_room) => {
                            if &scope_room_id != room_id {
                                continue;
                            }

                            if self.can_notify_room != *can_notify_room {
                                self.can_notify_room = *can_notify_room;
                                if self.is_searching && has_focus {
                                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                                    self.update_user_list(cx, &search_text, scope);
                                } else {
                                    self.redraw(cx);
                                }
                            }
                        },
                        MentionableTextInputAction::RoomMembersLoaded(room_id) => {
                            if &scope_room_id == room_id && self.members_loading {
                                self.members_loading = false;
                                if self.is_searching && has_focus {
                                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                                    self.update_user_list(cx, &search_text, scope);
                                }
                            }
                        },
                        _ => {},
                    }
                }
            }

            // Close popup if focus is lost
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

    /// Check if members are loading and show loading indicator if needed.
    ///
    /// Returns true if we should return early because we're in the loading state.
    fn handle_members_loading_state(&mut self, cx: &mut Cx, room_members: &Option<std::sync::Arc<Vec<RoomMember>>>) -> bool {
        let Some(room_members) = room_members else {
            self.members_loading = true;
            self.show_loading_indicator(cx);
            return true;
        };

        let members_are_empty = room_members.is_empty();

        if members_are_empty && !self.members_loading {
            // Members list is empty and we're not already showing loading - start loading state
            self.members_loading = true;
            self.show_loading_indicator(cx);
            return true;
        } else if !members_are_empty && self.members_loading {
            // Members have been loaded, stop loading state
            self.members_loading = false;
            // Reset popup height to ensure proper calculation for user list
            let popup = self.cmd_text_input.view(id!(popup));
            popup.apply_over(cx, live! { height: Fit });
        } else if members_are_empty && self.members_loading {
            // Still loading and members are empty - keep showing loading indicator
            return true;
        }

        false
    }

    // Try to add @room mention item to the list
    // Returns true if @room item was added
    fn try_add_room_mention_item(&mut self, cx: &mut Cx, search_text: &str, room_id: &OwnedRoomId, is_desktop: bool) -> bool {
        if !self.can_notify_room || !("@room".contains(search_text) || search_text.is_empty()) {
            return false;
        }

        let Some(ptr) = self.room_mention_list_item else { return false };
        let room_mention_item = WidgetRef::new_from_ptr(cx, Some(ptr));
        let mut room_avatar_shown = false;

        let avatar_ref = room_mention_item.avatar(id!(user_info.room_avatar));

        // Get room avatar fallback text from room display name
        let room_name_first_char = get_client()
            .and_then(|client| client.get_room(room_id))
            .and_then(|room| room.cached_display_name().map(|name| name.to_string()))
            .and_then(|name| name.graphemes(true).next().map(|s| s.to_uppercase()))
            .filter(|s| s != "@" && s.chars().all(|c| c.is_alphabetic()))
            .unwrap_or_else(|| "R".to_string());

        if let Some(client) = get_client().and_then(|c| c.get_room(room_id).and_then(|r| r.avatar_url())).flatten() {
            ...
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
        true
    }

    // Find and sort matching members based on search text
    fn find_and_sort_matching_members(&self, search_text: &str, room_members: &std::sync::Arc<Vec<RoomMember>>, max_matched_members: usize) -> Vec<(String, RoomMember)> {
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
                prioritized_members.push((priority, display_name, member.clone()));
            }
        }

        // Sort by priority (lower number = higher priority)
        prioritized_members.sort_by_key(|(priority, _, _)| *priority);

        // Convert to the format expected by the rest of the code
        prioritized_members
            .into_iter()
            .map(|(_, display_name, member)| (display_name, member))
            .collect()
    }

    // Add user mention items to the list
    // Returns the number of items added
    fn add_user_mention_items(&mut self, cx: &mut Cx, matched_members: Vec<(String, RoomMember)>, user_items_limit: usize, is_desktop: bool) -> usize {
        let mut items_added = 0;

        for (index, (display_name, member)) in matched_members.into_iter().take(user_items_limit).enumerate() {
            let Some(user_list_item_ptr) = self.user_list_item else { continue };
            let item = WidgetRef::new_from_ptr(cx, Some(user_list_item_ptr));

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
            items_added += 1;

            // Set keyboard focus to the first item
            if index == 0 {
                // If @room exists, it's index 0, otherwise first user is index 0
                self.cmd_text_input.set_keyboard_focus_index(0);
            }
        }

        items_added
    }

    // Update popup visibility and layout
    fn update_popup_visibility(&mut self, cx: &mut Cx, has_items: bool) {
        let popup = self.cmd_text_input.view(id!(popup));

        if has_items {
            popup.set_visible(cx, true);
            if self.is_searching {
                self.cmd_text_input.text_input_ref().set_key_focus(cx);
            }
        } else {
            // If there are no matching items, just hide the entire popup and clear search state
            popup.apply_over(cx, live! { height: Fit });
            self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
            // Clear search state
            self.is_searching = false;
            self.current_mention_start_index = None;
        }
    }

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
                if start_pos >= text.len() || text.get(start_pos..start_pos+1).is_some_and(|c| c != "@") {
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

        // 2. Check if members are loading and handle loading state
        if self.handle_members_loading_state(cx, &room_props.room_members) {
            return;
        }

        // 3. Get room members (we know they exist because handle_members_loading_state returned false)
        let room_members = room_props.room_members.as_ref().unwrap();

        // Clear old list items, prepare to populate new list
        self.cmd_text_input.clear_items();

        if !self.is_searching {
            return;
        }

        let is_desktop = cx.display_context.is_desktop();
        let max_visible_items = if is_desktop { 10 } else { 5 };
        let mut items_added = 0;

        // 4. Try to add @room mention item
        let has_room_item = self.try_add_room_mention_item(cx, search_text, room_id, is_desktop);
        if has_room_item {
            items_added += 1;
        }

        // 5. Find and sort matching members
        let max_matched_members = max_visible_items * 2;  // Buffer for better UX
        let matched_members = self.find_and_sort_matching_members(search_text, room_members, max_matched_members);

        // 6. Add user mention items
        let user_items_limit = max_visible_items.saturating_sub(has_room_item as usize);
        let user_items_added = self.add_user_mention_items(cx, matched_members, user_items_limit, is_desktop);
        items_added += user_items_added;

        // 7. Update popup visibility based on whether we have items
        self.update_popup_visibility(cx, items_added > 0);
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
    ///    - User mentions: markdown links in format [displayname](matrix:to/@userid:server.com)
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
                if let Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(sync_event)
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

    /// Checks if text contains Matrix user mention links in any supported format
    fn contains_matrix_user_mentions(&self, text: &str) -> bool {
        self::mention_utils::contains_matrix_user_mentions(text)
    }

    /// Reconstructs the text content to include proper mention markdown when the text
    /// has been stripped of markdown by other clients (like Element) but we still have
    /// the mention information from the original event.
    fn reconstruct_text_with_mentions(&mut self, cx: &mut Cx) {
        let current_text = self.text();

        // Check if the text already contains markdown mentions - if so, don't modify it
        if self.contains_matrix_user_mentions(&current_text) {
            return;
        }

        let mut reconstructed_text = current_text.clone();

        // First, try to convert HTML format mentions to Markdown
        if current_text.contains("<a href=") {
            reconstructed_text = self::mention_utils::convert_html_mentions_to_markdown(&current_text);
        }

        // If we still don't have markdown mentions, try to reconstruct from plain text
        if !self.contains_matrix_user_mentions(&reconstructed_text) {
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

    /// Fallback method that extracts mentions by analyzing the text content.
    /// This is used when the original event data is not available.
    ///
    /// Enhanced with better validation and bounds checking.
    fn extract_mentions_from_text(&mut self) {
        let text = self.text();

        // Basic input validation
        if text.is_empty() || text.len() > 65536 {  // Reasonable message length limit
            return;
        }

        // === Part 1: Find all @room mentions ===
        // Search pattern: "@room"
        // Example: "Hello @room everyone" -> finds @room at position 6
        let mut pos = 0;
        let mut room_mention_count = 0;
        const MAX_ROOM_MENTIONS: usize = 50;  // Prevent excessive processing

        while let Some(found_pos) = text[pos..].find("@room") {
            room_mention_count += 1;
            if room_mention_count > MAX_ROOM_MENTIONS {
                log!("Warning: Found more than {} @room mentions, stopping processing", MAX_ROOM_MENTIONS);
                break;
            }

            let absolute_pos = pos + found_pos;

            // Bounds check
            if absolute_pos >= text.len() {
                break;
            }

            // Validate: @room must be at text start OR preceded by whitespace
            // Also ensure it's followed by whitespace or end of text
            let is_valid_room_mention = {
                let preceded_ok = absolute_pos == 0 ||
                    text.chars().nth(absolute_pos.saturating_sub(1))
                        .is_some_and(|c| c.is_whitespace());

                let followed_ok = {
                    let room_end = absolute_pos + 5; // "@room".len() = 5
                    room_end >= text.len() ||
                    text.chars().nth(room_end)
                        .is_none_or(|c| c.is_whitespace() || c.is_ascii_punctuation())
                };

                preceded_ok && followed_ok
            };

            if is_valid_room_mention {
                self.possible_room_mention = true;
            }

            // Continue searching from next character to find multiple @room mentions
            pos = absolute_pos + 1;
        }

        // === Part 2: Find all user mention patterns ===
        // Search pattern: "](matrix:u/" which is part of [displayname](matrix:u/@userid:server.com)
        pos = 0;
        let mut user_mention_count = 0;
        const MAX_USER_MENTIONS: usize = 100;  // Prevent excessive processing

        while let Some(found_pos) = text[pos..].find("](matrix:u/") {
            user_mention_count += 1;
            if user_mention_count > MAX_USER_MENTIONS {
                log!("Warning: Found more than {} user mention patterns, stopping processing", MAX_USER_MENTIONS);
                break;
            }

            let link_end = pos + found_pos; // Position right before "]"

            // Bounds check
            if link_end >= text.len() {
                break;
            }

            // Find the corresponding opening bracket by searching backwards
            if let Some(bracket_start) = text[..link_end].rfind('[') {
                // Validate bracket pairing - ensure reasonable distance
                if link_end.saturating_sub(bracket_start) > 256 {  // Display name too long
                    pos = link_end + 1;
                    continue;
                }

                // Validate: mention must be at text start OR preceded by whitespace
                let is_valid_user_mention = bracket_start == 0 ||
                    text.chars().nth(bracket_start.saturating_sub(1))
                        .is_some_and(|c| c.is_whitespace());

                if is_valid_user_mention {
                    // Extract user ID from the URL part
                    let search_start = link_end + "](matrix:u/".len();

                    // Bounds check for user ID search
                    if search_start >= text.len() {
                        pos = link_end + 1;
                        continue;
                    }

                    if let Some(user_id_start_rel) = text[search_start..].find("@") {
                        let user_id_start_abs = search_start + user_id_start_rel;

                        // Bounds check
                        if user_id_start_abs >= text.len() {
                            pos = link_end + 1;
                            continue;
                        }

                        if let Some(user_id_end_rel) = text[user_id_start_abs..].find(")") {
                            let user_id_end_abs = user_id_start_abs + user_id_end_rel;

                            // Bounds check and length validation
                            if user_id_end_abs > text.len() ||
                               user_id_end_abs.saturating_sub(user_id_start_abs) > 256 {
                                pos = link_end + 1;
                                continue;
                            }

                            // Extract the full user ID string
                            let user_id_str = &text[user_id_start_abs..user_id_end_abs];

                            // Validate user ID format before parsing
                            if !user_id_str.is_empty() &&
                               user_id_str.len() <= 255 &&
                               user_id_str.contains(':') &&
                               !user_id_str.contains('\n') &&
                               !user_id_str.contains('\r') {

                                // Try to parse as a valid Matrix user ID
                                match user_id_str.try_into() {
                                    Ok(user_id) => {
                                        self.possible_mentions.insert(user_id, user_id_str.to_string());
                                    }
                                    Err(e) => {
                                        log!("Warning: Failed to parse user ID '{}': {}", user_id_str, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Continue searching from after the current pattern
            pos = link_end + 1;
        }

        // === Part 3: Find matrix.to user mentions ===
        // Search pattern: "](https://matrix.to/#/@" which is part of [displayname](https://matrix.to/#/@userid:server.com)
        pos = 0;
        user_mention_count = 0;

        while let Some(found_pos) = text[pos..].find("](https://matrix.to/#/@") {
            user_mention_count += 1;
            if user_mention_count > MAX_USER_MENTIONS {
                log!("Warning: Found more than {} matrix.to user mention patterns, stopping processing", MAX_USER_MENTIONS);
                break;
            }

            let link_end = pos + found_pos; // Position right before "]"

            // Bounds check
            if link_end >= text.len() {
                break;
            }

            // Find the corresponding opening bracket by searching backwards
            if let Some(bracket_start) = text[..link_end].rfind('[') {
                // Validate bracket pairing - ensure reasonable distance
                if link_end.saturating_sub(bracket_start) > 256 {  // Display name too long
                    pos = link_end + 1;
                    continue;
                }

                // Validate: mention must be at text start OR preceded by whitespace
                let is_valid_user_mention = bracket_start == 0 ||
                    text.chars().nth(bracket_start.saturating_sub(1))
                        .is_some_and(|c| c.is_whitespace());

                if is_valid_user_mention {
                    // Extract user ID from the URL part
                    let search_start = link_end + "](https://matrix.to/#/@".len();

                    // Bounds check for user ID search
                    if search_start >= text.len() {
                        pos = link_end + 1;
                        continue;
                    }

                    if let Some(user_id_end_rel) = text[search_start..].find(")") {
                        let user_id_end_abs = search_start + user_id_end_rel;

                        // Bounds check and length validation
                        if user_id_end_abs > text.len() ||
                           user_id_end_abs.saturating_sub(search_start) > 256 {
                            pos = link_end + 1;
                            continue;
                        }

                        // Extract the full user ID string (already includes @)
                        let user_id_str = &text[search_start..user_id_end_abs];

                        // Validate user ID format before parsing
                        if !user_id_str.is_empty() &&
                           user_id_str.len() <= 255 &&
                           user_id_str.contains(':') &&
                           !user_id_str.contains('\n') &&
                           !user_id_str.contains('\r') {

                            // Parse the user ID
                            if let Ok(user_id) = matrix_sdk::ruma::UserId::parse(user_id_str) {
                                // Extract display name
                                let display_name = utils::safe_substring_by_byte_indices(
                                    &text,
                                    bracket_start + 1,
                                    link_end
                                );

                                // Add to possible mentions
                                self.possible_mentions.insert(user_id, display_name.to_string());
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

