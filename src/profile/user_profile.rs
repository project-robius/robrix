use std::{borrow::Cow, ops::{Deref, DerefMut}, sync::Arc};
use makepad_widgets::*;
use matrix_sdk::{room::{RoomMember, RoomMemberRole}, ruma::{events::room::member::MembershipState, OwnedMxcUri, OwnedRoomId, OwnedUserId}};
use crate::{
    avatar_cache::{self, AvatarCacheEntry}, shared::avatar::AvatarWidgetExt, sliding_sync::{get_client, is_user_ignored, submit_async_request, MatrixRequest}, utils
};

use super::user_profile_cache::{self, get_user_profile_and_room_member, get_user_room_member_info};

/// The currently-known state of a user's avatar.
#[derive(Clone, Debug)]
#[allow(unused)]
pub enum AvatarState {
    /// It isn't yet known if this user has an avatar.
    Unknown,
    /// It is known that this user does or does not have an avatar.
    Known(Option<OwnedMxcUri>),
    /// This user does have an avatar, and it has been fetched successfully.
    Loaded(Arc<[u8]>),
    /// This user does have an avatar, but we failed to fetch it.
    Failed,
}
impl AvatarState {
    /// Returns the avatar data, if in the `Loaded` state.
    pub fn data(&self) -> Option<&Arc<[u8]>> {
        if let AvatarState::Loaded(data) = self {
            Some(data)
        } else {
            None
        }
    }

    /// Returns the avatar URI, if in the `Known` state and it exists.
    pub fn uri(&self) -> Option<&OwnedMxcUri> {
        if let AvatarState::Known(Some(uri)) = self {
            Some(uri)
        } else {
            None
        }
    }
}

/// Information retrieved about a user: their displayable name, ID, and known avatar state.
#[derive(Clone, Debug)]
pub struct UserProfile {
    pub user_id: OwnedUserId,
    /// The user's default display name, if set.
    /// Note that a user may have per-room display names,
    /// so this should be considered a fallback.
    pub username: Option<String>,
    pub avatar_state: AvatarState,
}
impl UserProfile {
    /// Returns the user's displayable name, using the user ID as a fallback.
    pub fn displayable_name(&self) -> &str {
        if let Some(un) = self.username.as_ref() {
            if !un.is_empty() {
                return un.as_str();
            }
        }
        self.user_id.as_str()
    }

    /// Returns the first "letter" (Unicode grapheme) of the user's name or user ID,
    /// skipping any leading "@" characters.
    #[allow(unused)]
    pub fn first_letter(&self) -> &str {
        self.username.as_deref()
            .and_then(|un| utils::user_name_first_letter(un))
            .or_else(|| utils::user_name_first_letter(self.user_id.as_str()))
            .unwrap_or_default()
    }
}


/// Basic info needed to populate the contents of an avatar widget.
#[derive(Clone, Debug)]
pub struct UserProfileAndRoomId {
    pub user_profile: UserProfile,
    pub room_id: OwnedRoomId,
}
impl Deref for UserProfileAndRoomId {
    type Target = UserProfile;
    fn deref(&self) -> &Self::Target {
        &self.user_profile
    }
}
impl DerefMut for UserProfileAndRoomId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.user_profile
    }
}

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::helpers::*;
    import crate::shared::styles::*;
    import crate::shared::avatar::*;

    // Copied from Moxin
    FadeView = <CachedView> {
        draw_bg: {
            instance opacity: 1.0

            fn pixel(self) -> vec4 {
                let color = sample2d_rt(self.image, self.pos * self.scale + self.shift) + vec4(self.marked, 0.0, 0.0, 0.0);
                return Pal::premul(vec4(color.xyz, color.w * self.opacity))
            }
        }
    }

    // Copied from Moxin
    //
    // Customized button widget, based on the RoundedView shaders with some modifications
    // which is a better fit with our application UI design
    UserProfileActionButton = <Button> {
        width: Fit,
        height: Fit,
        spacing: 10,
        padding: {top: 10, bottom: 10, left: 15, right: 15}

        draw_bg: {
            instance color: #EDFCF2
            instance color_hover: #fff
            instance border_width: 1.2
            instance border_color: #D0D5DD
            instance border_color_hover: #fff
            instance radius: 3.0

            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            fn get_border_color(self) -> vec4 {
                return mix(self.border_color, mix(self.border_color, self.border_color_hover, 0.2), self.hover)
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_width,
                    self.border_width,
                    self.rect_size.x - (self.border_width * 2.0),
                    self.rect_size.y - (self.border_width * 2.0),
                    max(1.0, self.radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_width > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_width)
                }
                return sdf.result;
            }
        }

        draw_icon: {
            instance color: #000
            instance color_hover: #000
            uniform rotation_angle: 0.0,

            fn get_color(self) -> vec4 {
                return mix(self.color, mix(self.color, self.color_hover, 0.2), self.hover)
            }

            // Support rotation of the icon
            fn clip_and_transform_vertex(self, rect_pos: vec2, rect_size: vec2) -> vec4 {
                let clipped: vec2 = clamp(
                    self.geom_pos * rect_size + rect_pos,
                    self.draw_clip.xy,
                    self.draw_clip.zw
                )
                self.pos = (clipped - rect_pos) / rect_size

                // Calculate the texture coordinates based on the rotation angle
                let angle_rad = self.rotation_angle * 3.14159265359 / 180.0;
                let cos_angle = cos(angle_rad);
                let sin_angle = sin(angle_rad);
                let rot_matrix = mat2(
                    cos_angle, -sin_angle,
                    sin_angle, cos_angle
                );
                self.tex_coord1 = mix(
                    self.icon_t1.xy,
                    self.icon_t2.xy,
                    (rot_matrix * (self.pos.xy - vec2(0.5))) + vec2(0.5)
                );

                return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                    clipped.x,
                    clipped.y,
                    self.draw_depth + self.draw_zbias,
                    1.
                )))
            }
        }
        icon_walk: {width: 16, height: 16}

        draw_text: {
            text_style: <REGULAR_TEXT>{font_size: 10},
            color: #000
            fn get_color(self) -> vec4 {
                return self.color;
            }
        }
    }


    ICON_BLOCK_USER  = dep("crate://self/resources/icons/forbidden.svg")
    ICON_CLOSE       = dep("crate://self/resources/icons/close.svg")
    ICON_DOUBLE_CHAT = dep("crate://self/resources/icons/double_chat.svg")
    ICON_COPY        = dep("crate://self/resources/icons/copy.svg")
    ICON_JUMP        = dep("crate://self/resources/icons/go_back.svg")


    UserProfileView = <ScrollXYView> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        spacing: 15,
        flow: Down,

        show_bg: true,
        draw_bg: {
            color: #f3f3fa
            // 241, 244, 251
        }

        avatar = <Avatar> {
            width: 150,
            height: 150,
            margin: 10.0,
        }

        user_name = <Label> {
            width: Fill, height: Fit
            draw_text: {
                wrap: Line,
                color: #000,
                text_style: <USERNAME_TEXT_STYLE>{ },
            }
            text: "User Name"
        }

        user_id = <Label> {
            width: Fill, height: Fit
            draw_text: {
                wrap: Line,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <MESSAGE_TEXT_STYLE>{ font_size: 10 },
            }
            text: "User ID"
        }

        <LineH> { padding: 15 }

        <View> {
            width: Fill,
            height: Fit,
            flow: Down,
            spacing: 15,
            align: {x: 0.0, y: 0.0}

            membership_title_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 15}
                draw_text: {
                    wrap: Word,
                    text_style: <USERNAME_TEXT_STYLE>{},
                    color: #000
                }
                text: "Membership in this room"
            }

            membership_status_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 30}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11.5},
                }
                text: "Unknown"
            }

            role_info_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 30}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11.5},
                }
                text: "Unknown"
            }

            <LineH> { padding: 15 }

            <Label> {
                width: Fill, height: Fit
                padding: {left: 15}
                draw_text: {
                    wrap: Line,
                    text_style: <USERNAME_TEXT_STYLE>{},
                    color: #000
                }
                text: "Actions"
            }
        }

        actions = <View> {
            width: Fill, height: Fit
            flow: Down,
            spacing: 10
            padding: {left: 25, bottom: 50 }


            direct_message_button = <UserProfileActionButton> {
                // TODO: support this button. Once this is implemented, uncomment the line in draw_walk()
                enabled: false,
                draw_icon: {
                    svg_file: (ICON_DOUBLE_CHAT)
                }
                icon_walk: {width: 22, height: 16, margin: {left: -5, right: -3, top: 1, bottom: -1} }
                text: "Direct Message"
            }

            copy_link_to_user_button = <UserProfileActionButton> {
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {right: -2} }
                text: "Copy Link to User"
            }

            jump_to_read_receipt_button = <UserProfileActionButton> {
                enabled: false, // TODO: support this button
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 14, height: 16, margin: {left: -1, right: 2}}
                text: "Jump to Read Receipt"
            }

            ignore_user_button = <UserProfileActionButton> {
                draw_icon: {
                    svg_file: (ICON_BLOCK_USER)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0
                }
                text: "Ignore (Block) User"
                draw_text:{
                    color: (COLOR_DANGER_RED),
                }
            }
        }
    }


    UserProfileSlidingPane = {{UserProfileSlidingPane}} {
        flow: Overlay,
        width: Fill,
        height: Fill,
        align: {x: 1.0, y: 0}

        bg_view = <View> {
            width: Fill
            height: Fill
            visible: false,
            show_bg: true
            draw_bg: {
                fn pixel(self) -> vec4 {
                    return vec4(0., 0., 0., 0.7)
                }
            }
        }

        main_content = <FadeView> {
            width: 300,
            height: Fill
            flow: Overlay,

            user_profile_view = <UserProfileView> { }

            // The "X" close button on the top left
            close_button = <UserProfileActionButton> {
                width: Fit,
                height: Fit,
                align: {x: 0.0, y: 0.0},
                margin: 7,
                padding: 10,

                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #fff;
                    }
                }
                draw_bg: {
                    color: #777,
                    color_hover: #fff,
                }
                icon_walk: {width: 16, height: 16}
            }
        }

        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.4}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 300, draw_bg: {opacity: 1.0} }}
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {main_content = { width: 0, draw_bg: {opacity: 0.0} }}
                }
            }
        }
    }
}


#[derive(Clone, DefaultNone, Debug)]
pub enum ShowUserProfileAction {
    ShowUserProfile(UserProfileAndRoomId),
    None,
}

/// Information needed to populate/display the user profile sliding pane.
#[derive(Clone, Debug)]
pub struct UserProfilePaneInfo {
    pub profile_and_room_id: UserProfileAndRoomId,
    pub room_name: String,
    pub room_member: Option<RoomMember>,
}
impl Deref for UserProfilePaneInfo {
    type Target = UserProfileAndRoomId;
    fn deref(&self) -> &Self::Target {
        &self.profile_and_room_id
    }
}
impl DerefMut for UserProfilePaneInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.profile_and_room_id
    }
}
impl UserProfilePaneInfo {
    fn membership_title(&self) -> String {
        if self.room_name.is_empty() {
            format!("Membership in Room {}", self.room_id.as_str())
        } else {
            format!("Membership in {}", self.room_name)
        }
    }

    fn membership_status(&self) -> &str {
        self.room_member.as_ref().map_or(
            "Not a Member",
            |member| match member.membership() {
                MembershipState::Join => "Status: Joined",
                MembershipState::Leave => "Status: Left",
                MembershipState::Ban => "Status: Banned",
                MembershipState::Invite => "Status: Invited",
                MembershipState::Knock => "Status: Knocking",
                _ => "Status: Unknown",
            }
        )
    }

    fn role_in_room(&self) -> Cow<'_, str> {
        self.room_member.as_ref().map_or(
            "Role: Unknown".into(),
            |member| match member.suggested_role_for_power_level() {
                RoomMemberRole::Administrator => "Role: Admin".into(),
                RoomMemberRole::Moderator => "Role: Moderator".into(),
                RoomMemberRole::User => "Role: Standard User".into(),
            }
        )
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct UserProfileSlidingPane {
    #[deref] view: View,
    #[animator] animator: Animator,

    #[rust] info: Option<UserProfilePaneInfo>,
}

impl Widget for UserProfileSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        // if !self.visible { return; }

        // Close the pane if the close button is clicked, the back mouse button is clicked,
        // the escape key is pressed, or the back button is pressed.
        let close_pane = match event {
            Event::Actions(actions) => self.button(id!(close_button)).clicked(actions),
            Event::MouseUp(mouse) => mouse.button == 3, // the "back" button on the mouse
            Event::KeyUp(key) => key.key_code == KeyCode::Escape,
            Event::BackPressed => true,
            _ => false,
        };
        if close_pane {
            self.animator_play(cx, id!(panel.hide));
            self.view(id!(bg_view)).set_visible(false);
            return;
        }

        // A UI Signal indicates this user profile was updated by a background task.
        if let Event::Signal = event {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);

            // Re-fetch the currently-displayed user profile info from the cache in case it was updated.
            let mut redraw_this_pane = false;
            if let Some(our_info) = self.info.as_mut() {
                if let (Some(new_profile), room_member) = get_user_profile_and_room_member(
                    cx,
                    &our_info.user_id,
                    &our_info.room_id,
                ) {
                    our_info.user_profile = new_profile;
                    our_info.room_member = room_member;
                    // Use the avatar URI from the `room_member`, as it will be the most up-to-date
                    // and specific to the room that this user profile sliding pane is currently being shown for.
                    if let Some(avatar_uri) = our_info.room_member.as_ref()
                        .and_then(|rm| rm.avatar_url().map(|u| u.to_owned()))
                    {
                        our_info.avatar_state = AvatarState::Known(Some(avatar_uri));
                    }
                    // If we know the avatar URI, try to get/fetch the actual avatar image data.
                    if let AvatarState::Known(Some(uri)) = &our_info.avatar_state {
                        if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, uri.to_owned()) {
                            our_info.avatar_state = AvatarState::Loaded(data);
                        }
                    }
                    redraw_this_pane = true;
                }
            }
            if redraw_this_pane {
                // log!("Redrawing user profile pane due to user profile update");
                self.redraw(cx);
            }
        }

        let Some(info) = self.info.as_ref() else { return };

        if let Event::Actions(actions) = event {

            // TODO: handle actions for the `direct_message_button`

            if self.button(id!(copy_link_to_user_button)).clicked(actions) {
                let matrix_to_uri = info.user_id.matrix_to_uri().to_string();
                cx.copy_to_clipboard(&matrix_to_uri);
                // TODO: show a toast message instead of a log message
                log!("Copied user ID to clipboard: {matrix_to_uri}");
            }

            // TODO: implement the third button: `jump_to_read_receipt_button`,
            //       which involves calling `Timeline::latest_user_read_receipt()`
            //       or `Room::load_user_receipt()`, which are async functions.

            // The `ignore_user_button` require room membership info.
            if let Some(room_member) = info.room_member.as_ref() {
                if self.button(id!(ignore_user_button)).clicked(actions) {
                    submit_async_request(MatrixRequest::IgnoreUser {
                        ignore: !room_member.is_ignored(),
                        room_id: info.room_id.clone(),
                        room_member: room_member.clone(),
                    });
                    log!("Submitting request to {}ignore user {}.",
                        if room_member.is_ignored() { "un" } else { "" },
                        info.user_id,
                    );
                }
            }
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.visible = true;
        let Some(info) = self.info.as_ref() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };

        // Set the user name, using the user ID as a fallback.
        self.label(id!(user_name)).set_text(info.displayable_name());
        self.label(id!(user_id)).set_text(info.user_id.as_str());

        // Set the avatar image, using the user name as a fallback.
        let avatar_ref = self.avatar(id!(avatar));
        info.avatar_state
            .data()
            .and_then(|data| avatar_ref.show_image(None, |img| utils::load_png_or_jpg(&img, cx, &data)).ok())
            .unwrap_or_else(|| avatar_ref.show_text(None, info.displayable_name()));

        // Set the membership status and role in the room.
        self.label(id!(membership_title_label)).set_text(&info.membership_title());
        self.label(id!(membership_status_label)).set_text(info.membership_status());
        self.label(id!(role_info_label)).set_text(info.role_in_room().as_ref());

        // Draw and enable/disable the buttons according to user and room membership info:
        // * `direct_message_button` is disabled if the user is the same as the account user,
        //    since you cannot direct message yourself.
        // * `copy_link_to_user_button` is always enabled with the same text.
        // * `jump_to_read_receipt_button` is always enabled with the same text.
        // * `ignore_user_button` is disabled if the user is not a member of the room,
        //    or if the user is the same as the account user, since you cannot ignore yourself.
        //    * The button text changes to "Unignore" if the user is already ignored.
        let is_pane_showing_current_account = info.room_member.as_ref()
            .map(|rm| rm.is_account_user())
            .unwrap_or_else(|| get_client()
                .map_or(false, |client| client.user_id() == Some(&info.user_id))
            );

        // TODO: uncomment the line below once the `direct_message_button` logic is implemented.
        // self.button(id!(direct_message_button)).set_enabled(!is_pane_showing_current_account);

        let ignore_user_button = self.button(id!(ignore_user_button));
        ignore_user_button.set_enabled(!is_pane_showing_current_account && info.room_member.is_some());
        // Unfortunately the Matrix SDK's RoomMember type does not properly track
        // the `ignored` state of a user, so we have to maintain it separately.
        let is_ignored = info.room_member.as_ref()
            .is_some_and(|rm| is_user_ignored(rm.user_id()));
        ignore_user_button.set_text(
            if is_ignored { "Unignore (Unblock) User" } else { "Ignore (Block) User" }
        );

        self.view.draw_walk(cx, scope, walk)
    }
}


impl UserProfileSlidingPane {
    /// Returns `true` if the pane is both currently visible *and*
    /// animator is in the `show` state.
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        self.visible && self.animator_in_state(cx, id!(panel.show))
    }

    /// Sets the info to be displayed in this user profile sliding pane.
    ///
    /// If the `room_member` field is `None`, this function will attempt to
    /// retrieve the user's room membership info from the local cache.
    /// If the user's room membership info is not found in the cache,
    /// it will submit a request to asynchronously fetch it from the server.
    pub fn set_info(&mut self, _cx: &mut Cx, mut info: UserProfilePaneInfo) {
        if info.room_member.is_none() {
            if let Some(room_member) = get_user_room_member_info(
                _cx,
                &info.user_id,
                &info.room_id,
            ) {
                log!("Found user {} room member info in cache", info.user_id);
                if let Some(uri) = room_member.avatar_url() {
                    info.avatar_state = AvatarState::Known(Some(uri.to_owned()));
                }
                info.room_member = Some(room_member);
            }
        }
        if info.room_member.is_none() {
            log!("Did not find user {} room member info in cache, fetching from server.", info.user_id);
            // TODO: use the extra `via` parameters from `matrix_to_uri.via()`.
            submit_async_request(MatrixRequest::GetUserProfile {
                user_id: info.user_id.to_owned(),
                room_id: Some(info.room_id.clone()),
                local_only: false,
            });
        }
        if let AvatarState::Known(Some(uri)) = &info.avatar_state {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(_cx, uri.clone()) {
                info.avatar_state = AvatarState::Loaded(data);
            }
        }
        self.info = Some(info);
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        self.animator_play(cx, id!(panel.show));
        self.view(id!(bg_view)).set_visible(true);
        self.redraw(cx);
    }
}

impl UserProfileSlidingPaneRef {
    /// See [`UserProfileSlidingPane::is_currently_shown()`]
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_currently_shown(cx)
    }

    /// See [`UserProfileSlidingPane::set_info()`]
    pub fn set_info(&self, _cx: &mut Cx, info: UserProfilePaneInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_info(_cx, info);
    }

    /// See [`UserProfileSlidingPane::show()`]
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }
}
