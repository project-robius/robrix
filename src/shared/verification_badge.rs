use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

use crate::{
    shared::styles::{COLOR_FG_ACCEPT_GREEN, COLOR_FG_DANGER_RED},
    sliding_sync::get_client,
    verification::VerificationStateAction,
};

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
        icon_walk: { width: 19, margin: 0}
        margin: {left: 0, right: 3, top: 2, bottom: 0}
    }

    pub IconYes = <View> {
        visible: false
        width: Fit, height: Fit
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_YES),
                fn get_color(self) -> vec4 {
                    return (COLOR_FG_ACCEPT_GREEN);
                }
            }
        }
    }

    pub IconNo = <View> {
        visible: false
        width: Fit, height: Fit
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_NO),
                fn get_color(self) -> vec4 {
                    return (COLOR_FG_DANGER_RED);
                }
            }
        }
    }

    pub IconUnk = <View> {
        visible: false
        width: Fit, height: Fit
        <VerificationIcon> {
            draw_icon: {
                svg_file: (VERIFICATION_UNK),
                fn get_color(self) -> vec4 {
                    return #x888888;
                }
            }
        }
    }

    pub VerificationBadge = {{VerificationBadge}} {
        width: Fit, height: Fit
        flow: Overlay
        align: { x: 1.0, y: 0 }

        verification_icons = <View> {
            flow: Overlay
            align: { x: 1.0, y: 0 }
            width: Fit, height: Fit

            icon_yes = <IconYes> {}
            icon_no = <IconNo> {}
            icon_unk = <IconUnk> {}
        }
    }
}

#[derive(Live, Widget)]
pub struct VerificationBadge {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    verification_state: VerificationState,
}

impl LiveHook for VerificationBadge {
    fn after_apply_from_doc(&mut self, cx: &mut Cx) {
        if let Some(client) = get_client() {
            self.verification_state = client.encryption().verification_state().get();
            self.update_icon_visibility(cx);
        }
    }
}

impl Widget for VerificationBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(VerificationStateAction::Update(state)) = action.downcast_ref() {
                    self.verification_state = *state;
                    self.update_icon_visibility(cx);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl VerificationBadge {
    fn update_icon_visibility(&mut self, cx: &mut Cx) {
        let (yes, no, unk) = match self.verification_state {
            VerificationState::Unknown => (false, false, true),
            VerificationState::Unverified => (false, true, false),
            VerificationState::Verified => (true, false, false),
        };

        self.view(ids!(icon_yes)).set_visible(cx, yes);
        self.view(ids!(icon_no)).set_visible(cx, no);
        self.view(ids!(icon_unk)).set_visible(cx, unk);
        self.redraw(cx);
    }
}

impl VerificationBadgeRef {
    /// Returns verification-related string content and background color for a tooltip.
    pub fn tooltip_content(&self) -> (&'static str, Option<Vec4>) {
        match self.borrow().map(|v| v.verification_state) {
            Some(VerificationState::Verified) => (
                "This device is fully verified.",
                Some(COLOR_FG_ACCEPT_GREEN),
            ),
            Some(VerificationState::Unverified) => (
                "This device is unverified. To view your encrypted message history, \
                please verify Robrix from another client.",
                Some(COLOR_FG_DANGER_RED),
            ),
            _ => ("Verification state is unknown.", None),
        }
    }
}
