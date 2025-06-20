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

use makepad_widgets::text::selection::Cursor;
use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;

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
            color: #fff,
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
    }
}

// /// A special string used to denote the start of a mention within
// /// the actual text being edited.
// /// This is used to help easily locate and distinguish actual mentions
// /// from normal `@` characters.
// const MENTION_START_STRING: &str = "\u{8288}@\u{8288}";


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
    #[deref] cmd_text_input: CommandTextInput,
    /// Template for user list items
    #[live] user_list_item: Option<LivePtr>,
    /// List of available room members for mentions
    #[rust] room_members: Arc<Vec<RoomMember>>,
    /// Position where the @ mention starts
    #[rust] current_mention_start_index: Option<usize>,
    /// The set of users (user ID and display name) that were mentioned
    /// at one point in this text input.
    ///
    /// As characters may have been deleted/removed after the user-to-mention
    /// was selected, this list is a *superset* of possible users
    /// who may have been mentioned.
    /// Thus, all of these mentions may not exist in the final text input content;
    /// this is just a list of users to search the final sent message for
    /// when adding in new mentions.
    #[rust] possible_mentions: BTreeMap<OwnedUserId, String>,
    /// Indicates if currently in mention search mode
    #[rust] is_searching: bool,
    #[rust] room_id: Option<OwnedRoomId>,
}

impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.cmd_text_input.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                return;
            }

            if self.cmd_text_input.should_build_items(actions) {
                let search_text = self.cmd_text_input.search_text().to_lowercase();
                self.update_user_list(cx, &search_text);
            }

            if let Some(action) =
                actions.find_widget_action(self.cmd_text_input.text_input_ref().widget_uid())
            {
                if let TextInputAction::Changed(text) = action.cast() {
                    self.handle_text_change(cx, scope, text);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.cmd_text_input.draw_walk(cx, scope, walk)
    }
}

impl MentionableTextInput {
    // Handles user selection from mention popup
    fn on_user_selected(&mut self, cx: &mut Cx, _scope: &mut Scope, selected: WidgetRef) {
        let username = selected.label(id!(user_info.username)).text();
        let user_id_str = selected.label(id!(user_id)).text();
        let Ok(user_id): Result<OwnedUserId, _> = user_id_str.try_into() else {
            return;
        };

        if let Some(start_idx) = self.current_mention_start_index {
            let text_input_ref = self.cmd_text_input.text_input_ref();
            let current_text = text_input_ref.text();
            let head = text_input_ref.borrow().map_or(0, |p| p.cursor().index);

            // For now, we insert the markdown link to the mentioned user directly
            // instead of the user's display name because we don't yet have a way
            // to track the mentioned display name and replace it later.
            let mention_to_insert = format!(
                "[{username}]({}) ",
                user_id.matrix_to_uri(),
            );
            // let mention_to_insert = format!("@{username} ");

            self.possible_mentions.insert(user_id, username);

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
                Cursor { index: new_pos, prefer_next_row: false },
                false,
            );
        }

        self.close_mention_popup(cx);
    }

    // Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, _scope: &mut Scope, text: String) {
        // Currently an inserted mention consists of a markdown link,
        // which is "[USERNAME](matrix_to_uri)", so of course this must be at least 6 characters.
        // (In reality it has to be a lot more, but whatever...)
        if text.trim().len() < 6 {
            self.possible_mentions.clear();
        }

        let cursor_pos = self.cmd_text_input.text_input_ref().borrow().map_or(0, |p| p.cursor().index);

        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            self.current_mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            let search_text = utils::safe_substring_by_byte_indices(
                &text,
                trigger_pos + 1,
                cursor_pos
            ).to_lowercase();

            self.update_user_list(cx, &search_text);
            self.cmd_text_input.view(id!(popup)).set_visible(cx, true);
        } else if self.is_searching {
            self.close_mention_popup(cx);
        }
    }

    // Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str) {
        self.cmd_text_input.clear_items();

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
            let popup = self.cmd_text_input.view(id!(popup));

            if member_count == 0 {
                popup.apply_over(cx, live! { height: Fit });
                self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
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
                        avatar.show_text(cx, None, &display_name);
                    }
                } else {
                    avatar.show_text(cx, None, &display_name);
                }

                self.cmd_text_input.add_item(item.clone());

                if index == 0 {
                    self.cmd_text_input.set_keyboard_focus_index(0);
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
            .next_back();

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
        self.current_mention_start_index = None;
        self.is_searching = false;


        self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
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

    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map(|inner| inner.cmd_text_input.text_input_ref())
            .unwrap_or_default()
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
