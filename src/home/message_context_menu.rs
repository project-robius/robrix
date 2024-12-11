use makepad_widgets::*;

use super::room_screen::MessageAction;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import makepad_draw::shader::std::*;
    import makepad_draw::shader::draw_color::DrawColor;

    import crate::shared::styles::*;
    import crate::shared::icon_button::*;

    ICO_REPLY = dep("crate://self/resources/icons/reply.svg")

    ReplyButton = <RobrixIconButton> {
	width: Fill
	height: Fit
	align: {x: 0}

	draw_bg: {
	    radius: 0.
	}
	draw_icon: {
	    svg_file: (ICO_REPLY)
	}
	icon_walk: {width: 10, height: 10, margin: {top: 4.0}, }

	text: "Reply"
    }

    ViewSourceButton = <RobrixIconButton> {
	width: Fill
	height: Fit
	align: {x: 0}

	draw_bg: {
	    radius: 0.
	}
	draw_icon: {
	    svg_file: (ICO_REPLY)
	}
	icon_walk: {width: 10, height: 10, margin: {top: 4.0}, }

	text: "View Source"
    }

    MessageActionBar = {{MessageActionBar}} {
        width: Fit
        height: Fit

        menu_content = <RoundedView> {
            width: Fit,
            height: 20,
            flow: Right,
	    padding: 1

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

    MessageContextMenu = {{MessageContextMenu}} {
        width: Fit
        height: Fit
        flow: Overlay

        menu_content = <RoundedView> {
            width: 150,
            height: Fit,
            flow: Down,
	    padding: 1

            draw_bg: {
                color: #fff,
                border_width: 1.0,
                border_color: #D0D5DD,
                radius: 4.
            }

	    reply_button = <RobrixIconButton> {
		width: Fill
		height: Fit
		align: {x: 0}

		draw_bg: {
		    radius: 0.
		}
		draw_icon: {
		    svg_file: (ICO_REPLY)
		}
		icon_walk: {width: 10, height: 10, margin: {top: 4.0}, }

		text: "Reply"
	    }

	    view_source_button = <RobrixIconButton> {
		width: Fill
		height: Fit
		align: {x: 0}

		draw_bg: {
		    radius: 0.
		}
		draw_icon: {
		    svg_file: (ICO_REPLY)
		}
		icon_walk: {width: 10, height: 10, margin: {top: 4.0}, }

		text: "View Source"
	    }
        }
    }
}


// Base menu trait.
// Base menu trait.

trait Menu {

    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope);

}



enum MessageMenuButton {

    Reply,

    ViewSource,

}



// Message specific menu. Allows to centralize the functionality accross various menus implementations.

trait MessageMenu: Menu {
    fn item_id(&self) -> Option<usize>;
    fn room_screen_widget_uid(&self) -> Option<WidgetUid>;
    fn button_mapping(&self) -> Vec<(ButtonRef, MessageMenuButton)>;
    fn extract_context(&self) -> Option<(usize, WidgetUid)> {
        let item_id = self.item_id()?;
        let room_screen_widget_uid = self.room_screen_widget_uid()?;
        Some((item_id, room_screen_widget_uid))
    }

    /// default handler for every button variant. Overwrite for custom handler.
    fn handle_button_default(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        button: &MessageMenuButton,
    ) {
        let Some((item_id, room_screen_widget_uid)) = self.extract_context() else {
            return;
        };

        match button {
            MessageMenuButton::Reply => {
                cx.widget_action(
                    room_screen_widget_uid,
                    &scope.path,
                    MessageAction::MessageReplyButtonClicked(item_id),
                );
            }

            MessageMenuButton::ViewSource => {
                cx.widget_action(
                    room_screen_widget_uid,
                    &scope.path,
                    MessageAction::ViewSourceButtonClicked(item_id),
                );
            }
        };

        self.signal_close(cx, scope);
    }

    fn handle_buttons(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        for (button_ref, button_enum) in self.button_mapping() {
            if button_ref.clicked(actions) {
                self.handle_button_default(cx, scope, &button_enum);
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

impl Menu for MessageContextMenu {
    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(room_screen_widget_uid) = self.room_screen_widget_uid {
            cx.widget_action(
                room_screen_widget_uid,
                &scope.path,
                MessageAction::ContextMenuClose,
            );
        }
    }
}

impl MessageMenu for MessageContextMenu {
    fn item_id(&self) -> Option<usize> {
        self.item_id
    }

    fn room_screen_widget_uid(&self) -> Option<WidgetUid> {
        self.room_screen_widget_uid
    }

    fn button_mapping(&self) -> Vec<(ButtonRef, MessageMenuButton)> {
        vec![
            (self.button(id!(reply_button)), MessageMenuButton::Reply),
            (self.button(id!(view_source_button)), MessageMenuButton::ViewSource),
        ]
    }
}

impl WidgetMatchEvent for MessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
	self.handle_buttons(cx, actions, scope)
    }
}

impl MessageContextMenu {
    pub fn initialize_with_data(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
	self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.item_id = Some(item_id);
        self.redraw(cx);
    }
}

impl MessageContextMenuRef {
    pub fn initialize_with_data(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.initialize_with_data(cx, room_screen_widget_uid, item_id);
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

impl Menu for MessageActionBar {
    fn signal_close(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(room_screen_widget_uid) = self.room_screen_widget_uid {
            cx.widget_action(
                room_screen_widget_uid,
                &scope.path,
                MessageAction::ActionBarClose,
            );
        }
    }
}

impl MessageMenu for MessageActionBar {
    fn item_id(&self) -> Option<usize> {
        self.item_id
    }

    fn room_screen_widget_uid(&self) -> Option<WidgetUid> {
        self.room_screen_widget_uid
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
    pub fn selected(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
	self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.item_id = Some(item_id);
        self.redraw(cx);
    }
}

impl MessageActionBarRef {
    pub fn selected(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.selected(cx, room_screen_widget_uid, item_id);
    }
}
