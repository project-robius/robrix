//! The NavigationTabBar shows a bar of radio-button icons
//! on the left side bar or along the bottom.
//! These buttons allow the user navigate/switch between
//! the top-level views in Robrix.
//!
//! Their order in Mobile view (horizontally from left to right) is:
//! 1. Home [house icon]: the main view with the rooms list and the room content
//!    * TODO: add a SpacesBar: a skinny scrollable PortalList showing all Spaces avatars.
//!      * In the Mobile view, this will be shown horizontally on the bottom of the main view
//!        (just above the current NavigationTabBar).
//!      * In the Desktop view, this will be "embedded" within the NavigationTabBar itself.
//!        We could optionally allow users to "pop it out", just to the right of the NavigationTabBar
//!        such that it could occupy the full height of the app window.
//! 2. Add/Join [plus sign icon]: a new view to handle adding (joining) existing rooms, exploring public rooms,
//!    or creating new rooms/spaces.
//! 3. Spaces: a button that toggles the `SpacesBar` (shows/hides it).
//!    This is NOT a regular radio button.
//! 4. Activity [an inbox, alert bell, or notifications icon]:  like Cinny, this shows a new view
//!    with a list of notifications, mentions, invitations, etc.
//! 5. Profile/Settings [ProfileIcon]: the existing ProfileIcon with the verification badge.
//!    Upon click, this shows the SettingsScreen as normal.
//!
//! The order in Desktop view (vertically from top to bottom) is:
//! 1. Profile/Settings
//! 2. Home
//! 3. ----- separator -----
//!      (the Spaces Bar)
//!    ----- separator -----
//! 4. Activity/Inbox
//! 5. Add/Join
//!

use makepad_widgets::*;

use crate::{
    avatar_cache::{self, AvatarCacheEntry}, login::login_screen::LoginAction, logout::logout_confirm_modal::LogoutAction, profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache::{self, UserProfileUpdate},
    }, shared::{
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
    use crate::shared::avatar::*;

    NAVIGATION_TAB_BAR_SIZE = 68

    COLOR_NAVIGATION_TAB_FG = #1C274C
    COLOR_NAVIGATION_TAB_FG_HOVER = #1C274C
    COLOR_NAVIGATION_TAB_FG_ACTIVE = #1C274C
    COLOR_NAVIGATION_TAB_BG        = (COLOR_SECONDARY)
    COLOR_NAVIGATION_TAB_BG_HOVER  = (COLOR_SECONDARY * 0.85)
    COLOR_NAVIGATION_TAB_BG_ACTIVE = (COLOR_SECONDARY * 0.5)

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    // A RadioButton styled to fit within our NavigationTabBar.
    pub NavigationTabButton = <RadioButton> {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 5),
        padding: 5,
        margin: 3, 
        align: {x: 0.5, y: 0.5}
        flow: Down,
        
        icon_walk: {margin: 0, width: (NAVIGATION_TAB_BAR_SIZE/2.2), height: Fit}
        // Fully hide the text with zero size, zero margin, and zero spacing
        label_walk: {margin: 0, width: 0, height: 0}
        spacing: 0,

        draw_bg: {
            radio_type: Tab,

            border_size: 0.0
            border_color: #0000
            uniform inset: vec4(0.0, 0.0, 0.0, 0.0)
            border_radius: 4.0

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        COLOR_NAVIGATION_TAB_BG,
                        COLOR_NAVIGATION_TAB_BG_HOVER,
                        self.hover
                    ),
                    COLOR_NAVIGATION_TAB_BG_ACTIVE,
                    self.active
                )
            }

            fn get_border_color(self) -> vec4 {
                return self.border_color
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                sdf.box(
                    self.inset.x + self.border_size,
                    self.inset.y + self.border_size,
                    self.rect_size.x - (self.inset.x + self.inset.z + self.border_size * 2.0),
                    self.rect_size.y - (self.inset.y + self.inset.w + self.border_size * 2.0),
                    max(1.0, self.border_radius)
                )
                sdf.fill_keep(self.get_color())
                if self.border_size > 0.0 {
                    sdf.stroke(self.get_border_color(), self.border_size)
                }
                return sdf.result;
            }
        }

        draw_text: {
            color: (COLOR_NAVIGATION_TAB_FG)
            color_hover: (COLOR_NAVIGATION_TAB_FG_HOVER)
            color_active: (COLOR_NAVIGATION_TAB_FG_ACTIVE)

            text_style: <THEME_FONT_BOLD>{font_size: 9}

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }
        }

        draw_icon: {
            uniform color: (COLOR_NAVIGATION_TAB_FG)
            uniform color_hover: (COLOR_NAVIGATION_TAB_FG_HOVER)
            uniform color_active: (COLOR_NAVIGATION_TAB_FG_ACTIVE)
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.focus
                    ),
                    self.color_active,
                    self.active
                )
            }
        }
    }

    ProfileIcon = {{ProfileIcon}}<RoundedView> {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 8)
        flow: Overlay
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
            align: { x: 0.5, y: 0.0 }
            margin: { left: 42 }
            verification_badge = <VerificationBadge> {}
        }
    }

    HomeButton = <NavigationTabButton> {
        draw_icon: { svg_file: (ICON_HOME) }
        animator: { active = { default: on } }
    }

    SettingsButton = <NavigationTabButton> {
        draw_icon: { svg_file: (ICON_SETTINGS) }
    }

    AddRoomButton = <NavigationTabButton> {
        draw_icon: {
            svg_file: (ICON_ADD),
            color: (COLOR_FG_DISABLED),
        }
        animator: { disabled = { default: on } }
    }

    Separator = <LineH> { margin: 8 }

    pub NavigationTabBar = {{NavigationTabBar}}<AdaptiveView> {
        Desktop = {
            flow: Down,
            align: {x: 0.5}
            padding: {top: 40., bottom: 8}
            width: (NAVIGATION_TAB_BAR_SIZE), 
            height: Fill

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
            width: Fill,
            height: (NAVIGATION_TAB_BAR_SIZE)

            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            home_button = <HomeButton> {}

            add_room_button = <AddRoomButton> {}

            settings_button = <SettingsButton> {}

            <CachedWidget> {
                profile_icon = <ProfileIcon> {}
            }
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
}

impl Widget for NavigationTabBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            // Handle one of the radio buttons being clicked (selected).
            let radio_button_set = self.view.radio_button_set(ids_array!(
                home_button,
                add_room_button,
                settings_button,
            ));
            match radio_button_set.selected(cx, actions) {
                Some(0) => cx.action(NavigationBarAction::GoToHome),
                Some(1) => cx.action(NavigationBarAction::GoToAddRoom),
                Some(2) => cx.action(NavigationBarAction::OpenSettings),
                _ => { }
            }

            for action in actions {
                // If another widget programmatically selected a new tab,
                // update our radio buttons accordingly.
                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    let radio_button_to_select = match tab {
                        SelectedTab::Home     => self.view.radio_button(ids!(home_button)),
                        SelectedTab::AddRoom  => self.view.radio_button(ids!(add_room_button)),
                        SelectedTab::Settings => self.view.radio_button(ids!(settings_button)),
                    };
                    radio_button_to_select.select(cx, scope);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


/// Which tab is currently selected in the NavigationTabBar.
#[derive(Clone, Debug, Default)]
pub enum SelectedTab {
    #[default]
    Home,
    AddRoom,
    Settings,
    // AlertsInbox,

    // Once we support spaces and shortcut buttons (like directs only, etc),
    // we can add them here.
}


/// Actions for navigating through the top-level views of the app,
/// e.g., when the user clicks/taps on a button in the NavigationTabBar.
#[derive(Debug)]
pub enum NavigationBarAction {
    /// Go to the main rooms content view.
    GoToHome,
    /// Go the add/join/explore room view.
    GoToAddRoom,
    /// Go to the Settings view (open the `SettingsScreen`).
    OpenSettings,
    /// Close the Settings view (`SettingsScreen`), returning to the previous page.
    CloseSettings,
    /// The given tab was selected programmatically, e.g., after closing the settings screen.
    /// This is needed just to ensure that the proper tab radio button is marked as selected.
    TabSelected(SelectedTab),
    // GoToAlertsInbox
    // GoToSpace { space_id: OwnedRoomId }, // TODO: uncomment once we impl the SpacesBar
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
