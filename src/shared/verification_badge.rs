use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

use crate::{
    shared::styles::{COLOR_FG_ACCEPT_GREEN, COLOR_FG_DANGER_RED},
    sliding_sync::get_client,
    verification::VerificationStateAction,
};


// First, define the verification icons component layout
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VERIFICATION_YES = crate_resource("self:resources/icons/verification_yes.svg")

    mod.widgets.VERIFICATION_NO = crate_resource("self:resources/icons/verification_no.svg")
    mod.widgets.VERIFICATION_UNK = crate_resource("self:resources/icons/verification_unk.svg")

    mod.widgets.VerificationIcon = Icon {

        icon_walk: Walk{ width: 19, margin: 0}
        margin: Inset{left: 0, right: 3, top: 2, bottom: 0}
    }

    mod.widgets.IconYes = View {

        visible: false
        width: Fit, height: Fit
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg_file: (mod.widgets.VERIFICATION_YES),
                get_color: fn() -> vec4 {
                    return (COLOR_FG_ACCEPT_GREEN);
                }
            }
        }
    }

    mod.widgets.IconNo = View {

        visible: false
        width: Fit, height: Fit
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg_file: (mod.widgets.VERIFICATION_NO),
                get_color: fn() -> vec4 {
                    return (COLOR_FG_DANGER_RED);
                }
            }
        }
    }

    mod.widgets.IconUnk = View {

        visible: false
        width: Fit, height: Fit
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg_file: (mod.widgets.VERIFICATION_UNK),
                get_color: fn() -> vec4 {
                    return #x888888;
                }
            }
        }
    }

    mod.widgets.VerificationBadge = #(VerificationBadge::register_widget(vm)) {

        width: Fit, height: Fit
        flow: Overlay
        align: Align{ x: 1.0, y: 0 }

        verification_icons := View {
            flow: Overlay
            align: Align{ x: 1.0, y: 0 }
            width: Fit, height: Fit

            icon_yes := mod.widgets.IconYes {}
            icon_no := mod.widgets.IconNo {}
            icon_unk := mod.widgets.IconUnk {}
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct VerificationBadge {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    verification_state: VerificationState,
}

impl Widget for VerificationBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.verification_state == VerificationState::Unknown {
            if let Some(client) = get_client() {
                self.verification_state = client.encryption().verification_state().get();
                self.update_icon_visibility(cx);
            }
        }

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

        self.view(cx, ids!(icon_yes)).set_visible(cx, yes);
        self.view(cx, ids!(icon_no)).set_visible(cx, no);
        self.view(cx, ids!(icon_unk)).set_visible(cx, unk);
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
            _ => (
                "Verification state is unknown.",
                None,
            ),
        }
    }
}
