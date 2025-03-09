use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    pub AutoFitImage = {{AutoFitImage}} {
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
struct AutoFitImage {
    #[deref] view: View,
    #[rust] status: ImageStatus,
    #[rust] target_size: Option<DVec2>,
}


impl Widget for AutoFitImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(target_size) = self.target_size {
            let image = self.view.image(id!(image));
            if let Event::Actions(_) | Event::WindowGeomChange(_) = event {
                let current_size = self.view.area().rect(cx).size;
                let new_status = if current_size.x > target_size.x { ImageStatus::Size } else { ImageStatus::Smallest };
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
        }
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AutoFitImageRef {
    pub fn set_visible(&self, cx: &mut Cx, visible: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.visible = visible;
        inner.redraw(cx);
    }
    /// Feel free to call this method, which can set the max width and height of the image.
    ///
    /// The max width and height will be the original size of the image if this function is not called.
    pub fn set_target_size(&self, target_size: Option<DVec2>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.target_size = target_size;
    }
}

pub fn get_image_resolution(data: &[u8]) -> Option<DVec2> {
    if data.len() < 8 {
        return None;
    }

    // Check PNG
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        if data.len() >= 24 && &data[12..16] == b"IHDR" {
            let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            return Some(DVec2 {x: width as f64, y: height as f64});
        }
    }
    // Check JPEG
    else if data.starts_with(&[0xFF, 0xD8]) {
        let mut offset = 2;
        while offset + 4 < data.len() {
            if data[offset] == 0xFF {
                let marker = data[offset + 1];
                if marker == 0xC0 || marker == 0xC2 { // SOF0 或 SOF2
                    if offset + 9 <= data.len() {
                        let height = u16::from_be_bytes([data[offset + 5], data[offset + 6]]) as u32;
                        let width = u16::from_be_bytes([data[offset + 7], data[offset + 8]]) as u32;
                        return Some(DVec2 {x: width as f64, y: height as f64});
                    }
                    break;
                }
                let len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
                offset += len + 2;
            } else {
                break;
            }
        }
    }
    None
}
