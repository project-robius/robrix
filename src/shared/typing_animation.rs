use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::landing::model_card::ModelCard;

    ANIMATION_SPEED = 0.33

    // 1. Set the width and height to the same value.
    // 2. Set the radius to half of the width/height.
    TypingSign = <CircleView> {
        width: 4
        height: 4
        draw_bg: {
            radius: 2.0
            fn get_color(self) -> vec4 {
                let top_color = #121570;
                let bottom_color = #A4E0EF;
                let gradient_ratio = self.pos.y;
                return mix(top_color, bottom_color, gradient_ratio);
            }
        }
    }

    TypingAnimation = {{TypingAnimation}} {
        width: Fit,
        height: Fit,

        flow: Down,
        spacing: 10,
        align: {x: 0.0, y: 0.5},

        content = <View> {
            width: Fit,
            height: Fit,
            spacing: 10,
            circle1 = <TypingSign> {}
            circle2 = <TypingSign> {}
            circle3 = <TypingSign> {}
        }

        animator: {
            circle1 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle1 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle1 = { margin: {top: 0.0} }}}
                }
            }

            circle2 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle2 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle2 = { margin: {top: 0.0} }}}
                }
            }

            circle3 = {
                default: down,
                down = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle3 = { margin: {top: 10.0} }}}
                }
                up = {
                    redraw: true,
                    from: {all: Forward {duration: (ANIMATION_SPEED * 0.5)}}
                    apply: {content = { circle3 = { margin: {top: 0.0} }}}
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TypingAnimation {
    #[deref]
    view: View,

    #[animator]
    animator: Animator,

    #[rust]
    timer: Timer,

    #[rust]
    current_animated_circle: usize,
}

impl Widget for TypingAnimation {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.timer.is_event(event).is_some() {
            self.update_animation(cx);
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
        // log!("update animation -----");
        self.current_animated_circle = (self.current_animated_circle + 1) % 3;

        match self.current_animated_circle {
            0 => {
                self.animator_play(cx, id!(circle1.up));
                self.animator_play(cx, id!(circle3.down));
            }
            1 => {
                self.animator_play(cx, id!(circle1.down));
                self.animator_play(cx, id!(circle2.up));
            }
            2 => {
                self.animator_play(cx, id!(circle2.down));
                self.animator_play(cx, id!(circle3.up));
            }
            _ => unreachable!(),
        };

        self.timer = cx.start_timeout(0.33 * 0.5);
    }
}

impl TypingAnimationRef {
    pub fn animate(&mut self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.update_animation(cx);
    }

    pub fn stop_animation(&mut self) {
        let Some(mut inner) = self.borrow_mut() else {
            return;
        };
        inner.timer = Timer::default();
    }
}
