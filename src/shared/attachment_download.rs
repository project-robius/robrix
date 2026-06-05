//! Save a Matrix media attachment to disk via the rfd save dialog.
//! Used by the inline message button and the image viewer overlay.

use std::sync::Arc;

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

/// Info about a download that has begun or recently completed.
pub struct PendingDownload {
    pub mxc: OwnedMxcUri,
    pub state: PendingDownloadState,
}

pub enum PendingDownloadState {
    /// The download request has been submitted to and is being handled by
    /// the backend worker task.
    InProgress,
    /// The download was successful, and will show a success indicator for a few seconds.
    JustSucceeded,
    /// The download failed, and will show an error indicator for a few seconds.
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

/// What the download section below a message should show.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DownloadDisplayState {
    /// Default: show the download button.
    #[default]
    Idle,
    /// Show a loading spinner and cancel button.
    InProgress,
    /// Briefly show a green success button.
    Succeeded,
    /// Briefly show a red failed button.
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
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn start_attachment_download(
    info: DownloadableAttachment,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    use crate::sliding_sync::{MatrixRequest, spawn_async_task, submit_async_request};

    let dialog = build_save_dialog(&info);
    spawn_async_task(async move {
        match dialog.save_file().await {
            Some(handle) => {
                submit_async_request(MatrixRequest::DownloadMediaToFile {
                    media_source: info.media_source,
                    save_path: handle.path().to_path_buf(),
                    filename: info.filename,
                    update_sender,
                });
            }
            // User cancelled. The action handler already marked this mxc as
            // pending, so revert it now or the spinner stays forever.
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
/// round-trip and writes straight to disk.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn save_loaded_attachment(info: DownloadableAttachment, bytes: Arc<[u8]>) {
    use crate::sliding_sync::spawn_async_task;

    let dialog = build_save_dialog(&info);
    spawn_async_task(async move {
        let Some(handle) = dialog.save_file().await else { return };
        let save_path = handle.path().to_path_buf();
        match tokio::fs::write(&save_path, &bytes[..]).await {
            Ok(()) => enqueue_popup_notification(
                format!("Saved \"{}\" to {}", info.filename, save_path.display()),
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
pub fn save_loaded_attachment(_info: DownloadableAttachment, _bytes: Arc<[u8]>) {
    enqueue_popup_notification(
        "Saving attachments is not yet supported on mobile.",
        PopupKind::Error,
        Some(5.0),
    );
}
