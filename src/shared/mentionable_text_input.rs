//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
//! TODO for the future:
//!   1. Is it not possible to mention (@) yourself ?
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::typing_animation::TypingAnimationWidgetRefExt;
use crate::utils;

use makepad_widgets::{text::selection::Cursor, *};
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
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

    pub FOCUS_HOVER_COLOR = #eaecf0
    pub KEYBOARD_FOCUS_OR_COLOR_HOVER = #1C274C

    // Template for user list items in the mention dropdown
    UserListItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: (COLOR_PRIMARY),
            uniform border_radius: 6.0,
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
        padding: {left: 8, right: 8, top: 4, bottom: 4}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: (COLOR_PRIMARY),
            uniform border_radius: 6.0,
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
        padding: {left: 16, right: 16, top: 16, bottom: 16},
        flow: Right,
        spacing: 8.0,
        align: {x: 0., y: 0.5}

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
                color: #000,
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
                color: (COLOR_PRIMARY)
                border_size: 1.0
                border_color: #D0D5DD
                border_radius: 8.0
            }

            header_view = {
                header_label = {
                    text: "Users in this Room"
                    draw_text: {
                        color: #000
                    }
                }
                draw_bg: {
                    color: #D0D5DD
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
            center = {
                text_input = {
                    empty_text: "Start typing..."
                    draw_bg: {
                        color: (COLOR_PRIMARY)
                        instance border_radius: 2.0
                        instance border_size: 0.0
                        instance border_color: #D0D5DD
                        instance inset: vec4(0.0, 0.0, 0.0, 0.0)

                        fn get_color(self) -> vec4 {
                            return self.color
                        }

                        fn get_border_color(self) -> vec4 {
                            return self.border_color
                        }

                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                            sdf.box(
                                self.inset.x + self.border_size,
                                self.inset.y + self.border_size,
                                self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                                self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                                max(1.0, self.border_radius)
                            )
                            sdf.fill_keep(self.get_color())
                            if self.border_size > 0.0 {
                                sdf.stroke(self.get_border_color(), self.border_size)
                            }
                            return sdf.result
                        }
                    }

                    draw_text: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: <MESSAGE_TEXT_STYLE>{}
                        fn get_color(self) -> vec4 {
                            return mix(self.color, #B, 0.0)
                        }
                    }

                    draw_cursor: {
                        instance focus: 0.0
                        uniform border_radius: 0.5
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius)
                            sdf.fill(mix(#fff, #bbb, self.focus));
                            return sdf.result
                        }
                    }

                    draw_selection: {
                        instance hover: 0.0
                        instance focus: 0.0
                        uniform border_radius: 2.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius)
                            sdf.fill(mix(#eee, #ddd, self.focus));
                            return sdf.result
                        }
                    }
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

/// Information about mentions in the text input.
#[derive(Clone, Debug, Default)]
pub struct MentionInfo {
    /// The set of user IDs that were explicitly mentioned by selecting from the list.
    pub user_ids: BTreeSet<OwnedUserId>,
    /// Whether the `@room` option was explicitly selected.
    pub room: bool,
}

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
    /// Current room ID
    #[rust] room_id: Option<OwnedRoomId>,
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
                        log!("close_mention_popup 1");
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

                        // 只有当文本中不再包含 "@room" 时才重置 possible_room_mention
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
                                // First check if any mention markers have been deleted
                                // If no "[" or "](", the user may have deleted all user mentions
                                if !text.contains('[') || !text.contains("](") {
                                    // Clear all possible user mentions
                                    self.possible_mentions.clear();

                                    // 只有当文本中不再包含 "@room" 时才重置 possible_room_mention
                                    if !text.contains("@room") {
                                        self.possible_room_mention = false;
                                    }
                                }

                                // Simplified detection logic
                                // Core issue is popup header appearing when backspace deleting text after selecting non-room user
                                // Simplest solution: always close menu when markdown link features are detected
                                // This way user won't see menu at any point during link deletion

                                // Modification: only close menu in specific situations
                                // Don't close all text containing link format, this would prevent consecutive @mentions

                                // Only check character before cursor to determine if deleting a link
                                let cursor_pos = self.cmd_text_input.text_input_ref().borrow().map_or(0, |p| p.cursor().index);
                                let is_deleting_link = if cursor_pos > 0 && cursor_pos <= text.len() {
                                    // 使用字形处理而不是直接的字节索引
                                    let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(&text, cursor_pos);
                                    let text_graphemes: Vec<&str> = text.graphemes(true).collect();

                                    if cursor_grapheme_idx > 0 && cursor_grapheme_idx <= text_graphemes.len() {
                                        let prev_grapheme = text_graphemes[cursor_grapheme_idx - 1];
                                        // Only close menu when cursor is right after a right parenthesis or right bracket
                                        prev_grapheme == ")" || prev_grapheme == "]"
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };

                                if is_deleting_link {
                                    // Only close menu when cursor is before right parenthesis or right bracket
                                    if self.is_searching {
                                        log!("close_mention_popup 2");
                                        self.close_mention_popup(cx);

                                        // If text still contains @ symbol, keep link features but don't trigger menu
                                        if text.contains('@') {
                                            // Update text state but don't show menu
                                            self.cmd_text_input.text_input_ref().set_text(cx, &text);
                                            break;
                                        }
                                    }
                                }

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
                            let scope_room_id = scope.props.get::<RoomScreenProps>().map(|props| &props.room_id);

                            // If Scope has room_id and doesn't match action's room_id, this action may be for another room
                            // This is important to avoid applying actions to the wrong room context
                            if let Some(scope_id) = scope_room_id {
                                if scope_id != room_id {
                                    log!("MentionableTextInput({:?}) ignoring PowerLevelsUpdated because scope room_id ({}) doesn't match action room_id ({})",
                                        self.widget_uid(), scope_id, room_id);
                                    continue; // Skip this action
                                }
                            }

                            // If Scope has no room_id (edge case), fall back to checking internal component state
                            // This should rarely happen with proper Scope usage
                            if scope_room_id.is_none() {
                                if let Some(internal_id) = &self.room_id {
                                    if internal_id != room_id {
                                        log!("MentionableTextInput({:?}) ignoring PowerLevelsUpdated because internal room_id ({}) doesn't match action room_id ({})",
                                            self.widget_uid(), internal_id, room_id);
                                        continue; // Skip this action
                                    }
                                }
                            }

                            // After validation, we can update component state
                            log!("MentionableTextInput({:?}) received valid PowerLevelsUpdated for room {}: can_notify={}",
                                self.widget_uid(), room_id, can_notify_room);

                            // If internal room_id is not set or doesn't match action, update it
                            // Note: Prioritize room_id from Scope to maintain consistency with parent widgets
                            if self.room_id.as_ref() != Some(room_id) {
                                self.room_id = Some(room_id.clone());
                                log!("MentionableTextInput({:?}) updated internal room_id to {}", self.widget_uid(), room_id);
                            }

                            // Only update and possibly redraw when can_notify_room state actually changes
                            if self.can_notify_room != *can_notify_room {
                                self.can_notify_room = *can_notify_room;
                                log!("MentionableTextInput({:?}) updated can_notify_room to {}", self.widget_uid(), can_notify_room);

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
                            let scope_room_id = scope.props.get::<RoomScreenProps>().map(|props| &props.room_id);

                            // Only process if this action is for the current room
                            if let Some(scope_id) = scope_room_id {
                                if scope_id == room_id {
                                    log!("MentionableTextInput({:?}) received RoomMembersLoaded for room {}",
                                         self.widget_uid(), room_id);

                                    // If we were showing loading, hide it and refresh the list
                                    if self.members_loading {
                                        log!("MentionableTextInput: Stopping loading state due to members loaded");
                                        self.members_loading = false;

                                        // If currently searching, refresh the user list
                                        if self.is_searching && has_focus {
                                            let search_text = self.cmd_text_input.search_text().to_lowercase();
                                            self.update_user_list(cx, &search_text, scope);
                                        }
                                    }
                                }
                            }
                        },
                        _ => {},
                    }
                }
            }

            if !has_focus && self.cmd_text_input.view(id!(popup)).visible() {
                    log!("close_mention_popup 3");
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
            // Improved logic for detecting @room selection, not relying on whether user_id is empty
            // Check if current selected item is an @room item
            // Directly check the generic text label
            let mut is_room_mention_detected = false;

            // Try checking for label with "@room" text content
            let inner_label = selected.label(id!(room_mention)); // Default label
            if inner_label.text() == "@room" {
                is_room_mention_detected = true;
            }

            // If above method fails, fall back to comparing widget_uid or checking if user_id is empty
            let is_room_mention = if !is_room_mention_detected {
                log!("Falling back to alternative @room detection methods");

                // Method 2: Check if user_id label exists - user items have it, @room items don't
                let has_user_id = selected.label(id!(user_id)).text().len() > 0;

                !has_user_id
            } else {
                true
            };

            log!("Item selected is_room_mention: {}", is_room_mention);

            let mention_to_insert = if is_room_mention {
                // User selected @room
                log!("User selected @room mention");
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
                log!("User selected mention: {} ({}))", username, user_id);
                // 不再重置 self.possible_room_mention，保留之前选择的 @room 状态
                // 允许同时有 @room 和 @user 提及

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
        log!("close_mention_popup 4");
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
                log!("close_mention_popup 5");
                self.close_mention_popup(cx);
            }
            return;
        }

        // Check if text is too short - a full mention with markdown link requires at least 6 chars
        // "[USERNAME](matrix_to_uri)"
        if trimmed_text.len() < 6 {
            self.possible_mentions.clear();

            // Special handling: if text is short and currently searching, close menu when:
            if self.is_searching && !trimmed_text.contains('@') {
                // No @ symbol means nothing to search for
                log!("close_mention_popup 6");
                self.close_mention_popup(cx);
                return;
            }
        }

        let cursor_pos = self.cmd_text_input.text_input_ref().borrow().map_or(0, |p| p.cursor().index);

        // Early returns for various conditions where popup shouldn't be shown:

        // 1. Check if cursor is within a markdown link
        if self.is_cursor_within_markdown_link(&text, cursor_pos) {
            if self.is_searching {
                log!("close_mention_popup 7 - cursor within markdown link");
                self.close_mention_popup(cx);
            }
            return;
        }

        // 2. Check if user is deleting a link (cursor after ] or ) character)
        let is_deleting_link = cursor_pos > 0 &&
                               cursor_pos <= text.len() &&
                               {

                                   let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(&text, cursor_pos);
                                   let text_graphemes: Vec<&str> = text.graphemes(true).collect();
                                   if cursor_grapheme_idx > 0 && cursor_grapheme_idx <= text_graphemes.len() {
                                       let prev_grapheme = text_graphemes[cursor_grapheme_idx - 1];
                                       prev_grapheme == ")" || prev_grapheme == "]"
                                   } else {
                                       false
                                   }
                               };

        if is_deleting_link {
            if self.is_searching {
                log!("close_mention_popup 8 - deleting link");
                self.close_mention_popup(cx);
            }
            return;
        }

        // 3. Check if cursor is inside "@room" text
        // This allows user to continue @mentioning others after @room
        if text.contains("@room") {
            for room_pos in text.match_indices("@room").map(|(i, _)| i) {
                let end_pos = room_pos + 5; // "@room" length is 5

                // Only close menu when cursor is inside @room
                // If cursor is after @room, allow continuing to @ other users
                if cursor_pos > room_pos && cursor_pos <= end_pos {
                    if self.is_searching {
                        log!("close_mention_popup 9 - cursor inside @room");
                        self.close_mention_popup(cx);
                    }
                    return;
                }
            }
        }

        // 4. Check if cursor is inside an existing markdown link when typing @ symbol
        if text.contains('[') && text.contains("](") && text.contains(')') && text.contains('@') {
            // Find all markdown link ranges
            let link_ranges = self.find_markdown_link_ranges(&text);

            // Check if cursor is within any link range
            let in_any_link = link_ranges.iter().any(|(start, end)|
                cursor_pos >= *start && cursor_pos <= *end
            );

            if in_any_link {
                // If cursor is inside a link, close popup
                if self.is_searching {
                    log!("close_mention_popup 10 - cursor inside a link");
                    self.close_mention_popup(cx);
                }
                return;
            }
        }

        // Continue with the remaining parts of text change handling
        self.handle_text_change_continued(cx, scope, &text, cursor_pos);
    }

    // Helper method to find all markdown link ranges in text
    fn find_markdown_link_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        let mut link_ranges = Vec::new();
        let mut open_bracket_pos = None;

        let text_graphemes: Vec<&str> = text.graphemes(true).collect();
        let byte_positions = utils::build_grapheme_byte_positions(text);

        for (i, g) in text_graphemes.iter().enumerate() {
            if *g == "[" && i < byte_positions.len() {
                open_bracket_pos = Some(byte_positions[i]);
            } else if *g == "]" {
                if let Some(open) = open_bracket_pos {
                    if i + 1 < text_graphemes.len() && text_graphemes[i + 1] == "(" {
                        for j in i + 2..text_graphemes.len() {
                            if text_graphemes[j] == ")" && j < byte_positions.len() {
                                link_ranges.push((open, byte_positions[j] + 1));
                                break;
                            }
                        }
                    }
                    open_bracket_pos = None;
                }
            }
        }

        link_ranges
    }

    // Continues the handle_text_change method with trigger position detection
    fn handle_text_change_continued(&mut self, cx: &mut Cx, scope: &mut Scope, text: &str, cursor_pos: usize) {
        // Look for trigger position for @ menu
        if let Some(trigger_pos) = self.find_mention_trigger_position(text, cursor_pos) {
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
                    log!("close_mention_popup 11");
                    self.close_mention_popup(cx);
                }
                return;
            }

            self.current_mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            let search_text = utils::safe_substring_by_byte_indices(
                text,
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
            log!("close_mention_popup 12");
            self.close_mention_popup(cx);
        }
    }

    // Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // 1. Get Props and check Scope validity
        let Some(room_props) = scope.props.get::<RoomScreenProps>() else {
            log!("MentionableTextInput::update_user_list: RoomScreenProps not found in scope. Clearing list.");
            self.cmd_text_input.clear_items(); // Clear list since there's no valid data source
            self.cmd_text_input.view(id!(popup)).set_visible(cx, false); // Hide popup
            self.redraw(cx);
            return;
        };

        // 2. Check if internal room_id is already set (should be set by PowerLevelsUpdated)
        if self.room_id.is_none() {
            // If internal room_id is not set, initialize it from current scope
            log!("MentionableTextInput: Initializing internal room_id from scope: {}", room_props.room_id);
            self.room_id = Some(room_props.room_id.clone());
        }

        // 3. Core check: Does current scope's room_id match component's internal room_id
        // This is crucial for proper Scope usage - ensure we're using the correct context
        let internal_room_id = self.room_id.as_ref().unwrap(); // Must exist at this point
        if internal_room_id != &room_props.room_id {
            log!("MentionableTextInput Warning: Scope room_id ({}) does not match internal room_id ({}). Updating internal room_id.",
                    room_props.room_id, internal_room_id);

            // Important fix: When switching rooms, update component's internal room_id to match current scope
            // This ensures we're working with the current context as passed through Scope, rather than stale state
            self.room_id = Some(room_props.room_id.clone());

            // Clear current list, prepare to update with new room's members
            self.cmd_text_input.clear_items();
        }

        // Always use room_members provided in current scope
        // These member lists should come from TimelineUiState.room_members_map and are already the correct list for current room
        let room_members = &room_props.room_members;

        // 4. Check if members are loaded or still loading
        let members_are_empty = room_members.is_empty();
        log!("MentionableTextInput: members_are_empty={}, members_loading={}, room_members.len()={}",
             members_are_empty, self.members_loading, room_members.len());

        if members_are_empty && !self.members_loading {
            // Members list is empty and we're not already showing loading - start loading state
            log!("MentionableTextInput: Room members list is empty, showing loading indicator");
            self.members_loading = true;
            self.show_loading_indicator(cx);
            return;
        } else if !members_are_empty && self.members_loading {
            // Members have been loaded, stop loading state
            log!("MentionableTextInput: Room members loaded ({} members), hiding loading indicator", room_members.len());
            self.members_loading = false;
        } else if members_are_empty && self.members_loading {
            // Still loading and members are empty - keep showing loading indicator
            log!("MentionableTextInput: Still waiting for room members to load");
            return;
        }

        // Clear old list items, prepare to populate new list
        self.cmd_text_input.clear_items();

        if self.is_searching {
            let is_desktop = cx.display_context.is_desktop();
            let mut matched_members = Vec::new();
            let red_color_vec4 = Some(vec4(1.0, 0.2, 0.2, 1.0));

            // Add @room option if search text matches "@room" or is empty and user has permission
            log!("Checking @room permission. Can notify: {}, search_text: {}", self.can_notify_room, search_text);
            if self.can_notify_room && ("@room".contains(&search_text) || search_text.is_empty()) {
                log!("Adding @room option to mention list");
                let room_mention_item = match self.room_mention_list_item {
                    Some(ptr) => WidgetRef::new_from_ptr(cx, Some(ptr)),
                    None => {
                        log!("Error: room_mention_list_item pointer is None");
                        return;
                    }
                };
                let mut room_avatar_shown = false;

                // Set up room avatar
                let avatar_ref = room_mention_item.avatar(id!(room_avatar));

                // First try to get room avatar from current room Props
                // Use self.room_id instead of room_props.room_id to ensure getting correct room avatar
                // When switching rooms, self.room_id has already been updated to match room_props.room_id in previous code
                if let Some(room_id) = self.room_id.as_ref() {
                    if let Some(client) = get_client() {
                        if let Some(room) = client.get_room(room_id) {
                            if let Some(avatar_url) = room.avatar_url() {
                                log!("Found room avatar URL for @room: {}", avatar_url);

                                match get_or_fetch_avatar(cx, avatar_url.to_owned()) {
                                    AvatarCacheEntry::Loaded(avatar_data) => {
                                        // Display room avatar
                                        let result = avatar_ref.show_image(cx, None, |cx, img| {
                                            utils::load_png_or_jpg(&img, cx, &avatar_data)
                                        });
                                        if result.is_ok() {
                                            room_avatar_shown = true;
                                            log!("Successfully showed @room avatar with room avatar image");
                                        } else {
                                            log!("Failed to show @room avatar with room avatar image");
                                        }
                                    },
                                    AvatarCacheEntry::Requested => {
                                        log!("Room avatar was requested for @room but not loaded yet");
                                        // 临时显示文字"R"
                                        avatar_ref.show_text(cx, red_color_vec4, None, "R");
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
                } else {
                    log!("No room_id available for @room avatar");
                }

                // If unable to display room avatar, show letter R with red background
                if !room_avatar_shown {
                    avatar_ref.show_text(cx, red_color_vec4, None, "R");
                }


                self.cmd_text_input.add_item(room_mention_item);
            }

            // Limit the number of matched members to avoid performance issues
            const MAX_MATCHED_MEMBERS: usize = MAX_VISIBLE_ITEMS * 2;  // Buffer for better UX

            // First pass: find exact prefix matches (better UX)
            for member in room_members.iter() {
                if matched_members.len() >= MAX_MATCHED_MEMBERS {
                    break;
                }

                let display_name = member
                    .display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| member.user_id().to_string());

                let display_name_lower = display_name.to_lowercase();
                if display_name_lower.starts_with(search_text) {
                    matched_members.push((display_name, member));
                }
            }

            // Second pass: find contains matches if we have room for more
            if matched_members.len() < MAX_MATCHED_MEMBERS {
                for member in room_members.iter() {
                    if matched_members.len() >= MAX_MATCHED_MEMBERS {
                        break;
                    }

                    let display_name = member
                        .display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| member.user_id().to_string());

                    let display_name_lower = display_name.to_lowercase();
                    // Skip if already matched in first pass
                    if !display_name_lower.starts_with(search_text) && display_name_lower.contains(search_text) {
                        matched_members.push((display_name, member));
                    }
                }
            }

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
            let total_items_in_list = member_count + if "@room".contains(&search_text) { 1 } else { 0 };

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

                item.apply_over(cx, live! {
                    show_bg: true,
                    cursor: Hand,
                    padding: {left: 8., right: 8., top: 4., bottom: 4.}
                });

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
                if index == 0 && !"@room".contains(&search_text) { // If @room was added, it's the first item
                    self.cmd_text_input.set_keyboard_focus_index(1); // Focus the first user if @room is index 0
                } else if index == 0 && "@room".contains(&search_text) {
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

        // First check for markdown link ranges in the text
        let mut link_ranges = Vec::new();
        let mut open_bracket_pos = None;

        for (i, g) in text_graphemes.iter().enumerate() {
            if *g == "[" && i < byte_positions.len() {
                open_bracket_pos = Some(byte_positions[i]);
            } else if *g == "]" {
                if let Some(open) = open_bracket_pos {
                    if i + 1 < text_graphemes.len() && text_graphemes[i + 1] == "(" {
                        for j in i + 2..text_graphemes.len() {
                            if text_graphemes[j] == ")" && j < byte_positions.len() {
                                link_ranges.push((open, byte_positions[j] + 1));
                                break;
                            }
                        }
                    }
                    open_bracket_pos = None;
                }
            }
        }

        // Check if current cursor or @ symbol is within any link range
        let cursor_in_any_link = link_ranges.iter().any(|(start, end)|
            cursor_pos >= *start && cursor_pos <= *end
        );

        // If cursor is inside a link, don't trigger menu
        if cursor_in_any_link {
            return None;
        }

        // Check if @ symbol before cursor is inside a link
        if cursor_grapheme_idx > 0 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            if cursor_grapheme_idx - 1 < byte_positions.len() {
                let at_byte_pos = byte_positions[cursor_grapheme_idx - 1];
                let at_in_any_link = link_ranges.iter().any(|(start, end)|
                    at_byte_pos >= *start && at_byte_pos <= *end
                );

                if at_in_any_link {
                    return None;
                }
            }
        }

        // Check if inside a markdown link - using more robust detection function
        if self.is_cursor_within_markdown_link(text, cursor_pos) {
            return None;
        }

        // Check if cursor is near "@room", if so should not trigger mention menu
        // Also need to handle "@room " case (with space)
        if cursor_grapheme_idx >= 5 {
            // Check if exactly "@room"
            let possible_room_mention = text_graphemes[cursor_grapheme_idx-5..cursor_grapheme_idx].join("");
            if possible_room_mention == "@room" {
                return None;
            }

            // Check if "@room " with space
            if cursor_grapheme_idx >= 6 {
                let possible_room_with_space = text_graphemes[cursor_grapheme_idx-6..cursor_grapheme_idx-1].join("");
                let last_char = text_graphemes[cursor_grapheme_idx-1];
                if possible_room_with_space == "@room" && last_char.trim().is_empty() {
                    return None;
                }
            }
        }

        // Special handling: Only prevent menu display when user is deleting @room
        // Allow user to continue @mentioning others after @room
        let before_cursor = text_graphemes[..cursor_grapheme_idx].join("");
        // Check if text contains only "@room" with no other content
        if before_cursor.trim() == "@room" {
            return None;
        }

        // Check if cursor is at space after @room, also indicating possible @room deletion
        if cursor_grapheme_idx > 5 {
            let last_five = text_graphemes[cursor_grapheme_idx-5..cursor_grapheme_idx].join("");
            let is_at_room_space = last_five == "@room" &&
                                    cursor_grapheme_idx < text_graphemes.len() &&
                                    text_graphemes[cursor_grapheme_idx].trim().is_empty();
            if is_at_room_space {
                return None;
            }
        }

        // Check if character before cursor is ] or ), indicating user is deleting a link
        if cursor_grapheme_idx > 0 && cursor_grapheme_idx <= text_graphemes.len() {
            let prev_grapheme = text_graphemes[cursor_grapheme_idx - 1];
            if prev_grapheme == ")" || prev_grapheme == "]" {
                return None;
            }
        }

        // Check if cursor is immediately after @ symbol
        // Only trigger if @ is preceded by whitespace or beginning of text
        if cursor_grapheme_idx > 0 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1 ||
                (cursor_grapheme_idx > 1 && text_graphemes[cursor_grapheme_idx - 2].trim().is_empty());
            if is_preceded_by_whitespace_or_start && cursor_grapheme_idx - 1 < byte_positions.len() {
                return Some(byte_positions[cursor_grapheme_idx - 1]);
            }
        }

        // Special case:

        // if text only one @ symbol, and cursor is after @ symbol
        if text_graphemes.len() == 1 && text_graphemes[0] == "@" && cursor_grapheme_idx == 1 {
            return Some(byte_positions[0]);
        }

        // Detect scenarios where multiple people are mentioned consecutively
        // If there is a space before the @, this might indicate consecutive mentions
        if cursor_grapheme_idx > 1 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            let prev_char = text_graphemes[cursor_grapheme_idx - 2];
            if prev_char.trim().is_empty() && cursor_grapheme_idx - 1 < byte_positions.len() {
                // If there is a space before the @, this might indicate consecutive mentions
                return Some(byte_positions[cursor_grapheme_idx - 1]);
            }
        }

        // Find the last @ symbol before the cursor
        let last_at_pos = text_graphemes[..cursor_grapheme_idx]
            .iter()
            .enumerate()
            .filter(|(_, g)| **g == "@")
            .map(|(i, _)| i)
            .next_back();

        if let Some(at_idx) = last_at_pos {
            // Extract the text after the @ symbol up to the cursor position
            let mention_text = &text_graphemes[at_idx + 1..cursor_grapheme_idx];

            // Validate the mention format
            if self.is_valid_mention_text(mention_text) {
                // Additional check: ensure this @ is not within any link range
                // Ensure at_idx is within bounds of byte_positions
                if at_idx < byte_positions.len() {
                    let at_byte_pos = byte_positions[at_idx];
                    let at_in_any_link = link_ranges.iter().any(|(start, end)|
                        at_byte_pos >= *start && at_byte_pos <= *end
                    );

                    if !at_in_any_link {
                        return Some(byte_positions[at_idx]);
                    }
                }
            }
        }

        None
    }

    // Check if the cursor is inside a markdown link
    fn is_cursor_within_markdown_link(&self, text: &str, cursor_pos: usize) -> bool {
        let cursor_grapheme_idx = utils::byte_index_to_grapheme_index(text, cursor_pos);
        let text_graphemes: Vec<&str> = text.graphemes(true).collect();

        // First, check the simple case:
        // if the character before the cursor is ')' or ']', it means a link was just deleted
        if cursor_grapheme_idx > 0 && cursor_grapheme_idx <= text_graphemes.len() {
            let prev_grapheme = text_graphemes[cursor_grapheme_idx - 1];
            if prev_grapheme == ")" || prev_grapheme == "]" {
                return true;
            }
        }

        // Check if the cursor is inside a complete markdown link
        // Look backward for a possible starting "[", and forward for a possible ending ")"
        // First, search backward for the nearest "[" position
        let mut open_bracket_grapheme_idx = None;
        for i in (0..cursor_grapheme_idx).rev() {
            if text_graphemes[i] == "[" {
                open_bracket_grapheme_idx = Some(i);
                break;
            }
        }

        // Then, search forward for the nearest ")" position
        let mut close_paren_grapheme_idx = None;
        for i in cursor_grapheme_idx..text_graphemes.len() {
            if text_graphemes[i] == ")" {
                close_paren_grapheme_idx = Some(i);
                break;
            }
        }

        // If both a possible "[" and ")" are found, check if there is a "](" in between, indicating a complete link format
        if let (Some(open_idx), Some(close_idx)) = (open_bracket_grapheme_idx, close_paren_grapheme_idx) {
            if open_idx < close_idx {
                let link_text = text_graphemes[open_idx..=close_idx].join("");
                return link_text.contains("](");
            }
        }

        false
    }

    // Add helper method to extract validation logic
    fn is_valid_mention_text(&self, graphemes: &[&str]) -> bool {
        // Empty or first character is whitespace is invalid
        if graphemes.is_empty() || graphemes[0].trim().is_empty() {
            return false;
        }

        // Check for consecutive whitespace
        for i in 0..graphemes.len().saturating_sub(1) {
            if graphemes[i].trim().is_empty() && graphemes[i + 1].trim().is_empty() {
                return false;
            }
        }

        // Check if text contains link-characteristic characters
        // If it contains any of these characters, may be editing/deleting a link, not creating a new mention
        let text_to_check = graphemes.join("");
        if text_to_check.contains('(') || text_to_check.contains(')') ||
           text_to_check.contains('[') || text_to_check.contains(']') {
            return false;
        }

        // Don't completely block text containing "room" from triggering the menu, which would prevent @mentioning others after @room
        // Only block triggering for exact matches to "room" or "room "
        if text_to_check == "room" || text_to_check == "room " {
            return false;
        }

        // Check if it contains newline characters
        !graphemes.iter().any(|g| g.contains('\n'))
    }

    // Shows the loading indicator when members are being fetched
    fn show_loading_indicator(&mut self, cx: &mut Cx) {
        log!("MentionableTextInput: show_loading_indicator called");

        // Clear any existing items
        self.cmd_text_input.clear_items();

        // Create loading indicator widget
        let loading_item = match self.loading_indicator {
            Some(ptr) => {
                log!("MentionableTextInput: Creating loading indicator from pointer");
                WidgetRef::new_from_ptr(cx, Some(ptr))
            },
            None => {
                log!("Error: loading_indicator pointer is None");
                return;
            }
        };

        // Start the loading animation
        log!("MentionableTextInput: Starting loading animation");
        loading_item.typing_animation(id!(loading_animation)).start_animation(cx);

        // Add the loading indicator to the popup
        log!("MentionableTextInput: Adding loading item to popup");
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
        // When text is set externally (e.g., for editing), clear mention state
        self.possible_mentions.clear();
        self.possible_room_mention = false;
        self.members_loading = false; // Reset loading state when text is set
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

    /// Returns information about the mentions that were explicitly selected from the list.
    pub fn get_mention_info(&self) -> MentionInfo {
        MentionInfo {
            user_ids: self.possible_mentions.keys().cloned().collect(),
            room: self.possible_room_mention,
        }
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

    /// Returns information about the mentions that were explicitly selected from the list.
    pub fn get_mention_info(&self) -> MentionInfo {
        self.borrow().map_or_else(Default::default, |inner| inner.get_mention_info())
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

    /// Returns the list of users mentioned in the given html message content.
    pub fn get_real_mentions_in_html_text(&self, html: &str) -> BTreeSet<OwnedUserId> {
        let Some(inner) = self.borrow() else { return BTreeSet::new() };
        let mut real_mentions = BTreeSet::new();
        for (user_id, username) in &inner.possible_mentions {
            if html.contains(&format!(
                "<a href=\"{}\">{}</a>",
                user_id.matrix_to_uri(),
                username,
            )) {
                real_mentions.insert(user_id.clone());
            }
        }
        real_mentions
    }

    /// Returns the list of users mentioned in the given markdown message content.
    pub fn get_real_mentions_in_markdown_text(&self, markdown: &str) -> BTreeSet<OwnedUserId> {
        let Some(inner) = self.borrow() else { return BTreeSet::new() };
        let mut real_mentions = BTreeSet::new();
        for (user_id, username) in &inner.possible_mentions {
            if markdown.contains(&format!(
                "[{}]({})",
                username,
                user_id.matrix_to_uri(),
            )) {
                real_mentions.insert(user_id.clone());
            }
        }
        real_mentions
    }
}
