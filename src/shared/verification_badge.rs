use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

use crate::{
    shared::callout_tooltip::TooltipAction, sliding_sync::get_client,
    verification::VerificationStateAction,
};

use super::styles::{COLOR_ACCEPT_GREEN, COLOR_DANGER_RED, COLOR_DISABLE_GRAY};

// First, define the verification icons component layout
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;
    use crate::shared::styles::*;
    use crate::shared::my_tooltip::*;

    VERIFICATION_YES = dep("crate://self/resources/icons/verification_yes.svg")
    VERIFICATION_NO = dep("crate://self/resources/icons/verification_no.svg")
    VERIFICATION_UNK = dep("crate://self/resources/icons/verification_unk.svg")

    VerificationIcon = <Icon> {
        icon_walk: { width: 23 }
    }

    pub IconYes = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_YES),
                fn get_color(self) -> vec4 {
                    return #x00BF00;
                }
            }
        }
    }

    pub IconNo = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_NO),
                fn get_color(self) -> vec4 {
                    return #xBF0000;
                }
            }
        }
    }

    pub IconUnk = <View> {
        visible: false
        width: 31, height: 31
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_UNK),
                fn get_color(self) -> vec4 {
                    return #x333333;
                }
            }
        }
    }

    pub VerificationBadge = {{VerificationBadge}} {
        width: Fit, height: Fit
        flow: Overlay
        align: { x: 0.5, y: 0.5 }

        verification_icons = <View> {
            flow: Overlay
            align: { x: 0.5, y: 0.5 }
            width: 31, height: 31

            icon_yes = <IconYes> {}
            icon_no = <IconNo> {}
            icon_unk = <IconUnk> {}
        }
    }
}

pub fn verification_state_str(state: VerificationState) -> &'static str {
    match state {
        VerificationState::Verified => "This device is fully verified.",
        VerificationState::Unverified => "This device is unverified. To view your encrypted message history, please verify Robrix from another client.",
        VerificationState::Unknown => " Verification state is unknown.",
    }
}

pub fn verification_state_color(state: VerificationState) -> Vec4 {
    let rgb = match state {
        VerificationState::Verified => COLOR_ACCEPT_GREEN,
        VerificationState::Unverified => COLOR_DANGER_RED,
        VerificationState::Unknown => COLOR_DISABLE_GRAY,
    };
    vec4(rgb.x, rgb.y, rgb.z, 1.0)
}

#[derive(Live, Widget)]
pub struct VerificationBadge {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    verification_state: VerificationState,
}

impl LiveHook for VerificationBadge {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        if let Some(client) = get_client() {
            let current_verification_state = client.encryption().verification_state().get();
            if self.verification_state != current_verification_state {
                self.verification_state = current_verification_state;
                self.update_icon_visibility(cx);
            }
        }
    }
}

impl Widget for VerificationBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(VerificationStateAction::Update(state)) = action.downcast_ref() {
                    if self.verification_state != *state {
                        self.verification_state = *state;
                        self.update_icon_visibility(cx);
                    }
                }
            }
        }

        let badge = self.view(id!(verification_icons));
        let badge_area = badge.area();
        let should_hover_in = match event.hits(cx, badge_area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(..) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(..) => true,
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => true,
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
                false
            }
            _ => false,
        };
        if should_hover_in {
            let badge_rect = badge_area.rect(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                TooltipAction::HoverIn {
                    widget_rect: badge_rect,
                    text: verification_state_str(self.verification_state).to_string(),
                    bg_color: Some(verification_state_color(self.verification_state)),
                    text_color: None,
                },
            );
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl VerificationBadge {
    pub fn update_icon_visibility(&mut self, cx: &mut Cx) {
        let (yes, no, unk) = match self.verification_state {
            VerificationState::Unknown => (false, false, true),
            VerificationState::Unverified => (false, true, false),
            VerificationState::Verified => (true, false, false),
        };

        self.view(id!(icon_yes)).set_visible(cx, yes);
        self.view(id!(icon_no)).set_visible(cx, no);
        self.view(id!(icon_unk)).set_visible(cx, unk);
        self.redraw(cx);
    }
}
