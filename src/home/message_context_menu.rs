use makepad_widgets::*;

use super::room_screen::MessageAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    ICO_REPLY = dep("crate://self/resources/icons/reply.svg")

    ReplyButton = <RobrixIconButton> {
        width: Fit
        height: Fit

        draw_bg: {
            radius: 4.
        }
        draw_icon: {
            svg_file: (ICO_REPLY)
        }
        icon_walk: {width: 10, height: 10}

        text: "Reply"
    }

    ViewSourceButton = <RobrixIconButton> {
        width: Fit
        height: Fit

        draw_bg: {
            radius: 4.
        }
        draw_icon: {
            svg_file: (ICO_REPLY)
        }
        icon_walk: {width: 10, height: 10}

        text: "View Source"
    }

    pub MessageActionBar = {{MessageActionBar}} {
        width: Fit
        height: Fit
        flow: Overlay

        menu_content = <RoundedView> {
            width: Fit,
            height: Fit,
            flow: Right,
            padding: 2

            draw_bg: {
                color: #fff,
                border_width: 1.0,
                border_color: #D0D5DD,
                radius: 4.
            }

            reply_button = <ReplyButton> {
                text: ""
            }
        }
    }

    pub MessageContextMenu = {{MessageContextMenu}} {
        width: Fit
        height: Fit
        flow: Overlay

        menu_content = <RoundedView> {
            width: 150,
            height: Fit,
            flow: Down,
            padding: 2

            draw_bg: {
                color: #fff,
                border_width: 1.0,
                border_color: #D0D5DD,
                radius: 4.
            }

            reply_button = <ReplyButton> {
                width: Fill
                align: {x: 0}
                text: "Reply"
            }
        }
    }
}

/// The message menu available buttons.
enum MessageMenuButton {
    Reply,
}

impl MessageMenuButton {
    /// default handler for every button variant.
    fn handle_default(
        &self,
        cx: &mut Cx,
        scope: &mut Scope,
        button: &MessageMenuButton,
        item_id: usize,
        room_screen_widget_uid: WidgetUid,
    ) {
        match button {
            MessageMenuButton::Reply => {
                cx.widget_action(
                    room_screen_widget_uid,
                    &scope.path,
                    MessageAction::MessageReplyButtonClicked(item_id),
                );
            }
        }
    }
}

/// Message menu trait. Allows to centralize the functionality across various menus implementations.
trait MessageMenu {
    fn item_id(&self) -> Option<usize>;
    fn room_screen_widget_uid(&self) -> Option<WidgetUid>;
    /// A map of the button type and the menu's button ref.
    fn button_mapping(&self) -> Vec<(ButtonRef, MessageMenuButton)>;
    /// Function used for closing the menu
    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope);

    /// helper function for getting the MessageMenu context data.
    fn extract_context(&self) -> Option<(usize, WidgetUid)> {
        let item_id = self.item_id()?;
        let room_screen_widget_uid = self.room_screen_widget_uid()?;
        Some((item_id, room_screen_widget_uid))
    }

    /// handler function used for every button. uses defaults, overwrite for custom handling.
    fn handle_button(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        button: &MessageMenuButton,
    ) {
        let Some((item_id, room_screen_widget_uid)) = self.extract_context() else {
            return;
        };

        button.handle_default(cx, scope, button, item_id, room_screen_widget_uid);
        self.signal_close(cx, scope);
    }

    /// the button handler function. runs the button handler for each one in the map.
    fn handle_buttons(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        for (button_ref, button_enum) in self.button_mapping() {
            if button_ref.clicked(actions) {
                self.handle_button(cx, scope, &button_enum);
            }
        }
    }
}


// -- MessageContextMenu

#[derive(Live, LiveHook, Widget)]
pub struct MessageContextMenu {
    #[deref]
    view: View,
    #[rust]
    item_id: Option<usize>,
    #[rust]
    room_screen_widget_uid: Option<WidgetUid>,
    #[rust]
    message_widget_uid: Option<WidgetUid>,
}

impl Widget for MessageContextMenu {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MessageMenu for MessageContextMenu {
    fn item_id(&self) -> Option<usize> {
        self.item_id
    }

    fn room_screen_widget_uid(&self) -> Option<WidgetUid> {
        self.room_screen_widget_uid
    }

    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(message_widget_uid) = self.message_widget_uid {
            cx.widget_action(
                message_widget_uid,
                &scope.path,
                MessageAction::ContextMenuClose,
            );
        }
    }

    fn button_mapping(&self) -> Vec<(ButtonRef, MessageMenuButton)> {
        vec![
            (self.button(id!(reply_button)), MessageMenuButton::Reply),
        ]
    }
}

impl WidgetMatchEvent for MessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        self.handle_buttons(cx, actions, scope)
    }
}

impl MessageContextMenu {
    pub fn initialize_with_data(
        &mut self,
        cx: &mut Cx,
        room_screen_widget_uid: WidgetUid,
        message_widget_uid: WidgetUid,
        item_id: usize,
    ) {
        self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.message_widget_uid = Some(message_widget_uid);
        self.item_id = Some(item_id);
        self.redraw(cx);
    }
}

impl MessageContextMenuRef {
    pub fn initialize_with_data(&self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, message_widget_uid: WidgetUid, item_id: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.initialize_with_data(cx, room_screen_widget_uid, message_widget_uid, item_id);
        }
    }

    pub fn message_widget_uid(&self) -> Option<WidgetUid> {
        self.borrow().and_then(|inner| inner.message_widget_uid)
    }
}

// -- MessageActionBar

#[derive(Live, LiveHook, Widget)]
pub struct MessageActionBar {
    #[deref]
    view: View,
    #[rust]
    item_id: Option<usize>,
    #[rust]
    room_screen_widget_uid: Option<WidgetUid>,
    #[rust]
    message_widget_uid: Option<WidgetUid>,
}

impl Widget for MessageActionBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MessageMenu for MessageActionBar {
    fn item_id(&self) -> Option<usize> {
        self.item_id
    }

    fn room_screen_widget_uid(&self) -> Option<WidgetUid> {
        self.room_screen_widget_uid
    }

    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(message_widget_uid) = self.message_widget_uid {
            cx.widget_action(
                message_widget_uid,
                &scope.path,
                MessageAction::ActionBarClose,
            );
        }
    }

    fn button_mapping(&self) -> Vec<(ButtonRef, MessageMenuButton)> {
        vec![
            (self.button(id!(reply_button)), MessageMenuButton::Reply),
        ]
    }
}

impl WidgetMatchEvent for MessageActionBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        self.handle_buttons(cx, actions, scope)
    }
}

impl MessageActionBar {
    pub fn selected(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, message_widget_uid: WidgetUid, item_id: usize) {
        self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.message_widget_uid = Some(message_widget_uid);
        self.item_id = Some(item_id);
        self.redraw(cx);
    }
}

impl MessageActionBarRef {
    pub fn initialize_with_data(&self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, message_widget_uid: WidgetUid, item_id: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.selected(cx, room_screen_widget_uid, message_widget_uid, item_id);
        }
    }

    pub fn message_widget_uid(&self) -> Option<WidgetUid> {
        self.borrow_mut().and_then(|inner| inner.message_widget_uid)
    }
}
