use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    ANIMATION_DURATION = 0.65

    TypingAnimation = {{TypingAnimation}} {
        width: 24,
        height: 15,
        flow: Down,
        show_bg: true,
        draw_bg: {
            uniform freq: 5.0
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let color = vec4(0.0, 0.0, 0.0, 1.0);
                // // Create three circle SDFs
                sdf.circle(
                    self.rect_size.x * 0.25, 
                    self.rect_size.y * 0.3 * sin(self.time * self.freq) + self.rect_size.y * 0.4, 
                    1.6
                );
                sdf.fill(color);
                sdf.circle(
                    self.rect_size.x * 0.5, 
                    self.rect_size.y * 0.3 * sin(self.time * self.freq + 180.0) + self.rect_size.y * 0.4, 
                    1.6
                );
                sdf.fill(color);
                sdf.circle(
                    self.rect_size.x * 0.75, 
                    self.rect_size.y * 0.3 * sin(self.time * self.freq) + self.rect_size.y * 0.4, 
                    1.6
                );
                sdf.fill(color);
                return sdf.result;
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TypingAnimation {
    #[deref] view: View,
    #[live] time: f32,
    #[rust] next_frame: NextFrame,
    #[rust] is_play: bool,
}

impl Widget for TypingAnimation {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(ne) = self.next_frame.is_event(event) {
            self.time = (ne.frame % 360) as f32;
            self.redraw(cx);
            if !self.is_play {
                return
            }
            self.next_frame = cx.new_next_frame();
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl TypingAnimationRef {
    /// Starts animation of the bouncing dots.
    pub fn animate(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_play = true;
            inner.next_frame = cx.new_next_frame();
        }
    }
    /// Stops animation of the bouncing dots.
    pub fn stop_animation(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.is_play = false;
        }
    }
}
