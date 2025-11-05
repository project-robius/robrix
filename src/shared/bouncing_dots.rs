use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub BouncingDots = {{BouncingDots}} {
        width: 24,
        height: 12,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: #x000,
            uniform anim_time: 0.0,
            uniform freq: 0.9,  // Animation frequency
            uniform phase_offset: 5.0, // Phase difference
            uniform dot_radius: 1.5, // Dot radius
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                let amplitude = self.rect_size.y * 0.21;
                let center_y = self.rect_size.y * 0.5;
                // Create three circle SDFs
                sdf.circle(
                    self.rect_size.x * 0.25, 
                    amplitude * sin(self.anim_time * 2.0 * PI * self.freq) + center_y, 
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.5, 
                    amplitude * sin(self.anim_time * 2.0 * PI * self.freq + self.phase_offset) + center_y, 
                    self.dot_radius
                );
                sdf.fill(self.color);
                sdf.circle(
                    self.rect_size.x * 0.75, 
                    amplitude * sin(self.anim_time * 2.0 * PI * self.freq + self.phase_offset * 2) + center_y, 
                    self.dot_radius
                );
                sdf.fill(self.color);
                return sdf.result;
            }
        }

        animator: {
            dots = {
                default: off,
                off = {
                    from: {all: Forward {duration: 0.0}}
                    apply: {
                        draw_bg: {anim_time: 0.0}
                    }
                }
                on = {
                    from: {all: Loop {duration: 1.0, end: 1.0}}
                    apply: {
                        draw_bg: {anim_time: [{time: 0.0, value: 0.0}, {time: 1.0, value: 1.0}]}
                    }
                }
            }
        }
        
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct BouncingDots {
    #[deref] view: View,
    #[animator] animator: Animator,
}
impl Widget for BouncingDots {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.animator_handle_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


impl BouncingDotsRef {
    /// Starts animation of the bouncing dots.
    pub fn start_animation(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_play(cx, ids!(dots.on));
        }
    }
    /// Stops animation of the bouncing dots.
    pub fn stop_animation(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_play(cx, ids!(dots.off));
        }
    }
}
