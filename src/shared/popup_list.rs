use crossbeam_queue::SegQueue;
use makepad_widgets::*;

static POPUP_NOTIFICATION: SegQueue<String> = SegQueue::new();

/// Displays a new popup notification with the given message.
/// 
/// Popup notifications will be shown in the order they were enqueued,
/// and are currently only removed when manually closed by the user.
pub fn enqueue_popup_notification(message: String) {
    POPUP_NOTIFICATION.push(message);
    Cx::post_action(PopupNotificationAction::Open);
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")

    PopupDialog = <RoundedView> {
        width: 275
        height: Fit
        margin: {top: 20, right: 20}
        padding: 0

        show_bg: true
        draw_bg: {
            color: #fff
            instance border_radius: 4.0
            fn pixel(self) -> vec4 {
                let border_color = #d4;
                let border_width = 1;
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let body = #fff

                sdf.box(
                    1.,
                    1.,
                    self.rect_size.x - 2.0,
                    self.rect_size.y - 2.0,
                    self.border_radius
                )
                sdf.fill_keep(body)

                sdf.stroke(
                    border_color,
                    border_width
                )
                return sdf.result
            }
        }
    }

    pub PopupList = {{PopupList}} {
        width: 275,
        height: Fit
        flow: Down
        spacing: 0,
        popup_content: <PopupDialog> {
            flow: Right
            padding: {top: 5, right: 5, bottom: 5, left: 10}
            align: {y: 0.0}

            <View> {
                width: Fill,
                height: Fit,
                align: {x: 0.0, y: 0.5}
                padding: {left: 5, top: 10, bottom: 10, right: 0}
                popup_label = <Label> {
                    width: Fill,
                    height: Fit,
                    draw_text: {
                        color: #000,
                        text_style: <MESSAGE_TEXT_STYLE>{ font_size: 10 },
                        wrap: Word
                    }
                }
            }
            // The "X" close button on the top right
            close_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                margin: {left: 0, top: 4, bottom: 4, right: 4},
                padding: 8,
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x888;
                    }
                }
                icon_walk: {width: 12, height: 12}
            }
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
    popups: Vec<(View, String)>,
}

impl LiveHook for PopupList {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for (button, _ ) in self.popups.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(popup_content).as_field()) {
                button.apply(cx, apply, index, nodes);
            }
        }
    }
}

impl Widget for PopupList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        while let Some(message) = POPUP_NOTIFICATION.pop() {
            self.push(cx, message);            
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
        for (view, data) in self.popups.iter_mut() {
            view.label(id!(popup_label)).set_text(cx, data);
            let walk = walk.with_margin_bottom(10.0);
            let _ = view.draw_walk(cx, scope, walk);
        }
        cx.end_turtle();
        DrawStep::done()
    }
}
impl PopupList {
    /// Adds a new popup with a close button to the right side of the screen. 
    /// 
    /// The popup's content is a string given by the `message` parameter.
    /// New popup will be displayed below the previous ones. 
    pub fn push(&mut self, cx: &mut Cx, message: String) {
        self.popups.push((View::new_from_ptr(cx, self.popup_content), message));
        self.redraw(cx);
    }
}
impl WidgetMatchEvent for PopupList {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let mut removed_indices = Vec::new();
        for (i, (view, _data)) in self.popups.iter().enumerate() {
            if view.button(id!(close_button)).clicked(actions) {
                removed_indices.push(i);
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
            Cx::post_action(PopupNotificationAction::Close);
        }
    }
}

impl PopupListRef {
    /// See [`PopupList::push()`].
    pub fn push(&self, cx: &mut Cx, message: String) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.push(cx, message);
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