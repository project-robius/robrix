//! The spaces dock (TODO: rename this) shows a side bar or bottom bar
//! of radio buttons that allow the user to navigate/switch between
//! various top-level views in Robrix.
//!
//! Here's their order (in Mobile view, horizontally from left to right):
//! 1. Home [house icon]: the main view with the rooms list and the room content
//!    * TODO: add a SpacesBar: a skinny scrollable PortalList showing all Spaces avatars.
//!      * In the Mobile view, this will be shown horizontally on the bottom of the main view
//!        (just above the current NavigationTabBar).
//!      * In the Desktop view, this will be shown vertically on the left of the main view
//!        (just to the right of the current NavigationTabBar).
//!        * We could also optionally embed it directly into the current NavigationTabBar too (like Element).
//!    * Search should only be available within the main Home view.
//! 2. Add/Join [plus sign icon]: a new view to handle adding (joining) existing rooms, exploring public rooms,
//!    or creating new rooms/spaces.
//! 3. Activity [an inbox or notifications icon]:  like Cinny, this shows a new view
//!    with a list of notifications, mentions, invitations, etc.
//! 4. Profile/Settings [profile icon]: the existing ProfileIcon with the verification badge
//!
//! The order in Desktop view (vertically from top to bottom) is:
//! 1. Profile/Settings
//! 2. Home
//! 3. Activity/Inbox
//! 4. Add/Join
//! 5. (Spaces Bar)
//!
//!
//! TODO: for now, go back to using radio button app tabs like we were before.
//! We can also place the existing ProfileIcon on the bottom left, but just make it non-clickable.

use makepad_widgets::*;

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, login::login_screen::LoginAction, logout::logout_confirm_modal::LogoutAction, profile::{
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
    use crate::shared::icon_button::*;
    use crate::shared::verification_badge::*;
    use crate::shared::avatar::*;

    NAVIGATION_TAB_BAR_SIZE = 68
    COLOR_NAVIGATION_TAB_BAR_ICON = #1C274C

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    ProfileIcon = {{ProfileIcon}}<RoundedView> {
        flow: Overlay
        width: (NAVIGATION_TAB_BAR_SIZE - 6), height: (NAVIGATION_TAB_BAR_SIZE - 6)
        align: { x: 0.5, y: 0.5 }
        cursor: Default,

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

    HomeButton = <RobrixIconButton> {
        width: Fit
        height: Fit,
        padding: {top: 12, left: 12, right: 12, bottom: 12}
        spacing: 0

        draw_bg: {
            // border_color: (COLOR_PRIMARY_DARKER),
            color: #00000000,
            color_hover: (COLOR_PRIMARY)
            // border_radius: 5
        }
        draw_icon: {
            svg_file: (ICON_HOME),
            color: (COLOR_NAVIGATION_TAB_BAR_ICON),
        }
        icon_walk: {width: 25, height: Fit, margin: 0}
    }

    SettingsButton = <RobrixIconButton> {
        width: Fit
        height: Fit,
        padding: {top: 12, left: 12, right: 12, bottom: 12}
        spacing: 0

        draw_bg: {
            // border_color: (COLOR_PRIMARY_DARKER),
            color: #00000000,
            color_hover: (COLOR_PRIMARY)
            // border_radius: 5
        }
        draw_icon: {
            svg_file: (ICON_SETTINGS),
            color: (COLOR_NAVIGATION_TAB_BAR_ICON),
        }
        icon_walk: {width: 25, height: Fit, margin: 0}
    }

    AddRoomButton = <RobrixIconButton> {
        width: Fit
        height: Fit,
        padding: {top: 12, left: 12, right: 12, bottom: 12}
        spacing: 0
        enabled: false,

        draw_bg: {
            color: #00000000,
            color_hover: (COLOR_PRIMARY)
        }
        draw_icon: {
            svg_file: (ICON_ADD),
            // color: (COLOR_NAVIGATION_TAB_BAR_ICON),
            color: (COLOR_FG_DISABLED),
        }
        icon_walk: {width: 25, height: Fit, margin: 0}
    }

    Separator = <LineH> { margin: {top: 2, bottom: 2, left: 10, right: 10} }

    pub NavigationTabBar = {{NavigationTabBar}}<AdaptiveView> {
        Desktop = {
            flow: Down, spacing: 5
            align: {x: 0.5}
            padding: {top: 40., bottom: 8}
            width: (NAVIGATION_TAB_BAR_SIZE), height: Fill
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <CachedWidget> {
                profile_icon = <ProfileIcon> {}
            }

            home_button = <HomeButton> {}

            add_room_button = <AddRoomButton> {}

            <Separator> {}

            <Filler> {}

            // TODO: SpacesBar goes here, which should be a vertically-scrollable PortalList
            //       in this case, and a show/hidable horizontally-scrollable one in Mobile mode.
            
            <Filler> {}

            <Separator> {}
            
            settings_button = <SettingsButton> {}
        }

        Mobile = {
            flow: Right
            align: {x: 0.5, y: 0.5}
            padding: {top: 10, right: 10, bottom: 10, left: 10}
            width: Fill, height: (NAVIGATION_TAB_BAR_SIZE)
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Filler> {}

            home_button = <HomeButton> {}

            <Filler> {}

            add_room_button = <AddRoomButton> {}

            <Filler> {}

            settings_button = <SettingsButton> {}
            
            <Filler> {}
            
            <CachedWidget> {
                profile_icon = <ProfileIcon> {}
            }

            <Filler> {}
        }
    }
}

/// The icon in the NavigationTabBar that show the user's avatar.
///
/// Clicking on this icon will open the settings screen.
#[derive(Live, Widget)]
pub struct ProfileIcon {
    #[deref] view: View,
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
                    continue;
                }

                if let Some(LogoutAction::ClearAppState { .. }) = action.downcast_ref() {
                    self.own_profile = None;
                    self.view.redraw(cx);
                    continue;
                }
            }
        }

        let area = self.view.area();
        match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(_) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(_) => {
                let (verification_str, bg_color) = self.view
                    .verification_badge(ids!(verification_badge))
                    .tooltip_content();
                let text = self.own_profile.as_ref().map_or_else(
                    || format!("Not logged in.\n\n{}", verification_str),
                    |p| format!("Logged in as \"{}\".\n\n{}", p.displayable_name(), verification_str)
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
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
            }
            _ => { }
        };

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let our_own_avatar = self.view.avatar(ids!(our_own_avatar));
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


/// The tab bar with buttons that navigate through top-level app pages.
///
/// * In the "desktop" (wide) layout, this is a vertical bar on the left.
/// * In the "mobile" (narrow) layout, this is a horizontal bar on the bottom.
#[derive(Live, LiveHook, Widget)]
pub struct NavigationTabBar {
    #[deref] view: AdaptiveView,
    #[rust] is_settings_shown: bool,
}

impl Widget for NavigationTabBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            if !self.is_settings_shown
                && self.view.button(ids!(settings_button)).clicked(actions)
            {
                self.is_settings_shown = true;
                cx.action(SettingsAction::OpenSettings);
            }

            if self.view.button(ids!(home_button)).clicked(actions) {
                cx.action(SettingsAction::CloseSettings);
            }

            for action in actions {
                if let Some(SettingsAction::CloseSettings) = action.downcast_ref() {
                    self.is_settings_shown = false;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
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
