use std::{borrow::Cow, cell::RefCell, collections::{btree_map::Entry, BTreeMap}, ops::{Deref, DerefMut}, sync::Arc};
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{room::{RoomMember, RoomMemberRole}, ruma::{events::room::member::MembershipState, OwnedRoomId, OwnedUserId, UserId}};
use crate::{
    shared::avatar::AvatarWidgetExt, sliding_sync::{get_client, submit_async_request, MatrixRequest}, utils
};

thread_local! {
    /// A cache of each user's profile and the rooms they are a member of, indexed by user ID.
    static USER_PROFILE_CACHE: RefCell<BTreeMap<OwnedUserId, UserProfileCacheEntry>> = RefCell::new(BTreeMap::new());
}
struct UserProfileCacheEntry {
    user_profile: UserProfile,
    room_members: BTreeMap<OwnedRoomId, RoomMember>,
}

/// The queue of user profile updates waiting to be processed by the UI thread's event handler.
static PENDING_USER_PROFILE_UPDATES: SegQueue<UserProfileUpdate> = SegQueue::new();

/// Enqueues a new user profile update and signals the UI
/// such that the new update will be handled by the user profile sliding pane widget.
pub fn enqueue_user_profile_update(update: UserProfileUpdate) {
    PENDING_USER_PROFILE_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// A user profile update, which can include changes to a user's full profile
/// and/or room membership info.
pub enum UserProfileUpdate {
    /// A fully-fetched user profile, with info about the user's membership in a given room.
    Full {
        new_profile: UserProfile,
        room_id: OwnedRoomId,
        room_member: RoomMember,
    },
    /// An update to the user's room membership info only, without any profile changes.
    RoomMemberOnly {
        room_id: OwnedRoomId,
        room_member: RoomMember,
    },
    /// An update to the user's profile only, without changes to room membership info.
    UserProfileOnly(UserProfile),
}
impl UserProfileUpdate {
    /// Returns the user ID associated with this update.
    #[allow(unused)]
    pub fn user_id(&self) -> &UserId {
        match self {
            UserProfileUpdate::Full { new_profile, .. } => &new_profile.user_id,
            UserProfileUpdate::RoomMemberOnly { room_member, .. } => &room_member.user_id(),
            UserProfileUpdate::UserProfileOnly(profile) => &profile.user_id,
        }
    }

    /// Applies this update to the given user profile pane info,
    /// only if the user_id and room_id match that of the update.
    ///
    /// Returns `true` if the update resulted in any actual content changes.
    #[allow(unused)]
    fn apply_to_current_pane(&self, info: &mut UserProfilePaneInfo) -> bool {
        match self {
            UserProfileUpdate::Full { new_profile, room_id, room_member } => {
                if info.user_id == new_profile.user_id {
                    info.avatar_info.user_profile = new_profile.clone();
                    if &info.room_id == room_id {
                        info.room_member = Some(room_member.clone());
                    }
                    return true;
                }
            }
            UserProfileUpdate::RoomMemberOnly { room_id, room_member } => {
                log!("Applying RoomMemberOnly update to user profile pane info: user_id={}, room_id={}, ignored={}",
                    room_member.user_id(), room_id, room_member.is_ignored(),
                );
                if info.user_id == room_member.user_id() && &info.room_id == room_id {
                    log!("RoomMemberOnly update matches user profile pane info, updating room member.");
                    info.room_member = Some(room_member.clone());
                    return true;
                }
            }
            UserProfileUpdate::UserProfileOnly(new_profile) => {
                if info.user_id == new_profile.user_id {
                    info.avatar_info.user_profile = new_profile.clone();
                    return true;
                }
            }
        }
        false
    }

    /// Applies this update to the given user profile info cache.
    fn apply_to_cache(self, cache: &mut BTreeMap<OwnedUserId, UserProfileCacheEntry>) {
        match self {
            UserProfileUpdate::Full { new_profile, room_id, room_member } => {
                match cache.entry(new_profile.user_id.to_owned()) {
                    Entry::Occupied(mut entry) => {
                        let entry_mut = entry.get_mut();
                        entry_mut.user_profile = new_profile;
                        entry_mut.room_members.insert(room_id, room_member);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry {
                            user_profile: new_profile,
                            room_members: {
                                let mut room_members_map = BTreeMap::new();
                                room_members_map.insert(room_id, room_member);
                                room_members_map
                            },
                        });
                    }
                }
            }
            UserProfileUpdate::RoomMemberOnly { room_id, room_member } => {
                match cache.entry(room_member.user_id().to_owned()) {
                    Entry::Occupied(mut entry) =>{
                        entry.get_mut().room_members.insert(room_id, room_member);
                    }
                    Entry::Vacant(entry) => {
                        // This shouldn't happen, but we can still technically handle it correctly.
                        warning!("BUG: User profile cache entry not found for user {} when handling RoomMemberOnly update", room_member.user_id());
                        entry.insert(UserProfileCacheEntry {
                            user_profile: UserProfile {
                                user_id: room_member.user_id().to_owned(),
                                username: None,
                                avatar_img_data: None,
                            },
                            room_members: {
                                let mut room_members_map = BTreeMap::new();
                                room_members_map.insert(room_id, room_member);
                                room_members_map
                            },
                        });
                    }
                }
            }
            UserProfileUpdate::UserProfileOnly(new_profile) => {
                match cache.entry(new_profile.user_id.to_owned()) {
                    Entry::Occupied(mut entry) => entry.get_mut().user_profile = new_profile,
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry {
                            user_profile: new_profile,
                            room_members: BTreeMap::new(),
                        });
                    }
                }
            }
        }
    }
}

/// Information retrieved about a user: their displayable name, ID, and avatar image.
#[derive(Clone, Debug)]
pub struct UserProfile {
    pub user_id: OwnedUserId,
    pub username: Option<String>,
    pub avatar_img_data: Option<Arc<[u8]>>,
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


/// Information needed to display an avatar widget: a user profile and room ID.
#[derive(Clone, Debug)]
pub struct AvatarInfo {
    pub user_profile: UserProfile,
    pub room_id: OwnedRoomId,
}
impl Deref for AvatarInfo {
    type Target = UserProfile;
    fn deref(&self) -> &Self::Target {
        &self.user_profile
    }
}
impl DerefMut for AvatarInfo {
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
    ShowUserProfile(AvatarInfo),
    None,
}

/// Information needed to populate/display the user profile sliding pane.
#[derive(Clone, Debug)]
pub struct UserProfilePaneInfo {
    pub avatar_info: AvatarInfo,
    pub room_name: String,
    pub room_member: Option<RoomMember>,
}
impl Deref for UserProfilePaneInfo {
    type Target = AvatarInfo;
    fn deref(&self) -> &Self::Target {
        &self.avatar_info
    }
}
impl DerefMut for UserProfilePaneInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.avatar_info
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

        // Handle the user profile info being updated by a background task.
        let mut redraw_this_pane = false;
        if let Event::Signal = event {
            USER_PROFILE_CACHE.with_borrow_mut(|cache| {
                while let Some(update) = PENDING_USER_PROFILE_UPDATES.pop() {
                    // Insert the updated info into the cache
                    update.apply_to_cache(cache);
                }

                // Apply this update to the current user profile pane (if relevant).
                if let Some(our_info) = self.info.as_mut() {
                    if let Some(new_info) = cache.get(&our_info.user_id) {
                        our_info.user_profile = new_info.user_profile.clone();
                        our_info.room_member = new_info.room_members.get(&our_info.room_id).cloned();
                        redraw_this_pane = true;
                    }
                }
            });
        }

        if redraw_this_pane {
            // log!("Redrawing user profile pane due to user profile update");
            self.redraw(cx);
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
        info.avatar_img_data.as_ref()
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
        ignore_user_button.set_text(info.room_member.as_ref()
            .and_then(|rm| rm.is_ignored().then_some("Unignore (Unblock) User"))
            .unwrap_or("Ignore (Block) User")
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
    pub fn set_info(&mut self, mut info: UserProfilePaneInfo) {
        if info.room_member.is_none() {
            USER_PROFILE_CACHE.with_borrow(|cache| {
                if let Some(entry) = cache.get(&info.user_id) {
                    if let Some(room_member) = entry.room_members.get(&info.room_id) {
                        log!("Found user {} room member info in cache", info.user_id);
                        info.room_member = Some(room_member.clone());
                    }
                }
            });
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
    pub fn set_info(&self, info: UserProfilePaneInfo) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.set_info(info);
    }

    /// See [`UserProfileSlidingPane::show()`]
    pub fn show(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show(cx);
    }
}
