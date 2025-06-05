use crossbeam_queue::SegQueue;
use makepad_widgets::*;

static POPUP_NOTIFICATION: SegQueue<PopupItem> = SegQueue::new();

/// Displays a new popup notification with the given message and auto dismissal duration.
/// 
/// Popup notifications will be shown in the order they were enqueued,
/// and can be removed when manually closed by the user or automatically.
pub fn enqueue_popup_notification(message: String, auto_dismiss_duration: Option<f64>) {
    POPUP_NOTIFICATION.push(PopupItem {
        message,
        auto_dismiss_duration,
    });
}

/// Popup notification item.
#[derive(Default)]
pub struct PopupItem {
    /// Text to be displayed in the popup.
    pub message: String,
    /// Duration in seconds after which the popup will be automatically closed.
    pub auto_dismiss_duration: Option<f64>,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::RobrixIconButton;
    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")

    pub PopupNotificationBase = {{RobrixPopupNotification}} {}
    PopupDialog = <RoundedView> {
        width: 275
        height: Fit
        padding: {top: 5, right: 5, bottom: 5, left: 10}
        flow: Overlay
        align: {y: 0.0}
        show_bg: true
        draw_bg: {
            color: #fff
            fn pixel(self) -> vec4 {
                let border_color = #d4;
                let border_size = 1;
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
                    border_size
                )
                return sdf.result
            }
        }
        progress_bar = <View> {
            width: Fill,
            height: Fit,
            show_bg: true
            draw_bg: {
                instance right_margin: 25.0,
                instance top_margin: 35.0,
                instance border_radius: 2.,
                instance progress_bar_width: 15.0,
                instance progress_bar_color: (COLOR_AVATAR_BG_IDLE),
                instance progress_bar_background_color: (COLOR_DISABLE_GRAY),
                instance display_progress_bar: 1.0
                uniform anim_time: 0.5,
                uniform anim_duration: 2.0,
                color: #fff
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    let rect_size = self.rect_size;
                    if self.display_progress_bar > 0.5 {
                        sdf.box(
                            rect_size.x - self.right_margin,
                            self.top_margin,
                            self.progress_bar_width,
                            rect_size.y - self.top_margin,
                            max(1.0, self.border_radius)
                        )
                        sdf.fill(self.progress_bar_background_color);
                        sdf.box(
                            rect_size.x - self.right_margin,
                            self.top_margin,
                            self.progress_bar_width,
                            min(rect_size.y - self.top_margin, (rect_size.y - self.top_margin) * self.anim_time / self.anim_duration),
                            max(1.0, self.border_radius)
                        )
                        sdf.fill(self.progress_bar_color);
                    }
                    return sdf.result
                }
            }
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
                spacing: 0,
                align: {x: 0.5, y: 0.0}
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x888;
                    }
                }
                icon_walk: {width: 12, height: 12}
            }
        }

        animator: {
            mode = {
                default: close_slider,
                close_slider = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        progress_bar = {
                            draw_bg: {anim_time: 0.0}
                        }
                    }
                }
                slide_down = {
                    redraw: true,
                    from: {all: Forward {duration: 100000.0}}
                    apply: {
                        progress_bar = {
                            draw_bg: {anim_time: 100000.0}
                        }
                    }
                }
            }
            hover = {
                default: off
                off = {
                    apply: { }
                }
                on = {
                    apply: { }
                }
            }
            down = {
                default: off
                off = {
                    apply: { }
                }
                on = {
                    apply: { }
                }
            }
        }
    }
    pub RobrixPopupNotification = <PopupNotificationBase> {
        width: 275
        height: Fit
        flow: Down
        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <PopupDialog> {}
    }
    pub PopupList = <View> {
        width: Fill,
        height: Fill,
        align: {x: 0.99, y: 0.05}
        <RobrixPopupNotification>{}
    }
}

#[derive(Live, Widget)]
pub struct RobrixPopupNotification {
    #[live]
    content: Option<LivePtr>,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,

    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    // A list of tuples containing individual widgets, their content and the close timer in the order they were added.
    #[rust]
    popups: Vec<(View, String, Timer)>,
}

impl LiveHook for RobrixPopupNotification {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        self.draw_list.redraw(cx);
        for (view, _, _) in self.popups.iter_mut() {
            if let Some(index) = nodes.child_by_name(index, live_id!(popup_content).as_field()) {
                view.apply(cx, apply, index, nodes);
            }
        }
    }
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        while let Some(popup_item) = POPUP_NOTIFICATION.pop() {
            self.push(cx, popup_item);
        }
        if self.popups.is_empty() {
            return;
        }

        let mut removed_indices = Vec::new();
        for (index, (view, _message, close_popup_timer)) in self.popups.iter_mut().enumerate() {
            if close_popup_timer.is_event(event).is_some() {
                removed_indices.push(index);
            }
            view.handle_event(cx, event, scope);
        }
        self.widget_match_event(cx, event, scope);
        if removed_indices.is_empty() {
            return;
        }
        for &i in removed_indices.iter() {
            self.popups.remove(i);
        }
        self.draw_bg.redraw(cx);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.draw_list.begin_overlay_reuse(cx);
        self.draw_bg.begin(cx, walk, self.layout);
        if !self.popups.is_empty() {
            cx.begin_turtle(walk, self.layout);
            for (view, _, _) in self.popups.iter_mut() {
                let walk = walk.with_margin_bottom(5.0);
                let _ = view.draw_walk(cx, scope, walk);
            }
            cx.end_turtle();
        }
        self.draw_bg.end(cx);
        self.draw_list.end(cx);
        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    /// Adds a new popup with a close button to the right side of the screen.
    ///
    /// The popup's content is a string given by the `PopupItem` parameter.
    /// New popup will be displayed below the previous ones.
    pub fn push(&mut self, cx: &mut Cx, popup_item: PopupItem) {
        let mut view = View::new_from_ptr(cx, self.content);
        view.label(id!(popup_label))
            .set_text(cx, &popup_item.message);
        let close_timer = if let Some(duration) = popup_item.auto_dismiss_duration {
            view.apply_over(
                cx,
                live! {
                    progress_bar = {
                        draw_bg: {anim_duration: (duration)}
                    }
                },
            );
            view.animator_play(cx, id!(mode.slide_down));
            cx.start_timeout(duration)
        } else {
            view.apply_over(
                cx,
                live! {
                    progress_bar = {
                        draw_bg: {display_progress_bar: (0.0)}
                    }
                },
            );
            Timer::empty()
        };
        self.popups.push((view, popup_item.message, close_timer));
        self.redraw(cx);
    }
}

impl WidgetMatchEvent for RobrixPopupNotification {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let mut removed_indices = Vec::new();
        for (i, (view, _data, close_timer)) in self.popups.iter_mut().enumerate() {
            if view.button(id!(close_button)).clicked(actions) {
                removed_indices.push(i);
                cx.stop_timer(*close_timer);
                view.animator_cut(cx, id!(mode.close_slider));
            }
        }
        if removed_indices.is_empty() {
            return;
        }
        for &i in removed_indices.iter() {
            self.popups.remove(i);
        }
        self.draw_bg.redraw(cx);
    }
}
