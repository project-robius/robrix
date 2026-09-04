//! A widget that displays upload progress with a progress bar, status label,
//! and cancel/retry buttons.

use makepad_widgets::*;
use futures_util::future::AbortHandle;

use matrix_sdk::ruma::OwnedTransactionId;
use matrix_sdk_ui::timeline::TimelineEventItemId;

use crate::shared::file_upload_modal::{AttachmentUpload, FileUploadAttemptId, submit_attachment_upload};
use crate::sliding_sync::{MatrixRequest, TimelineKind, submit_async_request};
use crate::shared::progress_bar::ProgressBarWidgetRefExt;
use crate::shared::styles::COLOR_FG_DANGER_RED;
use crate::home::send_status_indicator::upload_progress_text;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.UploadProgressView = set_type_default() do #(UploadProgressView::register_widget(vm)) {
        visible: false,
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset { top: 10, bottom: 10, left: 15, right: 15 }
        spacing: 15,

        show_bg: true,
        draw_bg +: {
            color: (COLOR_BG_PREVIEW)
            border_radius: 4.0
        }

        // Header with file name and cancel button
        header := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 0.0, y: 0.5},
            spacing: 10,

            file_name_label := Label {
                width: Fill,
                flow: Flow.Right {wrap: true}
                padding: 0,
                margin: Inset { left: 1 }
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT)
                }
                text: ""
            }

            cancel_button := RobrixNegativeIconButton {
                width: Fit,
                align: Align{x: 0.5, y: 0.5}
                padding: 13,
                draw_icon.svg: (ICON_FORBIDDEN)
                icon_walk: Walk{width: 16, height: 16, margin: Inset{left: -2, right: -1} }
                text: "Cancel"
            }
        }

        progress_bar := ProgressBar { }

        status_view := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 0.0, y: 0.5},
            spacing: 10,

            status_label := Label {
                width: Fill,
                flow: Flow.Right {wrap: true}
                padding: 0,
                margin: Inset { left: 1 }
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 11 },
                    color: (COLOR_TEXT)
                }
                text: ""
            }

            retry_button := RobrixIconButton {
                visible: false,
                padding: 13
                text: "Retry"
            }
        }
    }
}

/// The current state of the upload view.
#[derive(Clone, Debug, Default)]
#[allow(clippy::large_enum_variant)]
pub enum UploadViewState {
    /// Normal state - upload in progress or ready.
    #[default]
    Normal,
    /// Error state - upload failed.
    Error {
        upload: Option<AttachmentUpload>,
    },
}

/// How the cancel button stops the upload, which differs once it reaches the send queue.
enum UploadCancel {
    /// Still reading the file, so the task can just be aborted.
    Reading(AbortHandle),
    /// Queued, so cancelling means discarding the local echo.
    Queued(OwnedTransactionId),
}

/// A widget showing upload progress with cancel/retry functionality.
#[derive(Script, ScriptHook, Widget)]
pub struct UploadProgressView {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// How to stop the upload this view is showing.
    #[rust] cancel: Option<UploadCancel>,
    /// The room this upload belongs to, so another room's input bar doesn't show it.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// The upload attempt currently represented by this view.
    #[rust] upload_id: Option<FileUploadAttemptId>,
    /// Current state of the upload view.
    #[rust] state: UploadViewState,
}

impl Widget for UploadProgressView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // Handle cancel button
            if self.button(cx, ids!(cancel_button)).clicked(actions) {
                log!("Upload cancel requested for {:?}.", self.upload_id);
                match self.cancel.take() {
                    Some(UploadCancel::Reading(handle)) => handle.abort(),
                    Some(UploadCancel::Queued(transaction_id)) => {
                        if let Some(timeline_kind) = self.timeline_kind.clone() {
                            submit_async_request(MatrixRequest::RedactMessage {
                                timeline_kind,
                                timeline_event_id: TimelineEventItemId::TransactionId(transaction_id),
                                reason: None,
                            });
                        }
                    }
                    None => { }
                }
                self.hide_current(cx);
            }

            // Handle retry button
            if self.button(cx, ids!(retry_button)).clicked(actions) {
                if let UploadViewState::Error { upload } = &mut self.state
                    && let Some(upload) = upload.take()
                {
                    self.hide_current(cx);
                    submit_attachment_upload(upload);
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl UploadProgressView {
    /// Shows the upload progress view with the given file name.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        file_name: &str,
        abort_handle: AbortHandle,
        timeline_kind: TimelineKind,
    ) {
        self.set_visible(cx, true);
        self.upload_id = Some(upload_id);
        self.timeline_kind = Some(timeline_kind);
        self.cancel = Some(UploadCancel::Reading(abort_handle));
        self.state = UploadViewState::Normal;

        self.label(cx, ids!(file_name_label)).set_text(cx, &format!("Sending:  {file_name}"));
        self.label(cx, ids!(status_label)).set_text(cx, "Preparing upload...");
        self.reset_status_label_color(cx);
        let retry_button = self.button(cx, ids!(retry_button));
        retry_button.set_visible(cx, false);
        retry_button.reset_hover(cx);
        let cancel_button = self.button(cx, ids!(cancel_button));
        cancel_button.set_visible(cx, true);
        cancel_button.reset_hover(cx);

        self.reset_progress_bar(cx);

        self.redraw(cx);
    }

    /// The file has been read and handed to the send queue, so the cancel button
    /// now discards the local echo instead of aborting the read.
    pub fn set_queuing(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    ) {
        if self.upload_id != Some(upload_id) {
            return;
        }
        self.cancel = Some(UploadCancel::Queued(transaction_id));
        // Nothing's uploading yet: the queue reads the file back out of the media
        // store, then encrypts all of it if the room is encrypted.
        self.label(cx, ids!(status_label)).set_text(cx, if is_encrypted {
            "Encrypting..."
        } else {
            "Starting upload..."
        });
        self.redraw(cx);
    }

    /// Hides the view while a different room is being shown, since the upload
    /// belongs to the room it was started in.
    pub fn hide_if_other_room(&mut self, cx: &mut Cx, timeline_kind: &TimelineKind) {
        if self.timeline_kind.as_ref().is_some_and(|kind| kind != timeline_kind) {
            self.set_visible(cx, false);
            self.redraw(cx);
        }
    }

    /// Hides the upload progress view if it belongs to the given upload attempt.
    pub fn hide(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        if self.upload_id == Some(upload_id) {
            self.hide_current(cx);
        }
    }

    fn hide_current(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.upload_id = None;
        self.timeline_kind = None;
        self.cancel = None;
        self.state = UploadViewState::Normal;
        self.button(cx, ids!(retry_button)).set_visible(cx, false);
        self.reset_status_label_color(cx);
        self.reset_progress_bar(cx);
        self.redraw(cx);
    }

    /// Updates the progress bar if it belongs to the given upload attempt.
    pub fn set_progress(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId, current: usize, total: usize) {
        if self.upload_id != Some(upload_id) || total == 0 {
            return;
        }
        if let UploadViewState::Error { .. } = self.state {
            return;
        }
        let fraction = (current as f32 / total as f32).clamp(0.0, 1.0);
        self.child_by_path(ids!(progress_bar)).as_progress_bar().set_progress(cx, fraction);
        self.label(cx, ids!(status_label)).set_text(cx, &upload_progress_text(current, total));
        self.reset_status_label_color(cx);
        self.redraw(cx);
    }

    /// Shows an error state with the given message if it belongs to the given upload attempt.
    pub fn show_error(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId, error: &str, upload: AttachmentUpload, retryable: bool) {
        if self.upload_id != Some(upload_id) {
            return;
        }
        self.cancel = None;
        self.state = UploadViewState::Error {
            upload: retryable.then_some(upload),
        };
        self.button(cx, ids!(cancel_button)).set_visible(cx, true);

        // Update UI for error state
        self.label(cx, ids!(status_label))
            .set_text(cx, &format!("Error: {}", error));
        let retry_button = self.button(cx, ids!(retry_button));
        retry_button.set_visible(cx, retryable);
        if retryable {
            retry_button.reset_hover(cx);
        }

        self.set_status_label_color(cx, COLOR_FG_DANGER_RED);
        let progress_bar = self.child_by_path(ids!(progress_bar)).as_progress_bar();
        progress_bar.set_progress_color(cx, COLOR_FG_DANGER_RED);
        progress_bar.set_progress(cx, 1.0);

        self.redraw(cx);
    }

    fn reset_progress_bar(&mut self, cx: &mut Cx) {
        self.child_by_path(ids!(progress_bar)).as_progress_bar().reset_progress_color(cx);
        self.child_by_path(ids!(progress_bar)).as_progress_bar().set_progress(cx, 0.0);
    }

    fn set_status_label_color(&mut self, cx: &mut Cx, color: Vec4) {
        let mut status_label = self.label(cx, ids!(status_label));
        script_apply_eval!(cx, status_label, {
            draw_text +: { color: #(color) }
        });
    }

    fn reset_status_label_color(&mut self, cx: &mut Cx) {
        let mut status_label = self.label(cx, ids!(status_label));
        script_apply_eval!(cx, status_label, {
            draw_text +: { color: mod.widgets.COLOR_TEXT }
        });
    }
}

impl UploadProgressViewRef {
    /// Shows the upload progress view with the given file name.
    pub fn show(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        file_name: &str,
        abort_handle: AbortHandle,
        timeline_kind: TimelineKind,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, upload_id, file_name, abort_handle, timeline_kind);
        }
    }

    /// Hides the upload progress view if it belongs to the given upload attempt.
    pub fn hide(&self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx, upload_id);
        }
    }

    /// See [`UploadProgressView::set_queuing()`].
    pub fn set_queuing(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_queuing(cx, upload_id, transaction_id, is_encrypted);
        }
    }

    /// See [`UploadProgressView::hide_if_other_room()`].
    pub fn hide_if_other_room(&self, cx: &mut Cx, timeline_kind: &TimelineKind) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide_if_other_room(cx, timeline_kind);
        }
    }

    /// See [`UploadProgressView::set_progress()`].
    pub fn set_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, current: usize, total: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, upload_id, current, total);
        }
    }

    /// Shows an error state with the given message if it belongs to the given upload attempt.
    pub fn show_error(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, error: &str, upload: AttachmentUpload, retryable: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, upload_id, error, upload, retryable);
        }
    }
}
