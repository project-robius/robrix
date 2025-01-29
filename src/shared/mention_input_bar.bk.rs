use makepad_widgets::*;
use crate::profile::user_profile::{UserProfile};
use crate::profile::user_profile_cache::{get_user_profile_and_room_member};
use matrix_sdk::room::RoomMember;
use crate::shared::avatar::AvatarWidgetRefExt;
use matrix_sdk::ruma::{OwnedRoomId, RoomId};
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::avatar_cache::*;
use crate::utils;
use makepad_widgets::display_context::DisplayContext;

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

    // 用户列表项模板定义
    UserListItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 8., right: 8., top: 4., bottom: 4.}
        show_bg: true
        draw_bg: {color: #fff}
        flow: Down  // Default to vertical flow for mobile
        spacing: 8.0

        // Container for avatar and username (will be horizontal in both layouts)
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

        // Matrix URL (will be positioned differently based on layout)
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

        // 位置按钮配置
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
            keyboard_focus_color: (THEME_COLOR_CTRL_HOVER)
            pointer_hover_color: (THEME_COLOR_CTRL_HOVER * 0.85)

            inline_search: true

            // Configure the popup search area
            popup = {
                list = {
                    height: 200.0  // Fixed height in pixels
                    clip_y: true
                }
            }

            persistent = {
                center = {
                    text_input = {
                        empty_message: "Write a message (in Markdown) ..."

                        // Match RobrixTextInput background style
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
                                return sdf.result;
                            }
                        }

                        // Match RobrixTextInput text style
                        draw_text: {
                            color: (MESSAGE_TEXT_COLOR)
                            text_style: <MESSAGE_TEXT_STYLE>{}
                            fn get_color(self) -> vec4 {
                                return mix(
                                    self.color,
                                    #B,
                                    self.is_empty
                                )
                            }
                        }

                        // Match RobrixTextInput cursor style
                        draw_cursor: {
                            instance focus: 0.0
                            uniform border_radius: 0.5
                            fn pixel(self) -> vec4 {
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                sdf.box(
                                    0.,
                                    0.,
                                    self.rect_size.x,
                                    self.rect_size.y,
                                    self.border_radius
                                )
                                sdf.fill(mix(#fff, #bbb, self.focus));
                                return sdf.result
                            }
                        }

                        // Match RobrixTextInput selection style
                        draw_selection: {
                            instance hover: 0.0
                            instance focus: 0.0
                            uniform border_radius: 2.0
                            fn pixel(self) -> vec4 {
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                sdf.box(
                                    0.,
                                    0.,
                                    self.rect_size.x,
                                    self.rect_size.y,
                                    self.border_radius
                                )
                                sdf.fill(mix(#eee, #ddd, self.focus));
                                return sdf.result
                            }
                        }
                    }
                }
            }

        }

        // 发送按钮配置
        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

// Define the actions that our component can emit
#[derive(Clone, Debug)]
pub enum MentionInputBarAction {
    MessageChanged(String),
    UserMentioned(String),
}

// Main component implementation
#[derive(Live, Widget)]
pub struct MentionInputBar {
    #[deref]
    view: View,
    // Store the template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    room_members: Vec<RoomMember>,
    #[rust]
    current_input: String,
    #[rust]
    mention_start_index: Option<usize>,
    #[rust]
    is_searching: bool,
}

impl LiveHook for MentionInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        // Set initial focus to the input field
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .set_key_focus(cx);
    }
}

impl Widget for MentionInputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // 首先开始绘制基础视图
        let mut ret = self.view.draw_walk(cx, scope, walk);

        // 获取文本输入组件的引用并处理 IME
        let message_input = self.command_text_input(id!(message_input));
        let text_input = message_input.text_input_ref();

        // 获取输入区域并设置 IME 位置
        let area = text_input.area();
        cx.show_text_ime(area, DVec2::default());

        // 继续绘制，直到所有子组件都完成绘制
        while !ret.is_done() {
            ret = self.view.draw_walk(cx, scope, walk);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let mut message_input = self.command_text_input(id!(message_input));

        if let Event::Actions(actions) = event {
            // 1. 处理用户选择
            if let Some(selected) = message_input.item_selected(actions) {
                let username = selected.label(id!(user_info.label)).text();
                if let Some(start_idx) = self.mention_start_index {
                    let mut current_text = self.current_input.clone();
                    let before_mention = &current_text[..start_idx];
                    let after_mention = &current_text[message_input.text_input_ref().borrow()
                        .map_or(0, |p| p.get_cursor().head.index)..];

                    let new_text = format!("{} {} {}", before_mention, username, after_mention);
                    self.set_text(cx, &new_text);

                    // 重置状态并关闭弹框
                    self.mention_start_index = None;
                    self.is_searching = false;
                    message_input.view(id!(popup)).set_visible(cx, false);
                }

                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    MentionInputBarAction::UserMentioned(username),
                );
            }

            // 2. 处理文本变化
            if let Some(action) = actions.find_widget_action(message_input.text_input_ref().widget_uid()) {
                if let TextInputAction::Change(text) = action.cast() {
                    self.current_input = text.clone();
                    let cursor_pos = message_input.text_input_ref().borrow()
                        .map_or(0, |p| p.get_cursor().head.index);

                    // 检查当前光标位置是否在任何已存在的 @ 后面
                    let prev_at_pos = text[..cursor_pos].rfind('@')
                        .filter(|&idx| !text[idx..cursor_pos].contains(char::is_whitespace));

                    if text.chars().nth(cursor_pos.saturating_sub(1)) == Some('@') {
                        // 新输入了 @
                        self.mention_start_index = Some(cursor_pos - 1);
                        self.is_searching = true;
                        self.show_user_list(cx, &mut message_input, "");
                    } else if let Some(start_idx) = prev_at_pos {
                        // 光标在已存在的 @ 后面
                        self.mention_start_index = Some(start_idx);
                        self.is_searching = true;
                        let search_text = text[start_idx + 1..cursor_pos].to_lowercase();
                        self.show_user_list(cx, &mut message_input, &search_text);
                    } else {
                        // 不在任何 @ 上下文中
                        self.mention_start_index = None;
                        self.is_searching = false;
                        message_input.view(id!(popup)).set_visible(cx, false);
                    }

                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        MentionInputBarAction::MessageChanged(text),
                    );
                }

                // 3. 处理特殊键盘事件
                if let TextInputAction::KeyDownUnhandled(ke) = action.cast() {
                    match ke.key_code {
                        KeyCode::Escape => {
                            if self.is_searching {
                                self.mention_start_index = None;
                                self.is_searching = false;
                                message_input.view(id!(popup)).set_visible(cx, false);
                                self.redraw(cx);
                            }
                        }
                        KeyCode::ArrowUp | KeyCode::ArrowDown if self.is_searching => {
                            message_input.view(id!(popup)).set_visible(cx, true);
                        }
                        _ => {}
                    }
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }
}

// Implement public methods for the component
impl MentionInputBar {
    fn show_user_list(&mut self, cx: &mut Cx, message_input: &mut CommandTextInputRef, search_text: &str) {
        self.update_user_list(cx, message_input, search_text);
        message_input.view(id!(popup)).set_visible(cx, true);
        self.redraw(cx);
    }

    fn update_user_list(&mut self, cx: &mut Cx, message_input: &mut CommandTextInputRef, search_text: &str) {
        message_input.clear_items();

        // 只在搜索状态下显示用户列表
        if self.is_searching {
            let is_desktop = if cx.display_context.is_desktop() {
                log!("DisplayContext Have Desktop === === === ");
                true
            } else {
                log!("DisplayContext None === === === ");
                false
            };

            // 过滤并显示匹配的用户
            for member in &self.room_members {
                let display_name = member.display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| member.user_id().to_string());

                if display_name.to_lowercase().contains(search_text) {
                    let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                    item.label(id!(user_info.label)).set_text(cx, &display_name);

                    let matrix_url = format!("{}:matrix.org", member.user_id());
                    item.label(id!(matrix_url)).set_text(cx, &matrix_url);

                    if is_desktop {
                        item.apply_over(cx, live!(
                            flow: Right,
                            align: {y: 0.5}
                        ));
                        item.view(id!(user_info.filler)).set_visible(cx, true);
                    } else {
                        item.apply_over(cx, live!(
                            flow: Down,
                            spacing: 4.0
                        ));
                        item.view(id!(user_info.filler)).set_visible(cx, false);
                        item.label(id!(matrix_url)).apply_over(cx, live!(
                            margin: {left: 0.}
                        ));
                    }

                    // 处理头像显示
                    let avatar = item.avatar(id!(user_info.avatar));
                    if let Some(mxc_uri) = member.avatar_url() {
                        if let Some(avatar_data) = get_avatar(cx, mxc_uri) {
                            avatar.show_image(cx, None, |cx, img| {
                                utils::load_png_or_jpg(&img, cx, &avatar_data)
                            });
                        } else {
                            avatar.show_text(cx, None, &display_name);
                        }
                    } else {
                        avatar.show_text(cx, None, &display_name);
                    }

                    message_input.add_item(item);
                }
            }
        }
    }

    pub fn text(&self) -> String {
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .text()
    }

    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        let message_input = self.command_text_input(id!(message_input));
        message_input.text_input_ref().set_text(cx, text);
        self.draw_bg.redraw(cx);  // Explicitly trigger a redraw
        self.current_input = text.to_string();  // Update our internal state
    }

    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());

        submit_async_request(MatrixRequest::FetchRoomMembers {
            room_id: room_id
        });
    }

    pub fn set_room_members(&mut self, members: Vec<RoomMember>) {
        log!("Setting {} members to MentionInputBar", members.len());
        self.room_members = members;
    }
}

// Implement methods for component references
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
