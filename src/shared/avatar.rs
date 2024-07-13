//! An avatar holds either an image thumbnail or a single-character text label.
//!
//! The Avatar view (either text or image) is masked by a circle.
//! 
//! By default, an avatar displays the one-character text label.
//! You can use [AvatarRef::set_text] to set the content of that text label,
//! or [AvatarRef::show_image] to display an image instead of the text.

use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};

use crate::{
    profile::user_profile::ShowUserProfileAction,
    utils,
};

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
        flow: Overlay,
        cursor: Hand,

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
                width: Fit, height: Fit,
                padding: { top: 1.0 } // for better vertical alignment
                draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 15. }
                    color: #777,
                }
                text: "?"
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


#[derive(Clone, Debug)]
pub struct AvatarInfo {
    pub user_name: String,
    pub user_id: OwnedUserId,
    pub room_id: OwnedRoomId,
    pub avatar_img_data: Option<Arc<[u8]>>,
}
impl AvatarInfo {
    pub fn user_name(&self) -> &str {
        if self.user_name.is_empty() {
            self.user_id.as_str()
        } else {
            self.user_name.as_str()
        }
    }

    /// Returns the first "letter" (Unicode grapheme) of the user's name or user ID,
    /// skipping any leading "@" characters.
    pub fn first_letter(&self) -> &str {
        utils::user_name_first_letter(&self.user_name)
            .or_else(|| utils::user_name_first_letter(self.user_id.as_str()))
            .unwrap_or_default()
    }
}


#[derive(LiveHook, Live, Widget)]
pub struct Avatar {
    #[deref] view: View,

    #[rust] info: Option<AvatarInfo>,
}

impl Widget for Avatar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(info) = self.info.clone() else {
            return;
        };
        let area = self.view.area();
        let widget_uid = self.widget_uid();
        match event.hits(cx, area) {
            Hit::FingerDown(_fde) => {
                cx.set_key_focus(area);
            }
            Hit::FingerUp(fue) => if fue.was_tap() {
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    ShowUserProfileAction::ShowUserProfile(info),
                );
            }
            _ =>()
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }

    fn set_text(&mut self, v: &str) {
        let f = utils::user_name_first_letter(v)
            .unwrap_or("?");
        self.label(id!(text_view.text)).set_text(f);
        self.view(id!(img_view)).set_visible(false);
        self.view(id!(text_view)).set_visible(true);
    }
}

impl Avatar {
    /// Sets the text content of this avatar, making the user name visible
    /// and the image invisible.
    ///
    /// ## Arguments
    /// * `info`: information about the user represented by this avatar.
    ///    * Set this to `Some` to enable a user to click/tap on the Avatar itself.
    ///    * Set this to `None` to disable the click/tap action.
    /// * `user_name`: the displayable user name for this avatar.
    ///    Only the first non-`@` letter (Unicode grapheme) is displayed.
    pub fn show_text<T: AsRef<str>>(
        &mut self,
        info: Option<(OwnedUserId, OwnedRoomId)>,
        user_name: T,
    ) {
        self.info = info.map(|(user_id, room_id)| AvatarInfo {
            user_name: user_name.as_ref().to_string(),
            user_id,
            room_id,
            avatar_img_data: None,
        });
        self.set_text(user_name.as_ref());
    }

    /// Sets the image content of this avatar, making the image visible
    /// and the user name text invisible.
    ///
    /// ## Arguments
    /// * `info`: information about the user represented by this avatar:
    ///    the user name, user ID, room ID, and avatar image data.
    ///    * Set this to `Some` to enable a user to click/tap on the Avatar itself.
    ///    * Set this to `None` to disable the click/tap action.
    /// * `image_set_function`: - a function that is passed in an [ImageRef] that refers
    ///    to the image that will be displayed in this avatar.
    ///    This allows the caller to set the image contents in any way they want.
    ///    If `image_set_function` returns an error, no change is made to the avatar.
    pub fn show_image<F, E>(
        &mut self,
        info: Option<(String, OwnedUserId, OwnedRoomId, Arc<[u8]>)>,
        image_set_function: F,
    ) -> Result<(), E>
        where F: FnOnce(ImageRef) -> Result<(), E>
    {
        self.info = info.map(|(user_name, user_id, room_id, img_data)| AvatarInfo {
            user_name,
            user_id,
            room_id,
            avatar_img_data: Some(img_data),
        });

        let img_ref = self.image(id!(img_view.img));
        let res = image_set_function(img_ref);
        if res.is_ok() {
            self.view(id!(img_view)).set_visible(true);
            self.view(id!(text_view)).set_visible(false);
        }
        res
    }

    /// Returns whether this avatar is currently displaying an image or text.
    pub fn status(&mut self) -> AvatarDisplayStatus {
        if self.view(id!(img_view)).is_visible() {
            AvatarDisplayStatus::Image
        } else {
            AvatarDisplayStatus::Text
        }
    }
}

impl AvatarRef {
    /// See [`Avatar::show_text()`].
    pub fn show_text<T: AsRef<str>>(
        &self,
        info: Option<(OwnedUserId, OwnedRoomId)>,
        user_name: T,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_text(info, user_name);
        }
    }

    /// See [`Avatar::show_image()`].
    pub fn show_image<F, E>(
        &self,
        info: Option<(String, OwnedUserId, OwnedRoomId, Arc<[u8]>)>,
        image_set_function: F,
    ) -> Result<(), E>
        where F: FnOnce(ImageRef) -> Result<(), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(info, image_set_function)
        } else {
            Ok(())
        }
    }

    /// See [`Avatar::status()`].
    pub fn status(&self) -> AvatarDisplayStatus {
        if let Some(mut inner) = self.borrow_mut() {
            inner.status()
        } else {
            AvatarDisplayStatus::Text
        }
    }    
}

/// What an Avatar instance is currently displaying.
pub enum AvatarDisplayStatus {
    /// The avatar is displaying a text label.
    Text,
    /// The avatar is displaying an image.
    Image,
}
