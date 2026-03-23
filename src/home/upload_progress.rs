//! A composite widget for displaying upload progress with a label and cancel button.

use makepad_widgets::*;

use crate::shared::file_upload_modal::FileData;
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};
use crate::shared::progress_bar::ProgressBarWidgetExt;

/// Actions emitted by the UploadProgressView widget.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, DefaultNone)]
pub enum UploadProgressViewAction {
    /// The cancel button was clicked.
    Cancelled,
    /// The retry button was clicked with the file data to retry.
    Retry(FileData),
    None,
}

/// Action sent from the async upload task to provide the abort handle.
#[derive(Clone, Debug, DefaultNone)]
pub enum UploadAbortHandleAction {
    Ready(tokio::task::AbortHandle),
    None,
}

/// Tracks whether the upload view is in an error state with retry data.
#[derive(Debug, Clone, Default)]
pub enum UploadViewState {
    /// Normal state (uploading or hidden)
    #[default]
    Normal,
    /// Error state with file data available for retry
    Error(Box<FileData>),
}

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::progress_bar::ProgressBar;
    use crate::shared::icon_button::RobrixIconButton;

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

            retry_button = <RobrixIconButton> {
                visible: false,
                align: {x: 0.5, y: 0.5}
                padding: 15,
                draw_icon: {
                    svg_file: (ICON_ROTATE_CW)
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_FG_ACCEPT_GREEN),
                    color: (COLOR_BG_ACCEPT_GREEN)
                }
                text: "Retry"
                draw_text:{
                    color: (COLOR_FG_ACCEPT_GREEN),
                }
            }

            cancel_upload_button = <RobrixIconButton> {
                align: {x: 0.5, y: 0.5}
                padding: 15,
                draw_icon: {
                    svg_file: (ICON_FORBIDDEN)
                    color: (COLOR_FG_DANGER_RED),
                }
                icon_walk: {width: 16, height: 16, margin: {left: -2, right: -1} }

                draw_bg: {
                    border_color: (COLOR_FG_DANGER_RED),
                    color: (COLOR_BG_DANGER_RED)
                }
                text: "Cancel"
                draw_text:{
                    color: (COLOR_FG_DANGER_RED),
                }
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
    #[rust]
    upload_abort_handle: Option<tokio::task::AbortHandle>,
    /// Current state of the upload view
    #[rust]
    state: UploadViewState,
}

impl Widget for UploadProgressView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl WidgetMatchEvent for UploadProgressView {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        // Handle retry button being clicked (only visible in error state)
        if self.button(ids!(retry_button)).clicked(actions) {
            if let UploadViewState::Error(file_data) =
                std::mem::take(&mut self.state)
            {
                log!("Retrying upload");
                // Hide retry button
                self.button(ids!(retry_button)).set_visible(cx, false);
                // Reset to uploading state
                self.view
                    .label(ids!(progress_label))
                    .set_text(cx, "Uploading...");
                self.view.progress_bar(ids!(progress)).set_value(cx, 0.0);
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    UploadProgressViewAction::Retry(*file_data),
                );
            }
        }

        // Handle cancel button being clicked
        if self.button(ids!(cancel_upload_button)).clicked(actions) {
            if matches!(self.state, UploadViewState::Error(_)) {
                // In error state, just dismiss the error view
                self.hide(cx);
            } else {
                // Normal state, cancel the upload
                log!("Upload cancelled by user");
                // Abort the upload task if we have a handle
                if let Some(abort_handle) = self.upload_abort_handle.take() {
                    abort_handle.abort();
                }
                // Hide the progress bar immediately
                self.hide(cx);
                enqueue_popup_notification("Upload cancelled", PopupKind::Info, Some(3.0));

                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    UploadProgressViewAction::Cancelled,
                );
            }
        }
    }
}

impl UploadProgressView {
    /// Hides the progress view and resets state.
    pub fn hide(&mut self, cx: &mut Cx) {
        self.state = UploadViewState::Normal;
        // Hide retry button
        self.button(ids!(retry_button)).set_visible(cx, false);
        self.set_visible(cx, false);
        self.redraw(cx);
    }

    /// Shows an error state with the given error message and stores the file data for retry.
    pub fn show_error(&mut self, cx: &mut Cx, error: String, file_data: FileData) {
        self.state = UploadViewState::Error(Box::new(file_data));
        self.upload_abort_handle = None;

        // Update the label to show the error
        self.view
            .label(ids!(progress_label))
            .set_text(cx, &format!("Upload failed: {}", error));

        // Show the retry button
        self.button(ids!(retry_button)).set_visible(cx, true);

        // Set progress bar to 0
        self.view.progress_bar(ids!(progress)).set_value(cx, 0.0);

        // Make sure the view is visible
        self.set_visible(cx, true);
        self.redraw(cx);
    }

    /// Sets the current progress value of the upload progress view.
    ///
    /// This updates the displayed text and the progress bar to reflect the current
    /// progress of the upload. The `current` and `total` parameters are used to
    /// calculate the progress percentage. If `total` is zero, the progress
    /// percentage will be set to 0.0. If `current` is greater than or equal to
    /// `total`, the progress percentage will be set to 100.0, indicating that
    /// the upload is complete. Value is absolute instead of percentage or decimal.
    pub fn set_value(&mut self, cx: &mut Cx, current: u64, total: u64) {
        if current == 0 && total == 0 {
            // No progress to show, hide the view
            self.hide(cx);
            return;
        }
        // Upload complete, hide the progress view
        if current >= total && total > 0 {
            self.hide(cx);
            return;
        }
        let progress_percentage = if total > 0 {
            ((current as f64 / total as f64) * 100.0).min(100.0)
        } else {
            0.0
        };
        self.view
            .label(ids!(progress_label))
            .set_text(cx, &format!("Uploading... {:.0}%", progress_percentage));
        self.view
            .progress_bar(ids!(progress))
            .set_value(cx, progress_percentage);
    }

    /// Sets the abort handle for the upload task, allowing the progress view to cancel the upload if needed.
    pub fn set_abort_handle(&mut self, handle: tokio::task::AbortHandle) {
        self.upload_abort_handle = Some(handle);
    }
}

impl UploadProgressViewRef {
    /// Hides the progress view.
    pub fn hide(&self, cx: &mut Cx) {
        if let Some(mut view) = self.borrow_mut() {
            view.hide(cx);
        }
    }

    /// Sets the current progress value of the upload progress view.
    pub fn set_value(&self, cx: &mut Cx, current: u64, total: u64) {
        if let Some(mut view) = self.borrow_mut() {
            view.set_value(cx, current, total);
        }
    }

    /// Sets the abort handle for the upload task, allowing the progress view to cancel the upload if needed.
    pub fn set_abort_handle(&self, handle: tokio::task::AbortHandle) {
        if let Some(mut view) = self.borrow_mut() {
            view.set_abort_handle(handle);
        }
    }

    /// Shows an error state with the given error message and stores the file data for retry.
    pub fn show_error(&self, cx: &mut Cx, error: String, file_data: FileData) {
        if let Some(mut view) = self.borrow_mut() {
            view.show_error(cx, error, file_data);
        }
    }
}
