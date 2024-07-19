use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}, ops::Deref, sync::Arc};
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{room::RoomMember, ruma::{OwnedRoomId, OwnedUserId}};
use crate::{
    shared::avatar::AvatarWidgetExt,
    utils,
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

/// A fully-fetched user profile, with info about the user's membership in a given room.
pub struct UserProfileUpdate {
    pub new_profile: UserProfile,
    pub room_member: Option<(OwnedRoomId, RoomMember)>,
}
impl Deref for UserProfileUpdate {
    type Target = UserProfile;
    fn deref(&self) -> &Self::Target {
        &self.new_profile
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

            role_info_title_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 15}
                draw_text: {
                    wrap: Word,
                    text_style: <USERNAME_TEXT_STYLE>{},
                    color: #000
                }
                text: "Role in this room"
            }

            role_info_label = <Label> {
                width: Fill, height: Fit
                padding: {left: 30}
                draw_text: {
                    wrap: Line,
                    color: (MESSAGE_TEXT_COLOR),
                    text_style: <MESSAGE_TEXT_STYLE>{ font_size: 11.5},
                }
                text: "Default"
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
                draw_icon: {
                    svg_file: (ICON_JUMP)
                }
                icon_walk: {width: 14, height: 16, margin: {left: -1, right: 2}}
                text: "Jump to Read Receipt"
            }

            block_user_button = <UserProfileActionButton> {
                draw_icon: {
                    svg_file: (ICON_BLOCK_USER)
                    color: (COLOR_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_DANGER_RED),
                    color: #fff0f0
                }
                text: "Ignore/Block User"
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
impl UserProfilePaneInfo {
    fn role_in_room_title(&self) -> String {
        if self.room_name.is_empty() {
            format!("Role in Room ID: {}", self.room_id.as_str())
        } else {
            format!("Role in {}", self.room_name)
        }
    }

    fn role_in_room(&self) -> &str {
        // TODO: acquire a user's role in the room to set their `role_info_label``
        //       Also, note that the user may not even be a member of the room.
        "<TODO>"
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
        }

        // Handle the user profile info being updated by a background task.
        let mut redraw_this_pane = false;
        if let Event::Signal = event {
            USER_PROFILE_CACHE.with_borrow_mut(|cache| {
                while let Some(update) = PENDING_USER_PROFILE_UPDATES.pop() {
                    // If the `update` applies to the info being displayed by this
                    // user profile sliding pane, update its `info.
                    if let Some(our_info) = self.info.as_mut() {
                        if our_info.user_profile.user_id == update.user_id {
                            our_info.avatar_info.user_profile = update.new_profile.clone();
                            // If the update also includes room member info for the room
                            // that we're currently displaying the user profile info for, update that too.
                            if let Some((room_id, room_member)) = update.room_member.clone() {
                                if room_id == our_info.room_id {
                                    our_info.room_member = Some(room_member);
                                }
                            }
                            redraw_this_pane = true;
                        }
                    }
                    // Insert the updated info into the cache
                    match cache.entry(update.user_id.clone()) {
                        Entry::Occupied(mut entry) => {
                            let entry_mut = entry.get_mut();
                            entry_mut.user_profile = update.new_profile;
                            if let Some((room_id, room_member)) = update.room_member {
                                entry_mut.room_members.insert(room_id, room_member);
                            }
                        }
                        Entry::Vacant(entry) => {
                            let mut room_members_map = BTreeMap::new();
                            if let Some((room_id, room_member)) = update.room_member {
                                room_members_map.insert(room_id, room_member);
                            }
                            entry.insert(
                                UserProfileCacheEntry {
                                    user_profile: update.new_profile,
                                    room_members: room_members_map,
                                }
                            );
                        }
                    }
                }
            });
        }

        if redraw_this_pane {
            self.redraw(cx);
        }

        // TODO: handle button clicks for things like:
        // * direct_message_button
        // * copy_link_to_user_button
        // * jump_to_read_receipt_button
        // * block_user_button

    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let Some(info) = self.info.clone() else {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        };
        self.visible = true;

        // Set the user name, using the user ID as a fallback.
        self.label(id!(user_name)).set_text(info.displayable_name());
        self.label(id!(user_id)).set_text(info.user_id.as_str());
        self.label(id!(role_info_title_label)).set_text(&info.role_in_room_title());

        // Set the avatar image, using the user name as a fallback.
        let avatar_ref = self.avatar(id!(avatar));
        info.avatar_img_data.as_ref()
            .and_then(|data| avatar_ref.show_image(None, |img| utils::load_png_or_jpg(&img, cx, &data)).ok())
            .unwrap_or_else(|| avatar_ref.show_text(None, info.displayable_name()));

        self.label(id!(role_info_label)).set_text(info.role_in_room());

        self.view.draw_walk(cx, scope, walk)
    }
}

impl UserProfileSlidingPaneRef {
    /// Returns `true` if the pane is both currently visible *and*
    /// animator is in the `show` state.
    pub fn is_currently_shown(&self, cx: &mut Cx) -> bool {
        self.borrow().map_or(false, |inner|
            inner.visible && inner.animator_in_state(cx, id!(panel.show))
        )
    }

    pub fn set_info(&self, info: UserProfilePaneInfo) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.info = Some(info);
        }
    }

    pub fn show(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.visible = true;
            inner.animator_play(cx, id!(panel.show));
            inner.view(id!(bg_view)).set_visible(true);
            inner.redraw(cx);
        }
    }


}
