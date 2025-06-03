use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::icon_button::*;
    use crate::shared::styles::*;

    PopupContent = <View> {
        width: Fill,
        height: Fit,
        flow: Right,
        show_bg: true,
        popup_text = <Label> {
            width: Fill,
            height: Fit,
            margin: {left: 40}
            draw_text: {
                color: #42660a,
                text_style: {
                    font_size: 10,
                }
                wrap: Word
            }
        }
        // Draw rounded edge rectangular progress bar.
        draw_bg: {
            instance progress_bar_right_margin: 10.0,
            instance progress_bar_bottom_margin: 10.0,
            instance border_radius: 2.,
            instance progress_bar_color: #639b0d,
            uniform anim_time: 0.0,
            uniform anim_duration: 10.0,
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let rect_size = self.rect_size;
                sdf.box(
                    self.progress_bar_right_margin,
                    0.0,
                    20.0,
                    self.rect_size.y * self.anim_time / self.anim_duration - self.progress_bar_bottom_margin,
                    max(1.0, self.border_radius)
                )
                sdf.fill(self.progress_bar_color);
                return sdf.result;
            }
        }
        animator: {
            mode = {
                default: close_slider,
                close_slider = {
                    redraw: true,
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        draw_bg: {anim_time: 0.0}
                    }
                }
                slide_down = {
                    redraw: true,
                    from: {all: Forward {duration: 100000.0}}
                    apply: {
                        draw_bg: {anim_time: 100000.0 }
                    }
                }
            }
        }
    }

    PopupDialog = <RoundedView> {
        width: Fill,
        height: Fit,
        flow: Right,
        draw_bg: {
            color: #d3f297,
        }
        popup_content = <PopupContent> {}
    }

    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        width: Fit,
        height: Fit,
        flow: Overlay,
        margin: {top: 0.0},
        visible: false,
        content: <PopupDialog> {}
    }
}
/// Popup notification item
#[derive(Default)]
pub struct PopupItem {
    /// Text to be displayed in the popup.
    pub message: String,
    /// Duration in seconds after which the popup will be automatically closed.
    pub auto_dismiss_duration: Option<f64>,
}

#[derive(Live, Widget, LiveHook)]
pub struct RobrixPopupNotification {
    #[live]
    #[find]
    content: View,
    #[live]
    text: String,
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[visible]
    #[live(true)]
    visible: bool,
    #[rust]
    close_popup_timer: Timer,
    #[animator]
    animator: Animator,
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.close_popup_timer.is_event(event).is_some() {
            self.close(cx);
        }
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        self.content.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        self.content.draw_all(cx, scope);
        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    pub fn open(&mut self, cx: &mut Cx, auto_dismiss_duration: Option<f64>) {
        // End shortly after 0.5 to ensure the slide_down animation is complete.
        if let Some(duration) = auto_dismiss_duration {
            self.close_popup_timer = cx.start_timeout(duration + 0.8);
            self.view(id!(popup_content)).apply_over(
                cx,
                live! {
                    draw_bg: {
                        anim_duration: (duration)
                    }
                },
            );
            self.view(id!(popup_content))
                .animator_play(cx, id!(mode.slide_down));
        }
        self.visible = true;
        self.redraw(cx);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.view(id!(popup_content))
            .animator_play(cx, id!(mode.close_slider));
        cx.widget_action(
            self.widget_uid(),
            &Scope::empty().path,
            RobrixPopupNotificationAction::Ended,
        );
        self.redraw(cx);
    }
}

impl RobrixPopupNotificationRef {
    pub fn open(&self, cx: &mut Cx, auto_dismiss_duration: Option<f64>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open(cx, auto_dismiss_duration);
        }
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }
}

#[derive(DefaultNone, Clone, Debug)]
pub enum RobrixPopupNotificationAction {
    Ended,
    None,
}
