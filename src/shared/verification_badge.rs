use makepad_widgets::*;
use matrix_sdk::encryption::VerificationState;

use crate::{
    profile::user_verification_cache::UserVerificationState,
    shared::lazy_view::LazyViewWidgetExt,
    sliding_sync::get_client,
    verification::VerificationStateAction,
};


// First, define the verification icons component layout
script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.VERIFICATION_YES = crate_resource("self://resources/icons/verification_yes.svg")
    mod.widgets.VERIFICATION_YES_BG = crate_resource("self://resources/icons/verification_yes_bg.svg")

    mod.widgets.VERIFICATION_NO = crate_resource("self://resources/icons/verification_no.svg")
    mod.widgets.VERIFICATION_NO_BG = crate_resource("self://resources/icons/verification_no_bg.svg")

    mod.widgets.VERIFICATION_UNK = crate_resource("self://resources/icons/verification_unk.svg")
    mod.widgets.VERIFICATION_UNK_BG = crate_resource("self://resources/icons/verification_unk_bg.svg")

    mod.widgets.VerificationIcon = Icon {
        width: Fill, height: Fill
        icon_walk: Walk {
            width: Fill,
            height: Fill,
            margin: 0,
        }
        margin: 0
    }

    // Each verification icon is a colored shield with the inner glyph carved
    // out (transparent). Stacked behind it is a white-tinted shield-only
    // version of the same SVG so the avatar doesn't show through the carved
    // glyph area — the white shows through instead.
    mod.widgets.IconYes = View {

        visible: false
        width: Fill, height: Fill
        clip_x: false, clip_y: false
        flow: Overlay
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_YES_BG),
                color: (COLOR_TEXT_ON_ACCENT),
            }
        }
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_YES),
                color: (COLOR_BG_ACCEPT_GREEN_SOLID),
            }
        }
    }

    mod.widgets.IconNo = View {

        visible: false
        width: Fill, height: Fill
        clip_x: false, clip_y: false
        flow: Overlay
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_NO_BG),
                color: (COLOR_TEXT_ON_ACCENT),
            }
        }
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_NO),
                color: (COLOR_BG_DANGER_RED_SOLID),
            }
        }
    }

    mod.widgets.IconUnk = View {

        visible: false
        width: Fill, height: Fill
        clip_x: false, clip_y: false
        flow: Overlay
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_UNK_BG),
                color: (COLOR_TEXT_ON_ACCENT),
            }
        }
        mod.widgets.VerificationIcon {
            draw_icon +: {
                svg: (mod.widgets.VERIFICATION_UNK),
                color: (COLOR_VERIFICATION_BADGE_UNKNOWN),
            }
        }
    }

    // Shows whether we've verified another user, or that their identity was reset.
    mod.widgets.UserVerificationBadge = #(UserVerificationBadge::register_widget(vm)) {
        visible: false
        width: 15, height: 15
        flow: Overlay
        clip_x: false, clip_y: false
        verified_icon := LazyView { template: mod.widgets.IconYes { visible: true } }
        identity_reset_icon := LazyView { template: mod.widgets.IconNo { visible: true } }
    }

    mod.widgets.VerificationBadge = #(VerificationBadge::register_widget(vm)) {

        width: Fit, height: Fit
        flow: Overlay
        align: Align{ x: 1.0, y: 0 }
        tooltip_bg_verified: (COLOR_BG_ACCEPT_GREEN_SOLID)
        tooltip_bg_unverified: (COLOR_BG_DANGER_RED_SOLID)

        // The badge's size is on this view rather than on its icons,
        // so that the mobile nav bar can shrink it into its floating pill.
        verification_icons := View {
            flow: Overlay
            align: Align{ x: 1.0, y: 0 }
            width: (NAVIGATION_TAB_BAR_SIZE * 0.28 + 3.0), height: (NAVIGATION_TAB_BAR_SIZE * 0.28 + 2.0)
            padding: Inset{right: 3, top: 2}

            icon_yes := mod.widgets.IconYes {}
            icon_no := mod.widgets.IconNo {}
            icon_unk := mod.widgets.IconUnk {}
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct UserVerificationBadge {
    #[deref] view: View,
}

impl Widget for UserVerificationBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl UserVerificationBadgeRef {
    /// Shows the icon for the given user's verification state, or hides this badge if there's nothing to show.
    pub fn show_state(&self, cx: &mut Cx, state: Option<UserVerificationState>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let is_verified = state == Some(UserVerificationState::Verified);
        let is_identity_reset = state == Some(UserVerificationState::VerificationViolation);
        inner.view.lazy_view(cx, ids!(verified_icon)).set_visible(cx, is_verified);
        inner.view.lazy_view(cx, ids!(identity_reset_icon)).set_visible(cx, is_identity_reset);
        inner.view.set_visible(cx, is_verified || is_identity_reset);
    }
}

#[derive(Script, Widget)]
pub struct VerificationBadge {
    #[deref]
    view: View,
    #[rust(VerificationState::Unknown)]
    verification_state: VerificationState,
    #[live]
    tooltip_bg_verified: Vec4,
    #[live]
    tooltip_bg_unverified: Vec4,
}

impl ScriptHook for VerificationBadge {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        _apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        vm.with_cx_mut(|cx| {
            if let Some(client) = get_client() {
                self.verification_state = client.encryption().verification_state().get();
                self.update_icon_visibility(cx);
            }
        });
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

        self.view(cx, ids!(icon_yes)).set_visible(cx, yes);
        self.view(cx, ids!(icon_no)).set_visible(cx, no);
        self.view(cx, ids!(icon_unk)).set_visible(cx, unk);
        self.redraw(cx);
    }
}

impl VerificationBadgeRef {
    /// Returns verification-related string content and background color for a tooltip.
    pub fn tooltip_content(&self) -> (&'static str, Option<Vec4>) {
        let Some(inner) = self.borrow() else { return ("", None) };
        match inner.verification_state {
            VerificationState::Verified => (
                "and verified ",
                Some(inner.tooltip_bg_verified),
            ),
            VerificationState::Unverified => (
                "but not verified ",
                Some(inner.tooltip_bg_unverified),
            ),
            _ => (
                "",
                None,
            ),
        }
    }
}
