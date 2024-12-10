use makepad_widgets::*;
use matrix_sdk::ruma::EventId;

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
        flow: Overlay

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

impl MessageContextMenu {
    pub fn selected(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
	self.room_screen_widget_uid = Some(room_screen_widget_uid);
        self.item_id = Some(item_id);
        self.redraw(cx);
    }
}

impl MessageContextMenuRef {
    pub fn selected(&mut self, cx: &mut Cx, room_screen_widget_uid: WidgetUid, item_id: usize) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.selected(cx, room_screen_widget_uid, item_id);
    }
}

impl WidgetMatchEvent for MessageContextMenu {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
	// If widget is not ready do nothing.
	let Some(item_id) = self.item_id else {
	    return;
	};
	let Some(room_screen_widget_uid) = self.room_screen_widget_uid else {
	    return;
	};

	let widget_uid = self.widget_uid();

        if self.button(id!(reply_button)).clicked(actions) {
            cx.widget_action(
		room_screen_widget_uid,
		&scope.path,
		MessageAction::MessageReplyButtonClicked(
		    item_id,
		)
	    );

	    cx.widget_action(
		widget_uid,
		&scope.path,
		MessageAction::ContextMenuClose
	    );
        }

        if self.button(id!(view_source_button)).clicked(actions) {
            cx.widget_action(
		room_screen_widget_uid,
		&scope.path,
		MessageAction::ViewSourceButtonClicked(
		    item_id,
		)
	    );

	    cx.widget_action(
		widget_uid,
		&scope.path,
		MessageAction::ContextMenuClose
	    );
        }

    }
}

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

impl WidgetMatchEvent for MessageActionBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
	// If widget is now ready do nothing.
	let Some(item_id) = self.item_id else {
	    return;
	};
	let Some(room_screen_widget_uid) = self.room_screen_widget_uid else {
	    return;
	};

	let widget_uid = self.widget_uid();

        if self.button(id!(reply_button)).clicked(actions) {
            cx.widget_action(
		room_screen_widget_uid,
		&scope.path,
		MessageAction::MessageReplyButtonClicked(
		    item_id,
		)
	    );

	    cx.widget_action(
		widget_uid,
		&scope.path,
		MessageAction::ActionBarClose
	    );
        }

    }
}
