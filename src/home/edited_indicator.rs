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

use crate::{shared::callout_tooltip::TooltipAction, utils::unix_time_millis_to_datetime};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;

    pub EDITED_INDICATOR_FONT_SIZE  = 9.5
    pub EDITED_INDICATOR_FONT_COLOR = #666666

    pub EditedIndicator = {{EditedIndicator}} {
        width: Fit, height: Fit
        flow: Right,
        padding: 0,
        margin: 0,

        // visible: false, // default to hidden
        my_label = <Label> {
            width: Fit, height: Fit
            flow: Right, // do not wrap
            padding: 0,
            margin: 0,
            draw_text: {
                text_style: <TIMESTAMP_TEXT_STYLE> { font_size: 10 },
                // color: #x0,
                color: (COLOR_ROBRIX_PURPLE),
            }
            text = "(edited)",
        }
        // edit_html = <Html> {
        //     width: Fit, height: Fit
        //     flow: Right, // do not wrap
        //     padding: 0,

        //     font_size: (EDITED_INDICATOR_FONT_SIZE),
        //     font_color: (EDITED_INDICATOR_FONT_COLOR),
        //     draw_normal: { color: (EDITED_INDICATOR_FONT_COLOR) },
        //     draw_block: {
        //         line_color: (MESSAGE_TEXT_COLOR)
        //     }
        //     body = "<u>(edited)</u>",
        // }
    }
}

/// A interactive label that indicates a message has been edited.
#[derive(Live, LiveHook, Widget)]
pub struct EditedIndicator {
    #[deref] view: View,

    #[rust] latest_edit_ts: DateTime<Local>,
}

impl Widget for EditedIndicator {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let area = self.view.area();
        let should_hover_in = match event.hits(cx, area) {
            Hit::FingerLongPress(_)
            | Hit::FingerHoverOver(..) // TODO: remove once CalloutTooltip bug is fixed
            | Hit::FingerHoverIn(..) => true,
            Hit::FingerUp(fue) if fue.is_over && fue.is_primary_hit() => {
                log!("todo: show edit history.");
                false
            },
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
                    widget_rect: area.rect(cx),
                    text: format!("Last edited {}", self.latest_edit_ts.format(locale_extended_fmt_en_us)),
                    bg_color: None,
                    text_color: None,
                },
            );
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl EditedIndicator {
    pub fn set_latest_edit(&mut self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        log!("set_latest_edit called for EditedIndicator, event ID: {:?}", event_tl_item.event_id());
        if let Some(aste) = event_tl_item
            .latest_edit_json()
            .and_then(|json| json.deserialize().ok())
        {
            log!("Latest edit found: {:?}", aste);
            if let Some(ts) = unix_time_millis_to_datetime(aste.origin_server_ts()) {
                log!("Setting latest edit timestamp to: {:?}", ts);
                self.latest_edit_ts = ts;
            }
        }
        self.label(id!(my_label)).set_text(cx, "(edited)");
        self.visible = true;
        self.redraw(cx);
    }
}

impl EditedIndicatorRef {
    pub fn set_latest_edit(&self, cx: &mut Cx, event_tl_item: &EventTimelineItem) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_latest_edit(cx, event_tl_item);
        }
    }
}


/// Actions emitted by an `EditedIndicator` widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum EditedIndicatorAction {
    ShowEditHistory,
    None,
}
