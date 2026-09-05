//! A widget that displays upload progress with a progress bar, status label,
//! and cancel/retry buttons.

use makepad_widgets::*;
use futures_util::future::AbortHandle;
use matrix_sdk::ruma::OwnedTransactionId;
use matrix_sdk_ui::timeline::TimelineEventItemId;

use crate::shared::file_upload_modal::{AttachmentUpload, FileUploadAttemptId, submit_attachment_upload};
use crate::sliding_sync::{MatrixRequest, TimelineKind, submit_async_request};
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};
use crate::shared::progress_bar::ProgressBarWidgetRefExt;
use crate::shared::styles::{COLOR_FG_DANGER_RED, COLOR_TEXT};
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

/// The state of an in-progress upload within a timeline (RoomScreen).
///
/// * While that timeline is being shown, it's owned by the `UploadProgressView` widget.
/// * While that timeline is hidden, it's owned by that timeline's `RoomInputBarState`.
pub struct UploadState {
    upload_id: FileUploadAttemptId,
    file_name: String,
    /// The timeline that the upload is being sent to.
    timeline_kind: TimelineKind,
    phase: UploadPhase,
}

#[allow(clippy::large_enum_variant)]
enum UploadPhase {
    /// Still reading the file, so cancelling just aborts the task.
    Reading(AbortHandle),
    /// The upload has been enqueued on the send queue,
    /// so cancelling it means discarding the local echo.
    Queued {
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    },
    /// The upload is currently in progress.
    Uploading {
        transaction_id: OwnedTransactionId,
        current_bytes: usize,
        total_bytes: usize,
    },
    /// There was an error, and it may be possible to retry the upload.
    Failed {
        error: String,
        retry: Option<AttachmentUpload>,
    },
}

/// A widget showing upload progress with cancel/retry functionality.
#[derive(Script, Widget)]
pub struct UploadProgressView {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    #[rust] state: Option<UploadState>,
}

impl ScriptHook for UploadProgressView {
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| self.populate(cx));
    }
}

impl Widget for UploadProgressView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event && self.state.is_some() {
            if self.button(cx, ids!(cancel_button)).clicked(actions)
                && let Some(upload) = self.state.take()
            {
                match upload.phase {
                    UploadPhase::Reading(abort_handle) => abort_handle.abort(),
                    UploadPhase::Queued { transaction_id, .. }
                    | UploadPhase::Uploading { transaction_id, .. } => {
                        submit_async_request(MatrixRequest::RedactMessage {
                            timeline_kind: upload.timeline_kind,
                            timeline_event_id: TimelineEventItemId::TransactionId(transaction_id),
                            reason: None,
                        });
                    }
                    // It never reached the queue, so there's nothing to stop.
                    UploadPhase::Failed { .. } => {}
                }
                self.populate(cx);
            }
            if self.button(cx, ids!(retry_button)).clicked(actions)
                && let Some(UploadState { phase: UploadPhase::Failed { retry, .. }, .. }) = &mut self.state
                && let Some(attachment) = retry.take()
            {
                self.state = None;
                self.populate(cx);
                submit_attachment_upload(attachment);
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl UploadProgressView {
    /// Shows the progress view for a new upload, which is still being read from disk.
    pub fn show(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        file_name: &str,
        abort_handle: AbortHandle,
        timeline_kind: TimelineKind,
    ) {
        self.state = Some(UploadState {
            upload_id,
            file_name: file_name.to_owned(),
            timeline_kind,
            phase: UploadPhase::Reading(abort_handle),
        });
        self.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.button(cx, ids!(retry_button)).reset_hover(cx);
        self.populate(cx);
    }

    /// The file was handed to the send queue, so cancelling now discards the local echo.
    pub fn set_as_queued(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    ) {
        if let Some(upload) = self.state.as_mut().filter(|u| u.upload_id == upload_id) {
            upload.phase = UploadPhase::Queued { transaction_id, is_encrypted };
            self.populate(cx);
        }
    }

    pub fn set_progress(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        current_bytes: usize,
        total_bytes: usize
    ) {
        if total_bytes > 0
            && let Some(upload) = self.state.as_mut().filter(|u| u.upload_id == upload_id)
            && let UploadPhase::Queued { transaction_id, .. }
                | UploadPhase::Uploading { transaction_id, .. } = &upload.phase
        {
            let transaction_id = transaction_id.clone();
            upload.phase = UploadPhase::Uploading { transaction_id, current_bytes, total_bytes };
            self.populate(cx);
        }
    }

    /// The upload failed before it got to the send queue.
    ///
    /// If `retryable_upload` is `Some`, we'll show a retry button that re-submits the upload.
    pub fn show_error(
        &mut self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        error: &str,
        retryable_upload: Option<AttachmentUpload>,
    ) {
        if let Some(state) = self.state.as_mut().filter(|u| u.upload_id == upload_id) {
            state.phase = UploadPhase::Failed {
                error: error.to_owned(),
                retry: retryable_upload,
            };
            self.populate(cx);
        }
    }

    /// The upload is done (however it ended) and the message's own indicator takes over.
    pub fn hide(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        if self.state.as_ref().is_some_and(|u| u.upload_id == upload_id) {
            self.state = None;
            self.populate(cx);
        }
    }

    /// Called when this room's/thread's timeline was rebuilt, in which case
    /// the bkgd upload task holds dead channel endpoints so we can't get updates.
    pub fn on_timeline_reconnected(&mut self, cx: &mut Cx) {
        let Some(upload) = self.state.take() else { return };
        match upload.phase {
            UploadPhase::Reading(abort_handle) => {
                abort_handle.abort();
                enqueue_popup_notification(
                    format!("Sending \"{}\" was interrupted, please try again.", upload.file_name),
                    PopupKind::Warning,
                    Some(7.0),
                );
            }
            UploadPhase::Queued { .. } | UploadPhase::Uploading { .. } => { }
            UploadPhase::Failed { .. } => self.state = Some(upload),
        }
        self.populate(cx);
    }

    /// Takes and returns the upload state to be saved with its room state,
    /// which also hides this view.
    pub fn save_state(&mut self) -> Option<UploadState> {
        self.visible = false;
        self.state.take()
    }

    /// Restores the given upload state into this view.
    pub fn restore_state(&mut self, cx: &mut Cx, upload: Option<UploadState>) {
        self.state = upload;
        self.button(cx, ids!(cancel_button)).reset_hover(cx);
        self.button(cx, ids!(retry_button)).reset_hover(cx);
        self.populate(cx);
    }

    /// Populates everything from `self.state` into this view's child widgets.
    fn populate(&mut self, cx: &mut Cx) {
        let Some(upload) = &self.state else {
            self.set_visible(cx, false);
            self.redraw(cx);
            return;
        };
        let file_name = format!("Sending:  {}", upload.file_name);
        let (status, fraction) = match &upload.phase {
            UploadPhase::Reading(_) => ("Preparing upload...".to_string(), 0.0),
            UploadPhase::Queued { is_encrypted: true, .. } => ("Encrypting...".to_string(), 0.0),
            UploadPhase::Queued { is_encrypted: false, .. } => ("Starting upload...".to_string(), 0.0),
            UploadPhase::Uploading { current_bytes, total_bytes, .. } => (
                upload_progress_text(*current_bytes, *total_bytes),
                if *total_bytes > 0 { (*current_bytes as f32 / *total_bytes as f32).clamp(0.0, 1.0) } else { 0.0 },
            ),
            UploadPhase::Failed { error, .. } => (format!("Error: {error}"), 1.0),
        };
        let is_failed = matches!(upload.phase, UploadPhase::Failed { .. });
        let can_retry = matches!(upload.phase, UploadPhase::Failed { retry: Some(_), .. });

        self.set_visible(cx, true);
        self.label(cx, ids!(file_name_label)).set_text(cx, &file_name);
        let status_label = self.label(cx, ids!(status_label));
        status_label.set_text(cx, &status);
        self.button(cx, ids!(retry_button)).set_visible(cx, can_retry);
        let progress_bar = self.child_by_path(ids!(progress_bar)).as_progress_bar();
        progress_bar.set_progress(cx, fraction);
        if let Some(mut label) = status_label.borrow_mut() {
            label.draw_text.color = if is_failed { COLOR_FG_DANGER_RED } else { COLOR_TEXT };
        }
        if is_failed {
            progress_bar.set_progress_color(cx, COLOR_FG_DANGER_RED);
        } else {
            progress_bar.reset_progress_color(cx);
        }
        self.redraw(cx);
    }
}

impl UploadProgressViewRef {
    /// See [`UploadProgressView::show()`].
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

    /// See [`UploadProgressView::set_as_queued()`].
    pub fn set_as_queued(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        transaction_id: OwnedTransactionId,
        is_encrypted: bool,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_as_queued(cx, upload_id, transaction_id, is_encrypted);
        }
    }

    /// See [`UploadProgressView::set_progress()`].
    pub fn set_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, current_bytes: usize, total_bytes: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, upload_id, current_bytes, total_bytes);
        }
    }

    /// See [`UploadProgressView::show_error()`].
    pub fn show_error(
        &self,
        cx: &mut Cx,
        upload_id: FileUploadAttemptId,
        error: &str,
        retryable_upload: Option<AttachmentUpload>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, upload_id, error, retryable_upload);
        }
    }

    /// See [`UploadProgressView::hide()`].
    pub fn hide(&self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx, upload_id);
        }
    }

    /// See [`UploadProgressView::on_timeline_reconnected()`].
    pub fn on_timeline_reconnected(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.on_timeline_reconnected(cx);
        }
    }

    /// See [`UploadProgressView::save_state()`].
    pub fn save_state(&self) -> Option<UploadState> {
        self.borrow_mut().and_then(|mut inner| inner.save_state())
    }

    /// See [`UploadProgressView::restore_state()`].
    pub fn restore_state(&self, cx: &mut Cx, upload: Option<UploadState>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.restore_state(cx, upload);
        }
    }

    /// Whether this view is showing a current upload in progress.
    pub fn has_active_upload(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.state.is_some())
    }
}
