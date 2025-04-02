//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
//! TODO for the future:
//!   1. Add a header to the user list to display the current number of users in the room.
//!   2. Implement scrolling functionality for the user list.
//!   3. Enable sorting for the user list to show currently online users.
//!   4. Optimize performance and add a loading animation for the user list.
//!   5. @Room
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::utils;

use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;

    // Template for user list items in the mention dropdown
    UserListItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 8., right: 8., top: 4., bottom: 4.}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: #fff,
            uniform radius: 6.0,
            instance hover: 0.0,
            instance selected: 0.0

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                // Draw rounded rectangle with configurable radius
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.radius);

                if self.selected > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR)
                } else if self.hover > 0.0 {
                    sdf.fill(KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR)
                } else {
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

            label = <Label> {
                height: Fit,
                draw_text: {
                    color: #000,
                    text_style: {font_size: 14.0}
                }
            }

            filler = <FillerX> {}
        }

        matrix_url = <Label> {
            height: Fit,
            draw_text: {
                color: #666,
                text_style: {font_size: 12.0}
            }
        }
    }

    pub MentionableTextInput = {{MentionableTextInput}} {
        width: Fill,
        height: Fit
        trigger: "@"
        inline_search: true

        keyboard_focus_color: (KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR),
        pointer_hover_color: (KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR)

        popup = {
            spacing: 0.0
            padding: 0.0

            header_view = {
                header_label = {
                    text: "Users List"
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
                    empty_message: "Start typing..."
                    draw_bg: {
                        color: (COLOR_PRIMARY)
                        instance radius: 2.0
                        instance border_width: 0.0
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
                                self.inset.x + self.border_width,
                                self.inset.y + self.border_width,
                                self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                                self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                                max(1.0, self.radius)
                            )
                            sdf.fill_keep(self.get_color())
                            if self.border_width > 0.0 {
                                sdf.stroke(self.get_border_color(), self.border_width)
                            }
                            return sdf.result
                        }
                    }

                    draw_text: {
                        color: (MESSAGE_TEXT_COLOR)
                        text_style: <MESSAGE_TEXT_STYLE>{}
                        fn get_color(self) -> vec4 {
                            return mix(self.color, #B, self.is_empty)
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
    }
}

/// Actions emitted by the MentionableTextInput component
#[allow(dead_code)]
#[derive(Clone, Debug, DefaultNone)]
pub enum MentionableTextInputAction {
    /// Room members list has been updated
    RoomMembersUpdated(Arc<Vec<RoomMember>>),
    /// Room ID has been updated (new)
    RoomIdChanged(OwnedRoomId),
    /// Default empty action
    None,
}

/// Widget that extends CommandTextInput with @mention capabilities
#[derive(Live, LiveHook, Widget)]
pub struct MentionableTextInput {
    /// Base command text input
    #[deref]
    view: CommandTextInput,
    /// Template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    /// List of available room members for mentions
    #[rust]
    room_members: Arc<Vec<RoomMember>>,
    /// Position where the @ mention starts
    #[rust]
    mention_start_index: Option<usize>,
    /// Indicates if currently in mention search mode
    #[rust]
    is_searching: bool,
    #[rust]
    room_id: Option<OwnedRoomId>,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if let Some(selected) = self.view.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                return;
            }

            if self.view.should_build_items(actions) {
                let search_text = self.view.search_text().to_lowercase();
                self.update_user_list(cx, &search_text);
            }

            if let Some(action) =
                actions.find_widget_action(self.view.text_input_ref().widget_uid())
            {
                if let TextInputAction::Change(text) = action.cast() {
                    self.handle_text_change(cx, scope, text);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MentionableTextInput {
    // Handles user selection from mention popup
    fn on_user_selected(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        let username = selected.label(id!(user_info.label)).text();

        if let Some(start_idx) = self.mention_start_index {
            let current_text = self.view.text();
            let head = self.view.text_input_ref().borrow().map_or(0, |p| p.get_cursor().head.index);

            // Use utility function to safely replace text
            let mention = utils::safe_replace_by_byte_indices(
                &current_text,
                start_idx,
                head,
                &format!("@{username} ")
            );

            self.view.set_text(cx, &mention);

            // Calculate new cursor position
            let before_and_insert = format!("{}@{} ",
                &current_text[..start_idx],
                username
            );
            let new_pos = before_and_insert.len();

            self.view.text_input_ref().set_cursor(new_pos, new_pos);

        }

        self.close_mention_popup(cx);
    }

    // Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, scope: &mut Scope, text: String) {
        let cursor_pos =
            self.view.text_input_ref().borrow().map_or(0, |p| p.get_cursor().head.index);

        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            self.mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            let search_text = utils::safe_substring_by_byte_indices(
                &text,
                trigger_pos + 1,
                cursor_pos
            ).to_lowercase();

            self.update_user_list(cx, &search_text);
            self.view.view(id!(popup)).set_visible(cx, true);
        } else if self.is_searching {
            self.close_mention_popup(cx);
        }
    }

    // Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str) {
        self.view.clear_items();

        if self.is_searching {
            let is_desktop = cx.display_context.is_desktop();
            let mut matched_members = Vec::new();

            for member in self.room_members.iter() {
                let display_name = member
                    .display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| member.user_id().to_string());

                if display_name.to_lowercase().contains(search_text) {
                    matched_members.push((display_name, member));
                }
            }

            let member_count = matched_members.len();
            const MAX_VISIBLE_ITEMS: usize = 15;
            let popup = self.view.view(id!(popup));

            if member_count == 0 {
                popup.apply_over(cx, live! { height: Fit });
                self.view.view(id!(popup)).set_visible(cx, false);
                return;
            }

            let header_view = self.view.view(id!(popup.header_view));

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

            if member_count <= MAX_VISIBLE_ITEMS {
                let single_item_height = if is_desktop { 32.0 } else { 64.0 };
                let total_height =
                    (member_count as f64 * single_item_height) + header_height + estimated_spacing;
                popup.apply_over(cx, live! { height: (total_height) });
            } else {
                let max_height = if is_desktop { 400.0 } else { 480.0 };
                popup.apply_over(cx, live! { height: (max_height) });
            }

            for (index, (display_name, member)) in matched_members.into_iter().enumerate() {
                let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                item.label(id!(user_info.label)).set_text(cx, &display_name);

                // Use the full user ID string
                let user_id_str = member.user_id().as_str();
                item.label(id!(matrix_url)).set_text(cx, user_id_str);

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
                        avatar.show_text(cx, None, &display_name);
                    }
                } else {
                    avatar.show_text(cx, None, &display_name);
                }

                self.view.add_item(item.clone());

                if index == 0 {
                    self.view.set_keyboard_focus_index(0);
                }
            }

            self.view.view(id!(popup)).set_visible(cx, true);
            if self.is_searching {
                self.view.text_input_ref().set_key_focus(cx);
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

        // Check if cursor is immediately after @ symbol
        if cursor_grapheme_idx > 0 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            return Some(byte_positions[cursor_grapheme_idx - 1]);
        }

        // Find the last @ symbol before the cursor
        let last_at_pos = text_graphemes[..cursor_grapheme_idx]
            .iter()
            .enumerate()
            .filter(|(_, g)| **g == "@")
            .map(|(i, _)| i)
            .last();

        if let Some(at_idx) = last_at_pos {
            // Extract the text after the @ symbol up to the cursor position
            let mention_text = &text_graphemes[at_idx + 1..cursor_grapheme_idx];

            // Validate the mention format
            if self.is_valid_mention_text(mention_text) {
                return Some(byte_positions[at_idx]);
            }
        }

        None
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

        // Check if it contains newline characters
        !graphemes.iter().any(|g| g.contains('\n'))
    }

    // Cleanup helper for closing mention popup
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        self.mention_start_index = None;
        self.is_searching = false;

        self.view.view(id!(popup)).set_visible(cx, false);
        self.view.request_text_input_focus();
        self.redraw(cx);
    }

    /// Returns the current text content
    pub fn text(&self) -> String {
        self.view.text_input_ref().text()
    }

    /// Sets the text content
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.view.text_input_ref().set_text(cx, text);
        self.redraw(cx);
    }

    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());

        // Send the room ID changed event to widget listeners
        Cx::post_action(MentionableTextInputAction::RoomIdChanged(room_id));
    }

    pub fn get_room_id(&self) -> Option<OwnedRoomId> {
        self.room_id.clone()
    }

    /// Sets room members for mention suggestions
    pub fn set_room_members(&mut self, members: Arc<Vec<RoomMember>>) {
        self.room_members = members;
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

    /// Sets the room members for this text input
    pub fn set_room_members(&self, members: Arc<Vec<RoomMember>>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(members);
        }
    }

    pub fn set_room_id(&self, room_id: OwnedRoomId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_id(room_id);
        }
    }

    pub fn get_room_id(&self) -> Option<OwnedRoomId> {
        self.borrow().and_then(|inner| inner.get_room_id())
    }
}
