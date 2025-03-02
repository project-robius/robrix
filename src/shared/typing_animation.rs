use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub TypingAnimation = {{TypingAnimation}} {
        width: 24,
        height: 12,
        flow: Down,
        show_bg: true,
        draw_bg: {
            color: #x000
            uniform freq: 5.0,  // Animation frequency
            uniform phase_offset: 102.0, // Phase difference
            uniform dot_radius: 1.5, // Dot radius
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let amplitude = self.rect_size.y * 0.22;
                let center_y = self.rect_size.y * 0.5;
                // Create three circle SDFs
                sdf.circle(
                    self.rect_size.x * 0.25,
                    amplitude * sin(self.time * self.freq) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.5,
                    amplitude * sin(self.time * self.freq + self.phase_offset) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.75,
                    amplitude * sin(self.time * self.freq + self.phase_offset * 2) + center_y,
                    self.dot_radius
                );
                sdf.fill(self.color);
                return sdf.result;
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TypingAnimation {
    #[deref]
    view: View,
    #[live]
    time: f32,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    is_play: bool,
}
impl Widget for TypingAnimation {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(ne) = self.next_frame.is_event(event) {
            self.time += ne.time as f32;
            self.time = (self.time.round() as u32 % 360) as f32;
            self.redraw(cx);
            if !self.is_play {
                return;
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
