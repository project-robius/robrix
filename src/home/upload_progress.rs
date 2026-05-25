//! A widget that displays upload progress with a progress bar, status label,
//! and cancel/retry buttons.

use makepad_widgets::*;
use tokio::task::AbortHandle;

use crate::home::room_screen::RoomScreenProps;
use crate::shared::file_upload_modal::{AttachmentUpload, FileUploadAttemptId};
use crate::shared::progress_bar::ProgressBarWidgetRefExt;
use crate::shared::styles::COLOR_FG_DANGER_RED;
use crate::sliding_sync::TimelineKind;
use crate::utils::format_decimal_file_size;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.UploadProgressView = set_type_default() do #(UploadProgressView::register_widget(vm)) {
        visible: false,
        width: Fill,
        height: Fit,
        flow: Down,
        padding: Inset { top: 10, bottom: 10, left: 15, right: 15 }
        spacing: 8,

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

            Label {
                width: Fit,
                padding: 0,
                margin: 0
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT)
                }
                text: "Sending: "
            }

            file_name_label := Label {
                width: Fill,
                padding: 0,
                margin: 0
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
                padding: 0,
                margin: 0
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
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
pub enum UploadViewState {
    /// Normal state - upload in progress or ready.
    #[default]
    Normal,
    /// Error state - upload failed.
    Error {
        upload: AttachmentUpload,
    },
}

/// Actions emitted by the UploadProgressView.
#[derive(Clone, Debug, Default)]
pub enum UploadProgressViewAction {
    /// No action.
    #[default]
    None,
    /// User requested retry of a failed upload.
    Retry {
        upload: AttachmentUpload,
        timeline_kind: TimelineKind,
    },
}

/// A widget showing upload progress with cancel/retry functionality.
#[derive(Script, ScriptHook, Widget)]
pub struct UploadProgressView {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// Handle to abort the current upload task.
    #[rust] abort_handle: Option<AbortHandle>,
    /// The upload attempt currently represented by this view.
    #[rust] upload_id: Option<FileUploadAttemptId>,
    /// Current progress value (0.0 to 1.0).
    #[rust] progress: f32,
    /// Current state of the upload view.
    #[rust] state: UploadViewState,
}

impl Widget for UploadProgressView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // Handle cancel button
            if self.button(cx, ids!(cancel_button)).clicked(actions) {
                if let Some(handle) = self.abort_handle.take() {
                    log!("Upload cancel requested for {:?}, aborting upload task.", self.upload_id);
                    handle.abort();
                    log!("Upload abort requested for {:?}.", self.upload_id);
                } else {
                    log!("Upload cancel requested for {:?}, but no abort handle was available.", self.upload_id);
                }
                self.hide_current(cx);
            }

            // Handle retry button
            if self.button(cx, ids!(retry_button)).clicked(actions) {
                if let UploadViewState::Error { upload, .. } = &self.state {
                    if let Some(room_screen_props) = scope.props.get::<RoomScreenProps>() {
                        cx.widget_action(self.widget_uid(), UploadProgressViewAction::Retry {
                            upload: upload.clone(),
                            timeline_kind: room_screen_props.timeline_kind.clone(),
                        });
                    }
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
    pub fn show(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId, file_name: &str) {
        self.set_visible(cx, true);
        self.upload_id = Some(upload_id);
        self.abort_handle = None;
        self.state = UploadViewState::Normal;
        self.progress = 0.0;

        self.label(cx, ids!(file_name_label)).set_text(cx, file_name);
        self.label(cx, ids!(status_label)).set_text(cx, "Starting upload...");
        let retry_button = self.button(cx, ids!(retry_button));
        retry_button.set_visible(cx, false);
        retry_button.reset_hover(cx);
        self.button(cx, ids!(cancel_button)).reset_hover(cx);

        self.reset_progress_bar(cx);

        self.redraw(cx);
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
        self.abort_handle = None;
        self.state = UploadViewState::Normal;
        self.button(cx, ids!(retry_button)).set_visible(cx, false);
        self.reset_progress_bar(cx);
        self.redraw(cx);
    }

    /// Updates the progress value if it belongs to the given upload attempt.
    pub fn set_progress(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId, current: u64, total: u64) {
        if self.upload_id != Some(upload_id) {
            return;
        }
        if let UploadViewState::Error { .. } = self.state {
            return
        }
        self.progress = if total > 0 {
            (current as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.child_by_path(ids!(progress_bar)).as_progress_bar()
            .set_progress(cx, self.progress);

        // Update status label
        let percent = (self.progress * 100.0) as u32;
        let status = format!(
            "Uploading... {}% ({} / {})",
            percent,
            format_decimal_file_size(current),
            format_decimal_file_size(total)
        );
        self.label(cx, ids!(status_label)).set_text(cx, &status);

        self.redraw(cx);
    }

    /// Sets the abort handle for the current upload task.
    pub fn set_abort_handle(&mut self, upload_id: FileUploadAttemptId, handle: AbortHandle) {
        if self.upload_id == Some(upload_id) && !matches!(self.state, UploadViewState::Error { .. }) {
            log!("Received abort handle for upload {upload_id:?}.");
            self.abort_handle = Some(handle);
        } else {
            log!("Discarding stale abort handle for upload {upload_id:?}; current upload is {:?}.", self.upload_id);
            handle.abort();
        }
    }

    /// Shows an error state with the given message if it belongs to the given upload attempt.
    pub fn show_error(&mut self, cx: &mut Cx, upload_id: FileUploadAttemptId, error: &str, upload: AttachmentUpload) {
        if self.upload_id != Some(upload_id) {
            return;
        }
        self.abort_handle = None;
        self.state = UploadViewState::Error {
            upload,
        };

        // Update UI for error state
        self.label(cx, ids!(status_label))
            .set_text(cx, &format!("Error: {}", error));
        let retry_button = self.button(cx, ids!(retry_button));
        retry_button.set_visible(cx, true);
        retry_button.reset_hover(cx);

        self.progress = 1.0;
        let progress_bar = self.child_by_path(ids!(progress_bar)).as_progress_bar();
        progress_bar.set_progress_color(cx, COLOR_FG_DANGER_RED);
        progress_bar.set_progress(cx, 1.0);

        self.redraw(cx);
    }

    fn reset_progress_bar(&mut self, cx: &mut Cx) {
        self.child_by_path(ids!(progress_bar)).as_progress_bar().reset_progress_color(cx);
        self.child_by_path(ids!(progress_bar)).as_progress_bar().set_progress(cx, 0.0);
    }
}

impl UploadProgressViewRef {
    /// Shows the upload progress view with the given file name.
    pub fn show(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, file_name: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, upload_id, file_name);
        }
    }

    /// Hides the upload progress view if it belongs to the given upload attempt.
    pub fn hide(&self, cx: &mut Cx, upload_id: FileUploadAttemptId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx, upload_id);
        }
    }

    /// Updates the progress value if it belongs to the given upload attempt.
    pub fn set_progress(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, current: u64, total: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, upload_id, current, total);
        }
    }

    /// Sets the abort handle for the current upload task.
    pub fn set_abort_handle(&self, upload_id: FileUploadAttemptId, handle: AbortHandle) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_abort_handle(upload_id, handle);
        }
    }

    /// Shows an error state with the given message if it belongs to the given upload attempt.
    pub fn show_error(&self, cx: &mut Cx, upload_id: FileUploadAttemptId, error: &str, upload: AttachmentUpload) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, upload_id, error, upload);
        }
    }
}
