use crossbeam_queue::SegQueue;
use makepad_widgets::*;

use super::popup_notification::{
    PopupItem, RobrixPopupNotificationAction, RobrixPopupNotificationWidgetExt,
};
/// A queue of pending popup notifications with the text and optional dismiss duration.
static POPUP_NOTIFICATION: SegQueue<PopupItem> = SegQueue::new();

/// Displays a new popup notification with the given message.
///
/// Popup notifications will be shown in the order they were enqueued,
/// and are currently only removed when manually closed by the user.
pub fn enqueue_popup_notification(message: String, auto_dismiss_duration: Option<f64>) {
    POPUP_NOTIFICATION.push(PopupItem {
        message,
        auto_dismiss_duration,
    });
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::popup_notification::RobrixPopupNotification;

    pub PopupList = {{PopupList}} {
        width: 300,
        height: Fit
        flow: Down
        popup_content: <View> {
            height: Fit,
            flow: Down
            padding: {top: 0, right: 5, bottom: 0, left: 5}
            align: {y: 0.0}
            spacing: 0,
            <View> {
                width: Fill,
                height: Fit,
                padding: 0,
                align: {x: 1.0},
                cancel_button = <RobrixIconButton> {
                    align: {x: 0.5, y:0.5}
                    margin: 0,
                    width: Fit, height: Fit,
                    padding: 5,
                    spacing: 0,
                    draw_bg: {
                        border_color: (COLOR_DANGER_RED),
                        color: #fff0f0 // light red
                        border_radius: 2
                    }
                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        color: (COLOR_DANGER_RED)
                    }
                    icon_walk: {width: 10, height: 10, margin: 0}
                }
            }

            robrix_popup = <RobrixPopupNotification> {}
        }
    }
}

/// A widget that displays a vertical list of popups.
#[derive(Live, Widget)]
pub struct PopupList {
    #[deref]
    view: View,
    #[layout]
    layout: Layout,
    /// A pointer to the popup content widget.
    #[live]
    popup_content: Option<LivePtr>,
    /// A list of tuples containing individual widgets and their content in the order they were added.
    #[rust]
    popups: Vec<(View, PopupItem)>,
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
}

impl LiveHook for PopupList {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for (button, _) in self.popups.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(popup_content).as_field()) {
                button.apply(cx, apply, index, nodes);
            }
        }
    }
}

impl Widget for PopupList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        while let Some(popup_item) = POPUP_NOTIFICATION.pop() {
            self.push(cx, popup_item);
        }
        for (view, _) in self.popups.iter_mut() {
            view.handle_event(cx, event, scope);
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.popups.is_empty() {
            return DrawStep::done();
        }
        cx.begin_turtle(walk, self.layout);
        self.draw_bg.begin(cx, self.walk, self.layout);
        for (view, _popup_item) in self.popups.iter_mut() {
            let walk = walk.with_margin_bottom(5.0);
            let _ = view.draw_walk(cx, scope, walk);
        }
        self.draw_bg.end(cx);
        cx.end_turtle();
        DrawStep::done()
    }
}
impl PopupList {
    /// Adds a new popup with a close button to the right side of the screen.
    ///
    /// The popup's content is a string given by the `message` parameter.
    /// New popup will be displayed below the previous ones.
    pub fn push(&mut self, cx: &mut Cx, popup_item: PopupItem) {
        let view = View::new_from_ptr(cx, self.popup_content);
        view.robrix_popup_notification(id!(robrix_popup))
            .open(cx, popup_item.auto_dismiss_duration);
        view.robrix_popup_notification(id!(robrix_popup))
            .label(id!(popup_text))
            .set_text(cx, &popup_item.message);
        self.popups.push((view, popup_item));
        self.redraw(cx);
    }
}

impl WidgetMatchEvent for PopupList {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let mut removed_indices = Vec::new();
        for (i, (view, _data)) in self.popups.iter().enumerate() {
            if view.button(id!(cancel_button)).clicked(actions) {
                removed_indices.push(i);
            }
            for action in actions {
                let widget_uid = view
                    .robrix_popup_notification(id!(robrix_popup))
                    .widget_uid();
                if let RobrixPopupNotificationAction::Ended =
                    action.as_widget_action().widget_uid_eq(widget_uid).cast()
                {
                    removed_indices.push(i);
                }
            }
        }
        if removed_indices.is_empty() {
            return;
        }
        for &i in removed_indices.iter() {
            self.popups.remove(i);
        }
        for (view, _) in self.popups.iter_mut() {
            view.redraw(cx);
        }
        if self.popups.is_empty() {
            self.draw_bg.redraw(cx);
        }
    }
}

impl PopupListRef {
    /// See [`PopupList::push()`].
    pub fn push(&self, cx: &mut Cx, popup_item: PopupItem) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.push(cx, popup_item);
        }
    }
}

/// Popup notification actions
#[derive(Clone, DefaultNone, Debug)]
pub enum PopupNotificationAction {
    None,
    /// Open popup notification layer
    Open,
    /// Close popup notification layer
    Close,
}
