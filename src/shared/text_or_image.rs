//! A `TextOrImage` view displays either a text label or an image.
//!
//! This is useful to display a loading message while waiting for an image to be fetched,
//! or to display an error message if the image fails to load, etc.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub TextOrImage = {{TextOrImage}} {
        width: Fill, height: Fit,
        flow: Overlay,

        text_view = <View> {
            visible: true,
            show_bg: true,
            draw_bg: {
                color: #dddddd
            }
            width: Fill, height: Fit,
            label = <Label> {
                width: Fill, height: Fit,
                draw_text: {
                    wrap: Word,
                    text_style: <MESSAGE_TEXT_STYLE> { }
                    color: (MESSAGE_TEXT_COLOR),
                }
            }
        }
        image_view = <View> {
            visible: false,
            cursor: NotAllowed, // we don't yet support clicking on the image
            width: Fill, height: Fit,
            image = <Image> {
                width: Fill, height: Fill,
                fit: Size,
            }
        }
    }
}


/// A view that holds an image or text content, and can switch between the two.
///
/// This is useful for displaying alternate text when an image is not (yet) available
/// or fails to load. It can also be used to display a loading message while an image
/// is being fetched.
#[derive(Live, Widget, LiveHook)]
pub struct TextOrImage {
    #[deref] view: View,
    #[rust] status: TextOrImageStatus,
    // #[rust(TextOrImageStatus::Text)] status: TextOrImageStatus,
    #[rust] size_in_pixels: (usize, usize),
}

impl Widget for TextOrImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
impl TextOrImage {
    /// Sets the text content, which will be displayed on future draw operations.
    ///
    /// ## Arguments
    /// * `text`: the text that will be displayed in this `TextOrImage`, e.g.,
    ///   a message like "Loading..." or an error message.
    pub fn show_text<T: AsRef<str>>(&mut self, cx: &mut Cx, text: T) {
        self.view(id!(image_view)).set_visible(cx, false);
        self.view(id!(text_view)).set_visible(cx, true);
        self.view.label(id!(text_view.label)).set_text(cx, text.as_ref());
        self.status = TextOrImageStatus::Text;
    }

    /// Sets the image content, which will be displayed on future draw operations.
    ///
    /// ## Arguments
    /// * `image_set_function`: this function will be called with an [ImageRef] argument,
    ///    which refers to the image that will be displayed within this `TextOrImage`.
    ///    This allows the caller to set the image contents in any way they want.
    ///    * If successful, the `image_set_function` should return the size of the image
    ///      in pixels as a tuple, `(width, height)`.
    ///    * If `image_set_function` returns an error, no change is made to this `TextOrImage`.
    pub fn show_image<F, E>(&mut self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(usize, usize), E>
    {
        let image_ref = self.view.image(id!(image_view.image));
        match image_set_function(cx, image_ref) {
            Ok(size_in_pixels) => {
                self.status = TextOrImageStatus::Image;
                self.size_in_pixels = size_in_pixels;
                self.view(id!(image_view)).set_visible(cx, true);
                self.view(id!(text_view)).set_visible(cx, false);
                Ok(())
            }
            Err(e) => {
                self.show_text(cx, "Failed to display image.");
                Err(e)
            }
        }
    }

    /// Returns whether this `TextOrImage` is currently displaying an image or text.
    pub fn status(&self) -> TextOrImageStatus {
        self.status
    }
}

impl TextOrImageRef {
    /// See [TextOrImage::show_text()].
    pub fn show_text<T: AsRef<str>>(&self, cx: &mut Cx, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_text(cx, text);
        }
    }

    /// See [TextOrImage::show_image()].
    pub fn show_image<F, E>(&self, cx: &mut Cx, image_set_function: F) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(usize, usize), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(cx, image_set_function)
        } else {
            Ok(())
        }
    }

    /// See [TextOrImage::status()].
    pub fn status(&self) -> TextOrImageStatus {
        if let Some(inner) = self.borrow() {
            inner.status()
        } else {
            TextOrImageStatus::Text
        }
    }
}

/// Whether a `TextOrImage` instance is currently displaying text or an image.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum TextOrImageStatus {
    #[default]
    Text,
    Image,
}
