use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub RobrixAutoFitImage = {{RobrixAutoFitImage}} {
        width: Fill, height: Fit
        image = <Image> {
            width: Fit, height: Fit,
            fit: Size
        }
    }
}

#[derive(Live, LiveHook, Widget)]
struct RobrixAutoFitImage {
    #[deref] view: View,
    #[rust(true)] current_is_size: bool,
    #[rust] original_image_size: DVec2,
    #[rust(false)] finished: bool,
}


impl Widget for RobrixAutoFitImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let image = self.view.image(id!(image));
        let self_rect_size = self.view.area().rect(cx).size;

        if image.area().rect(cx).size.x > 0. && !self.finished && image.has_texture() {
            self.original_image_size = image.area().rect(cx).size;
            self.finished = true;

            return;
        }

        let new_is_size = self_rect_size.x > self.original_image_size.x;

        if self.current_is_size != new_is_size {
            self.current_is_size = new_is_size;

            if new_is_size {
                image.apply_over(cx, live! {
                    width: Fill, height: Fill
                    fit: Size
                });
            } else {
                image.apply_over(cx, live! {
                    width: Fill, height: Fit
                    fit: Smallest
                });
            }
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RobrixAutoFitImageRef {
    pub fn set_visible(&self, cx: &mut Cx, visible: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.visible = visible;
        inner.redraw(cx);
    }
}
