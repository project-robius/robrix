//! The NavigationTabBar shows a bar of icon buttons that allow the user to
//! navigate or switch between various top-level views in Robrix.
//!
//! The bar is positioned either within the left side bar (in the wide "Desktop" view mode)
//! or along the bottom of the app window (in the narrow "Mobile" view mode).
//!
//! Their order in Mobile view (horizontally from left to right) is:
//! 1. Home (house icon): the main view that shows all rooms across all spaces.
//! 2. Add Room (plus sign icon): a separate view that allows adding (joining) existing rooms,
//!    exploring public rooms, or creating new rooms/spaces.
//! 3. Spaces: a button that toggles the `SpacesBar` (shows/hides it).
//!    * This is NOT a regular radio button, it's a separate toggle. 
//!    * This is only shown in Mobile view mode, because the `SpacesBar` is always shown
//!      within the NavigationTabBar itself in Desktop view mode.
//! 4. Activity (an inbox, alert bell, or notifications icon): a separate view that shows
//!    a list of notifications, mentions, invitations, etc.
//! 5. Profile/Settings (user profile avatar): the existing `ProfileIcon` with a
//!    verification badge.
//!    * Upon click, this shows the SettingsScreen as normal.
//!
//! The order in Desktop view (vertically from top to bottom) is:
//! 1. Home
//! 2. Add/Join
//! 3. ----- separator -----
//!      SpacesBar content
//!    ----- separator -----
//! 4. Activity/Inbox
//! 5. Profile/Settings
//!

use makepad_widgets::*;
use crate::{
    avatar_cache::{self, AvatarCacheEntry}, login::login_screen::LoginAction, logout::logout_confirm_modal::LogoutAction, profile::{
        user_profile::UserProfile,
        user_profile_cache::{self, UserProfileUpdate},
    }, shared::{
        avatar::{AvatarState, AvatarWidgetExt}, callout_tooltip::{CalloutTooltipOptions, TooltipAction, TooltipPosition}, styles::*, verification_badge::VerificationBadgeWidgetExt
    }, sliding_sync::{current_user_id, AccountDataAction}, utils::{self, RoomNameId}
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // A RadioButton styled to fit within our NavigationTabBar.
    mod.widgets.NavigationTabButton = RadioButton {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 5),
        padding: 5,
        margin: 3, 
        align: Align{x: 0.5, y: 0.5}
        flow: Down,
        
        icon_walk: Walk{margin: 0, width: (NAVIGATION_TAB_BAR_SIZE/2.2), height: Fit}
        // Fully hide the text with zero size, zero margin, and zero spacing
        label_walk: Walk{margin: 0, width: 0, height: 0}
        spacing: 0,

        draw_bg +: {
            radio_type: Tab,

            color: uniform((COLOR_NAVIGATION_TAB_BG))
            color_hover: uniform((COLOR_NAVIGATION_TAB_BG_HOVER))
            color_active: uniform((COLOR_NAVIGATION_TAB_BG_ACTIVE))

            border_size: 0.0
            border_color: #0000
            inset: uniform(vec4(0.0), 0.0, 0.0, 0.0)
            border_radius: 4.0

            get_color: fn() -> vec4 {
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

            get_border_color: fn() -> vec4 {
                return self.border_color
            }

            pixel: fn() -> vec4 {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
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

        draw_text +: {
            hover: instance(0.0)
            active: instance(0.0)
            color: (COLOR_NAVIGATION_TAB_FG)
            color_hover: uniform((COLOR_NAVIGATION_TAB_FG_HOVER))
            color_active: uniform((COLOR_NAVIGATION_TAB_FG_ACTIVE))

            text_style: theme.font_bold {font_size: 9}

            get_color: fn() -> vec4 {
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

        draw_icon +: {
            hover: instance(0.0)
            active: instance(0.0)
            color: (COLOR_NAVIGATION_TAB_FG)
            color_hover: uniform((COLOR_NAVIGATION_TAB_FG_HOVER))
            color_active: uniform((COLOR_NAVIGATION_TAB_FG_ACTIVE))
            get_color: fn() -> vec4 {
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
    }

    mod.widgets.ProfileIcon = #(ProfileIcon::register_widget(vm)) {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE - 8)
        flow: Overlay
        align: Align{ x: 0.5, y: 0.5 }
        cursor: Default,

        our_own_avatar := Avatar {
            width: 45, height: 45
            // If no avatar picture, use white text on a dark background.
            text_view := View {
                draw_bg +: {
                    background_color: (COLOR_FG_DISABLED),
                }
                text := Label { draw_text +: {
                    text_style: theme.font_regular { font_size: 16.0 },
                    color: (COLOR_PRIMARY),
                } }
            }
        }

        View {
            align: Align{ x: 0.5, y: 0.0 }
            margin: Inset{ left: 42 }
            verification_badge := VerificationBadge {}
        }
    }

    mod.widgets.HomeButton = mod.widgets.NavigationTabButton {
        draw_icon +: { svg_file: (ICON_HOME) }
    }

    mod.widgets.ToggleSpacesBarButton = RobrixIconButton {
        width: Fill,
        padding: 16
        spacing: 0,
        align: Align{x: 0.5, y: 0.5}
        draw_bg +: {
            color: (COLOR_SECONDARY)
        }
        draw_icon +: {
            svg_file: (ICON_SQUARES)
            color: (COLOR_NAVIGATION_TAB_FG)
        }
        icon_walk: Walk{width: (NAVIGATION_TAB_BAR_SIZE/2.2), height: Fit, margin: 0 }
    }

    mod.widgets.SettingsButton = mod.widgets.NavigationTabButton {
        draw_icon +: { svg_file: (ICON_SETTINGS) }
    }

    mod.widgets.AddRoomButton = mod.widgets.NavigationTabButton {
        draw_icon +: { svg_file: (ICON_ADD) }
    }

    mod.widgets.Separator = LineH { margin: 8 }

    mod.widgets.NavigationTabBar = #(NavigationTabBar::register_widget(vm)) {
        Desktop := View {
            flow: Down,
            align: Align{x: 0.5}
            padding: Inset{top: 40., bottom: 8}
            width: (NAVIGATION_TAB_BAR_SIZE), 
            height: Fill

            show_bg: true
            draw_bg +: {
                color: (COLOR_SECONDARY)
            }

            CachedWidget {
                profile_icon := mod.widgets.ProfileIcon {}
            }

            CachedWidget {
                home_button := mod.widgets.HomeButton {}
            }

            CachedWidget {
                add_room_button := mod.widgets.AddRoomButton {}
            }

            mod.widgets.Separator {}

            CachedWidget {
                root_spaces_bar := mod.widgets.SpacesBar {}
            }

            mod.widgets.Separator {}
            
            CachedWidget {
                settings_button := mod.widgets.SettingsButton {}
            }
        }

        Mobile := RoundedView {
            flow: Right
            align: Align{x: 0.5, y: 0.5}
            width: Fill,
            height: (NAVIGATION_TAB_BAR_SIZE)

            show_bg: true
            draw_bg +: {
                color: (COLOR_SECONDARY)
                border_radius: 4.0
            }

            CachedWidget {
                home_button := mod.widgets.HomeButton {}
            }

            CachedWidget {
                add_room_button := mod.widgets.AddRoomButton {}
            }

            toggle_spaces_bar_button := mod.widgets.ToggleSpacesBarButton {}

            CachedWidget {
                settings_button := mod.widgets.SettingsButton {}
            }

            CachedWidget {
                profile_icon := mod.widgets.ProfileIcon {}
            }
        }
    }
}

/// The icon in the NavigationTabBar that show the user's avatar.
///
/// Clicking on this icon will open the settings screen.
#[derive(Script, ScriptHook, Widget)]
pub struct ProfileIcon {
    #[deref] view: View,
    #[rust] own_profile: Option<UserProfile>,
}

impl Widget for ProfileIcon {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.own_profile.is_none() {
            self.own_profile = get_own_profile(cx);
        }

        // A UI Signal indicates that a user profile or avatar may have been updated.
        if let Event::Signal = event {
            let mut needs_redraw = false;
            // Refetch our profile if we don't have it yet.
            if self.own_profile.is_none() {
                user_profile_cache::process_user_profile_updates(cx);
                self.own_profile = get_own_profile(cx);
                needs_redraw = true;
            }
            // If we're waiting for an avatar image, process avatar updates.
            if let Some(p) = self.own_profile.as_mut() && p.avatar_state.uri().is_some() {
                avatar_cache::process_avatar_updates(cx);
                let new_data = p.avatar_state.update_from_cache(cx);
                needs_redraw |= new_data.is_some();
                if new_data.is_some() {
                    user_profile_cache::enqueue_user_profile_update(
                        UserProfileUpdate::UserProfileOnly(p.clone())
                    );
                }
            }
            if needs_redraw {
                self.view.redraw(cx);
            }
        }

        // Handle actions related to the currently-logged-in user account,
        // such as changing their avatar, display name, etc.
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

                // Handle account data changes (e.g., avatar updated/removed)
                match action.downcast_ref() {
                    Some(AccountDataAction::AvatarChanged(None)) => {
                        // Update both this widget's local profile info and the user profile cache.
                        if let Some(p) = self.own_profile.as_mut() {
                            p.avatar_state = AvatarState::Known(None);
                            user_profile_cache::enqueue_user_profile_update(
                                UserProfileUpdate::UserProfileOnly(p.clone())
                            );
                            self.view.redraw(cx);
                        }
                        continue;
                    }
                    Some(AccountDataAction::AvatarChanged(Some(new_uri))) => {
                        if let Some(p) = self.own_profile.as_mut() {
                            p.avatar_state = AvatarState::Known(Some(new_uri.clone()));
                            p.avatar_state.update_from_cache(cx);
                            user_profile_cache::enqueue_user_profile_update(
                                UserProfileUpdate::UserProfileOnly(p.clone())
                            );
                            self.view.redraw(cx);
                        }
                        continue;
                    }
                    Some(AccountDataAction::AvatarChangeFailed(_)) => {
                        // this is only handled in the account settings screen
                        continue;
                    }
                    Some(AccountDataAction::DisplayNameChanged(new_display_name)) => {
                        if let Some(p) = self.own_profile.as_mut() {
                            p.username = new_display_name.clone();
                            user_profile_cache::enqueue_user_profile_update(
                                UserProfileUpdate::UserProfileOnly(p.clone())
                            );
                            self.view.redraw(cx);
                        }
                        continue;
                    }
                    Some(AccountDataAction::DisplayNameChangeFailed(_)) => {
                        // this is only handled in the account settings screen
                        continue;
                    }
                    _ => {}
                }
            }
        }

        let area = self.view.area();
        match event.hits(cx, area) {
            Hit::FingerLongPress(_) | Hit::FingerHoverIn(_) => {
                let (verification_str, bg_color) = self.view
                    .verification_badge(cx, ids!(verification_badge))
                    .tooltip_content();
                let text = self.own_profile.as_ref().map_or_else(
                    || format!("Not logged in.\n\n{}", verification_str),
                    |p| format!("Logged in as \"{}\".\n\n{}", p.displayable_name(), verification_str)
                );
                let mut options = CalloutTooltipOptions {
                    position: if cx.display_context.is_desktop() { TooltipPosition::Right} else { TooltipPosition::Top},
                    ..Default::default()
                };
                if let Some(c) = bg_color {
                    options.bg_color = c;
                }
                cx.widget_action(
                    self.widget_uid(), 
                    TooltipAction::HoverIn {
                        text,
                        widget_rect: area.rect(cx),
                        options,
                    },
                );
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(),  TooltipAction::HoverOut);
            }
            _ => { }
        };

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let our_own_avatar = self.view.avatar(cx, ids!(our_own_avatar));
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
#[derive(Script, ScriptHook, Widget)]
pub struct NavigationTabBar {
    #[deref] view: AdaptiveView,

    #[rust] is_spaces_bar_shown: bool,
}

impl Widget for NavigationTabBar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            // Handle one of the radio buttons being clicked (selected).
            let radio_button_set = self.view.radio_button_set(cx, ids_array!(
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

            if self.view.button(cx, ids!(toggle_spaces_bar_button)).clicked(actions) {
                self.is_spaces_bar_shown = !self.is_spaces_bar_shown;
                cx.action(NavigationBarAction::ToggleSpacesBar);
            }

            for action in actions {
                // If another widget programmatically selected a new tab,
                // update our radio buttons accordingly.
                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    match tab {
                        SelectedTab::Home     => self.view.radio_button(cx, ids!(home_button)).select(cx, scope),
                        SelectedTab::AddRoom  => self.view.radio_button(cx, ids!(add_room_button)).select(cx, scope),
                        SelectedTab::Settings => self.view.radio_button(cx, ids!(settings_button)).select(cx, scope),
                        SelectedTab::Space { .. } => {
                            for rb in radio_button_set.iter() {
                                if let Some(mut rb_inner) = rb.borrow_mut() {
                                    rb_inner.animator_play(cx, ids!(active.off));
                                }
                            }
                        }
                    }
                    continue;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}


/// Which top-level view is currently shown, and which navigation tab is selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SelectedTab {
    #[default]
    Home,
    AddRoom,
    Settings,
    // AlertsInbox,
    Space { space_name_id: RoomNameId },
}


/// Actions for navigating through the top-level views of the app,
/// e.g., when the user clicks/taps on a button in the NavigationTabBar.
///
/// ## Tip: you only want to handle `TabSelected`
/// The most important variant is `TabSelected`, which is most likely the action
/// that you want to handle in other widgets, if you care about which
/// top-level navigation tab is currently selected.
/// This is because the `TabSelected` variant will always occur even if the
/// other actions do not occur --- for example, if the user chooses to jump
/// to a different view (or back to a previous view) without explicitly clicking
/// a navigation tab button, e.g., via a keyboard shortcut, or programmatically.
///
/// Only one widget, the `HomeScreen`, should emit the `TabSelected` action.
/// All other widgets should handle only that action in order to ensure
/// consistent behavior.
///
/// ## More details
/// There are 3 kinds of actions within this one enum:
/// 1. "Leading-edge" ("request") actions emitted by the NavigationTabBar
///    when the user selects a particular button/space.
///    * Includes `GoToHome`, `GoToAddRoom`, `GoToSpace`, `OpenSettings`, `CloseSettings`.
/// 2. "Trailing-edge" ("response") actions that are emitted by the `HomeScreen` widget
///    in response to a leading-edge action.
///    * This includes only the `TabSelected` variant.
///    * This is what all other widgets should handle if they want/need to respond
///      to changes in the top-level app-wide navigation selection.
/// 3. Other actions that aren't requests/responses to navigate to a different view.
///    * This only includes the `ToggleSpacesBar` variant.
#[derive(Debug, PartialEq, Eq)]
pub enum NavigationBarAction {
    /// Go to the main rooms content view.
    GoToHome,
    /// Go the add/join/explore room view.
    GoToAddRoom,
    /// Go to the Settings view (open the `SettingsScreen`).
    OpenSettings,
    /// Close the Settings view (`SettingsScreen`), returning to the previous view.
    CloseSettings,
    /// Go the space screen for the given space.
    GoToSpace { space_name_id: RoomNameId },

    // TODO: add GoToAlertsInbox, once we add that button/screen

    /// The given tab was selected as the active top-level view.
    /// This is needed to ensure that the proper tab is marked as selected. 
    TabSelected(SelectedTab),
    /// Toggle whether the SpacesBar is shown, i.e., show/hide it.
    /// This is only applicable in the Mobile view mode, because the SpacesBar
    /// is always shown in Desktop view mode.
    ToggleSpacesBar,
}


/// Returns the current user's profile and avatar, if available.
pub fn get_own_profile(cx: &mut Cx) -> Option<UserProfile> {
    let mut own_profile = None;
    if let Some(own_user_id) = current_user_id() {
        let avatar_uri_to_fetch = user_profile_cache::with_user_profile(
            cx,
            own_user_id,
            None,
            true,
            |new_profile, _rooms| {
                let avatar_uri_to_fetch = new_profile.avatar_state.uri().cloned();
                own_profile = Some(new_profile.clone());
                avatar_uri_to_fetch
            },
        );
        // If we have an avatar URI to fetch, try to fetch it.
        if let Some(Some(avatar_uri)) = avatar_uri_to_fetch {
            if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, &avatar_uri) {
                if let Some(p) = own_profile.as_mut() {
                    p.avatar_state = AvatarState::Loaded(data);
                    // Update the user profile cache with the new avatar data.
                    user_profile_cache::enqueue_user_profile_update(
                        UserProfileUpdate::UserProfileOnly(p.clone())
                    );
                }
            }
        }
    }

    own_profile
}
