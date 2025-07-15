use makepad_widgets::*;

use crate::{
    avatar_cache::{self, AvatarCacheEntry},
    login::{login_screen::LoginAction, logout_confirm_modal::LogoutConfirmModalAction},
    profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache,
    },
    shared::{
        avatar::AvatarWidgetExt,
        callout_tooltip::TooltipAction,
        popup_list::{enqueue_popup_notification, PopupItem},
        styles::{COLOR_DISABLE_GRAY, COLOR_ROBRIX_PURPLE}, verification_badge::VerificationBadgeWidgetExt,
    },
    sliding_sync::current_user_id,
    utils,
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

        avatar = <Avatar> {
            width: 45, height: 45
            // If no avatar picture, use white text on a dark background.
            text_view = {
                draw_bg: {
                    background_color: (COLOR_DISABLE_GRAY),
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

    Separator = <LineH> {
        margin: {left: 15, right: 15}
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

            <Home> {}
            
            <Separator> {}

            <Filler> {}

            <LogoutButton> {}
            
            <ProfileIcon> {}
        }

        // TODO: make this horizontally scrollable
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

            <Home> {}
            
            <Filler> {}

            <LogoutButton> {}
            
            <ProfileIcon> {}

            <Filler> {}
        }
    }
}

#[derive(Live, Widget)]
pub struct ProfileIcon {
    #[deref] view: View,
    #[rust] own_profile: Option<UserProfile>,
}


impl LiveHook for ProfileIcon {
    fn after_apply_from_doc(&mut self, cx: &mut Cx) {
        if self.own_profile.is_none() {
            self.get_own_profile(cx);
        }
    }
}

impl Widget for ProfileIcon {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // A UI Signal indicates that a user profile or avatar may have been updated in the background.
        if let Event::Signal = event {
            user_profile_cache::process_user_profile_updates(cx);
            avatar_cache::process_avatar_updates(cx);
            if self.own_profile.is_none() {
                self.get_own_profile(cx);
            }
        }

        // TODO: handle login/logout actions, as well as actions related to
        //       the currently-logged-in user's account (such as them changing
        //       their avatar, display name, etc.)

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                    log!("[ProfileIcon] Login successful, calling get_own_profile().");
                    self.get_own_profile(cx);
                }
            }
        }

        let area = self.view.area();
        match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(_)
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
                // TODO: emit action to show profile/settings screen in the parent view.
                enqueue_popup_notification(PopupItem {
                    message: String::from("ProfileIcon & Settings screen is not yet implemented."),
                    auto_dismissal_duration: None,
                });
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
            }
            _ => { }
        };

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let avatar = self.view.avatar(id!(avatar));
        let Some(own_profile) = self.own_profile.as_ref() else {
            // If we don't have a profile, default to an unknown avatar.
            avatar.show_text(
                cx,
                Some(COLOR_DISABLE_GRAY),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                "",
            );
            return self.view.draw_walk(cx, scope, walk);
        };

        let mut drew_avatar = false;
        if let Some(avatar_img_data) = own_profile.avatar_state.data() {
            drew_avatar = avatar.show_image(
                cx,
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                |cx, img| utils::load_png_or_jpg(&img, cx, avatar_img_data),
            ).is_ok();
        }
        if !drew_avatar {
            self.view.avatar(id!(avatar)).show_text(
                cx,
                Some(COLOR_ROBRIX_PURPLE),
                None, // don't make this avatar clickable; we handle clicks on this ProfileIcon widget directly.
                own_profile.displayable_name(),
            );
        }

        self.view.draw_walk(cx, scope, walk)
    }
}


impl ProfileIcon {
    /// Re-obtains the current user's profile and avatar, if available.
    fn get_own_profile(&mut self, cx: &mut Cx) {
        let mut needs_redraw = false;
        if let Some(own_user_id) = current_user_id() {
            let avatar_uri_to_fetch = user_profile_cache::with_user_profile(cx, own_user_id, true, |new_profile, _rooms| {
                needs_redraw = self.own_profile.as_ref().is_none_or(
                    |p| p.displayable_name() != new_profile.displayable_name()
                );
                let avatar_uri_to_fetch = new_profile.avatar_state.uri().cloned();
                self.own_profile = Some(new_profile.clone());
                avatar_uri_to_fetch
            });
            if let Some(Some(avatar_uri)) = avatar_uri_to_fetch {
                if let AvatarCacheEntry::Loaded(data) = avatar_cache::get_or_fetch_avatar(cx, avatar_uri) {
                    if let Some(own_profile) = self.own_profile.as_mut() {
                        own_profile.avatar_state = AvatarState::Loaded(data);
                        needs_redraw = true;
                    }
                }
            }
            log!("[ProfileIcon] Updated own profile: {:?}", self.own_profile);
        }

        if needs_redraw {
            self.view.redraw(cx);
        }
    }
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