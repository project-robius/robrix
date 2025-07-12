use makepad_widgets::*;

use crate::{
    avatar_cache::{self, AvatarCacheEntry},
    login::login_screen::LoginAction,
    profile::{
        user_profile::{AvatarState, UserProfile},
        user_profile_cache,
    },
    shared::{
        avatar::AvatarWidgetExt,
        callout_tooltip::TooltipAction,
        popup_list::{enqueue_popup_notification, PopupItem},
        styles::{COLOR_DISABLE_GRAY, COLOR_ROBRIX_PURPLE},
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

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    Filler = <View> {
        height: Fill, width: Fill
    }

    ProfileIcon = {{ProfileIcon}} {
        flow: Overlay
        width: Fit, height: Fit
        align: { x: 0.5, y: 0.5 }

        avatar = <Avatar> {
            width: 40, height: 40
            // If no avatar picture, use white text on a dark background.
            text_view = {
                draw_bg: {
                    background_color: (COLOR_DISABLE_GRAY),
                }
                text = {
                    font_size: 16.0,
                    color: (COLOR_PRIMARY),
                }
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

    pub SpacesDock = <AdaptiveView> {
        // TODO: make this vertically scrollable
        Desktop = {
            flow: Down, spacing: 15
            align: {x: 0.5}
            padding: {top: 40., bottom: 20.}
            width: 68., height: Fill
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Home> {}
            
            <Separator> {}
            
            <ProfileIcon> {}

            <Filler> {}
        }

        // TODO: make this horizontally scrollable
        Mobile = {
            flow: Right
            align: {x: 0.5, y: 0.5}
            padding: {top: 10, right: 10, bottom: 10, left: 10}
            width: Fill, height: Fit
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Filler> {}

            <Home> {}
            
            <Filler> {}
            
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

        self.view.handle_event(cx, event, scope);

        // TODO: handle login/logout actions, as well as actions related to
        //       the currently-logged-in user's account (such as them changing
        //       their avatar, display name, etc.)

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                    self.get_own_profile(cx);
                }
            }
        }

        let area = self.area();
        let should_hover_in: bool;
        match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(..) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(..) => {
                should_hover_in = true;
            }
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => {
                // TODO: emit action to show profile/settings screen in the parent view.
                enqueue_popup_notification(PopupItem {
                    message: String::from("ProfileIcon & Settings screen is not yet implemented."),
                    auto_dismissal_duration: None,
                });
                should_hover_in = false;
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
                should_hover_in = false;
            }
            _ => {
                should_hover_in = false;
            }
        };
        if should_hover_in {
            let rect = area.rect(cx);
            let text = self.own_profile.as_ref().map_or_else(
                || format!("Not logged in.\n\nClick to view Profiles & Settings"),
                |p| format!("Logged in as \"{}\".\n\nClick to view Profile & Settings.", p.displayable_name())
            );
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                TooltipAction::HoverIn {
                    widget_rect: rect,
                    text,
                    bg_color: None,
                    text_color: None,
                },
            );
        }
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
                needs_redraw = self.own_profile.as_ref().map_or(
                    true,
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
        }

        if needs_redraw {
            self.view.redraw(cx);
        }
    }
}
