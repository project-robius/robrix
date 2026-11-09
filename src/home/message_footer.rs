//! The reactions, read receipts and send status under a message's content.

use makepad_widgets::*;

use crate::{
    home::{
        event_reaction_list::{ReactionListRef, ReactionListWidgetExt},
        room_read_receipt::{AvatarRowRef, AvatarRowWidgetExt},
        send_status_indicator::{SendStatusIndicatorRef, SendStatusIndicatorWidgetExt},
    },
    shared::html_or_plaintext::{HtmlOrPlaintextRef, HtmlOrPlaintextWidgetExt},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.MessageFooter = #(MessageFooter::register_widget(vm)) {
        width: Fill,
        height: Fit
        flow: Right,
        reaction_list := mod.widgets.ReactionList { }
        avatar_row := mod.widgets.AvatarRow {}
        send_status_indicator := mod.widgets.SendStatusIndicator {}
    }
}

/// The least room we leave between the end of a message's text and its read receipts on that line.
const MIN_GAP_AFTER_TEXT: f64 = 8.0;

/// A message's reactions, read receipts and send status, where the read receipts move up
/// to the end of the text's last line when they're all we show and they fit there.
#[derive(Script, Widget)]
pub struct MessageFooter {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    /// The message text right above us.
    #[rust] text_above: Option<HtmlOrPlaintextRef>,
    #[rust] child_refs: Option<MessageFooterChildRefs>,
}

struct MessageFooterChildRefs {
    reaction_list: ReactionListRef,
    avatar_row: AvatarRowRef,
    send_status_indicator: SendStatusIndicatorRef,
}

impl ScriptHook for MessageFooter {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        self.child_refs = None;
    }
}

impl Widget for MessageFooter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, mut walk: Walk) -> DrawStep {
        if let Some((shift_up, height)) = self.get_place_on_last_line(cx) {
            // We sit on a line the text already has, so we don't take up any room of our own.
            walk.margin.top -= shift_up;
            walk.margin.bottom += shift_up - height;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MessageFooter {
    /// Returns how far up we move to put our read receipts at the end of the text's last line,
    /// and how tall we are there, or `None` if they stay on their own line under it.
    fn get_place_on_last_line(&mut self, cx: &mut Cx2d) -> Option<(f64, f64)> {
        let text_above = self.text_above.clone()?;
        let child_refs = self.child_refs.get_or_insert_with(|| MessageFooterChildRefs {
            reaction_list: self.view.reaction_list(cx, ids!(reaction_list)),
            avatar_row: self.view.avatar_row(cx, ids!(avatar_row)),
            send_status_indicator: self.view.send_status_indicator(cx, ids!(send_status_indicator)),
        });
        if !child_refs.reaction_list.is_empty() || child_refs.send_status_indicator.is_shown() {
            return None;
        }
        let receipts_size = child_refs.avatar_row.get_one_line_size(cx)?;
        let receipts_margin_top = child_refs.avatar_row.walk(cx).margin.top;
        // The text has to be right above us, not above something like a link preview.
        let top = cx.turtle().pos().y;
        let text_rect = text_above.area().rect(cx);
        if (text_rect.pos.y + text_rect.size.y - top).abs() > 0.5 {
            return None;
        }
        let free_space = text_above.last_line_free_space(cx)?;
        if free_space.size.x < receipts_size.x + MIN_GAP_AFTER_TEXT {
            return None;
        }
        let receipts_top = free_space.pos.y + (free_space.size.y - receipts_size.y) / 2.0;
        Some((top - (receipts_top - receipts_margin_top), receipts_margin_top + receipts_size.y))
    }
}

/// Finds the footer in the given message view and tells it about the message text right above it.
pub fn link_footer_to_text(cx: &mut Cx, message_view: &View) -> MessageFooterRef {
    let footer = message_view.message_footer(cx, ids!(content.footer));
    if let Some(mut inner) = footer.borrow_mut() {
        inner.text_above = Some(message_view.html_or_plaintext(cx, ids!(content.message)));
    }
    footer
}
