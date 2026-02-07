//! A composite widget for displaying upload progress with a label and cancel button.

use makepad_widgets::*;

use crate::shared::popup_list::{PopupItem, PopupKind, enqueue_popup_notification};
use crate::shared::progress_bar::ProgressBarAction;

/// Actions emitted by the UploadProgressView widget.
#[derive(Clone, Debug, DefaultNone)]
pub enum UploadProgressViewAction {
    /// The cancel button was clicked.
    Cancelled,
    None,
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::progress_bar::ProgressBar;

    // Upload progress view with progress bar, label, and cancel button.
    pub UploadProgressView = {{UploadProgressView}} {
        visible: false,
        width: Fill,
        height: Fit,
        padding: {top: 8, bottom: 8, left: 10, right: 10}
        flow: Down,
        spacing: 5,

        header_row = <View> {
            width: Fill,
            height: Fit,
            flow: Right,
            align: {y: 0.5}
            spacing: 10,

            progress_label = <Label> {
                width: Fill,
                height: Fit,
                draw_text: {
                    text_style: <REGULAR_TEXT>{font_size: 10},
                    color: #666
                }
                text: "Uploading..."
            }

            cancel_upload_button = <Button> {
                width: Fit,
                height: Fit,
                padding: {top: 4, bottom: 4, left: 8, right: 8}
                draw_text: {
                    text_style: <REGULAR_TEXT>{font_size: 9},
                    color: #fff
                }
                draw_bg: {
                    color: #c44
                }
                text: "Cancel"
            }
        }

        progress = <ProgressBar> {}
    }
}

/// A composite widget for displaying upload progress with a label and cancel button.
#[derive(Live, LiveHook, Widget)]
pub struct UploadProgressView {
    #[deref]
    view: View,
    /// AbortHandle for cancelling an in-progress upload
    #[rust] upload_abort_handle: Option<tokio::task::AbortHandle>,
    /// Receiver for getting the upload task's AbortHandle
    #[rust] upload_abort_receiver: Option<crossbeam_channel::Receiver<tokio::task::AbortHandle>>,
}

impl Widget for UploadProgressView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Poll for the upload task's abort handle (for cancellation support)
        if let Some(receiver) = &self.upload_abort_receiver {
            if let Ok(handle) = receiver.try_recv() {
                self.upload_abort_handle = Some(handle);
                self.upload_abort_receiver = None;
            }
        }

        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for UploadProgressView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        // Handle cancel upload button being clicked.
        if self.button(ids!(cancel_upload_button)).clicked(actions) {
            log!("Upload cancelled by user");
            // Abort the upload task if we have a handle
            if let Some(abort_handle) = self.upload_abort_handle.take() {
                abort_handle.abort();
            }
            // Hide the progress bar immediately
            self.hide(cx);
            self.upload_abort_receiver = None;
            enqueue_popup_notification(PopupItem {
                message: String::from("Upload cancelled"),
                kind: PopupKind::Info,
                auto_dismissal_duration: Some(3.0)
            });

            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                UploadProgressViewAction::Cancelled,
            );
        }

        for action in actions {
            if let Some(ProgressBarAction::Update { current, total }) = action.downcast_ref() {
                if current >= total {
                    self.set_visible(cx, false);
                }
                let progress_percentage = if *total > 0 {
                    (*current as f64 / *total as f64) * 100.0
                } else {
                    0.0
                };
                self.view.label(ids!(progress_label)).set_text(cx, &format!("Uploading... {:.0}%", progress_percentage));
            }
        }
    }
}

impl UploadProgressView {
    /// Hides the progress view.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.redraw(cx);
    }

    /// Sets the abort receiver for cancellation support.
    pub fn set_abort_receiver(&mut self, receiver: crossbeam_channel::Receiver<tokio::task::AbortHandle>) {
        self.upload_abort_handle = None;
        self.upload_abort_receiver = Some(receiver);
    }
}

impl UploadProgressViewRef {
    /// Sets the abort receiver for cancellation support.
    pub fn set_abort_receiver(&self, receiver: crossbeam_channel::Receiver<tokio::task::AbortHandle>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_abort_receiver(receiver);
        }
    }
}
