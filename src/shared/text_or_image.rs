//! A `TextOrImage` view displays either a text label or an image.
//!
//! This is useful to display a loading message while waiting for an image to be fetched,
//! or to display an error message if the image fails to load, etc.

use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    TextOrImage = {{TextOrImage}} {
        text_view: <View> {
            width: Fill,
            height: Fill,
            label = <Label> {
                width: Fit, height: Fit,
                draw_text: {
                    text_style: <TEXT_SUB> { }
                    // color: #00f,
                }
            }
        }
        image_view: <View> {
            width: Fill,
            height: Fill,
            image = <Image> {
                width: Fill,
                height: Fill,
                fit: Stretch,
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
    #[redraw] #[live] text_view: View,
    #[redraw] #[live] image_view: View,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[rust] status: TextOrImageStatus,
    // #[rust(TextOrImageStatus::Text)] status: TextOrImageStatus,
    #[rust] size_in_pixels: (usize, usize),
}

impl Widget for TextOrImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.image_view.handle_event(cx, event, scope);
        self.text_view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        walk.width = Size::Fixed(self.size_in_pixels.0 as f64 / cx.current_dpi_factor());
        walk.height = Size::Fixed(self.size_in_pixels.1 as f64 / cx.current_dpi_factor());
        cx.begin_turtle(walk, self.layout);
        match self.status{
            TextOrImageStatus::Image => self.image_view.draw_all(cx, scope),
            TextOrImageStatus::Text  => self.text_view.draw_all(cx, scope),
        }
        cx.end_turtle();
        DrawStep::done()
    }
}
impl TextOrImage {
    /// Sets the text content, which will be displayed on future draw operations.
    ///
    /// ## Arguments
    /// * `text`: the text that will be displayed in this `TextOrImage`, e.g.,
    ///   a message like "Loading..." or an error message.
    pub fn show_text<T: AsRef<str>>(&mut self, text: T) {
        self.text_view.label(id!(label)).set_text(text.as_ref());
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
    pub fn show_image<F, E>(&mut self, image_set_function: F) -> Result<(), E>
        where F: FnOnce(ImageRef) -> Result<(usize, usize), E>
    {
        let img_ref = self.image_view.image(id!(image));
        match image_set_function(img_ref) {
            Ok(size_in_pixels) => {
                self.status = TextOrImageStatus::Image;
                self.size_in_pixels = size_in_pixels;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Returns whether this `TextOrImage` is currently displaying an image or text.
    pub fn status(&self) -> TextOrImageStatus {
        self.status
    }
}

impl TextOrImageRef {
    /// See [TextOrImage::show_text()].
    pub fn show_text<T: AsRef<str>>(&self, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_text(text);
        }
    }

    /// See [TextOrImage::show_image()].
    pub fn show_image<F, E>(&self, image_set_function: F) -> Result<(), E>
        where F: FnOnce(ImageRef) -> Result<(usize, usize), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(image_set_function)
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
