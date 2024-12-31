use makepad_widgets::*;

use crate::shared::adaptive_view::DisplayContext;
use crate::shared::color_tooltip::*;
use crate::shared::verification_badge::{VerificationBadge, VerificationText};
use crate::verification::VerificationStateAction;
use crate::sliding_sync::get_client;
use matrix_sdk::encryption::VerificationState;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::adaptive_view::AdaptiveView;
    use crate::shared::verification_badge::*;
    use crate::shared::color_tooltip::*;

    ICON_HOME = dep("crate://self/resources/icons/home.svg")
    ICON_SETTINGS = dep("crate://self/resources/icons/settings.svg")

    Filler = <View> {
        height: Fill, width: Fill
    }

    Profile = {{Profile}} {
        flow: Overlay
        width: Fit, height: Fit
        align: { x: 0.5, y: 0.5 }

        text_view = <View> {
            flow: Overlay
            width: 60, height: 60,
            align: { x: 0.5, y: 0.5 }
            show_bg: true,
            draw_bg: {
                instance background_color: (COLOR_AVATAR_BG_IDLE),
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);

                    let c = self.rect_size * 0.5;

                    let r = self.rect_size * 0.38;

                    sdf.circle(c.x, c.x, r.x);
                    sdf.fill_keep(self.background_color);
                    return sdf.result
                }
            }

            text = <Label> {
                width: Fit, height: Fit,
                padding: { top: 1.0 } // for better vertical alignment
                draw_text: {
                    text_style: { font_size: 13. }
                    color: #f,
                }
                text: "U"
            }
        }

        <View> {
            align: { x: 1.0, y: 0.0 }
            verification_badge = <VerificationBadge> {}
        }

        profile_tooltip = <ColorTooltip> {
            content: {
                width: 200
            }
        }

    }

    Separator = <LineH> {
        margin: {left: 15, right: 15}
    }

    Home = <RoundedView> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is because the icon is not correctly centered
        // within its parent
        padding: {top: 8, left: 8, right: 12, bottom: 8}
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER)
            radius: 4.0
            border_color: (COLOR_SELECTED_PRIMARY)
            border_width: 1.5
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

    Settings = <View> {
        width: Fit, height: Fit
        // FIXME: the extra padding on the right is because the icon is not correctly centered
        // within its parent
        padding: {top: 8, left: 8, right: 12, bottom: 8}
        align: {x: 0.5, y: 0.5}
        <Button> {
            enabled: false
            draw_bg: {
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    return sdf.result
                }
            }
            draw_icon: {
                svg_file: (ICON_SETTINGS),
                fn get_color(self) -> vec4 {
                    return (COLOR_TEXT_IDLE);
                    // return #x566287; // grayed-out #1C274C until enabled
                }
            }
            icon_walk: {width: 25, height: Fit}
        }
    }

    pub SpacesDock = <AdaptiveView> {
        Desktop = {
            flow: Down, spacing: 15
            align: {x: 0.5}
            padding: {top: 40., bottom: 20.}
            width: 68., height: Fill
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY)
            }

            <Profile> {}

            <Separator> {}

            <Home> {}

            <Filler> {}

            <Settings> {}
        }

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

            <Profile> {}

            <Filler> {}

            <Home> {}

            <Filler> {}

            <Settings> {}

            <Filler> {}
        }
    }
}

#[derive(Live, Widget)]
pub struct Profile {
    #[deref]
    view: View,
}

impl Widget for Profile {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let mut color: Vec4 = vec4(0.2, 0.2, 0.2, 1.0); // Default Grey Color
        let profile_rect = {
            let view = self.view(id!(text_view));
            view.area().rect(cx)
        }; // view borrow end

        if let Event::MouseMove(e) = event {

            let (is_mouse_over_icons, verification_text, tooltip_pos) = {
                if let Some(badge) = self
                    .widget(id!(verification_badge))
                    .borrow::<VerificationBadge>()
                {
                    let icons_rect = badge.get_icons_rect(cx);
                    let is_over = icons_rect.contains(e.abs);
                    let text =
                        VerificationText::from_state(badge.verification_state).get_text();
                    color = match badge.verification_state {
                        VerificationState::Verified => vec4(0.0, 0.75, 0.0, 1.0), // Green
                        VerificationState::Unverified => vec4(0.75, 0.0, 0.0, 1.0), // Red
                        VerificationState::Unknown => vec4(0.2, 0.2, 0.2, 1.0),   // Grey
                    };

                    let tooltip_pos = if cx.get_global::<DisplayContext>().is_desktop() {
                        DVec2 {
                            x: icons_rect.pos.x + icons_rect.size.x + 1.,
                            y: icons_rect.pos.y - 10.,
                        }
                    } else {
                        if let Some(tooltip) = self
                            .widget(id!(profile_tooltip))
                            .borrow::<ColorTooltip>()
                        {
                            tooltip.calculate_above_position(cx, profile_rect)
                        } else {
                            DVec2 { x: 0., y: 0. }
                        }
                    };
                    (is_over, text.to_string(), tooltip_pos)
                } else {
                    let tooltip_pos = DVec2 { x: 0., y: 0. };
                    (false, String::new(), tooltip_pos)
                }
            }; // badge borrow end

            if let Some(mut tooltip) = self
                .widget(id!(profile_tooltip))
                .borrow_mut::<ColorTooltip>()
            {
                if is_mouse_over_icons {
                    tooltip.show_with_options(cx, tooltip_pos, &verification_text, color);
                } else {
                    tooltip.hide(cx);
                }
            }
        }

        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for Profile {
    fn handle_action(&mut self, cx: &mut Cx, action: &Action) {
        if let Some(VerificationStateAction::Update(state)) = action.downcast_ref() {
            if let Some(mut badge) = self
                .widget(id!(verification_badge))
                .borrow_mut::<VerificationBadge>()
            {
                if badge.verification_state != *state {
                    badge.verification_state = *state;
                    badge.update_icon_visibility();
                    badge.redraw(cx);
                }
            }
        }
    }
}

impl LiveHook for Profile {
    fn after_new_from_doc(&mut self, cx:&mut Cx) {
        if let Some(client) = get_client() {
            let current_verification_state = client.encryption().verification_state().get();
            if let Some(mut badge) = self
                .widget(id!(verification_badge))
                .borrow_mut::<VerificationBadge>()
            {
                if badge.verification_state != current_verification_state {
                    badge.verification_state = current_verification_state;
                    badge.update_icon_visibility();
                    badge.redraw(cx);
                }
            }
        }
    }
}
