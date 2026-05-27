//! Save a Matrix media attachment to disk via the rfd save dialog.
//! Used by the inline message button and the image viewer overlay.

use std::sync::Arc;
use makepad_widgets::Cx;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use makepad_widgets::CxOsApi;
use matrix_sdk::ruma::{OwnedMxcUri, events::room::MediaSource};
use crate::home::room_screen::TimelineUpdate;
use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};

/// The mxc URI inside any media source, whether plain or encrypted.
pub fn media_source_mxc(source: &MediaSource) -> &OwnedMxcUri {
    match source {
        MediaSource::Plain(uri) => uri,
        MediaSource::Encrypted(file) => &file.url,
    }
}

/// One entry in `TimelineUiState.pending_downloads`. Tracks an attachment
/// download's lifecycle so the inline button can render the right view.
pub struct PendingDownload {
    pub mxc: OwnedMxcUri,
    pub state: PendingDownloadState,
}

pub enum PendingDownloadState {
    /// Fetch+write in flight. The matrix worker owns the corresponding
    /// `AbortHandle`; `MessageAction::CancelDownload` routes through
    /// `MatrixRequest::CancelDownload` to abort it.
    InProgress,
    /// Bytes hit disk. Shown for a few seconds before the entry is cleared.
    JustSucceeded,
    /// Fetch or write failed. Shown for a few seconds before the entry is cleared.
    JustFailed,
}

impl PendingDownloadState {
    pub fn display(&self) -> DownloadDisplayState {
        match self {
            Self::InProgress => DownloadDisplayState::InProgress,
            Self::JustSucceeded => DownloadDisplayState::Succeeded,
            Self::JustFailed => DownloadDisplayState::Failed,
        }
    }
}

/// What the inline download section should render. Decoupled from
/// `PendingDownloadState` so the `Message` widget doesn't need to know
/// about `AbortHandle`s.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DownloadDisplayState {
    /// Show the "Download …" button.
    #[default]
    Idle,
    /// Show the spinner + cancel button.
    InProgress,
    /// Briefly show a green "Saved" indicator.
    Succeeded,
    /// Briefly show a red "Failed" indicator.
    Failed,
}

/// How long (in seconds) the success/failure state stays visible before resetting the button.
pub const DOWNLOAD_RESULT_DURATION_SECS: f64 = 5.0;


#[derive(Clone, Debug)]
pub struct DownloadableAttachment {
    pub media_source: MediaSource,
    pub filename: String,
    pub size: Option<u64>,
    pub kind: DownloadKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadKind {
    File,
    Audio,
    Video,
    Image,
}
impl DownloadKind {
    pub fn button_text(&self) -> &'static str {
        match self {
            Self::File => "Download File",
            Self::Audio => "Download Audio",
            Self::Video => "Download Video",
            Self::Image => "Download Image",
        }
    }
}

/// Opens the rfd save dialog with sensible defaults for `info`.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn build_save_dialog(info: &DownloadableAttachment) -> rfd::AsyncFileDialog {
    let dialog = rfd::AsyncFileDialog::new().set_file_name(&info.filename);
    if let Some(user_dirs) = robius_directories::UserDirs::new() {
        let dir = user_dirs.download_dir()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| user_dirs.home_dir().to_path_buf());
        dialog.set_directory(dir)
    } else {
        dialog
    }
}

/// Opens the save dialog, then submits the actual fetch+write request.
/// Pass `update_sender` if the caller wants spinner updates routed back to
/// a specific timeline; pass `None` from contexts without one (e.g. the
/// image viewer overlay).
///
/// The dialog runs on a fresh OS thread (not a tokio task) because rfd's
/// macOS backend falls back to a sync dialog and panics when called from
/// a tokio worker thread.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn start_attachment_download(
    cx: &mut Cx,
    info: DownloadableAttachment,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    use crate::sliding_sync::{MatrixRequest, submit_async_request};

    let dialog_task = build_save_dialog(&info).save_file();
    cx.spawn_thread(move || {
        match futures::executor::block_on(dialog_task) {
            Some(handle) => {
                submit_async_request(MatrixRequest::DownloadMediaToFile {
                    media_source: info.media_source,
                    save_path: handle.path().to_path_buf(),
                    filename: info.filename,
                    update_sender,
                });
            }
            // User cancelled. The action handler already marked this mxc as
            // pending. Revert directly (skip the success/failure flash, since
            // dismissing the dialog isn't really a failure).
            None => {
                if let Some(sender) = update_sender {
                    let mxc = media_source_mxc(&info.media_source).clone();
                    let _ = sender.send(TimelineUpdate::AttachmentDownloadReset(mxc));
                    makepad_widgets::SignalToUI::set_ui_signal();
                }
            }
        }
    });
}

/// Like `start_attachment_download` but for callers that already have the
/// bytes in memory (e.g. the image viewer). Skips the matrix worker
/// round-trip and writes straight to disk on the same OS thread that
/// hosted the save dialog.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn save_loaded_attachment(cx: &mut Cx, info: DownloadableAttachment, bytes: Arc<[u8]>) {
    let dialog_task = build_save_dialog(&info).save_file();
    cx.spawn_thread(move || {
        let Some(handle) = futures::executor::block_on(dialog_task) else { return };
        let save_path = handle.path().to_path_buf();
        match std::fs::write(&save_path, &bytes[..]) {
            Ok(()) => enqueue_popup_notification(
                format!("Downloaded \"{}\".", info.filename),
                PopupKind::Success,
                Some(5.0),
            ),
            Err(e) => enqueue_popup_notification(
                format!("Failed to save \"{}\": {e}", info.filename),
                PopupKind::Error,
                None,
            ),
        }
    });
}

/// Mobile: rfd doesn't have a save dialog there, so just tell the user.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn start_attachment_download(
    _cx: &mut Cx,
    _info: DownloadableAttachment,
    _update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    enqueue_popup_notification(
        "Saving attachments is not yet supported on mobile.",
        PopupKind::Error,
        Some(5.0),
    );
}

#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn save_loaded_attachment(_cx: &mut Cx, _info: DownloadableAttachment, _bytes: Arc<[u8]>) {
    enqueue_popup_notification(
        "Saving attachments is not yet supported on mobile.",
        PopupKind::Error,
        Some(5.0),
    );
}
