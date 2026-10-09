//! A simple text label that shows a brief timestamp by default
//! and can show additional information (like a complete date) upon hover.

use chrono::{DateTime, Local};
use makepad_widgets::*;


script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.Timestamp = #(Timestamp::register_widget(vm)) {
        width: Fit, height: Fit
        flow: Flow.Right { wrap: false },

        ts_label := Label {
            width: Fit, height: Fit
            flow: Flow.Right { wrap: false },
            padding: 0,
            draw_text +: {
                text_style: TIMESTAMP_TEXT_STYLE {},
                color: (TIMESTAMP_TEXT_COLOR)
            }
        }
    }
}

/// A brief timestamp that shows the complete date on hover.
///
/// See the module-level docs for more detail.
#[derive(Script, ScriptHook, Widget)]
pub struct Timestamp {
    #[deref] view: View,
    #[rust] dt: DateTime<Local>,
    /// If this timestamp represents a time *span*, this is the ending time.
    #[rust] end: Option<DateTime<Local>>,
}

impl Widget for Timestamp {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();
        let should_hover_in = match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverIn(..) => true,
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => true,
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(),  TooltipAction::HoverOut);
                false
            }
            _ => false,
        };
        if should_hover_in {
            // TODO: use pure_rust_locales crate to format the time based on the chosen Locale.
            let locale_extended_fmt_en_us= "%a %b %-d, %Y, %r";
            let start = self.dt.format(locale_extended_fmt_en_us);
            let text = match self.end {
                Some(end) => format!("{start}\nto {}", end.format(locale_extended_fmt_en_us)),
                None => start.to_string(),
            };
            cx.widget_action(
                self.widget_uid(), 
                TooltipAction::HoverIn {
                    text,
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
        let locale_fmt_en_us = "%-I:%M%P";
        self.label(cx, ids!(ts_label)).set_text(
            cx,
            &dt.format(locale_fmt_en_us).to_string()
        );
        self.dt = dt;
        self.end = None;
    }

    /// Sets this timestamp to a span of time from `start` until `end`, if given.
    ///
    /// It still only shows the start time; its hover tooltip shows the end time too.
    pub fn set_date_time_span(&mut self, cx: &mut Cx, start: DateTime<Local>, end: Option<DateTime<Local>>) {
        self.set_date_time(cx, start);
        self.end = end;
    }
}

impl TimestampRef {
    pub fn set_date_time(&self, cx: &mut Cx, dt: DateTime<Local>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_date_time(cx, dt);
        }
    }

    /// See [`Timestamp::set_date_time_span()`].
    pub fn set_date_time_span(&self, cx: &mut Cx, start: DateTime<Local>, end: Option<DateTime<Local>>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_date_time_span(cx, start, end);
        }
    }
}
