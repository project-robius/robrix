//! A banner above the message composer, shown while a failed message
//! is blocking everything queued behind it in this room.

use std::cell::RefCell;
use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk_ui::timeline::TimelineEventItemId;

use crate::app::ConfirmDeleteAction;
use crate::home::send_status_indicator::{is_send_error_retryable, stringify_send_error};
use crate::shared::confirmation_modal::ConfirmationModalContent;
use crate::sliding_sync::{MatrixRequest, TimelineKind, submit_async_request};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.FailedSendBanner = set_type_default() do #(FailedSendBanner::register_widget(vm)) {
        ..mod.widgets.RoundedView

        visible: false
        width: Fill,
        height: Fit,
        flow: Down,
        spacing: 10,
        margin: Inset{left: 4, right: 4, bottom: 4},
        padding: Inset{left: 12.0, top: 10.0, bottom: 10.0, right: 10.0}

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0
            border_color: #E34B4F
            border_size: 2.0
        }

        reason_label := Label {
            width: Fill,
            height: Fit,
            padding: Inset{ top: 1, left: 1, right: 1, bottom: 0 }
            flow: Flow.Right { wrap: true },
            max_lines: 2,
            draw_text +: {
                color: (COLOR_FG_DANGER_RED),
                text_style: REGULAR_TEXT {font_size: 10}
            }
            text: ""
        }

        button_row := View {
            width: Fill,
            height: Fit,
            flow: Flow.Right { wrap: true },
            spacing: 10,

            retry_button := RobrixIconButton {
                padding: 10,
                draw_icon.svg: (ICON_SEND)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1}}
                text: "Retry"
            }

            cancel_send_button := RobrixNegativeIconButton {
                padding: 10,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -1, right: -1, top: -1}}
                text: "Cancel Send"
            }
        }
    }
}

/// The message that failed to send and is holding up this room's send queue.
#[derive(Clone)]
pub struct BlockedSend {
    pub timeline_kind: TimelineKind,
    pub timeline_event_id: TimelineEventItemId,
    pub error: Arc<matrix_sdk::Error>,
}

#[derive(Script, ScriptHook, Widget)]
pub struct FailedSendBanner {
    #[deref] view: View,
    #[rust] blocked: Option<BlockedSend>,
}

impl Widget for FailedSendBanner {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event
            && let Some(blocked) = &self.blocked
        {
            if self.button(cx, ids!(button_row.retry_button)).clicked(actions) {
                submit_async_request(MatrixRequest::RetrySend {
                    timeline_kind: blocked.timeline_kind.clone(),
                    timeline_event_id: blocked.timeline_event_id.clone(),
                });
            }
            // Cancelling throws away what the user wrote, so make them confirm it.
            if self.button(cx, ids!(button_row.cancel_send_button)).clicked(actions) {
                let timeline_kind = blocked.timeline_kind.clone();
                let timeline_event_id = blocked.timeline_event_id.clone();
                let content = ConfirmationModalContent {
                    title_text: "Cancel sending this message?".into(),
                    body_text: "It won't be sent, and what you wrote will be discarded.".into(),
                    accept_button_text: Some("Cancel Send".into()),
                    cancel_button_text: Some("Keep It".into()),
                    on_accept_clicked: Some(Box::new(move |_cx| {
                        submit_async_request(MatrixRequest::RedactMessage {
                            timeline_kind,
                            timeline_event_id,
                            reason: None,
                        });
                    })),
                    ..Default::default()
                };
                cx.action(ConfirmDeleteAction::Show(RefCell::new(Some(content))));
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl FailedSendBanner {
    fn show_or_hide(&mut self, cx: &mut Cx, blocked: Option<BlockedSend>) {
        // This runs on every timeline update, so don't touch the UI unless something changed.
        let is_same = match (&self.blocked, &blocked) {
            (None, None) => true,
            (Some(shown), Some(new)) => shown.timeline_event_id == new.timeline_event_id
                && Arc::ptr_eq(&shown.error, &new.error),
            _ => false,
        };
        if is_same { return }

        let Some(blocked) = blocked else {
            self.blocked = None;
            self.view.set_visible(cx, false);
            self.redraw(cx);
            return;
        };
        self.view.label(cx, ids!(reason_label)).set_text(
            cx,
            &format!("Couldn't send an earlier message: {}", stringify_send_error(&blocked.error)),
        );
        self.view.button(cx, ids!(button_row.retry_button))
            .set_visible(cx, is_send_error_retryable(&blocked.error));
        self.blocked = Some(blocked);
        self.view.set_visible(cx, true);
        self.redraw(cx);
    }
}

impl FailedSendBannerRef {
    /// See [`FailedSendBanner::show_or_hide()`].
    pub fn show_or_hide(&self, cx: &mut Cx, blocked: Option<BlockedSend>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.show_or_hide(cx, blocked);
    }
}
