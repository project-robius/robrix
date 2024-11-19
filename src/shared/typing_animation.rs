use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    ANIMATION_DURATION = 0.65

    // 1. Set the width and height to the same value.
    // 2. Set the radius to half of the width/height.
    EllipsisDot = <CircleView> {
        width: 3
        height: 3
        draw_bg: {
            radius: 1.5
            color: (TYPING_NOTICE_TEXT_COLOR)
        }
    }

    TypingAnimation = {{TypingAnimation}} {
        width: Fit,
        height: Fit,

        flow: Down,
        align: {x: 0.0, y: 0.5},
        
        content = <View> {
            width: Fit,
            height: Fit,
            spacing: 2,
            circle1 = <EllipsisDot> {}
            circle2 = <EllipsisDot> {}
            circle3 = <EllipsisDot> {}
        }

        animator: {
            circle1 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle1 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle1 = { margin: {top: 3.0} }}}
                }
            }

            circle2 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle2 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle2 = { margin: {top: 3.0} }}}
                }
            }

            circle3 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle3 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_DURATION * 0.5)}}
                    apply: {content = { circle3 = { margin: {top: 3.0} }}}
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TypingAnimation {
    #[deref] view: View,
    #[animator] animator: Animator,

    #[live(0.65)] animation_duration: f64,
    #[rust] timer: Option<Timer>,
    #[rust] current_animated_dot: CurrentAnimatedDot,
}

#[derive(Copy, Clone, Default)]
enum CurrentAnimatedDot {
    #[default]
    Dot1,
    Dot2,
    Dot3,
}
impl CurrentAnimatedDot {
    fn next(&self) -> Self {
        match self {
            Self::Dot1 => Self::Dot2,
            Self::Dot2 => Self::Dot3,
            Self::Dot3 => Self::Dot1,
        }
    }
}

impl Widget for TypingAnimation {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(timer) = self.timer {
            if timer.is_event(event).is_some() {
                self.update_animation(cx);
            }
        }
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TypingAnimation {
    pub fn update_animation(&mut self, cx: &mut Cx) {
        self.current_animated_dot = self.current_animated_dot.next();

        match self.current_animated_dot {
            CurrentAnimatedDot::Dot1 => {
                self.animator_play(cx, id!(circle1.up));
                self.animator_play(cx, id!(circle3.down));
            }
            CurrentAnimatedDot::Dot2 => {
                self.animator_play(cx, id!(circle1.down));
                self.animator_play(cx, id!(circle2.up));
            }
            CurrentAnimatedDot::Dot3 => {
                self.animator_play(cx, id!(circle2.down));
                self.animator_play(cx, id!(circle3.up));
            }
        };

        self.timer = Some(cx.start_timeout(self.animation_duration * 0.5));
    }
}

impl TypingAnimationRef {
    pub fn animate(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_animation(cx);
        }
    }

    pub fn stop_animation(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.timer = None;
        }
    }
}
