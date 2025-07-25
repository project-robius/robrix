use makepad_widgets::*;

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, login::{login_screen::LoginAction, logout_confirm_modal::LogoutConfirmModalAction}, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{self, UserProfileUpdate},
    }, settings::SettingsAction, shared::{
        avatar::AvatarWidgetExt,
        callout_tooltip::TooltipAction,
        styles::*,
        verification_badge::VerificationBadgeWidgetExt,
    }, sliding_sync::current_user_id, utils
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::verification_badge::*;
    use crate::login::logout_confirm_modal::LogoutConfirmModal;
    use crate::shared::avatar::*;

    SPACES_DOCK_SIZE = 68

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")
    ICON_LOGOUT = dep("crate://self/resources/icons/logout.svg")

    Filler = <View> {
        height: Fill, width: Fill
    }

    ProfileIcon = {{ProfileIcon}} {
        flow: Overlay
        width: (SPACES_DOCK_SIZE - 6), height: (SPACES_DOCK_SIZE - 6)
        align: { x: 0.5, y: 0.5 }
        cursor: Hand,

        our_own_avatar = <Avatar> {
            width: 45, height: 45
            // If no avatar picture, use white text on a dark background.
            text_view = {
                draw_bg: {
                    background_color: (COLOR_FG_DISABLED),
                }
                text = { draw_text: {
                    text_style: { font_size: 16.0 },
                    color: (COLOR_PRIMARY),
                } }
            }
        }

        <View> {
            align: { x: 1.0, y: 0.0 }
            verification_badge = <VerificationBadge> {}
        }
    }

    Home = <RoundedView> {
        width: Fit, height: Fit
        padding: {top: 8, left: 12, right: 12, bottom: 8}
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
            border_radius: 4.0
            border_color: (COLOR_ACTIVE_PRIMARY)
            border_size: 1.5
        }

        align: {x: 0.5, y: 0.5}
        <Icon> {
            draw_icon: {
                svg_file: (ICON_HOME),
                fn get_color(self) -> vec4 {
                    return #1C274C;
                }
            }
            icon_walk: {width: 25, height: Fit}
        }
    }

    LogoutButton= {{LogoutButton}} {
        width: Fit, height: Fit
        padding: {top: 8, left: 12, right: 12, bottom: 8}
        align: {x: 0.5, y: 0.5}
        logout_button = <Button> {
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }

            draw_icon: {
                svg_file: (ICON_LOGOUT),
                color: #444444
                color_hover: (COLOR_DANGER_RED)
                fn get_color(self) -> vec4 {
                    return mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    );
                }
            }

            icon_walk: {width: 25, height: Fit}
        }
    }

    pub SpacesDock = <AdaptiveView> {
        // TODO: make this vertically scrollable
        Desktop = {
            flow: Down, spacing: 15
            align: {x: 0.5}
            padding: {top: 40., bottom: 20.}
            width: (SPACES_DOCK_SIZE), height: Fill
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <CachedWidget> {
                profile_icon = <ProfileIcon> {}
            }

            <LineH> { margin: {left: 15, right: 15} }

            <Home> {}

            <Filler> {}

            <LogoutButton> {}
            
        }

        // TODO: make this horizontally scrollable via touch
        Mobile = {
            flow: Right
            align: {x: 0.5, y: 0.5}
            padding: {top: 10, right: 10, bottom: 10, left: 10}
            width: Fill, height: (SPACES_DOCK_SIZE)
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Filler> {}

            <CachedWidget> {
                profile_icon = <ProfileIcon> {}
            }

            <Filler> {}

            <LogoutButton> {}

            <Home> {}

            <Filler> {}
        }
    }
}

/// The icon in the SpacesDock that show the user's avatar.
///
/// Clicking on this icon will open the settings screen.
#[derive(Live, Widget)]
pub struct ProfileIcon {
    #[deref] view: View,
    #[rust] is_selected: bool,
    #[rust] own_profile: Option<UserProfile>,
}

impl LiveHook for ProfileIcon {
    fn after_update_from_doc(&mut self, cx: &mut Cx) {
        if self.own_profile.is_none() {
            self.own_profile = get_own_profile(cx);
        }
    }
}

impl Widget for ProfileIcon {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A UI Signal indicates that a user profile or avatar may have been updated in the background.
        if let Event::Signal = event {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);

            // Refetch our profile if we don't have it yet, or if we're waiting for an avatar image.
            if self.own_profile.as_ref().is_none_or(|p| p.avatar_state.uri().is_some()) {
                self.own_profile = get_own_profile(cx);
                if self.own_profile.is_some() {
                    self.view.redraw(cx);
                }
            }
        }

        // TODO: handle login/logout actions, as well as actions related to
        //       the currently-logged-in user's account (such as them changing
        //       their avatar, display name, etc.)

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                    self.own_profile = get_own_profile(cx);
                    self.view.redraw(cx);
                }

                if let Some(SettingsAction::CloseSettings) = action.downcast_ref() {
                    self.is_selected = false;
                    self.view.redraw(cx);
                }
            }
        }

        let area = self.view.area();
        match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(_) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(_) => {
                let (verification_str, bg_color) = self.view
                    .verification_badge(id!(verification_badge))
                    .tooltip_content();
                let text = self.own_profile.as_ref().map_or_else(
                    || format!("Not logged in.\n\n{}\n\nTap to view Profiles & Settings.", verification_str),
                    |p| format!("Logged in as \"{}\".\n\n{}\n\nTap to view Profile & Settings.", p.displayable_name(), verification_str)
                );
                let rect = area.rect(cx);
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    TooltipAction::HoverIn {
                        widget_rect: rect,
                        text,
                        bg_color,
                        text_color: None,
                    },
                );
            }
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => {
                cx.action(SettingsAction::OpenSettings);
                self.is_selected = true;
                // TODO: actually use the `is_selected` state by showing a
                //       blue border around the avatar icon (like the `Home` view above).
                self.view.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
            }
            _ => { }
        };

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let our_own_avatar = self.view.avatar(id!(our_own_avatar));
        let Some(own_profile) = self.own_profile.as_ref() else {
            // If we don't have a profile, default to an unknown avatar.
            our_own_avatar.show_text(
                cx,
                Some(COLOR_FG_DISABLED),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                "",
            );
            return self.view.draw_walk(cx, scope, walk);
        };

        let mut drew_avatar = false;
        if let Some(avatar_img_data) = own_profile.avatar_state.data() {
            drew_avatar = our_own_avatar.show_image(
                cx,
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                |cx, img| utils::load_png_or_jpg(&img, cx, avatar_img_data),
            ).is_ok();
        }
        if !drew_avatar {
            our_own_avatar.show_text(
                cx,
                Some(COLOR_ROBRIX_PURPLE),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                own_profile.displayable_name(),
            );
        }

        self.view.draw_walk(cx, scope, walk)
    }
}

/// Returns the current user's profile and avatar, if available.
pub fn get_own_profile(cx: &mut Cx) -> Option<UserProfile> {
    let mut own_profile = None;
    if let Some(own_user_id) = current_user_id() {
        let avatar_uri_to_fetch = user_profile_cache::with_user_profile(
            cx,
            own_user_id,
            true,
            |new_profile, _rooms| {
                let avatar_uri_to_fetch = new_profile.avatar_state.uri().cloned();
                own_profile = Some(new_profile.clone());
                avatar_uri_to_fetch
            },
        );
        // If we have an avatar URI to fetch, try to fetch it.
        let mut new_profile_with_avatar = None;
        if let Some(Some(avatar_uri)) = avatar_uri_to_fetch {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, avatar_uri) {
                if let Some(p) = own_profile.as_mut() {
                    p.avatar_state = AvatarState::Loaded(data);
                    new_profile_with_avatar = Some(p.clone());
                }
            }
        }
        // Update the user profile cache if we got new avatar data.
        if let Some(new_profile) = new_profile_with_avatar {
            user_profile_cache::enqueue_user_profile_update(
                UserProfileUpdate::UserProfileOnly(new_profile)
            );
        }
    }

    own_profile
}


#[derive(Live, LiveHook, Widget)]
pub struct LogoutButton{
    #[deref] view: View,
    /// Whether a LogoutConfirmModal dialog has been displayed.
    /// This prevents showing multiple modal dialogs if the user
    /// clicks the logout button repeatedly.
    #[rust(false)] has_shown_modal: bool,
}

impl Widget for LogoutButton{
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for LogoutButton {
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, _scope: &mut Scope) {
        let button = self.button(id!(logout_button));
        
        if button.clicked(actions) && !self.has_shown_modal {
            self.has_shown_modal = true;
            cx.action(LogoutConfirmModalAction::Open);
        }
        
        for action in actions.iter() {
            if let Some(LogoutConfirmModalAction::Close { .. }) = action.downcast_ref::<LogoutConfirmModalAction>() {
                self.has_shown_modal = false;
            }
        }
    }
}