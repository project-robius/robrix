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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageStatus {
    #[default] Size,
    Smallest
}

/// If View's width is larger than the image's width, we use `Size` to apply over the image.
///
/// Other conditions, we use `Smallest` to apply over the image.
#[derive(Live, LiveHook, Widget)]
struct RobrixAutoFitImage {
    #[deref] view: View,
    #[rust] status: ImageStatus,
    #[rust] target_size: DVec2,
}


impl Widget for RobrixAutoFitImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let image = self.view.image(id!(image));
        if image.area().rect(cx).size.x <= 0. || !image.has_texture() {
            self.target_size = image.area().rect(cx).size;
        }

        if let Event::Actions(_actions) = event {
            let self_rect_size = self.view.area().rect(cx).size;
            let new_status = if self_rect_size.x > self.target_size.x { ImageStatus::Size } else { ImageStatus::Smallest };
            if self.status != new_status {
                match new_status {
                    ImageStatus::Size => {
                        image.apply_over(cx, live! {
                            width: Fill, height: Fill
                            fit: Size
                        });
                    },
                    ImageStatus::Smallest => {
                        image.apply_over(cx, live! {
                            width: Fill, height: Fit
                            fit: Smallest
                        });
                    }
                }
                self.status = new_status;
            }
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
    /// Feel free to call this method, which can set the max width and height of the image.
    ///
    /// The max width and height will be the original size of the image if this function is not called.
    pub fn set_target_size(&self, target_size: DVec2) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.target_size = target_size;
    }
}
