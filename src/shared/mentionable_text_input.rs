//! MentionableTextInput component provides text input with @mention capabilities
//! Can be used in any context where user mentions are needed (message input, editing)
//!
//! TODO for the future:
//!   1. Add a header to the user list to display the current number of users in the room.
//!   2. Implement scrolling functionality for the user list.
//!   3. Enable sorting for the user list to show currently online users.
//!   4. Optimize performance and add a loading animation for the user list.
use crate::avatar_cache::*;
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::utils;

use makepad_widgets::*;
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
            color: #fff,
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
            color: #fff,
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

            header_view = {
                header_label = {
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
            center = {
                text_input = {
                    empty_message: "Start typing..."
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

                    draw_highlight: {
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
}


impl Widget for MentionableTextInput {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.cmd_text_input.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {

            let text_input_ref = self.cmd_text_input.text_input_ref(); // 获取内部 TextInput 的引用
            let text_input_uid = text_input_ref.widget_uid();
            // --- 修改开始 ---
            // 获取内部 TextInput 的 Area
            let text_input_area = text_input_ref.area();
            // 使用 cx 检查这个 Area 是否有键盘焦点
            let has_focus = cx.has_key_focus(text_input_area);
            // --- 修改结束 ---


            if let Some(selected) = self.cmd_text_input.item_selected(actions) {
                self.on_user_selected(cx, scope, selected);
                // return;
            }

            if self.cmd_text_input.should_build_items(actions) {
                // 只有当这个实例的 TextInput 有焦点时才构建列表
                if has_focus {
                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                    // update_user_list 内部已经包含了 Scope room_id 的检查
                    self.update_user_list(cx, &search_text, scope);
                } else {
                    // 如果没有焦点，但收到了构建请求（可能来自之前的状态），确保弹窗是关闭的
                    if self.cmd_text_input.view(id!(popup)).visible() {
                        self.close_mention_popup(cx);
                    }
                }
            }

            if let Some(action) =
                actions.find_widget_action(self.cmd_text_input.text_input_ref().widget_uid())
            {
                if let TextInputAction::Change(text) = action.cast() {
                    self.handle_text_change(cx, scope, text);
                }
            }

            for action in actions {
                // 检查是否是 TextInputAction
                if let Some(widget_action) = action.as_widget_action() {
                    // 确保 Action 来自我们自己的 TextInput
                    if widget_action.widget_uid == text_input_uid {
                        if let TextInputAction::Change(text) = widget_action.cast() {
                            // 只有当这个实例的 TextInput 有焦点时才处理文本变化
                            if has_focus {
                                // handle_text_change 内部会调用 update_user_list，
                                // update_user_list 内部有 Scope room_id 检查
                                self.handle_text_change(cx, scope, text);
                            }
                            // 找到了对应的 Change Action，可以跳出内层循环
                            break;
                        }
                    }
                }

                // Check for MentionableTextInputAction actions
                if let Some(action_ref) = action.downcast_ref::<MentionableTextInputAction>() {
                    match action_ref {
                        MentionableTextInputAction::PowerLevelsUpdated(room_id, can_notify_room) => {
                            // 检查 Scope 中的 room_id 与 action 中的 room_id 是否匹配
                            let scope_room_id = scope.props.get::<RoomScreenProps>().map(|props| &props.room_id);

                            // 如果 Scope 中有 room_id，并且不匹配 action 中的 room_id，则这个 action 可能是为另一个房间发送的
                            if let Some(scope_id) = scope_room_id {
                                if scope_id != room_id {
                                    log!("MentionableTextInput({:?}) ignoring PowerLevelsUpdated because scope room_id ({}) doesn't match action room_id ({})",
                                        self.widget_uid(), scope_id, room_id);
                                    continue; // 跳过这个 action
                                }
                            }

                            // 如果 Scope 中没有 room_id，则看组件内部状态是否与 action 匹配
                            if scope_room_id.is_none() {
                                if let Some(internal_id) = &self.room_id {
                                    if internal_id != room_id {
                                        log!("MentionableTextInput({:?}) ignoring PowerLevelsUpdated because internal room_id ({}) doesn't match action room_id ({})",
                                            self.widget_uid(), internal_id, room_id);
                                        continue; // 跳过这个 action
                                    }
                                }
                            }

                            // 通过检查后，可以更新组件状态
                            log!("MentionableTextInput({:?}) received valid PowerLevelsUpdated for room {}: can_notify={}",
                                self.widget_uid(), room_id, can_notify_room);

                            // 如果此时内部 room_id 未设置或与 action 不匹配，则更新它
                            // 注意：这里优先使用 Scope 中的 room_id
                            if self.room_id.as_ref() != Some(room_id) {
                                self.room_id = Some(room_id.clone());
                                log!("MentionableTextInput({:?}) updated internal room_id to {}", self.widget_uid(), room_id);
                            }

                            // 只有当 can_notify_room 状态实际改变时才更新并可能重绘
                            if self.can_notify_room != *can_notify_room {
                                self.can_notify_room = *can_notify_room;
                                log!("MentionableTextInput({:?}) updated can_notify_room to {}", self.widget_uid(), can_notify_room);

                                // 如果正在搜索，可能需要立即更新列表以显示/隐藏 @room
                                if self.is_searching && has_focus { // 确保有焦点时才更新列表
                                    let search_text = self.cmd_text_input.search_text().to_lowercase();
                                    self.update_user_list(cx, &search_text, scope);
                                } else {
                                    self.redraw(cx);
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
        let text_input_ref = self.cmd_text_input.text_input_ref();
        let current_text = text_input_ref.text();
        let head = text_input_ref.borrow().map_or(0, |p| p.get_cursor().head.index);

        if let Some(start_idx) = self.current_mention_start_index {
            // 1. 改进检测 @room 选择的逻辑，不要依赖于 user_id 是否为空
            // 获取原始的模板指针，用于比较是否是选择的 @room 选项
            let room_mention_template = self.room_mention_list_item;

            // 检查当前选中项是否是 @room 项
            // 直接检查通用的文本标签
            let mut is_room_mention = false;

            // 尝试检查文本内容为 @room 的标签
            let inner_label = selected.label(id!(room_mention)); // 默认标签
            if inner_label.text() == "@room" {
                is_room_mention = true;
            }

            // 如果上面的方法失败，退回到比较 widget_uid 或 user_id 为空的方式
            let is_room_mention = if !is_room_mention {
                log!("Falling back to alternative @room detection methods");

                // 方法 2: 看是否能找到 user_id 标签 - 用户项有它，@room 项没有
                let has_user_id = selected.label(id!(user_id)).text().len() > 0;

                !has_user_id
            } else {
                true
            };

            log!("Item selected is_room_mention: {}", is_room_mention);

            let mention_to_insert = if is_room_mention {
                // 用户选择了 @room
                log!("User selected @room mention");
                self.possible_room_mention = true;
                "@room ".to_string()
            } else {
                // 用户选择了特定用户
                let username = selected.label(id!(user_info.username)).text();
                let user_id_str = selected.label(id!(user_id)).text();
                let Ok(user_id): Result<OwnedUserId, _> = user_id_str.clone().try_into() else {
                    log!("Failed to parse user_id: {}", user_id_str);
                    return;
                };
                self.possible_mentions.insert(user_id.clone(), username.clone());
                log!("User selected mention: {} ({}))", username, user_id);
                self.possible_room_mention = false; // 选择用户取消 @room 提及

                // 目前，我们直接插入用户提及的 markdown 链接
                // 而不是用户的显示名称，因为我们还没有办法
                // 追踪提及的显示名称并稍后替换它。
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
            text_input_ref.set_cursor(new_pos, new_pos);
        }

        self.close_mention_popup(cx);
    }

    // Core text change handler that manages mention context
    fn handle_text_change(&mut self, cx: &mut Cx, scope: &mut Scope, text: String) {
        // Currently an inserted mention consists of a markdown link,
        // which is "[USERNAME](matrix_to_uri)", so of course this must be at least 6 characters.
        // (In reality it has to be a lot more, but whatever...)
        if text.trim().len() < 6 {
            self.possible_mentions.clear();
        }

        let cursor_pos = self.cmd_text_input.text_input_ref().borrow().map_or(0, |p| p.get_cursor().head.index);

        if let Some(trigger_pos) = self.find_mention_trigger_position(&text, cursor_pos) {
            self.current_mention_start_index = Some(trigger_pos);
            self.is_searching = true;

            let search_text = utils::safe_substring_by_byte_indices(
                &text,
                trigger_pos + 1,
                cursor_pos
            ).to_lowercase();

            // let current_members_count = self.room_id.as_ref()
            //     .and_then(|id| self.room_members_map.get(id))
            //     .map_or(0, |members| members.len());

            self.update_user_list(cx, &search_text,scope);
            self.cmd_text_input.view(id!(popup)).set_visible(cx, true);
        } else if self.is_searching {
            self.close_mention_popup(cx);
        }
    }

    // Updates the mention suggestion list based on search
    fn update_user_list(&mut self, cx: &mut Cx, search_text: &str, scope: &mut Scope) {
        // 1. 获取 Props 并检查 Scope 的有效性
        let Some(room_props) = scope.props.get::<RoomScreenProps>() else {
            log!("MentionableTextInput::update_user_list: RoomScreenProps not found in scope. Clearing list.");
            self.cmd_text_input.clear_items(); // 清空列表，因为没有有效数据源
            self.cmd_text_input.view(id!(popup)).set_visible(cx, false); // 隐藏弹窗
            self.redraw(cx);
            return;
        };

        // 2. 检查 internal room_id 是否已设置 (应该由 PowerLevelsUpdated 设置)
        if self.room_id.is_none() {
            // 如果内部 room_id 未设置，从当前 scope 初始化它
            log!("MentionableTextInput: Initializing internal room_id from scope: {}", room_props.room_id);
            self.room_id = Some(room_props.room_id.clone());
        }

        // 3. 核心检查：当前 scope 的 room_id 与组件内部 room_id 是否匹配
        let internal_room_id = self.room_id.as_ref().unwrap(); // 此时必定存在
        if internal_room_id != &room_props.room_id {
            log!("MentionableTextInput Warning: Scope room_id ({}) does not match internal room_id ({}). Updating internal room_id.",
                    room_props.room_id, internal_room_id);

            // 重要修复：当切换房间时，更新组件的内部 room_id 以匹配当前 scope
            self.room_id = Some(room_props.room_id.clone());

            // 清空当前列表，准备使用新房间的成员更新
            self.cmd_text_input.clear_items();
        }

        // 始终使用当前 scope 中提供的 room_members
        // 这些成员列表应该来自 TimelineUiState.room_members_map 并且已经是当前房间的正确列表
        let room_members = &room_props.room_members;

        // 清空旧列表项，准备填充新列表
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

                // 先尝试从当前房间 Props 中获取房间头像
                // 使用 self.room_id 而不是 room_props.room_id 来确保获取正确的房间头像
                // 当切换房间时，self.room_id 已在前面的代码中更新为与 room_props.room_id 一致
                if let Some(room_id) = self.room_id.as_ref() {
                    if let Some(client) = get_client() {
                        if let Some(room) = client.get_room(room_id) {
                            if let Some(avatar_url) = room.avatar_url() {
                                log!("Found room avatar URL for @room: {}", avatar_url);

                                match get_or_fetch_avatar(cx, avatar_url.to_owned()) {
                                    AvatarCacheEntry::Loaded(avatar_data) => {
                                        // 显示房间头像
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

                // 如果无法显示房间头像，显示带红色背景的R字母
                if !room_avatar_shown {
                    avatar_ref.show_text(cx, red_color_vec4, None, "R");
                }


                self.cmd_text_input.add_item(room_mention_item);
            }

            for member in room_members.iter() {
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

            // if member_count == 0 {
            //     popup.apply_over(cx, live! { height: Fit });
            //     self.cmd_text_input.view(id!(popup)).set_visible(cx, false);
            //     return;
            // }

            // Adjust height calculation to include the potential @room item
            let total_items_in_list = member_count + if "@room".contains(&search_text) { 1 } else { 0 };

            if total_items_in_list == 0 {
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

            if total_items_in_list <= MAX_VISIBLE_ITEMS {
                let single_item_height = if is_desktop { 32.0 } else { 64.0 };
                let total_height =
                    (total_items_in_list as f64 * single_item_height) + header_height + estimated_spacing;
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

        // Check if cursor is immediately after @ symbol
        // Only trigger if @ is preceded by whitespace or beginning of text
        if cursor_grapheme_idx > 0 && text_graphemes[cursor_grapheme_idx - 1] == "@" {
            let is_preceded_by_whitespace_or_start = cursor_grapheme_idx == 1 ||
                (cursor_grapheme_idx > 1 && text_graphemes[cursor_grapheme_idx - 2].trim().is_empty());
            if is_preceded_by_whitespace_or_start {
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
        // When text is set externally (e.g., for editing), clear mention state
        self.possible_mentions.clear();
        self.possible_room_mention = false;
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
