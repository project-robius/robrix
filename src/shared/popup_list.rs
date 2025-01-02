use crossbeam_queue::SegQueue;
use makepad_widgets::*;

static POPUP_NOTIFICATION: SegQueue<String> = SegQueue::new();
pub fn enqueue_popup_notification(update: String) {
    POPUP_NOTIFICATION.push(update);
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")

    PopupDialog = <RoundedView> {
        width: 200
        height: Fit
        margin: {top: 20, right: 20}
        padding: {top: 20, right: 20, bottom: 20, left: 20}
        spacing: 15

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

    PopupCloseButton = <Button> {
        width: Fit,
        height: Fit,
        margin: {top: -8}

        draw_icon: {
            svg_file: (ICO_CLOSE),
            fn get_color(self) -> vec4 {
                return #000;
            }
        }
        icon_walk: {width: 12, height: 12}
    }

    pub PopupList = {{PopupList}} {
        width: Fit
        height: Fit
        flow: Down
        popup = <PopupDialog> {
            close_button = <PopupCloseButton> {}
        }
        popup_meta: <PopupDialog> {
            room_status_label = <Label> {
                width: 110
                text: "......"
                draw_text: {
                    color: #000
                }
            }
            close_button = <PopupCloseButton> {}
        }
    }

}

#[derive(Live, Widget)]
pub struct PopupList {
    #[deref]
    view: View,
    #[layout]
    layout: Layout,
    #[live]
    popup_meta: Option<LivePtr>,
    #[rust]
    popups: Vec<View>,
    #[rust]
    popups_data: Vec<String>,
}
impl LiveHook for PopupList {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        for button in self.popups.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(popup_meta).as_field()) {
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
        for view in self.popups.iter_mut() {
            view.handle_event(cx, event, scope);
        }
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut data = self.popups_data.iter();
        cx.begin_turtle(walk, self.layout);
        for view in self.popups.iter_mut() {
            if let Some(status) = data.next_back() {
                view.label(id!(room_status_label)).set_text(&status);
                let walk = walk.with_margin_bottom(10.0);
                let _ = view.draw_walk(cx, scope, walk);
            }
        }
        cx.end_turtle();
        DrawStep::done()
    }
}
impl PopupList {
    fn push(&mut self, cx: &mut Cx, message: String) {
        self.popups_data.push(message);
        let meta = self.popup_meta;
        if self.popups.len() < self.popups_data.len() {
            for _ in self.popups.len()..self.popups_data.len() {
                self.popups.push(View::new_from_ptr(cx, meta));
            }
        }
        cx.redraw_area(self.area());
    }
}
impl WidgetMatchEvent for PopupList {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let mut removed = false;
        for (i, view) in self.popups.iter().enumerate() {
            if view.button(id!(close_button)).clicked(actions) {
                self.popups_data.remove(self.popups_data.len() - i - 1);
                removed = true;
            }
        }
        if removed {
            for view in self.popups.iter_mut() {
                view.redraw(cx);
            }
        }
    }
}

impl PopupListRef {
    /// Add a new popup to the list. The popup's content is a string given by the `message` parameter.
    /// The popup will be displayed in the order it was added. The popup will be removed from the list
    /// when it is closed by the user. The list will be redrawn after pushing a new popup.
    pub fn push(&self, cx: &mut Cx, message: String) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.push(cx, message);
        }
    }
}
