//! A simple text label that shows a brief timestamp by default
//! and can show additional information (like a complete date) upon hover.

use chrono::{DateTime, Local};
use makepad_widgets::*;

use crate::shared::callout_tooltip::{CalloutTooltipOptions, TooltipPosition};

use super::callout_tooltip::TooltipAction;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub Timestamp = {{Timestamp}} {
        width: Fit, height: Fit
        flow: Right,

        ts_label = <Label> {
            width: Fit, height: Fit
            flow: Right, // do not wrap
            padding: 0,
            draw_text: {
                text_style: <TIMESTAMP_TEXT_STYLE> {},
                color: (TIMESTAMP_TEXT_COLOR)
            }
        }
    }
}

/// A text input (with a search icon and cancel button) used to filter the rooms list.
///
/// See the module-level docs for more detail.
#[derive(Live, LiveHook, Widget)]
pub struct Timestamp {
    #[deref] view: View,

    #[rust] dt: DateTime<Local>,
}

impl Widget for Timestamp {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();
        let should_hover_in = match event.hits(cx, area) {
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
            // TODO: use pure_rust_locales crate to format the time based on the chosen Locale.
            let locale_extended_fmt_en_us= "%a %b %-d, %Y, %r";
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                TooltipAction::HoverIn {
                    text: self.dt.format(locale_extended_fmt_en_us).to_string(),
                    widget_rect: area.rect(cx),
                    options: CalloutTooltipOptions {
                        position: TooltipPosition::Right,
                        ..Default::default()
                    },
                },
            );
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl Timestamp {
    pub fn set_date_time(&mut self, cx: &mut Cx, dt: DateTime<Local>) {
        // TODO: use pure_rust_locales crate to format the time based on the chosen Locale.
        let locale_fmt_en_us = "%-I:%M %P";
        self.label(ids!(ts_label)).set_text(
            cx,
            &dt.format(locale_fmt_en_us).to_string()
        );
        self.dt = dt;
    }
}

impl TimestampRef {
    pub fn set_date_time(&self, cx: &mut Cx, dt: DateTime<Local>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_date_time(cx, dt);
        }
    }
}
