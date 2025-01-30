use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use crate::shared::avatar::AvatarWidgetRefExt;
use matrix_sdk::ruma::OwnedRoomId;
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::avatar_cache::*;
use crate::utils;

// UI 设计定义
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;

    // 定义图标资源
    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

    // 用户列表项的模板定义
    UserListItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 8., right: 8., top: 4., bottom: 4.}
        show_bg: true
        cursor: Hand
        draw_bg: {
            color: #fff,
            instance hover: 0.0,  // 添加实例状态
            instance selected: 0.0  // 添加实例状态

            fn pixel(self) -> vec4 {
                // 根据状态返回不同的颜色
                if self.selected > 0.0 {
                    return #eaecf0
                }
                if self.hover > 0.0 {
                    return #f5f5f5
                }
                return self.color
            }
        }
        flow: Down
        spacing: 8.0

        // 用户信息容器 (头像和用户名)
        user_info = <View> {
            width: Fill,
            height: Fit,
            flow: Right,
            spacing: 8.0
            align: {y: 0.5}

            // 用户头像
            avatar = <Avatar> {
                width: 24,
                height: 24,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 12.0 }
                }}}
            }

            // 用户名标签
            label = <Label> {
                height: Fit,
                draw_text: {
                    color: #000,
                    text_style: {font_size: 14.0}
                }
            }

            filler = <FillerX> {}
        }

        // Matrix ID 显示
        matrix_url = <Label> {
            height: Fit,
            draw_text: {
                color: #666,
                text_style: {font_size: 12.0}
            }
        }
    }

    // 主组件定义
    pub MentionInputBar = {{MentionInputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        align: {y: 0.5}
        padding: 10.
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        // 位置按钮
        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        // 用户列表项模板引用
        user_list_item: <UserListItem> {}

        // 消息输入框
        message_input = <CommandTextInput> {
            width: Fill,
            height: Fit
            margin: 0
            align: {y: 0.5}
            // 设置触发字符和搜索相关配置
            trigger: "@"

            inline_search: true

            // 弹出框配置
            popup = {
                draw_bg: {
                    color: #fff,
                    radius: 8.0,
                    border_width: 1.0,
                    border_color: #e5e5e5
                }
                list = {
                    height: 200.0
                    clip_y: true
                }
            }

            // 持久化视图配置
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

        // 发送按钮
        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

// 组件事件定义
#[derive(Clone, Debug)]
pub enum MentionInputBarAction {
    MessageChanged(String),
    UserMentioned(String),
}

// 主组件结构体定义
#[derive(Live, Widget)]
pub struct MentionInputBar {
    #[deref]
    view: View,
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

// Widget trait 实现
impl Widget for MentionInputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut ret = self.view.draw_walk(cx, scope, walk);

        // 处理 IME
        let text_input = self.command_text_input(id!(message_input)).text_input_ref();
        let area = text_input.area();
        cx.show_text_ime(area, DVec2::default());

        while !ret.is_done() {
            ret = self.view.draw_walk(cx, scope, walk);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // 让 CommandTextInput 处理所有基础事件
        self.view.handle_event(cx, event, scope);

        // 只处理 Actions 事件
        if let Event::Actions(actions) = event {
            let mut message_input = self.command_text_input(id!(message_input));

            // 1. 处理选择事件
            if let Some(selected) = message_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                return;
            }

            // 2. 处理搜索更新
            if message_input.should_build_items(actions) {
                let search_text = message_input.search_text().to_lowercase();
                self.update_user_list(cx, &mut message_input, &search_text);
            }

            // 3. 处理文本变化
            if let Some(action) = actions.find_widget_action(message_input.text_input_ref().widget_uid()) {
                if let TextInputAction::Change(text) = action.cast() {
                    self.handle_text_change(cx, &mut message_input, scope, text);
                }
            }
        }
    }
}

// WidgetMatchEvent trait 实现
impl WidgetMatchEvent for MentionInputBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let mut message_input = self.command_text_input(id!(message_input));

        for action in actions.iter() {
            if let Some(widget_action) = action.as_widget_action() {
                match widget_action.cast() {
                    TextInputAction::KeyDownUnhandled(ke) => {
                        log!("KeyDownUnhandled - key_code: {:?}", ke.key_code);
                        if matches!(ke.key_code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                            log!("Navigation key detected: {:?}", ke.key_code);

                            // 检查弹出框状态
                            let popup_visible = message_input.view(id!(popup)).is_visible();
                            log!("When arrow key pressed - popup visible: {}", popup_visible);

                            // 检查是否处于搜索状态
                            log!("Search state: is_searching={}, mention_start_index={:?}",
                                self.is_searching,
                                self.mention_start_index);
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. 检查键盘焦点状态
        let has_focus = cx.has_key_focus(message_input.text_input_ref().area());
        log!("Input has focus: {}", has_focus);

        // 1. 处理选择事件 - 优先级最高
        if let Some(selected) = message_input.item_selected(actions) {
            self.on_user_selected(cx, scope, selected);
            return; // 选择后直接返回，避免其他处理
        }

        // 2. 处理搜索更新
        if message_input.should_build_items(actions) {
            let search_text = message_input.search_text().to_lowercase();
            self.update_user_list(cx, &mut message_input, &search_text);
        }

        // 3. 处理文本变化
        if let Some(action) = actions.find_widget_action(message_input.text_input_ref().widget_uid()) {
            if let TextInputAction::Change(text) = action.cast() {
                self.handle_text_change(cx, &mut message_input, scope, text);
            }
        }

        // 4. 处理 Escape 键
        if message_input.text_input_ref().escape(actions) {
            self.close_mention_popup(cx);
        }
    }
}

// MentionInputBar 实现
impl MentionInputBar {

    fn update_item_highlights(&self, cx: &mut Cx, message_input: &CommandTextInputRef) {
        // 先获取当前选中的索引，避免多次借用
        let focus_idx = message_input.borrow()
            .map(|input| input.keyboard_focus_index)
            .flatten();

        // 在单独的作用域中遍历更新样式
        if let Some(input) = message_input.borrow() {
            for (idx, item) in input.selectable_widgets.iter().enumerate() {
                if Some(idx) == focus_idx {
                    // 选中状态使用浅灰色背景
                    item.apply_over(cx, live! {
                        draw_bg: {
                            color: #eaecf0  // 选中状态的浅灰色
                        }
                    });
                } else {
                    // 未选中状态使用白色背景
                    item.apply_over(cx, live! {
                        draw_bg: {
                            color: #fff
                        }
                    });
                }
            }
        }
    }

    fn on_user_selected(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        let username = selected.label(id!(user_info.label)).text();
        let message_input = self.command_text_input(id!(message_input));

        if let Some(start_idx) = self.mention_start_index {
            let current_text = message_input.text();
            let head = message_input.text_input_ref().borrow()
                .map_or(0, |p| p.get_cursor().head.index);

            // 构建提及文本
            let before = &current_text[..start_idx];
            let after = &current_text[head..];
            let mention = format!("{before} @{username} {after}");

            // 更新文本和光标
            message_input.set_text(cx, &mention);
            let new_pos = start_idx + username.len() + 2;
            message_input.text_input_ref().set_cursor(new_pos, new_pos);

            // 发送事件
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                MentionInputBarAction::UserMentioned(username),
            );
        }

        // 清理状态
        self.close_mention_popup(cx);
    }


    fn handle_text_change(&mut self, cx: &mut Cx, message_input: &mut CommandTextInputRef, scope: &mut Scope, text: String) {
        self.current_input = text.clone();
        let cursor_pos = message_input.text_input_ref().borrow()
            .map_or(0, |p| p.get_cursor().head.index);

        // 添加日志检查文本变化
        log!("Text changed: '{}', cursor at: {}", text, cursor_pos);

        // 检查是否在提及上下文中
        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            log!("Found trigger at position: {}", trigger_pos);
            self.mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            // 提取搜索文本并更新列表
            let search_text = text[trigger_pos + 1..cursor_pos].to_lowercase();
            log!("Extracted search text: '{}'", search_text);

            self.update_user_list(cx, message_input, &search_text);
            message_input.view(id!(popup)).set_visible(cx, true);
        } else {
            log!("No trigger found, closing popup");
            self.close_mention_popup(cx);
        }

        // 发送文本变化事件
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            MentionInputBarAction::MessageChanged(text),
        );
    }


    fn update_user_list(&mut self, cx: &mut Cx, message_input: &mut CommandTextInputRef, search_text: &str) {
        message_input.clear_items();

        if self.is_searching {
            let is_desktop = cx.display_context.is_desktop();
            let mut matched_members = Vec::new();


            // Ensure first item gets keyboard focus
            if !matched_members.is_empty() {
                if let Some(mut input) = message_input.borrow_mut() {
                    input.keyboard_focus_index = Some(0);
                }
            }

            // 添加日志看看搜索状态
            log!("Updating user list with search text: '{}', is_searching: {}",
                            search_text, self.is_searching);

            // 收集匹配的成员
            for member in &self.room_members {
                let display_name = member.display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| member.user_id().to_string());

                if display_name.to_lowercase().contains(search_text) {
                    matched_members.push((display_name, member));
                }
            }

            // 限制显示数量但保持合理的范围
            const MAX_DISPLAY: usize = 50;
            if matched_members.len() > MAX_DISPLAY {
                matched_members.truncate(MAX_DISPLAY);
            }

            log!("Found {} matching members", matched_members.len());

            for (index, (display_name, member)) in matched_members.into_iter().enumerate() {
                let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

                item.label(id!(user_info.label)).set_text(cx, &display_name);

                log!("Creating list item {} for user: {}", index, display_name);

                let safe_matrix_id = format!("{}:matrix.org", member.user_id().localpart());
                item.label(id!(matrix_url)).set_text(cx, &safe_matrix_id);

                // 正确设置高亮和交互状态
                item.apply_over(cx, live! {
                    show_bg: true,
                    cursor: Hand
                });

                if index == 0 {
                    message_input.borrow_mut().unwrap().keyboard_focus_index = Some(0);
                }

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

                // 设置头像
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
            }

            log!("Popup visible, resetting focus to input");
            // 确保弹出框可见并正确处理键盘焦点
            message_input.view(id!(popup)).set_visible(cx, true);

            // 重置焦点到搜索输入框
            if self.is_searching {
                message_input.text_input_ref().set_key_focus(cx);
            }

            let popup_visible = message_input.view(id!(popup)).is_visible();
            log!("After updating list - popup visible: {}", popup_visible);
        }
    }

    fn find_mention_trigger_position(&self, text: &str, cursor_pos: usize) -> Option<usize> {
        if cursor_pos == 0 {
            return None;
        }

        // 检查是否刚输入了 @
        if text.chars().nth(cursor_pos.saturating_sub(1)) == Some('@') {
            return Some(cursor_pos - 1);
        }

        // 检查是否在已存在的提及中
        text[..cursor_pos]
            .rfind('@')
            .filter(|&idx| {
                let after_trigger = &text[idx..cursor_pos];

                // 排除不合法的触发情况
                if after_trigger.len() == 1 {
                    return true;  // 只有 @ 符号时是合法的
                }

                // 检查 @ 后面是否直接跟着空格（非法触发）
                if after_trigger.chars().nth(1).map_or(false, |c| c.is_whitespace()) {
                    return false;
                }

                // 检查是否有连续的空格（用户故意终止）
                let chars: Vec<char> = after_trigger.chars().collect();
                for i in 0..chars.len().saturating_sub(1) {
                    if chars[i].is_whitespace() && chars[i + 1].is_whitespace() {
                        return false;
                    }
                }

                // 检查是否有换行符（终止提及）
                if after_trigger.contains('\n') {
                    return false;
                }

                true
            })
    }

    fn close_mention_popup(&mut self, cx: &mut Cx) {
        self.mention_start_index = None;
        self.is_searching = false;

        let message_input = self.command_text_input(id!(message_input));
        message_input.view(id!(popup)).set_visible(cx, false);

        // 确保主输入框获得焦点
        message_input.request_text_input_focus();

        self.redraw(cx);
    }

    // 公共接口方法
    pub fn text(&self) -> String {
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .text()
    }

    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        let message_input = self.command_text_input(id!(message_input));
        message_input.text_input_ref().set_text(cx, text);
        self.draw_bg.redraw(cx);
        self.current_input = text.to_string();
    }

    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());
        // 当房间 ID 改变时,获取新房间的成员列表
        submit_async_request(MatrixRequest::FetchRoomMembers {
            room_id: room_id
        });
    }

    pub fn set_room_members(&mut self, mut members: Vec<RoomMember>) {
        members.retain(|member| {
            let display_name = member.display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| member.user_id().to_string());

            !display_name.trim().is_empty()
        });

        // 对整个列表进行排序，以便默认显示时显示最常用的成员
        members.sort_by(|a, b| {
            // TODO: 首先按在线状态排序（如果有这个信息）
            // 然后按显示名称排序
            let a_name = a.display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| a.user_id().to_string());
            let b_name = b.display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| b.user_id().to_string());
            a_name.cmp(&b_name)
        });

        log!("Total valid members in MentionInputBar: {}", members.len());
        self.room_members = members;
    }
}

// LiveHook trait 实现 - 处理组件初始化
impl LiveHook for MentionInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        let message_input = self.command_text_input(id!(message_input));

        log!("Initializing MentionInputBar");
        // 检查初始配置
        if let Some(message_input) = message_input.borrow(){
            log!("CommandTextInput initial config - inline_search: {}",
                message_input.inline_search);
        }

        // 确保 CommandTextInput 的配置正确
        message_input.apply_over(cx, live! {
            trigger: "@",
            inline_search: true,
            keyboard_focus_color: #eaecf0,
            pointer_hover_color: #f5f5f5
        });

        log!("CommandTextInput configuration applied");
        // 设置输入框为初始焦点
        message_input.request_text_input_focus();
        log!("Initial focus requested");
    }
}

// 组件引用方法实现 - 提供外部访问接口
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
