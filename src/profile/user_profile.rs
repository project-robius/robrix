use std::{borrow::Cow, ops::{Deref, DerefMut}, sync::Arc};
use makepad_widgets::*;
use matrix_sdk::{room::{RoomMember, RoomMemberRole}, ruma::{events::room::member::MembershipState, OwnedMxcUri, OwnedRoomId, OwnedUserId}};
use crate::{
    avatar_cache::{self, AvatarCacheEntry}, shared::{avatar::AvatarWidgetExt, popup_list::{enqueue_popup_notification, PopupItem, PopupKind}}, sliding_sync::{current_user_id, is_user_ignored, submit_async_request, MatrixRequest}, utils
};

use super::user_profile_cache::{self, get_user_profile_and_room_member};

/// The currently-known state of a user's avatar.
#[derive(Clone)]
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
impl std::fmt::Debug for AvatarState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AvatarState::Unknown        => write!(f, "Unknown"),
            AvatarState::Known(Some(_)) => write!(f, "Known(Some)"),
            AvatarState::Known(None)    => write!(f, "Known(None)"),
            AvatarState::Loaded(data)   => write!(f, "Loaded({} bytes)", data.len()),
            AvatarState::Failed         => write!(f, "Failed"),
        }
    }
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
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::helpers::*;
    use crate::shared::styles::*;
    use crate::shared::avatar::*;
    use crate::shared::icon_button::*;

    use link::tsp_link::TspVerifyUser;

    ICON_DOUBLE_CHAT = dep("crate://self/resources/icons/double_chat.svg")

    UserProfileView = <ScrollXYView> {
        width: Fill,
        height: Fill,
        align: {x: 0.5, y: 0},
        padding: {left: 15, right: 15, top: 15, bottom: 50}
        spacing: 20,
        flow: Down,
        cursor: Default,

        show_bg: true,
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        personal_info = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0.0}
            padding: {left: 10, right: 10}
            spacing: 3
            flow: Down

            avatar = <Avatar> {
                width: 150,
                height: 150,
                margin: 10.0,
                text_view = { text = { draw_text: {
                    text_style: { font_size: 40.0 }
                }}}
            }

            user_name = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    wrap: Word,
                    color: #000,
                    text_style: <USERNAME_TEXT_STYLE>{ font_size: 12 },
                }
                text: "User Name"
            }

            user_id = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
                text: "User ID"
            }
        }

        <LineH> { padding: 15 }
        
        membership = <View> {
            width: Fill,
            height: Fit,
            flow: Down,
            spacing: 3,
            align: {x: 0.0, y: 0.0}
            padding: {left: 10, right: 10}

            membership_title_label = <Label> {
                width: Fill, height: Fit
                draw_text: {
                    wrap: Word,
                    text_style: <USERNAME_TEXT_STYLE>{ font_size: 11.5 },
                    color: #000
                }
                text: "Membership in this room"
            }

            membership_status_label = <Label> {
                margin: { left: 7 }
                width: Fill, height: Fit
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
                text: "Unknown"
            }

            role_info_label = <Label> {
                margin: { left: 7 }
                width: Fill, height: Fit
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11 },
                }
                text: "Unknown"
            }
        }

        <LineH> { padding: 15 }

        actions = <View> {
            width: Fill, height: Fit
            flow: Down,
            spacing: 10,
            padding: {left: 10., right: 10, bottom: 10}

            <Label> {
                width: Fill, height: Fit
                draw_text: {
                    wrap: Line,
                    text_style: <USERNAME_TEXT_STYLE>{ font_size: 11.5 },
                    color: #000
                }
                text: "Actions"
            }

            direct_message_button = <RobrixIconButton> {
                // TODO: support this button. Once this is implemented, uncomment the line in draw_walk()
                enabled: false,
                margin: 0,
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                draw_bg: {
                    color: (COLOR_ACTIVE_PRIMARY)
                }
                draw_icon: {
                    svg_file: (ICON_DOUBLE_CHAT)
                    color: (COLOR_PRIMARY)
                }
                draw_text: {
                    color: (COLOR_PRIMARY)
                    text_style: <REGULAR_TEXT> {}
                }
                icon_walk: {width: 22, height: 16, margin: {left: -5, right: -3, top: 1, bottom: -1} }
                text: "Direct Message"
            }

            copy_link_to_user_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_COPY)
                }
                icon_walk: {width: 16, height: 16, margin: {left: -1, right: -1} }
                text: "Copy Link to User"
            }

            jump_to_read_receipt_button = <RobrixIconButton> {
                enabled: false, // TODO: support this button
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 14, height: 16, margin: {left: -1, right: 1.5}}
                text: "Jump to Read Receipt"
            }

            ignore_user_button = <RobrixIconButton> {
                padding: {top: 10, bottom: 10, left: 12, right: 15}
                margin: 0,
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN)
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -0.5} }

                draw_bg: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                }
                text: "Ignore (Block) User"
                draw_text:{
                    color: (COLOR_FG_DANGER_RED),
                }
            }
        }

        // A view that allows the user to verify a new DID and associate it
        // with a particular Matrix User ID.
        tsp_verify_user = <TspVerifyUser> { }
    }


    pub UserProfileSlidingPane = {{UserProfileSlidingPane}} {
        visible: false,
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
                uniform bg_color: #000000BB
                fn pixel(self) -> vec4 {
                    return self.bg_color;
                }
            }
        }

        main_content = <FadeView> {
            width: 300,
            height: Fill
            flow: Overlay,
            align: {x: 1.0}

            user_profile_view = <UserProfileView> { }

            // The "X" close button on the top right
            close_button = <RobrixIconButton> {
                width: Fit,
                height: Fit,
                spacing: 0,
                margin: 7,
                padding: 15,

                draw_bg: {
                    color: (COLOR_SECONDARY)
                }
                draw_icon: {
                    svg_file: (ICON_CLOSE),
                    fn get_color(self) -> vec4 {
                        return #x0;
                    }
                }
                icon_walk: {width: 14, height: 14}
            }
        }

        animator: {
            panel = {
                default: hide,
                show = {
                    redraw: true,
                    from: {all: Forward {duration: 0.4}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        main_content = { margin: {right: 0} },
                        bg_view = {
                            draw_bg: { bg_color: #000000BB }
                        }
                    }
                }
                hide = {
                    redraw: true,
                    from: {all: Forward {duration: 0.5}}
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: {
                        main_content = { margin: {right: -300} },
                        bg_view = {
                            draw_bg: { bg_color: #x00000000 }
                        }
                    }
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
                RoomMemberRole::Creator => "Role: Creator".into(),
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
    #[rust] is_animating_out: bool,
}

impl Widget for UserProfileSlidingPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if !self.visible { return; }

        let animator_action = self.animator_handle_event(cx, event);
        if animator_action.must_redraw() {
            self.redraw(cx);
        }
        // If the animator is in the `hide` state and has finished animating out,
        // that means it has fully animated off-screen and can be set to invisible.
        if self.animator_in_state(cx, ids!(panel.hide)) {
            match (self.is_animating_out, animator_action.is_animating()) {
                (true, false) => {
                    self.visible = false;
                    cx.revert_key_focus();
                    self.view(ids!(bg_view)).set_visible(cx, false);
                    self.redraw(cx);
                    return;
                }
                (false, true) => {
                    self.is_animating_out = true;
                }
                _ => { }
            }
        }

        let area = self.view.area();

        // Close the pane if:
        // 1. The close button is clicked,
        // 2. The back navigational gesture/action occurs (e.g., Back on Android),
        // 3. The escape key is pressed if this pane has key focus,
        // 4. The back mouse button is clicked within this view,
        // 5. The user clicks/touches outside the main_content view area.
        let close_pane = {
            matches!(
                event,
                Event::Actions(actions) if self.button(ids!(close_button)).clicked(actions)
            )
            || event.back_pressed()
            || match event.hits_with_capture_overload(cx, area, true) {
                Hit::KeyUp(key) => key.key_code == KeyCode::Escape,
                Hit::FingerDown(_fde) => {
                    cx.set_key_focus(area);
                    false
                }
                Hit::FingerUp(fue) if fue.is_over => {
                    fue.mouse_button().is_some_and(|b| b.is_back())
                    || !self.view(ids!(main_content)).area().rect(cx).contains(fue.abs)
                }
                _ => false,
            }
        };
        if close_pane {
            self.animator_play(cx, ids!(panel.hide));
            self.redraw(cx);
            return;
        }

        // A UI Signal indicates that this user profile's info may have been updated by a background task.
        if let Event::Signal = event {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);

            // Re-fetch the currently-displayed user profile info from the cache in case it was updated.
            let mut redraw_this_pane = false;
            if let Some(our_info) = self.info.as_mut() {
                if let (Some(new_profile), room_member) = get_user_profile_and_room_member(
                    cx,
                    our_info.user_id.clone(),
                    &our_info.room_id,
                    false,
                ) {
                    let prev_avatar_state = our_info.avatar_state.clone();
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
                    // If the new avatar state is fully `Loaded`, keep it as is.
                    // If the new avatar state is *not* fully `Loaded`, but the previous one was, keep the previous one.
                    match (prev_avatar_state, &mut our_info.avatar_state) {
                        (_,                             AvatarState::Loaded(_)) => { }
                        (prev @ AvatarState::Loaded(_), existing_avatar_state ) => {
                            *existing_avatar_state = prev;
                        }
                        _ => { }
                    }
                    redraw_this_pane = true;
                }
            }
            if redraw_this_pane {
                self.redraw(cx);
            }
        }

        let Some(info) = self.info.as_ref() else { return };

        if let Event::Actions(actions) = event {

            // TODO: handle actions for the `direct_message_button`

            if self.button(ids!(copy_link_to_user_button)).clicked(actions) {
                let matrix_to_uri = info.user_id.matrix_to_uri().to_string();
                cx.copy_to_clipboard(&matrix_to_uri);
                enqueue_popup_notification(PopupItem {
                    message: String::from("Copied User ID to the clipboard."),
                    auto_dismissal_duration: Some(3.0),
                    kind: PopupKind::Success
                });
            }

            // TODO: implement the third button: `jump_to_read_receipt_button`,
            //       which involves calling `Timeline::latest_user_read_receipt()`
            //       or `Room::load_user_receipt()`, which are async functions.

            // The `ignore_user_button` require room membership info.
            if let Some(room_member) = info.room_member.as_ref() {
                if self.button(ids!(ignore_user_button)).clicked(actions) {
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
        let Some(info) = self.info.as_ref() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };

        // Set the user name, using the user ID as a fallback.
        self.label(ids!(user_name)).set_text(cx, info.displayable_name());
        self.label(ids!(user_id)).set_text(cx, info.user_id.as_str());

        // Set the avatar image, using the user name as a fallback.
        let avatar_ref = self.avatar(ids!(avatar));
        info.avatar_state
            .data()
            .and_then(|data| avatar_ref.show_image(cx, None, |cx, img| utils::load_png_or_jpg(&img, cx, data)).ok())
            .unwrap_or_else(|| avatar_ref.show_text(cx, None, None, info.displayable_name()));

        // Set the membership status and role in the room.
        self.label(ids!(membership_title_label)).set_text(cx, &info.membership_title());
        self.label(ids!(membership_status_label)).set_text(cx, info.membership_status());
        self.label(ids!(role_info_label)).set_text(cx, info.role_in_room().as_ref());

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
            .unwrap_or_else(|| current_user_id().is_some_and(|uid| uid == info.user_id));

        // TODO: uncomment the line below once the `direct_message_button` logic is implemented.
        // self.button(ids!(direct_message_button)).set_enabled(!is_pane_showing_current_account);

        let ignore_user_button = self.button(ids!(ignore_user_button));
        ignore_user_button.set_enabled(cx, !is_pane_showing_current_account && info.room_member.is_some());
        // Unfortunately the Matrix SDK's RoomMember type does not properly track
        // the `ignored` state of a user, so we have to maintain it separately.
        let is_ignored = info.room_member.as_ref()
            .is_some_and(|rm| is_user_ignored(rm.user_id()));
        ignore_user_button.set_text(
            cx,
            if is_ignored { "Unignore (Unblock) User" } else { "Ignore (Block) User" }
        );

        self.view.draw_walk(cx, scope, walk)
    }
}


impl UserProfileSlidingPane {
    /// Returns `true` if this pane is currently being shown.
    pub fn is_currently_shown(&self, _cx: &mut Cx) -> bool {
        self.visible
    }

    /// Sets the info to be displayed in this user profile sliding pane.
    ///
    /// If the `room_member` field is `None`, this function will attempt to
    /// retrieve the user's room membership info from the local cache,
    /// and then submit a request to asynchronously fetch it from the server
    /// if it's not found in the cache.
    pub fn set_info(&mut self, _cx: &mut Cx, mut info: UserProfilePaneInfo) {
        if info.room_member.is_none() {
            if let (new_profile, Some(room_member)) = get_user_profile_and_room_member(
                _cx,
                info.user_id.clone(),
                &info.room_id,
                true,
            ) {
                log!("Found user {} room member info in cache", info.user_id);
                // Update avatar state, preferring that of the room member info.
                if let Some(uri) = room_member.avatar_url() {
                    info.avatar_state = AvatarState::Known(Some(uri.to_owned()));
                }
                else if let Some(p) = new_profile.as_ref() {
                    match &p.avatar_state {
                        s @ AvatarState::Known(Some(_)) | s @ AvatarState::Loaded(_) => {
                            info.avatar_state = s.clone();
                        }
                        _ => {}
                    }
                }
                // Update displayable username.
                if info.username.is_none() {
                    info.username = room_member.display_name()
                        .map(|dn| dn.to_owned())
                        .or_else(|| new_profile.and_then(|p| p.username.clone()));
                }
                info.room_member = Some(room_member);
            }
        }
        if let AvatarState::Known(Some(uri)) = &info.avatar_state {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(_cx, uri.clone()) {
                info.avatar_state = AvatarState::Loaded(data);
            }
        }
        
        // If TSP is enabled, populate the TSP verification info for this user.
        #[cfg(feature = "tsp")] {
            use crate::tsp::verify_user::TspVerifyUserWidgetExt;
            self.view.tsp_verify_user(ids!(tsp_verify_user))
                .show(_cx, info.user_id.clone());
        }

        self.info = Some(info);
    }

    pub fn show(&mut self, cx: &mut Cx) {
        self.visible = true;
        cx.set_key_focus(self.view.area());
        self.animator_play(cx, ids!(panel.show));
        self.view(ids!(bg_view)).set_visible(cx, true);

        self.view.button(ids!(close_button)).reset_hover(cx);
        self.view.button(ids!(direct_message_button)).reset_hover(cx);
        self.view.button(ids!(copy_link_to_user_button)).reset_hover(cx);
        self.view.button(ids!(jump_to_read_receipt_button)).reset_hover(cx);
        self.view.button(ids!(ignore_user_button)).reset_hover(cx);
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
