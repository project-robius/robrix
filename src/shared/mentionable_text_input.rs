//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::bouncing_dots::BouncingDotsWidgetRefExt;
use crate::shared::styles::COLOR_UNKNOWN_ROOM_AVATAR;
use crate::utils;


use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::ruma::{events::{room::message::RoomMessageEventContent, Mentions}, OwnedRoomId, OwnedUserId};
use matrix_sdk::room::RoomMember;
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;
use crate::home::room_screen::RoomScreenProps;

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
    }
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
    /// Template for no matches indicator
    #[live] no_matches_indicator: Option<LivePtr>,
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
            .expect("BUG: RoomScreenProps should be available in Scope::props for MentionableTextInput")
            .room_name_id
            .room_id()
            .clone();

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
                if let Some(MentionableTextInputAction::PowerLevelsUpdated { room_id, can_notify_room }) = action.downcast_ref() {
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
                }
            }

            // Close popup if focus is lost
            if !has_focus && self.cmd_text_input.view(ids!(popup)).visible() {
                self.close_mention_popup(cx);
            }
        }

        // Check if we were waiting for members and they're now available
        if self.members_loading && self.is_searching {
            let room_props = scope
                .props
                .get::<RoomScreenProps>()
                .expect("RoomScreenProps should be available in scope");

            if let Some(room_members) = &room_props.room_members {
                if !room_members.is_empty() {
                    // Members are now available, update the list
                    self.members_loading = false;
                    let text_input = self.cmd_text_input.text_input(ids!(text_input));
                    let text_input_area = text_input.area();
                    let is_focused = cx.has_key_focus(text_input_area);

                    if is_focused {
                        let search_text = self.cmd_text_input.search_text().to_lowercase();
                        self.update_user_list(cx, &search_text, scope);
                    }
                }
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
    fn handle_members_loading_state(
        &mut self,
        cx: &mut Cx,
        room_members: &Option<std::sync::Arc<Vec<RoomMember>>>,
    ) -> bool {
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
            let popup = self.cmd_text_input.view(ids!(popup));
            popup.apply_over(cx, live! { height: Fit });
        } else if members_are_empty && self.members_loading {
            // Still loading and members are empty - keep showing loading indicator
            return true;
        }

        false
    }

    /// Tries to add the `@room` mention item to the list of selectable popup mentions.
    ///
    /// Returns true if @room item was added to the list and will be displayed in the popup.
    fn try_search_messages_mention_item(
        &mut self,
        cx: &mut Cx,
        search_text: &str,
        room_props: &RoomScreenProps,
        is_desktop: bool,
    ) -> bool {
        if !self.can_notify_room || !("@room".contains(search_text) || search_text.is_empty()) {
            return false;
        }

        let Some(ptr) = self.room_mention_list_item else { return false };
        let room_mention_item = WidgetRef::new_from_ptr(cx, Some(ptr));
        let mut room_avatar_shown = false;

        let avatar_ref = room_mention_item.avatar(ids!(user_info.room_avatar));

        // Get room avatar fallback text from room name (with automatic ID fallback)
        let room_label = room_props.room_name_id.to_string();
        let room_name_first_char = room_label
            .graphemes(true)
            .find(|g| *g != "#" && *g != "!" && *g != "@")
            .map(|s| s.to_uppercase())
            .filter(|s| s.chars().all(|c| c.is_alphabetic()))
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

    /// Find and sort matching members based on search text
    fn find_and_sort_matching_members(
        &self,
        search_text: &str,
        room_members: &std::sync::Arc<Vec<RoomMember>>,
        max_matched_members: usize,
    ) -> Vec<(String, RoomMember)> {
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

    /// Add user mention items to the list
    /// Returns the number of items added
    fn add_user_mention_items(
        &mut self,
        cx: &mut Cx,
        matched_members: Vec<(String, RoomMember)>,
        user_items_limit: usize,
        is_desktop: bool,
    ) -> usize {
        let mut items_added = 0;

        for (index, (display_name, member)) in matched_members.into_iter().take(user_items_limit).enumerate() {
            let Some(user_list_item_ptr) = self.user_list_item else { continue };
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

    /// Update popup visibility and layout
    fn update_popup_visibility(&mut self, cx: &mut Cx, has_items: bool) {
        let popup = self.cmd_text_input.view(ids!(popup));

        if has_items {
            popup.set_visible(cx, true);
            if self.is_searching {
                self.cmd_text_input.text_input_ref().set_key_focus(cx);
            }
        } else if self.is_searching {
            // If we're searching but have no items, show "no matches" message
            // Keep the popup open so users can correct their search
            self.show_no_matches_indicator(cx);
            popup.set_visible(cx, true);
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        } else {
            // Only hide popup if we're not actively searching
            popup.apply_over(cx, live! { height: Fit });
            popup.set_visible(cx, false);
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

        if let Some(start_idx) = self.current_mention_start_index {
            let room_mention_label = selected.label(ids!(user_info.room_mention));
            let room_mention_text = room_mention_label.text();
            let room_user_id_text = selected.label(ids!(room_user_id)).text();

            let is_room_mention = { room_mention_text == "Notify the entire room" && room_user_id_text == "@room" };

            let mention_to_insert = if is_room_mention {
                // Always set to true, don't reset previously selected @room mentions
                self.possible_room_mention = true;
                "@room ".to_string()
            } else {
                // User selected a specific user
                let username = selected.label(ids!(user_info.username)).text();
                let user_id_str = selected.label(ids!(user_id)).text();
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

    /// Core text change handler that manages mention context
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
            let popup = self.cmd_text_input.view(ids!(popup));
            let header_view = self.cmd_text_input.view(ids!(popup.header_view));
            header_view.set_visible(cx, true);

            self.update_user_list(cx, &search_text, scope);
            popup.set_visible(cx, true);
        } else if self.is_searching {
            self.close_mention_popup(cx);
        }
    }

    /// Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // 1. Get Props from Scope
        let room_props = scope.props.get::<RoomScreenProps>()
            .expect("RoomScreenProps should be available in scope for MentionableTextInput");

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
        let has_room_item = self.try_search_messages_mention_item(cx, search_text, room_props, is_desktop);
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

    /// Detects valid mention trigger positions in text
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
        if cursor_grapheme_idx > 0 && text_graphemes.get(cursor_grapheme_idx - 1) == Some(&"@") {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1 ||
                (cursor_grapheme_idx > 1 && text_graphemes.get(cursor_grapheme_idx - 2).is_some_and(|g| g.trim().is_empty()));
            if is_preceded_by_whitespace_or_start {
                if let Some(&byte_pos) = byte_positions.get(cursor_grapheme_idx - 1) {
                    return Some(byte_pos);
                }
            }
        }

        // Find the last @ symbol before the cursor for search continuation
        // Only continue if we're already in search mode
        if self.is_searching {
            let last_at_pos = text_graphemes.get(..cursor_grapheme_idx)
                .and_then(|slice| slice.iter()
                    .enumerate()
                    .filter(|(_, g)| **g == "@")
                    .map(|(i, _)| i)
                    .next_back());

            if let Some(at_idx) = last_at_pos {
                // Get the byte position of this @ symbol
                let &at_byte_pos = byte_positions.get(at_idx)?;

                // Extract the text after the @ symbol up to the cursor position
                let mention_text = text_graphemes.get(at_idx + 1..cursor_grapheme_idx)
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

    /// Helper function to check if a user matches the search text
    /// Checks both display name and Matrix ID for matching
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

    /// Helper function to determine match priority for sorting
    /// Lower values = higher priority (better matches shown first)
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

    /// Shows the loading indicator when members are being fetched
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create loading indicator widget
        let Some(ptr) = self.loading_indicator else { return };
        let loading_item = WidgetRef::new_from_ptr(cx, Some(ptr));

        // Start the loading animation
        loading_item.bouncing_dots(ids!(loading_animation)).start_animation(cx);

        // Add the loading indicator to the popup
        self.cmd_text_input.add_item(loading_item);

        // Setup popup dimensions for loading state
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));

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

    /// Shows the no matches indicator when no users match the search
    fn show_no_matches_indicator(&mut self, cx: &mut Cx) {
        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create no matches indicator widget
        let Some(ptr) = self.no_matches_indicator else { return };
        let no_matches_item = WidgetRef::new_from_ptr(cx, Some(ptr));

        // Add the no matches indicator to the popup
        self.cmd_text_input.add_item(no_matches_item);

        // Setup popup dimensions for no matches state
        let popup = self.cmd_text_input.view(ids!(popup));
        let header_view = self.cmd_text_input.view(ids!(popup.header_view));

        // Ensure header is visible
        header_view.set_visible(cx, true);

        // Let popup auto-size based on content
        popup.apply_over(cx, live! { height: Fit });

        // Maintain text input focus so user can continue typing
        if self.is_searching {
            self.cmd_text_input.text_input_ref().set_key_focus(cx);
        }
    }

    /// Cleanup helper for closing mention popup
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        self.current_mention_start_index = None;
        self.is_searching = false;
        self.members_loading = false; // Reset loading state when closing popup

        // Clear list items to avoid keeping old content when popup is shown again
        self.cmd_text_input.clear_items();

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
