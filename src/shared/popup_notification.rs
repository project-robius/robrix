use std::time::Instant;

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    ICO_CLOSE = dep("crate://self/resources/icons/close.svg")
    ICO_CHECK = dep("crate://self/resources/icons/checkmark.svg")

    Progress = <View> {
        width: 20,
        height: Fill,
        flow: Overlay,

        <RoundedView> {
            width: Fill,
            height: Fill,
            draw_bg: {
                color: #42660a,
                radius: 4.0,
            }
        }

        progress_bar = <RoundedView> {
            height: Fill,
            width: Fill,
            draw_bg: {
                color: #639b0d,   // #42660a
                radius: 4.0,
            }
        }
    }

    TipContent = <View> {
        width: Fill,
        height: Fill,
        spacing: 15.0,
        flow: Right,
        align: {
            x: 0.0,
            y: 0.5,
        }
        margin: { left: 20.0 }

        <Icon> {
            draw_icon: {
                svg_file: (ICO_CHECK),
                color: #42660a,
            }
            icon_walk: { width: 18, height: 18 }
        }

        <Label> {
            draw_text: {
                color: #42660a,
                text_style: {
                    font_size: 12
                }
            }
            text: "Successfully updated transaction",
        }

        close_icon = <View> {
            width: Fit,
            height: Fit,
            cursor: Hand,
            <Icon> {
                draw_icon: {
                    svg_file: (ICO_CLOSE),
                    color: #6cc328
                }
    
                icon_walk: { width: 16, height: 16 }
            }
        }
        
    }

    PopupDialog = <RoundedView> {
        width: 375,
        height: 100,
        flow: Right,

        show_bg: true,
        draw_bg: {
            color: #d3f297,
        }

        <Progress> {}
        <TipContent> {}
    }

    pub RobrixPopupNotification = {{RobrixPopupNotification}} {
        width: Fit
        height: Fit
        flow: Overlay
        abs_pos: vec2(10.0, 10.0)
        duration: 2.0

        draw_bg: {
            fn pixel(self) -> vec4 {
                return vec4(0., 0., 0., 0.0)
            }
        }

        content: <PopupDialog> {}

        animator: {
            mode = {
                default: close,
                open = {
                    redraw: true,
                    from: {all: Forward {duration: 2.0}}
                    ease: OutQuad
                    apply: {
                        abs_pos: vec2(60.0, 10.0),
                    }
                }
                close = {
                    redraw: true,
                    from: {all: Forward {duration: 1.0}}
                    ease: InQuad
                    apply: {
                        abs_pos: vec2(-1000.0, 10.0),
                    }
                }
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct RobrixPopupNotification {
    #[live]
    #[find]
    content: View,

    #[live]
    duration: f64,

    #[rust(DrawList2d::new(cx))]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,

    #[rust]
    opened: bool,

    #[rust]
    animation_timer: Timer,

    #[rust]
    duration_timer: Timer,

    #[rust]
    start_time: Option<Instant>,

    #[animator]
    animator: Animator,

    #[rust]
    redraw_timer: Timer,
}

impl LiveHook for RobrixPopupNotification {
    fn after_apply(&mut self, cx: &mut Cx, _apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        self.draw_list.redraw(cx);
    }
}

impl Widget for RobrixPopupNotification {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.opened {
            return;
        }

        if self.animation_timer.is_event(event).is_some() {
            self.start_time = Some(Instant::now());
            self.duration_timer = cx.start_timeout(self.duration);
        }

        if self.duration_timer.is_event(event).is_some() {
            self.update_animation(cx);
        }

        if self.redraw_timer.is_event(event).is_some() {
            if let Some(start_time) = self.start_time {
                let elapsed = start_time.elapsed().as_secs_f64();
                let progress = (elapsed / self.duration).min(1.0);
                let progress_bar_height = 100.0 * progress;
    
                self.view(id!(progress_bar)).apply_over(
                    cx,
                    live! {
                        height: (progress_bar_height)
                    } 
                );
            }
        };

        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        if let Event::MouseDown(e) = event {
            if self.view(id!(close_icon)).area().rect(cx).contains(e.abs) {
                self.close(cx);
                return;
            }
        }

        self.content.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw_list.begin_overlay_reuse(cx);

        cx.begin_pass_sized_turtle(self.layout);
        self.draw_bg.begin(cx, self.walk, self.layout);

        if self.opened {
            self.content.draw_all(cx, scope);
        }

        self.draw_bg.end(cx);

        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);

        DrawStep::done()
    }
}

impl RobrixPopupNotification {
    pub fn open(&mut self, cx: &mut Cx) {
        self.opened = true;
        self.animation_timer = cx.start_timeout(2.0);
        self.redraw_timer = cx.start_interval(0.016);
        self.animator_play(cx, id!(mode.open));
        self.redraw(cx);
    }

    pub fn close(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.redraw_timer);
        self.animator_play(cx, id!(mode.close));
        self.redraw(cx);
    }

    pub fn update_animation(&mut self, cx: &mut Cx) {
        if self.animator_in_state(cx, id!(mode.open)) {
            cx.stop_timer(self.redraw_timer);
            self.animator_play(cx, id!(mode.close));
        }
    }
}

impl RobrixPopupNotificationRef {
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open(cx);
        }
    }

    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }
}
