//! A widget that displays upload progress with a progress bar, status label,
//! and cancel/retry buttons.

use makepad_widgets::*;
use tokio::task::AbortHandle;

use crate::shared::file_upload_modal::FileData;
use crate::shared::progress_bar::ProgressBarWidgetRefExt;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.UploadProgressView = set_type_default() do #(UploadProgressView::register_widget(vm)) {
        visible: false,
        width: Fill,
        height: Fit,
        flow: Down,
        padding: 10,
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

            uploading_label := Label {
                width: Fit,
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT)
                }
                text: "Uploading: "
            }

            file_name_label := Label {
                width: Fill,
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 10 },
                    color: (COLOR_TEXT)
                }
                text: ""
            }

            cancel_button := RobrixNeutralIconButton {
                width: 24, height: 24,
                padding: 4,
                draw_icon +: { svg: (ICON_CLOSE) }
                icon_walk: Walk{width: 14, height: 14}
                text: ""
            }
        }

        // Progress bar
        progress_bar := ProgressBar {
            width: Fill,
            height: 6,
        }

        // Status/error area
        status_view := View {
            width: Fill,
            height: Fit,
            flow: Right,
            align: Align{x: 0.0, y: 0.5},
            spacing: 10,

            status_label := Label {
                width: Fill,
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 9 },
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            retry_button := RobrixPositiveIconButton {
                visible: false,
                padding: Inset{top: 4, bottom: 4, left: 8, right: 8}
                draw_text +: {
                    text_style: REGULAR_TEXT { font_size: 9 },
                }
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
        message: String,
        file_data: FileData,
    },
}

/// Actions emitted by the UploadProgressView.
#[derive(Clone, Debug, Default)]
pub enum UploadProgressViewAction {
    /// No action.
    #[default]
    None,
    /// User cancelled the upload.
    Cancelled,
    /// User requested retry of a failed upload.
    Retry(FileData),
}

/// A widget showing upload progress with cancel/retry functionality.
#[derive(Script, ScriptHook, Widget)]
pub struct UploadProgressView {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// Handle to abort the current upload task.
    #[rust] abort_handle: Option<AbortHandle>,
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
                    handle.abort();
                }
                cx.action(UploadProgressViewAction::Cancelled);
                self.hide(cx);
            }

            // Handle retry button
            if self.button(cx, ids!(retry_button)).clicked(actions) {
                if let UploadViewState::Error { file_data, .. } = &self.state {
                    let file_data = file_data.clone();
                    cx.action(UploadProgressViewAction::Retry(file_data));
                    self.hide(cx);
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
    pub fn show(&mut self, cx: &mut Cx, file_name: &str) {
        self.set_visible(cx, true);
        self.state = UploadViewState::Normal;
        self.progress = 0.0;

        self.label(cx, ids!(file_name_label)).set_text(cx, file_name);
        self.label(cx, ids!(status_label)).set_text(cx, "Starting upload...");
        self.button(cx, ids!(retry_button)).set_visible(cx, false);
        self.button(cx, ids!(cancel_button)).set_visible(cx, true);

        // Reset progress bar
        self.child_by_path(ids!(progress_bar)).as_progress_bar().set_progress(cx, 0.0);

        self.redraw(cx);
    }

    /// Hides the upload progress view.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.set_visible(cx, false);
        self.abort_handle = None;
        self.state = UploadViewState::Normal;
        self.redraw(cx);
    }

    /// Updates the progress value.
    pub fn set_progress(&mut self, cx: &mut Cx, current: u64, total: u64) {
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
            crate::utils::format_file_size(current),
            crate::utils::format_file_size(total)
        );
        self.label(cx, ids!(status_label)).set_text(cx, &status);

        self.redraw(cx);
    }

    /// Sets the abort handle for the current upload task.
    pub fn set_abort_handle(&mut self, handle: AbortHandle) {
        self.abort_handle = Some(handle);
    }

    /// Shows an error state with the given message.
    pub fn show_error(&mut self, cx: &mut Cx, error: &str, file_data: FileData) {
        self.state = UploadViewState::Error {
            message: error.to_string(),
            file_data,
        };

        // Update UI for error state
        self.label(cx, ids!(status_label))
            .set_text(cx, &format!("Error: {}", error));
        self.button(cx, ids!(retry_button)).set_visible(cx, true);
        self.button(cx, ids!(cancel_button)).set_visible(cx, true);

        // Set progress bar to error color - no longer apply color change via script_apply_eval
        // The progress bar will use the default color for now

        self.redraw(cx);
    }
}

impl UploadProgressViewRef {
    /// Shows the upload progress view with the given file name.
    pub fn show(&self, cx: &mut Cx, file_name: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, file_name);
        }
    }

    /// Hides the upload progress view.
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide(cx);
        }
    }

    /// Updates the progress value.
    pub fn set_progress(&self, cx: &mut Cx, current: u64, total: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_progress(cx, current, total);
        }
    }

    /// Sets the abort handle for the current upload task.
    pub fn set_abort_handle(&self, handle: AbortHandle) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_abort_handle(handle);
        }
    }

    /// Shows an error state with the given message.
    pub fn show_error(&self, cx: &mut Cx, error: &str, file_data: FileData) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_error(cx, error, file_data);
        }
    }
}
