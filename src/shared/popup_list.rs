use crossbeam_queue::SegQueue;
use makepad_widgets::*;

use crate::app::PopupNotificationAction;

static POPUP_NOTIFICATION: SegQueue<String> = SegQueue::new();
pub fn enqueue_popup_notification(update: String) {
    Cx::post_action(PopupNotificationAction::Open);
    POPUP_NOTIFICATION.push(update);
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
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
        width: 190,
        height: Fit
        flow: Down
        spacing: 0,
        padding: 0,
        popup_content: <PopupDialog> {
            flow: Down
            padding: 0.0,
            padding: {left: 20.0, bottom: 10.0}
            spacing: 0,
            <View> {
                width: Fill,
                height: Fit,
                padding: 2,
                align: {x: 0.98}
                close_button = <RobrixIconButton> {
                    width: 20,
                    height: 20,
                    margin: 0,
                    padding: 0,
                    draw_icon: {
                        svg_file: (ICON_CLOSE),
                        fn get_color(self) -> vec4 {
                            return #x0;
                        }
                    }
                    icon_walk: {width: 14, height: 14}
                }
            }
            
            popup_label = <Label> {
                width: Fill,
                text: "......"
                draw_text: {
                    color: #000,
                    wrap: Word
                }
            }
        }
    }

}

#[derive(Live, LiveHook, Widget)]
pub struct PopupList {
    #[deref]
    view: View,
    #[layout]
    layout: Layout,
    #[live]
    popup_content: Option<LivePtr>,
    /// A list of tuples containing individual widgets and their content in the order they were added.
    #[rust]
    popups: Vec<(View, String)>,
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
            view.label(id!(popup_label)).set_text(data);
            let walk = walk.with_margin_bottom(10.0);
            let _ = view.draw_walk(cx, scope, walk);
        }
        cx.end_turtle();
        DrawStep::done()
    }
}
impl PopupList {
    /// Add a new popup to the list. The popup's content is a string given by the `message` parameter.
    /// The popup will be displayed in the order it was added. The popup will be removed from the list
    /// when it is closed by the user. The list will be redrawn after pushing a new popup.
    fn push(&mut self, cx: &mut Cx, message: String) {
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
