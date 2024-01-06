//! An avatar holds either an image thumbnail or a single-character text label.
//!
//! By default, an avatar displays the one-character text label.
//! You can use [AvatarRef::set_text] to set the content of that text label,
//! or [AvatarRef::set_image] to display an image instead of the text.

use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")

    // An avatar view holds either an image thumbnail or a single character of text.
    // By default, the text label is visible, but can be replaced by an image
    // once it is available.
    Avatar = {{Avatar}} {
        width: 36.0,
        height: 36.0,
        // centered horizontally and vertically.
        align: { x: 0.5, y: 0.5 }
        // the text_view and img_view are overlaid on top of each other.
        flow: Overlay

        text_view = <RoundedView> {
            visible: true,
            align: { x: 0.5, y: 0.5 }
            draw_bg: {
                instance radius: 4.0,
                instance border_width: 1.0,
                // instance border_color: #ddd,
                color: #dfd
            }
            
            text = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 16. }
                }
                text: ""
            }
        }

        img_view = <View> {
            visible: false,
            img = <Image> {
                width: Fill, height: Fill,
                source: (IMG_DEFAULT_AVATAR),
            }
        }
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct Avatar {
    #[deref] view: View,
}

impl Widget for Avatar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl AvatarRef {
    /// Sets the text content of this avatar, making the text visible
    /// and the image invisible.
    ///
    /// ## Arguments
    /// * `text`: the text that will be displayed in this avatar.
    ///    This should be a single character, but we accept anything that can be 
    ///    treated as a `&str` in order to support multi-character Unicode.
    pub fn set_text<T: AsRef<str>>(&mut self, text: T) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.label(id!(text_view.text)).set_text(text.as_ref());
            inner.view(id!(img_view)).set_visible(false);
            inner.view(id!(text_view)).set_visible(true);
        }
    }

    /// Sets the image content of this avatar, making the image visible
    /// and the text invisible.
    ///
    /// ## Arguments
    /// * `image_set_function`: - a function that is passed in an [ImageRef] that refers
    ///    to the image that will be displayed in this avatar.
    ///    This allows the caller to set the image contents in any way they want.
    pub fn set_image<F: FnOnce(ImageRef)>(&mut self, image_set_function: F) {
        if let Some(mut inner) = self.borrow_mut() {
            let img_ref = inner.image(id!(img_view.img));
            image_set_function(img_ref);
            inner.view(id!(img_view)).set_visible(true);
            inner.view(id!(text_view)).set_visible(false);
        }
    }

    /// Returns whether this avatar is currently displaying an image or text.
    pub fn status(&self) -> AvatarDisplayStatus {
        if let Some(mut inner) = self.borrow_mut() {
            if inner.view(id!(img_view)).is_visible() {
                return AvatarDisplayStatus::Image;
            }
        }
        AvatarDisplayStatus::Text
    }
}

/// What an Avatar instance is currently displaying.
pub enum AvatarDisplayStatus {
    /// The avatar is displaying a text label.
    Text,
    /// The avatar is displaying an image.
    Image,
}
