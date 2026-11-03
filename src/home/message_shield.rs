//! `MessageShield`, a small red or gray shield next to a message whose authenticity has a problem.
//! Hovering over (or long-pressing) it shows a tooltip that explains the problem.

use makepad_widgets::*;
use matrix_sdk_ui::timeline::{EventTimelineItem, TimelineEventShieldState, TimelineEventShieldStateCode};

use crate::{settings::app_preferences::AppPreferencesGlobal, shared::lazy_view::{LazyViewRef, LazyViewWidgetExt}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // This fills the width of the timestamp column above it, which makes it easier to hover over.
    mod.widgets.MessageShield = #(MessageShield::register_widget(vm)) {
        visible: false
        width: Fill, height: Fit
        margin: Inset{top: 2}
        align: Align{x: 0.5}

        tooltip_bg_color_warning: (COLOR_BG_DANGER_RED_SOLID)
        tooltip_bg_color_notice: (COLOR_FG_DISABLED)

        View {
            width: 12, height: 12
            flow: Overlay
            warning_icon := LazyView { template: mod.widgets.IconNo { visible: true } }
            notice_icon := LazyView { template: mod.widgets.IconUnk { visible: true } }
        }
    }
}

#[derive(Script, Widget)]
pub struct MessageShield {
    #[deref] view: View,

    #[live] tooltip_bg_color_warning: Vec4f,
    #[live] tooltip_bg_color_notice: Vec4f,

    #[rust(TimelineEventShieldState::None)] shield: TimelineEventShieldState,
    #[rust] warning_icon: Option<LazyViewRef>,
    #[rust] notice_icon: Option<LazyViewRef>,
}

impl ScriptHook for MessageShield {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        self.warning_icon = None;
        self.notice_icon = None;
    }
}

impl Widget for MessageShield {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();
        let should_hover_in = match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverIn(..) => true,
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), TooltipAction::HoverOut);
                false
            }
            _ => false,
        };
        if !should_hover_in {
            return;
        }
        let (code, bg_color) = match self.shield {
            TimelineEventShieldState::Red { code } => (code, self.tooltip_bg_color_warning),
            TimelineEventShieldState::Grey { code } => (code, self.tooltip_bg_color_notice),
            TimelineEventShieldState::None => return,
        };
        cx.widget_action(
            self.widget_uid(),
            TooltipAction::HoverIn {
                text: shield_reason(code).to_string(),
                widget_rect: area.rect(cx),
                options: CalloutTooltipOptions {
                    bg_color: Some(bg_color),
                    ..Default::default()
                },
            },
        );
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !cx.global::<AppPreferencesGlobal>().0.show_message_shields {
            return DrawStep::done();
        }
        // Showing an icon builds it, so we only do that once we know it'll be drawn.
        self.warning_icon
            .get_or_insert_with(|| self.view.lazy_view(cx, ids!(warning_icon)))
            .set_visible(cx, matches!(self.shield, TimelineEventShieldState::Red { .. }));
        self.notice_icon
            .get_or_insert_with(|| self.view.lazy_view(cx, ids!(notice_icon)))
            .set_visible(cx, matches!(self.shield, TimelineEventShieldState::Grey { .. }));
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MessageShield {
    /// Shows the shield that the given message needs, or hides this widget if it needs none.
    pub fn show_for_message(&mut self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        // A deleted message has no content left (nor any encryption info) to warn about.
        self.shield = if event_tl_item.content().is_redacted() {
            TimelineEventShieldState::None
        } else {
            // Lax (non-strict) mode doesn't warn about users we just haven't verified.
            event_tl_item.get_shield(false)
        };
        self.visible = self.shield != TimelineEventShieldState::None;
        self.redraw(cx);
    }
}

impl MessageShieldRef {
    /// See [`MessageShield::show_for_message()`].
    pub fn show_for_message(&self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_for_message(cx, event_tl_item);
    }
}

/// Returns why a message shows a red or gray shield.
fn shield_reason(code: TimelineEventShieldStateCode) -> &'static str {
    match code {
        TimelineEventShieldStateCode::AuthenticityNotGuaranteed => "The authenticity of this encrypted message can't be guaranteed on this device.",
        TimelineEventShieldStateCode::UnknownDevice => "Encrypted by an unknown or deleted device.",
        TimelineEventShieldStateCode::UnsignedDevice => "Encrypted by a device not verified by its owner.",
        TimelineEventShieldStateCode::UnverifiedIdentity => "Encrypted by an unverified user.",
        TimelineEventShieldStateCode::VerificationViolation => "Encrypted by a previously-verified user whose identity was reset.",
        TimelineEventShieldStateCode::MismatchedSender => "The sender of this message doesn't match the owner of the device that sent it.",
        TimelineEventShieldStateCode::SentInClear => "This message is unencrypted.",
    }
}
