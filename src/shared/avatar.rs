//! An avatar holds either an image thumbnail or a single-character text label.
//!
//! The Avatar view (either text or image) is masked by a circle.
//!
//! By default, an avatar displays the one-character text label.
//! You can use [AvatarRef::set_text] to set the content of that text label,
//! or [AvatarRef::show_image] to display an image instead of the text.

use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::{room::RoomMember, ruma::{EventId, OwnedRoomId, OwnedUserId, RoomId, UserId}};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, profile::{user_profile::{AvatarState, ShowUserProfileAction, UserProfile, UserProfileAndRoomId}, user_profile_cache}, sliding_sync::{submit_async_request, MatrixRequest}, utils
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")

    // An avatar view holds either an image thumbnail or a single character of text.
    // By default, the text label is visible, but can be replaced by an image
    // once it is available.
    //
    // The Avatar view (either text or image) is masked by a circle.
    pub Avatar = {{Avatar}} {
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
                instance background_color: (COLOR_AVATAR_BG)

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
                padding: { top: 0.5 } // for better vertical alignment
                draw_text: {
                    text_style: <TITLE_TEXT>{ font_size: 15. }
                    color: #f,
                }
                text: "?"
            }
        }

        img_view = <View> {
            visible: false,
            align: { x: 0.5, y: 0.5 }
            img = <Image> {
                fit: Stretch,
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

    #[rust] info: Option<UserProfileAndRoomId>,
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
            Hit::FingerUp(fue) => if fue.is_over && fue.is_primary_hit() && fue.was_tap() {
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

    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        let f = utils::user_name_first_letter(v)
            .unwrap_or("?").to_uppercase();
        self.label(id!(text_view.text)).set_text(cx, &f);
        self.view(id!(img_view)).set_visible(cx, false);
        self.view(id!(text_view)).set_visible(cx, true);
    }
}

impl Avatar {
    /// Sets the text content of this avatar, making the user name visible
    /// and the image invisible.
    ///
    /// ## Arguments
    /// * `info`: information about the user represented by this avatar, including a tuple of
    ///   the user ID, displayable user name, and room ID.
    ///   * Set this to `Some` to enable a user to click/tap on the Avatar itself.
    ///   * Set this to `None` to disable the click/tap action.
    /// * `username`: the displayable text for this avatar, either a user name or user ID.
    ///   Only the first non-`@` letter (Unicode grapheme) is displayed.
    pub fn show_text<T: AsRef<str>>(
        &mut self,
        cx: &mut Cx,
        bg_color: Option<Vec4>,
        info: Option<AvatarTextInfo>,
        username: T,
    ) {
        self.info = info.map(|AvatarTextInfo { user_id, username, room_id }|
            UserProfileAndRoomId {
                user_profile: UserProfile {
                    user_id,
                    username,
                    avatar_state: AvatarState::Unknown,
                },
                room_id,
            }
        );
        self.set_text(cx, username.as_ref());
        
        // Apply background color if provided
        if let Some(color) = bg_color {
            self.view(id!(text_view)).apply_over(cx, live! {
                draw_bg: {
                    background_color: (color)
                }
            });
        }
    }

    /// Sets the image content of this avatar, making the image visible
    /// and the user name text invisible.
    ///
    /// ## Arguments
    /// * `info`: information about the user represented by this avatar:
    ///   the user name, user ID, room ID, and avatar image data.
    ///   * Set this to `Some` to enable a user to click/tap on the Avatar itself.
    ///   * Set this to `None` to disable the click/tap action.
    /// * `image_set_function`: - a function that is passed in the `&mut Cx`
    ///   and an [ImageRef] that refers to the image that will be displayed in this avatar.
    ///   This allows the caller to set the image contents in any way they want.
    ///   If `image_set_function` returns an error, no change is made to the avatar.
    pub fn show_image<F, E>(
        &mut self,
        cx: &mut Cx,
        info: Option<AvatarImageInfo>,
        image_set_function: F,
    ) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        let img_ref = self.image(id!(img_view.img));
        let res = image_set_function(cx, img_ref);
        if res.is_ok() {
            self.view(id!(img_view)).set_visible(cx, true);
            self.view(id!(text_view)).set_visible(cx, false);

            self.info = info.map(|AvatarImageInfo { user_id, username, room_id, img_data }|
                UserProfileAndRoomId {
                    user_profile: UserProfile {
                        user_id,
                        username,
                        avatar_state: AvatarState::Loaded(img_data),
                    },
                    room_id,
                }
            );
        }
        res
    }

    /// Returns whether this avatar is currently displaying an image or text.
    pub fn status(&mut self) -> AvatarDisplayStatus {
        if self.view(id!(img_view)).visible() {
            AvatarDisplayStatus::Image
        } else {
            AvatarDisplayStatus::Text
        }
    }

    /// Sets the given avatar and returns a displayable username based on the
    /// given profile and user ID of the sender of the event with the given event ID.
    ///
    /// If the user profile is not ready, this function will submit an async request
    /// to fetch the user profile from the server, but only if the event ID is `Some`.
    /// For Read Receipt cases, there is no user's profile. The Avatar cache is taken from the sender's profile
    ///
    /// This function will always choose a nice, displayable username and avatar.
    ///
    /// The specific behavior is as follows:
    /// * If the timeline event's sender profile *is* ready, then the `username` and `avatar`
    ///   will be the user's display name and avatar image, if available.
    ///   * If it's not ready, we attempt to fetch the user info from the user profile cache.
    /// * If no avatar image is available, then the `avatar` will be set to the first character
    ///   of the user's display name, if available.
    /// * If the user's display name is not available or has not been set, the user ID
    ///   will be used for the `username`, and the first character of the user ID for the `avatar`.
    /// * If the timeline event's sender profile isn't ready and the user ID isn't found in
    ///   our user profile cache , then the `username` and `avatar`  will be the user ID
    ///   and the first character of that user ID, respectively.
    ///
    /// ## Return
    /// Returns a tuple of:
    /// 1. The displayable username that should be used to populate the username field.
    /// 2. A boolean indicating whether the user's profile info has been completely drawn
    ///    (for purposes of caching it to avoid future redraws).
    pub fn set_avatar_and_get_username(
        &mut self,
        cx: &mut Cx,
        room_id: &RoomId,
        avatar_user_id: &UserId,
        avatar_profile_opt: Option<&TimelineDetails<Profile>>,
        event_id: Option<&EventId>,
        room_members_opt: Option<&Arc<Vec<RoomMember>>>, // Added this parameter
    ) -> (String, bool) {
        let mut username_opt: Option<String> = None;
        let mut avatar_state = AvatarState::Unknown; // Start as unknown

        // --- Determine username_opt and avatar_state based on priority ---

        // Priority 1: Check provided room members list (most specific info for this room)
        if let Some(room_members) = room_members_opt {
                if let Some(room_member) = room_members.iter().find(|m| m.user_id() == avatar_user_id) {
                    username_opt = room_member.display_name().map(|n| n.to_owned());
                    avatar_state = AvatarState::Known(room_member.avatar_url().map(|u| u.to_owned()));
                    // If found in the provided list, we consider this info "known" for this draw cycle.
                }
        }

        // Priority 2: Check timeline event sender profile (only if not found in room_members_opt)
        // This might provide a display name or avatar URL if the room_members_opt was None or didn't contain the user.
        // Note: We only check avatar_profile_opt if username_opt is still None,
        // but we always use its avatar_url if available, regardless of username_opt.
        if username_opt.is_none() {
            match avatar_profile_opt {
                Some(TimelineDetails::Ready(profile)) => {
                    username_opt = profile.display_name.clone();
                    // Use avatar URL from profile if available, otherwise keep the state from room_members_opt
                    if profile.avatar_url.is_some() {
                        avatar_state = AvatarState::Known(profile.avatar_url.clone());
                    }
                }
                Some(TimelineDetails::Unavailable) => {
                    // Profile unavailable, request details for the event if possible
                    if let Some(event_id) = event_id {
                        submit_async_request(MatrixRequest::FetchDetailsForEvent {
                            room_id: room_id.to_owned(),
                            event_id: event_id.to_owned(),
                        });
                    }
                    // Keep avatar_state as it was (likely Unknown or from room_members_opt), username_opt as None
                }
                Some(TimelineDetails::Pending) => {
                        // Keep avatar_state as it was, username_opt as None
                }
                Some(TimelineDetails::Error(e)) => {
                    error!("Error fetching timeline profile for user {}: {:?}", avatar_user_id, e);
                    // Keep avatar_state as it was, username_opt as None
                }
                None => {
                        // Keep avatar_state as it was, username_opt as None
                }
            }
        }


        // Priority 3: Check User Profile Cache (only if username not found yet and avatar state is still Unknown)
        // This is a fallback for users not in the provided list or timeline profile.
        if username_opt.is_none() && matches!(avatar_state, AvatarState::Unknown) {
                user_profile_cache::with_user_profile(cx, avatar_user_id.to_owned(), true, |profile, room_members_in_cache| { // Pass true to fetch if missing
                    // Note: The closure signature for with_user_profile was corrected in a previous step
                    // to include room_members_in_cache. We don't need it here for the fallback logic,
                    // but the signature requires it.
                    username_opt = profile.username.clone();
                    avatar_state = profile.avatar_state.clone(); // Use the cached avatar state
                });
                // If with_user_profile returned None, it means either it was Requested or not found and requested.
                // In either case, username_opt and avatar_state remain as they were (None/Unknown or from a partial cache hit).
        }


        // Determine the final displayable username
        let username = username_opt.clone().unwrap_or_else(|| avatar_user_id.to_string());

        // --- Handle Avatar Image fetching/loading based on the determined avatar_state ---

        let mut profile_drawn = false; // Assume not fully drawn initially

        match avatar_state.clone() { // Clone to avoid moving out of avatar_state
            AvatarState::Loaded(data) => {
                // Already loaded, just show the image
                let res = self.show_image(
                    cx,
                    Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned(), data.clone()).into()),
                    |cx, img| utils::load_png_or_jpg(&img, cx, &data),
                );
                if res.is_ok() {
                    profile_drawn = true; // Image successfully drawn
                } else {
                        // Failed to draw loaded image? Fallback to text.
                        self.show_text(
                            cx,
                            None,
                            Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                            &username,
                        );
                        profile_drawn = true; // Text is always considered drawn
                }
            }
            AvatarState::Known(Some(uri)) => {
                // Known URI, try to get/fetch from avatar cache
                match avatar_cache::get_or_fetch_avatar(cx, uri.clone()) {
                    AvatarCacheEntry::Loaded(data) => {
                        // Loaded from cache, show image
                        let res = self.show_image(
                            cx,
                            Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned(), data.clone()).into()),
                            |cx, img| utils::load_png_or_jpg(&img, cx, &data),
                        );
                        if res.is_ok() {
                            profile_drawn = true; // Image successfully drawn
                        } else {
                            // Failed to draw loaded image? Fallback to text.
                            self.show_text(
                                cx,
                                None,
                                Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                                &username,
                            );
                            profile_drawn = true; // Text is always considered drawn
                        }
                    }
                    AvatarCacheEntry::Failed => {
                        // Failed to fetch previously, show text
                        self.show_text(
                            cx,
                            None,
                            Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                            &username,
                        );
                        profile_drawn = true; // Text is always considered drawn
                    }
                    AvatarCacheEntry::Requested => {
                        // Request in flight, show text placeholder
                        self.show_text(
                            cx,
                            None,
                            Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                            &username,
                        );
                        profile_drawn = false; // Still waiting for image
                    }
                }
            }
            AvatarState::Known(None) | AvatarState::Failed => {
                // Known to have no avatar, or failed to determine/fetch, show text
                self.show_text(
                    cx,
                    None,
                    Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                    &username,
                );
                profile_drawn = true; // Text is always considered drawn
            }
            AvatarState::Unknown => {
                // Still unknown if avatar exists, show text placeholder
                self.show_text(
                    cx,
                    None,
                    Some((avatar_user_id.to_owned(), username_opt.clone(), room_id.to_owned()).into()),
                    &username,
                );
                profile_drawn = false; // Still waiting for info
            }
        }

        // Set the info field for click handling (ShowUserProfile action)
        self.info = Some(UserProfileAndRoomId {
            user_profile: UserProfile {
                user_id: avatar_user_id.to_owned(),
                username: username_opt, // Use the determined username_opt
                avatar_state: avatar_state, // Use the determined avatar_state
            },
            room_id: room_id.to_owned(),
        });


        (username, profile_drawn)
    }



    // pub fn set_avatar_and_get_username(
    //     &mut self,
    //     cx: &mut Cx,
    //     room_id: &RoomId,
    //     avatar_user_id: &UserId,
    //     avatar_profile_opt: Option<&TimelineDetails<Profile>>,
    //     event_id: Option<&EventId>,
    //     room_members_opt: Option<&Arc<Vec<RoomMember>>>,
    // ) -> (String, bool) {
    //     // Get the display name and avatar URL from the user's profile, if available,
    //     // or if the profile isn't ready, fall back to qeurying our user profile cache.
    //     let (username_opt, avatar_state) = match avatar_profile_opt {
    //         Some(TimelineDetails::Ready(profile)) => (
    //             profile.display_name.clone(),
    //             AvatarState::Known(profile.avatar_url.clone()),
    //         ),
    //         Some(not_ready) => {
    //             if matches!(not_ready, TimelineDetails::Unavailable) {
    //                 if let Some(event_id) = event_id {
    //                     submit_async_request(MatrixRequest::FetchDetailsForEvent {
    //                         room_id: room_id.to_owned(),
    //                         event_id: event_id.to_owned(),
    //                     });
    //                 }
    //             }
    //             // log!("populate_message_view(): sender profile not ready yet for event {not_ready:?}");
    //             user_profile_cache::with_user_profile(cx, avatar_user_id.to_owned(), true, |profile, room_members| {
    //                 room_members
    //                     .get(room_id)
    //                     .map(|rm| {
    //                         (
    //                             rm.display_name().map(|n| n.to_owned()),
    //                             AvatarState::Known(rm.avatar_url().map(|u| u.to_owned())),
    //                         )
    //                     })
    //                     .unwrap_or_else(|| (profile.username.clone(), profile.avatar_state.clone()))
    //             })
    //             .unwrap_or((None, AvatarState::Unknown))
    //         }
    //         None => {
    //             match user_profile_cache::with_user_profile(cx, avatar_user_id.to_owned(), true, |profile, room_members| {
    //                 room_members
    //                     .get(room_id)
    //                     .map(|rm| {
    //                         (
    //                             rm.display_name().map(|n| n.to_owned()),
    //                             AvatarState::Known(rm.avatar_url().map(|u| u.to_owned())),
    //                         )
    //                     })
    //                     .unwrap_or_else(|| (profile.username.clone(), profile.avatar_state.clone()))
    //             }) {
    //                 Some((profile_name, avatar_state)) => {
    //                     (profile_name, avatar_state)
    //                 }
    //                 None => {
    //                     (None, AvatarState::Unknown)
    //                 }
    //             }
    //         }
    //     };

    //     let (avatar_img_data_opt, profile_drawn) = match avatar_state.clone() {
    //         AvatarState::Loaded(data) => (Some(data), true),
    //         AvatarState::Known(Some(uri)) => match avatar_cache::get_or_fetch_avatar(cx, uri) {
    //             AvatarCacheEntry::Loaded(data) => (Some(data), true),
    //             AvatarCacheEntry::Failed => (None, true),
    //             AvatarCacheEntry::Requested => (None, false),
    //         },
    //         AvatarState::Known(None) | AvatarState::Failed => (None, true),
    //         AvatarState::Unknown => (None, false),
    //     };

    //     // Set sender to the display name if available, otherwise the user id.
    //     let username = username_opt
    //         .clone()
    //         .unwrap_or_else(|| avatar_user_id.to_string());

    //     // Set the sender's avatar image, or use the username if no image is available.
    //     avatar_img_data_opt
    //         .and_then(|data| {
    //             self.show_image(
    //                 cx,
    //                 Some((
    //                     avatar_user_id.to_owned(),
    //                     username_opt.clone(),
    //                     room_id.to_owned(),
    //                     data.clone()).into(),
    //                 ),
    //                 |cx, img| utils::load_png_or_jpg(&img, cx, &data),
    //             )
    //             .ok()
    //         })
    //         .unwrap_or_else(|| {
    //             self.show_text(
    //                 cx,
    //                 Some((avatar_user_id.to_owned(), username_opt, room_id.to_owned()).into()),
    //                 &username,
    //             )
    //         });
    //     (username, profile_drawn)
    // }
}

impl AvatarRef {
    /// See [`Avatar::show_text()`].
    pub fn show_text<T: AsRef<str>>(
        &self,
        cx: &mut Cx,
        bg_color: Option<Vec4>,
        info: Option<AvatarTextInfo>,
        username: T,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_text(cx, bg_color, info, username);
        }
    }

    /// See [`Avatar::show_image()`].
    pub fn show_image<F, E>(
        &self,
        cx: &mut Cx,
        info: Option<AvatarImageInfo>,
        image_set_function: F,
    ) -> Result<(), E>
        where F: FnOnce(&mut Cx, ImageRef) -> Result<(), E>
    {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_image(cx, info, image_set_function)
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

    /// See [`Avatar::set_avatar_and_get_username()`].
    pub fn set_avatar_and_get_username(
        &self,
        cx: &mut Cx,
        room_id: &RoomId,
        avatar_user_id: &UserId,
        avatar_profile_opt: Option<&TimelineDetails<Profile>>,
        event_id: Option<&EventId>,
        room_members_opt: Option<&Arc<Vec<RoomMember>>>
    ) -> (String, bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_avatar_and_get_username(cx, room_id, avatar_user_id, avatar_profile_opt, event_id, room_members_opt)
        } else {
            (avatar_user_id.to_string(), false)
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

/// Information about a text-based Avatar.
pub struct AvatarTextInfo {
    pub user_id: OwnedUserId,
    pub username: Option<String>,
    pub room_id: OwnedRoomId,
}
impl From<(OwnedUserId, Option<String>, OwnedRoomId)> for AvatarTextInfo {
    fn from((user_id, username, room_id): (OwnedUserId, Option<String>, OwnedRoomId)) -> Self {
        Self { user_id, username, room_id }
    }
}

/// Information about an image-based avatar.
pub struct AvatarImageInfo {
    pub user_id: OwnedUserId,
    pub username: Option<String>,
    pub room_id: OwnedRoomId,
    pub img_data: Arc<[u8]>,
}
impl From<(OwnedUserId, Option<String>, OwnedRoomId, Arc<[u8]>)> for AvatarImageInfo {
    fn from((user_id, username, room_id, img_data): (OwnedUserId, Option<String>, OwnedRoomId, Arc<[u8]>)) -> Self {
        Self { user_id, username, room_id, img_data }
    }
}
