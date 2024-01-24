//! An avatar holds either an image thumbnail or a single-character text label.
//!
//! The Avatar view (either text or image) is masked by a circle.
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
    //
    // The Avatar view (either text or image) is masked by a circle.
    Avatar = {{Avatar}} {
        width: 36.0,
        height: 36.0,
        // centered horizontally and vertically.
        align: { x: 0.5, y: 0.5 }
        // the text_view and img_view are overlaid on top of each other.
        flow: Overlay

        text_view = <View> {
            visible: true,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,
            draw_bg: {
                instance background_color: #bfb,
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    let c = self.rect_size * 0.5;
                    sdf.circle(c.x, c.x, c.x)
                    sdf.fill_keep(self.background_color);
                    return sdf.result
                }
            }
            
            text = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 15. }
                }
                text: ""
            }
        }

        img_view = <View> {
            visible: false,
            align: { x: 0.5, y: 0.5 }
            img = <Image> {
                fit: Smallest,
                width: Fill, height: Fill,
                source: (IMG_DEFAULT_AVATAR),
                draw_bg: {
                    fn pixel(self) -> vec4 {
                        let maxed = max(self.rect_size.x, self.rect_size.y);
                        let sdf = Sdf2d::viewport(self.pos * vec2(maxed, maxed));
                        let r = maxed * 0.5;
                        sdf.circle(r, r, r);
                        sdf.fill_keep(self.get_color());
                        return sdf.result
                    }
                }
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
    ///    If `image_set_function` returns an error, no change is made to the avatar.
    pub fn set_image<F, E>(&mut self, image_set_function: F) -> Result<(), E>
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
