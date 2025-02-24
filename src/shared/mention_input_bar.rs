//! MentionInputBar component provides a message input interface with @mention capabilities
//! Supports user mention autocomplete, avatar display, and desktop/mobile layouts
//!
//! TODO for the future:
//!   1. Add a header to the user list to display the current number of users in the room.
//!   2. Implement scrolling functionality for the user list.
//!   3. Enable sorting for the user list to show currently online users.
//!   4. Optimize performance and add a loading animation for the user list.
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::styles::KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR;
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::utils;
use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

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

    pub MentionInputBar = {{MentionInputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        align: {y: 0.5}
        padding: 10.
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        user_list_item: <UserListItem> {}

        message_input = <CommandTextInput> {
            width: Fill,
            height: Fit
            margin: 0
            align: {y: 0.5}
            trigger: "@"
            inline_search: true

            popup = {
                spacing: 0.0
                padding: 0.0
                clip_y: true
                draw_bg: {
                    color: #fff,
                    radius: 8.0,
                    border_width: 1.0,
                    border_color: #e5e5e5
                }

                header_view = {
                    visible: true,
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
                        empty_message: "Write a message (in Markdown) ..."
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
        }

        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

/// Actions emitted by the MentionInputBar component
#[derive(Clone, Debug)]
pub enum MentionInputBarAction {
    /// Triggered when message content changes
    MessageChanged(String),
    /// Triggered when a user is specifically mentioned
    UserMentioned(String),
}

/// Main component for message input with @mention support
/// Provides a text input area with autocomplete popup for user mentions
#[derive(Live, Widget)]
pub struct MentionInputBar {
    /// Base view properties
    #[deref]
    view: View,
    /// Template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    /// Current Matrix room ID
    #[rust]
    room_id: Option<OwnedRoomId>,
    /// List of available room members for mentions
    #[rust]
    room_members: Vec<RoomMember>,
    /// Current input text content
    #[rust]
    current_input: String,
    /// Position where the @ mention starts
    #[rust]
    mention_start_index: Option<usize>,
    /// Indicates if currently in mention search mode
    #[rust]
    is_searching: bool,
}

impl Widget for MentionInputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut ret = self.view.draw_walk(cx, scope, walk);

        let text_input = self.command_text_input(id!(message_input)).text_input_ref();
        let area = text_input.area();
        cx.show_text_ime(area, DVec2::default());

        while !ret.is_done() {
            ret = self.view.draw_walk(cx, scope, walk);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            let mut message_input = self.command_text_input(id!(message_input));

            if let Some(selected) = message_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                return;
            }

            if message_input.should_build_items(actions) {
                let search_text = message_input.search_text().to_lowercase();
                self.update_user_list(cx, &mut message_input, &search_text);
            }

            if let Some(action) =
                actions.find_widget_action(message_input.text_input_ref().widget_uid())
            {
                if let TextInputAction::Change(text) = action.cast() {
                    self.handle_text_change(cx, &mut message_input, scope, text);
                }
            }
        }
    }
}

impl MentionInputBar {
    // Handles user selection from mention popup
    // Manages text insertion and cursor positioning
    fn on_user_selected(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        let username = selected.label(id!(user_info.label)).text();
        let message_input = self.command_text_input(id!(message_input));

        if let Some(start_idx) = self.mention_start_index {
            let current_text = message_input.text();
            let head = message_input
                .text_input_ref()
                .borrow()
                .map_or(0, |p| p.get_cursor().head.index);

            let before = &current_text[..start_idx];
            let after = &current_text[head..];
            let mention = format!("{before} @{username} {after}");

            message_input.set_text(cx, &mention);
            let new_pos = start_idx + username.len() + 2;
            message_input.text_input_ref().set_cursor(new_pos, new_pos);

            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                MentionInputBarAction::UserMentioned(username),
            );
        }

        self.close_mention_popup(cx);
    }

    // Core text change handler that manages mention context
    // Controls popup visibility and search state
    fn handle_text_change(
        &mut self,
        cx: &mut Cx,
        message_input: &mut CommandTextInputRef,
        scope: &mut Scope,
        text: String,
    ) {
        self.current_input = text.clone();
        let cursor_pos = message_input
            .text_input_ref()
            .borrow()
            .map_or(0, |p| p.get_cursor().head.index);

        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            self.mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            // 提取搜索文本
            let search_text = text[trigger_pos + 1..cursor_pos].to_lowercase();
            self.update_user_list(cx, message_input, &search_text);
            message_input.view(id!(popup)).set_visible(cx, true);
        } else {
            self.close_mention_popup(cx);
        }

        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            MentionInputBarAction::MessageChanged(text),
        );
    }

    // Updates the mention suggestion list based on search
    // Handles member filtering, popup sizing, and layout adaptation
    fn update_user_list(
        &mut self,
        cx: &mut Cx,
        message_input: &mut CommandTextInputRef,
        search_text: &str,
    ) {
        message_input.clear_items();

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
            let popup = message_input.view(id!(popup));

            if member_count == 0 {
                popup.apply_over(cx, live! { height: Fit });
                message_input.view(id!(popup)).set_visible(cx, false);
                return;
            }

            if member_count <= MAX_VISIBLE_ITEMS {
                let single_item_height = if is_desktop { 32.0 } else { 64.0 };
                let total_height = (member_count as f64 * single_item_height) + 16.0;
                popup.apply_over(cx, live! { height: (total_height) });
            } else {
                let max_height = if is_desktop { 400.0 } else { 480.0 };
                popup.apply_over(cx, live! { height: (max_height) });
            }

            // FIXME:
            // Limit the number of members displayed to avoid errors: "HarfBuzz guarantees monotonic cluster values"
            const MAX_DISPLAY: usize = 200;
            if matched_members.len() > MAX_DISPLAY {
                matched_members.truncate(MAX_DISPLAY);
            }

            for (index, (display_name, member)) in matched_members.into_iter().enumerate() {
                let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                item.label(id!(user_info.label)).set_text(cx, &display_name);

                let safe_matrix_id = format!("@{}:matrix.org", member.user_id().localpart());
                item.label(id!(matrix_url)).set_text(cx, &safe_matrix_id);

                item.apply_over(
                    cx,
                    live! {
                        show_bg: true,
                        cursor: Hand,
                        padding: {left: 8., right: 8., top: 4., bottom: 4.}
                    },
                );

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

                message_input.add_item(item.clone());

                if index == 0 {
                    if let Some(mut message_input) = message_input.borrow_mut() {
                        message_input.set_keyboard_focus_index(0)
                    }
                }
            }

            message_input.view(id!(popup)).set_visible(cx, true);
            if self.is_searching {
                message_input.text_input_ref().set_key_focus(cx);
            }
        }
    }

    // Detects valid mention trigger positions in text
    // Handles various edge cases and validation rules
    fn find_mention_trigger_position(&self, text: &str, cursor_pos: usize) -> Option<usize> {
        if cursor_pos == 0 {
            return None;
        }

        if text.chars().nth(cursor_pos.saturating_sub(1)) == Some('@') {
            return Some(cursor_pos - 1);
        }

        text[..cursor_pos].rfind('@').filter(|&idx| {
            let after_trigger = &text[idx..cursor_pos];

            if after_trigger.len() == 1 {
                return true;
            }

            if after_trigger
                .chars()
                .nth(1)
                .map_or(false, |c| c.is_whitespace())
            {
                return false;
            }

            let chars: Vec<char> = after_trigger.chars().collect();
            for i in 0..chars.len().saturating_sub(1) {
                if chars[i].is_whitespace() && chars[i + 1].is_whitespace() {
                    return false;
                }
            }

            if after_trigger.contains('\n') {
                return false;
            }

            true
        })
    }

    // Cleanup helper for closing mention popup
    // Resets search state and manages focus
    fn close_mention_popup(&mut self, cx: &mut Cx) {
        self.mention_start_index = None;
        self.is_searching = false;

        let message_input = self.command_text_input(id!(message_input));
        message_input.view(id!(popup)).set_visible(cx, false);
        message_input.request_text_input_focus();
        self.redraw(cx);
    }

    /// Returns the current text content of the input bar
    pub fn text(&self) -> String {
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .text()
    }

    /// Sets the text content of the input bar
    ///
    /// # Arguments
    /// * `cx` - Widget context
    /// * `text` - New text content
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        let message_input = self.command_text_input(id!(message_input));
        message_input.text_input_ref().set_text(cx, text);
        self.current_input = text.to_string();
        self.redraw(cx);
    }

    /// Sets the Matrix room ID and initiates member list loading
    ///
    /// # Arguments
    /// * `room_id` - The Matrix room identifier
    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());
        submit_async_request(MatrixRequest::FetchRoomMembers { room_id });
    }

    /// Updates the list of available room members for mentions
    /// Filters invalid members and sorts by display name
    ///
    /// # Arguments
    /// * `members` - Vector of room members to process
    pub fn set_room_members(&mut self, mut members: Vec<RoomMember>) {
        members.retain(|member| {
            let display_name = member
                .display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| member.user_id().to_string());
            !display_name.trim().is_empty()
        });

        members.sort_by(|a, b| {
            let a_name = a
                .display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| a.user_id().to_string());
            let b_name = b
                .display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| b.user_id().to_string());
            a_name.cmp(&b_name)
        });

        self.room_members = members;
    }
}

impl LiveHook for MentionInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        let message_input = self.command_text_input(id!(message_input));

        message_input.apply_over(
            cx,
            live! {
                trigger: "@",
                inline_search: true,
                keyboard_focus_color: (KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR),
                pointer_hover_color: (KEYBOARD_FOCUS_OR_POINTER_HOVER_COLOR)
            },
        );

        message_input.request_text_input_focus();
    }
}

impl MentionInputBarRef {
    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn text(&self) -> Option<String> {
        self.borrow().map(|inner| inner.text())
    }

    pub fn set_room_id(&self, room_id: OwnedRoomId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_id(room_id);
        }
    }

    pub fn set_room_members(&self, members: Vec<RoomMember>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(members);
        }
    }
}
