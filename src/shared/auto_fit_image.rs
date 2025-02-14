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

/// If View's width is larger than the image's width, we use `Size` to apply over the image.
///
/// Other conditions, we use `Smallest` to apply over the image.
#[derive(Live, LiveHook, Widget)]
struct RobrixAutoFitImage {
    #[deref] view: View,
    #[rust(true)] current_is_size: bool,
    #[rust] threshold_image_size: DVec2,
    /// Whether we get the true origin size of the image.
    #[rust(false)] inisialized: bool,
}


impl Widget for RobrixAutoFitImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let image = self.view.image(id!(image));
        if image.area().rect(cx).size.x > 0. && !self.inisialized && image.has_texture() {
            self.threshold_image_size = image.area().rect(cx).size;
            self.inisialized = true;
        }
        match event {
            Event::Draw(_) | Event::WindowGeomChange(_) =>{
                log!("Runned");
                let self_rect_size = self.view.area().rect(cx).size;

                let new_should_be_size = self_rect_size.x > self.threshold_image_size.x;

                if self.current_is_size != new_should_be_size {
                    self.current_is_size = new_should_be_size;
                    if new_should_be_size {
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
                }
            }
            _ => {}
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
    /// USer can set the max width and height of the image.
    ///
    /// If this function is not called, the max width and height will be the original size of the image.
    pub fn set_max_width_height(&self, width: f64, height: f64) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.inisialized = true;
        inner.threshold_image_size = DVec2 {x: width, y: height};
    }
}
