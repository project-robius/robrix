//! A `TextOrImage` view displays a loading message while waiting for an image to be fetched.
//!
//! Once the image is fetched and loaded, it displays the image as normal.
//! If the image fails to load, it displays an error message permanently.

use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    TextOrImage = {{TextOrImage}} {
        width: Fit, height: Fit,
        flow: Overlay

        text_view = <View> {
            visible: true,
            text = <Label> {
                width: Fit, height: Fit,
                draw_text: {
                    text_style: <REGULAR_TEXT>{ font_size: 12. }
                }
                text: "Loading..."
            }
        }

        img_view = <View> {
            visible: false,
            img = <Image> {
                fit: Smallest,
                width: Fill, height: Fill,
            }
        }
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct TextOrImage {
    #[deref] view: View,
}

impl Widget for TextOrImage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl TextOrImageRef {
    /// Sets the text content, makin the text visible and the image invisible.
    ///
    /// ## Arguments
    /// * `text`: the text that will be displayed in this `TextOrImage`, e.g.,
    ///   a message like "Loading..." or an error message.
    pub fn set_text<T: AsRef<str>>(&self, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.label(id!(text_view.text)).set_text(text.as_ref());
            inner.view(id!(img_view)).set_visible(false);
            inner.view(id!(text_view)).set_visible(true);
        }
    }

    /// Sets the image content, making the image visible and the text invisible.
    ///
    /// ## Arguments
    /// * `image_set_function`: this function will be called with an [ImageRef] argument,
    ///    which refers to the image that will be displayed within this `TextOrImage`.
    ///    This allows the caller to set the image contents in any way they want.
    ///    If `image_set_function` returns an error, no change is made to this `TextOrImage`.
    pub fn set_image<F, E>(&self, image_set_function: F) -> Result<(), E>
        where F: FnOnce(ImageRef) -> Result<(), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            let img_ref = inner.image(id!(img_view.img));
            let res = image_set_function(img_ref);
            if res.is_ok() {
                inner.view(id!(img_view)).set_visible(true);
                inner.view(id!(text_view)).set_visible(false);
            }
            res
        } else {
            Ok(())
        }
    }

    /// Returns whether this `TextOrImage` is currently displaying an image or text.
    pub fn status(&self) -> DisplayStatus {
        if let Some(mut inner) = self.borrow_mut() {
            if inner.view(id!(img_view)).is_visible() {
                return DisplayStatus::Image;
            }
        }
        DisplayStatus::Text
    }
}

/// Whether a `TextOrImage` instance is currently displaying text or an image.
pub enum DisplayStatus {
    Text,
    Image,
}
