//! An indicator that is shown nearby a message that has been edited.
//!
//! This widget is basically just a clickable label that shows the text "(edited)"
//! with an underline to indicate that it is clickable.
//! Upon hover, it shows a tooltip with the date and time when the message was edited.
//!
//! On click, this widget opens a scrollabel modal dialog that shows the full edit history
//! of the message, including all previous content versions and their timestamps.

use chrono::{DateTime, Local};
use makepad_widgets::*;
use matrix_sdk_ui::timeline::EventTimelineItem;

use crate::{shared::callout_tooltip::{CalloutTooltipOptions, TooltipAction, TooltipPosition}, utils::unix_time_millis_to_datetime};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub EDITED_INDICATOR_FONT_SIZE  = 9.5
    pub EDITED_INDICATOR_FONT_COLOR = #666666

    pub EditedIndicator = {{EditedIndicator}} {
        visible: false, // default to hidden
        width: Fit, height: Fit
        flow: Right,
        padding: 0,
        margin: { top: 5 }

        // TODO: re-enable this once we have implemented the edit history modal
        // cursor: Hand,

        edit_html = <Html> {
            width: Fit, height: Fit
            flow: Right, // do not wrap
            padding: 0,
            margin: 0,

            font_size: (EDITED_INDICATOR_FONT_SIZE),
            font_color: (COLOR_ROBRIX_PURPLE),
            body: "(<u>edited</u>)",
        }
    }
}

/// A interactive label that indicates a message has been edited.
#[derive(Live, LiveHook, Widget)]
pub struct EditedIndicator {
    #[deref] view: View,
    #[rust] latest_edit_ts: Option<DateTime<Local>>,
}

impl Widget for EditedIndicator {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();
        let should_hover_in = match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(..) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(..) => true,
            // TODO: show edit history modal on click
            // Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => {
            //     log!("todo: show edit history.");
            //     false
            // },
            Hit::FingerHoverOut(_) => {
                cx.widget_action(self.widget_uid(), &scope.path, TooltipAction::HoverOut);
                false
            }
            _ => false,
        };
        if should_hover_in {
            // TODO: use pure_rust_locales crate to format the time based on the chosen Locale.
            let locale_extended_fmt_en_us= "%a %b %-d, %Y, %r";
            let text = if let Some(ts) = self.latest_edit_ts {
                format!("Last edited {}", ts.format(locale_extended_fmt_en_us))
            } else {
                "Last edit time unknown".to_string()
            };
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                TooltipAction::HoverIn {
                    text,
                    widget_rect: area.rect(cx),
                    options: CalloutTooltipOptions {
                        position: TooltipPosition::Right,
                        ..Default::default()
                    }
                },
            );
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl EditedIndicator {
    /// Sets this indicator to show the timestamp of the latest edit of the given `EventTimelineItem`.
    pub fn set_latest_edit(&mut self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        if let Some(aste) = event_tl_item
            .latest_edit_json()
            .and_then(|json| json.deserialize().ok())
        {
            self.latest_edit_ts = unix_time_millis_to_datetime(aste.origin_server_ts());
        }
        self.visible = true;
        self.redraw(cx);
    }
}

impl EditedIndicatorRef {
    /// See [`EditedIndicator::set_latest_edit()`].
    pub fn set_latest_edit(&self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_latest_edit(cx, event_tl_item);
        }
    }
}


/// Actions emitted by an `EditedIndicator` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum EditedIndicatorAction {
    /// The indicator was clicked, and thus we should open
    /// a modal/dialog showing the message's full edit history.
    ShowEditHistory,
    None,
}
