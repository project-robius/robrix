//! A banner above the message composer, shown when a member of this room had their identity reset.
//!
//! Someone else could be posing as that member, so the banner stays until we accept their new identity,
//! or withdraw our verification of them if we had verified them before.

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedUserId;

use crate::sliding_sync::{MatrixRequest, submit_async_request};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.IdentityResetBanner = set_type_default() do #(IdentityResetBanner::register_widget(vm)) {
        ..mod.widgets.RoundedView

        visible: false
        width: Fill,
        height: Fit,
        flow: Down,
        spacing: 10,
        margin: Inset{left: 10, right: 10, top: 6},
        padding: Inset{left: 12.0, top: 10.0, bottom: 10.0, right: 10.0}

        border_color_was_verified: (COLOR_FG_DANGER_RED)
        border_color_was_not_verified: (COLOR_BG_DISABLED)

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0
            border_color: (COLOR_BG_DISABLED)
            border_size: 2.0
        }

        message_label := Label {
            width: Fill,
            height: Fit,
            padding: Inset{ top: 1, left: 1, right: 1, bottom: 0 }
            flow: Flow.Right { wrap: true },
            draw_text +: {
                color: (COLOR_TEXT),
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: ""
        }

        button_row := View {
            width: Fill,
            height: Fit,
            flow: Flow.Right { wrap: true },
            spacing: 10,

            withdraw_verification_button := RobrixNegativeIconButton {
                padding: 10,
                draw_icon.svg: (VERIFICATION_NO)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1}}
                text: "Withdraw Verification"
            }

            dismiss_button := RobrixNeutralIconButton {
                padding: 10,
                draw_icon.svg: (ICON_CHECKMARK)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1}}
                text: "Dismiss"
            }
        }
    }
}

/// A room member whose identity was reset, which we haven't acknowledged yet.
#[derive(Clone, Debug, PartialEq)]
pub struct IdentityReset {
    pub user_id: OwnedUserId,
    pub display_name: Option<String>,
    /// Whether we had verified this member before their identity was reset.
    pub was_verified: bool,
}

#[derive(Script, ScriptHook, Widget)]
pub struct IdentityResetBanner {
    #[deref] view: View,

    #[live] border_color_was_verified: Vec4f,
    #[live] border_color_was_not_verified: Vec4f,

    #[rust] reset: Option<IdentityReset>,
}

impl Widget for IdentityResetBanner {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event
            && let Some(reset) = &self.reset
            && (self.button(cx, ids!(button_row.withdraw_verification_button)).clicked(actions)
                || self.button(cx, ids!(button_row.dismiss_button)).clicked(actions))
        {
            submit_async_request(MatrixRequest::AcknowledgeIdentityReset { user_id: reset.user_id.clone() });
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl IdentityResetBanner {
    fn show_or_hide(&mut self, cx: &mut Cx, reset: Option<IdentityReset>) {
        if self.reset == reset { return }

        let Some(reset) = reset else {
            self.reset = None;
            self.view.set_visible(cx, false);
            self.redraw(cx);
            return;
        };
        let name = match &reset.display_name {
            Some(display_name) => format!("{display_name}'s ({})", reset.user_id),
            None => format!("{}'s", reset.user_id),
        };
        let (text, border_color) = if reset.was_verified {
            (format!("{name} verified identity was reset."), self.border_color_was_verified)
        } else {
            (format!("{name} identity was reset."), self.border_color_was_not_verified)
        };
        self.view.label(cx, ids!(message_label)).set_text(cx, &text);
        self.view.button(cx, ids!(button_row.withdraw_verification_button)).set_visible(cx, reset.was_verified);
        self.view.button(cx, ids!(button_row.dismiss_button)).set_visible(cx, !reset.was_verified);
        self.view.draw_bg.set_dyn_instance(cx, id!(border_color), &[border_color.x, border_color.y, border_color.z, border_color.w]);
        self.reset = Some(reset);
        self.view.set_visible(cx, true);
        self.redraw(cx);
    }
}

impl IdentityResetBannerRef {
    /// See [`IdentityResetBanner::show_or_hide()`].
    pub fn show_or_hide(&self, cx: &mut Cx, reset: Option<IdentityReset>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_or_hide(cx, reset);
    }
}
