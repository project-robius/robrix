//! Download a Matrix media attachment and save it to storage.

use std::{sync::Arc, time::Duration};
use makepad_widgets::SignalToUI;
use matrix_sdk::ruma::{OwnedMxcUri, events::room::MediaSource};
use robius_share::ShareSheet;
use crate::{
    home::room_screen::TimelineUpdate,
    shared::popup_list::{PopupKind, enqueue_popup_notification},
    sliding_sync::{MatrixRequest, submit_async_request},
    temp_storage::get_temp_dir_path,
};

pub type TimelineUpdateSenderOption = Option<crossbeam_channel::Sender<TimelineUpdate>>;

/// The result of a download request.
pub enum MediaDownloadResult {
    Downloaded(Vec<u8>),
    Failed(String),
    Cancelled,
}

pub fn enqueue_already_downloading_notification() {
    const ALREADY_DOWNLOADING_MESSAGE: &str = "This media is already being downloaded.";
    enqueue_popup_notification(ALREADY_DOWNLOADING_MESSAGE, PopupKind::Warning, Some(4.0));
}

/// The mxc URI inside any media source, whether plain or encrypted.
pub fn media_source_mxc(source: &MediaSource) -> &OwnedMxcUri {
    match source {
        MediaSource::Plain(uri) => uri,
        MediaSource::Encrypted(file) => &file.url,
    }
}

/// Whether a pending transfer is a plain download or a share.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransferKind {
    Download,
    Share,
}

/// Info about a download or share that has begun or recently completed.
pub struct PendingDownload {
    pub mxc: OwnedMxcUri,
    pub state: PendingDownloadState,
    pub kind: TransferKind,
}

pub enum PendingDownloadState {
    /// The request has been submitted to and is being handled by the backend worker task.
    InProgress,
    /// It succeeded, and will show a success indicator for a few seconds.
    JustSucceeded,
    /// It failed, and will show an error indicator for a few seconds.
    JustFailed,
}
impl PendingDownloadState {
    pub fn display(&self, kind: TransferKind) -> DownloadDisplayState {
        match self {
            Self::InProgress => DownloadDisplayState::InProgress,
            Self::JustSucceeded => DownloadDisplayState::Succeeded(kind),
            Self::JustFailed => DownloadDisplayState::Failed,
        }
    }
}

/// What the download section below a message should show.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DownloadDisplayState {
    /// Default: show the download and share buttons.
    #[default]
    Idle,
    /// Show a loading spinner and cancel button.
    InProgress,
    /// Briefly show a green success button ("Downloaded" or "Shared").
    Succeeded(TransferKind),
    /// Briefly show a red failed button.
    Failed,
}

/// How long (in seconds) the success/failure state stays visible before resetting the button.
pub const DOWNLOAD_RESULT_DURATION_SECS: f64 = 5.0;

/// Metadata describing an attachment/media file to be downloaded.
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
    /// Returns the top-level MIME type for this kind of download.
    fn basic_mime_type(&self) -> Option<&'static str> {
        match self {
            Self::Image => Some("image/*"),
            Self::Audio => Some("audio/*"),
            Self::Video => Some("video/*"),
            Self::File => None,
        }
    }
}

/// Fetches the attachment via the media cache and then calls `on_downloaded`.
fn download_media(
    info: DownloadableAttachment,
    update_sender: TimelineUpdateSenderOption,
    on_downloaded: impl FnOnce(String, OwnedMxcUri, Vec<u8>, TimelineUpdateSenderOption) + Send + 'static,
) {
    let mxc = media_source_mxc(&info.media_source).clone();
    let filename = info.filename.clone();
    submit_async_request(MatrixRequest::DownloadMedia {
        media_source: info.media_source,
        filename: info.filename,
        on_download_result: Box::new(move |result| match result {
            MediaDownloadResult::Downloaded(bytes) => on_downloaded(filename, mxc, bytes, update_sender),
            MediaDownloadResult::Failed(e) => {
                enqueue_popup_notification(
                    format!("Failed to download \"{filename}\": {e}"),
                    PopupKind::Error,
                    None,
                );
                finish_download_indicator(&update_sender, Some(&mxc), DownloadOutcome::Failed);
            }
            MediaDownloadResult::Cancelled => {
                finish_download_indicator(&update_sender, Some(&mxc), DownloadOutcome::Cancelled)
            }
        }),
    });
}

/// Downloads the attachment and shows the native save dialog for it.
pub fn start_attachment_download(
    info: DownloadableAttachment,
    update_sender: TimelineUpdateSenderOption,
) {
    download_media(info, update_sender, |filename, mxc, bytes, sender| {
        show_save_dialog(filename, bytes, Some(mxc), sender);
    });
}

/// Saves an attachment already in memory directly to storage, without showing any dialog.
pub fn save_loaded_attachment(filename: String, bytes: Arc<[u8]>) {
    show_save_dialog(filename, bytes, None, None);
}

/// Downloads the attachment and opens the native share sheet for it (via a temp file,
/// since sharing needs a real path).
pub fn start_attachment_share(
    info: DownloadableAttachment,
    update_sender: TimelineUpdateSenderOption,
) {
    let mime = info.kind.basic_mime_type();
    download_media(info, update_sender, move |filename, mxc, bytes, sender| {
        // Success shows a "Shared" indicator; a share-step failure (already shown as
        // a popup) just resets, since the download itself did succeed.
        let outcome = if share_bytes(&filename, &mxc, &bytes, mime) {
            DownloadOutcome::Succeeded
        } else {
            DownloadOutcome::Cancelled
        };
        finish_download_indicator(&sender, Some(&mxc), outcome);
    });
}

/// Opens the native share sheet for an attachment already in memory.
pub fn share_loaded_attachment(info: &DownloadableAttachment, bytes: Arc<[u8]>) {
    let mxc = media_source_mxc(&info.media_source).clone();
    let mime = info.kind.basic_mime_type();
    let filename = info.filename.clone();
    // Don't write a large attachment file to disk on the main UI thread.
    std::thread::spawn(move || share_bytes(&filename, &mxc, &bytes, mime));
}

/// Writes the given `bytes` to a temp file (based on `filename`) and presents the native share sheet for it.
///
/// This is required because sharing a file requires a real path.
///
/// Returns true if the share sheet was shown. Enqueues a popup error upon any failure.
fn share_bytes(filename: &str, mxc: &OwnedMxcUri, bytes: &[u8], mime: Option<&str>) -> bool {
    let mut safe_name = sanitize_filename::sanitize(filename);
    if safe_name.is_empty() {
        safe_name = "shared_file".to_owned();
    }
    let dir = get_temp_dir_path().join(sanitize_filename::sanitize(mxc.as_str()));
    let path = dir.join(safe_name);
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, bytes)) {
        enqueue_popup_notification(
            format!("Failed to prepare \"{filename}\" for sharing: {e}"),
            PopupKind::Error,
            None,
        );
        return false;
    }
    let sheet = ShareSheet::new().set_title(format!("Share {filename}"));
    let sheet = match mime {
        Some(mime) => sheet.add_file_with_mime_type(&path, mime),
        None => sheet.add_file(&path),
    };
    if let Err(e) = sheet.share() {
        enqueue_popup_notification(
            format!("Couldn't open the share sheet for \"{filename}\": {e}"),
            PopupKind::Error,
            None,
        );
        return false;
    }
    true
}

enum DownloadOutcome {
    Succeeded,
    Failed,
    /// The user dismissed the save dialog; return the indicator to idle.
    Cancelled,
}

/// Shows the native save-file dialog and writes `data` to the user-chosen location.
fn show_save_dialog<D: AsRef<[u8]> + Send + 'static>(
    filename: String,
    data: D,
    mxc: Option<OwnedMxcUri>,
    update_sender: TimelineUpdateSenderOption,
) {
    use robius_file_picker::{PickedFile, FileDialog, StartLocation};
    let filename2 = filename.clone();
    let mxc2 = mxc.clone();
    let sender2 = update_sender.clone();
    let on_done = move |result: robius_file_picker::Result<Option<PickedFile>>| {
        match result {
            Ok(Some(_)) => {
                enqueue_popup_notification(
                    format!("Downloaded \"{filename2}\"."),
                    PopupKind::Success,
                    Some(DOWNLOAD_RESULT_DURATION_SECS),
                );
                finish_download_indicator(&update_sender, mxc.as_ref(), DownloadOutcome::Succeeded);
            }
            // User dismissed the save dialog. The bytes are discarded, but the
            // matrix media cache makes a re-download effectively instant, so we
            // just return the indicator to idle without a popup.
            Ok(None) => {
                finish_download_indicator(&update_sender, mxc.as_ref(), DownloadOutcome::Cancelled);
            }
            Err(e) => {
                enqueue_popup_notification(
                    format!("Failed to save \"{filename2}\": {e}"),
                    PopupKind::Error,
                    None,
                );
                finish_download_indicator(&update_sender, mxc.as_ref(), DownloadOutcome::Failed);
            }
        }
    };

    let res = FileDialog::new()
        .set_file_name(&filename)
        .set_start_location(StartLocation::Downloads)
        .save_data(data, on_done);
    if let Err(e) = res {
        enqueue_popup_notification(
            format!("Failed to open save dialog: {e}"),
            PopupKind::Error,
            None,
        );
        finish_download_indicator(&sender2, mxc2.as_ref(), DownloadOutcome::Failed);
    }
}

/// Handles the completion of a download, whether success, failure, or cancelled.
fn finish_download_indicator(
    update_sender: &TimelineUpdateSenderOption,
    mxc: Option<&OwnedMxcUri>,
    outcome: DownloadOutcome,
) {
    let Some(mxc) = mxc else { return };
    let Some(sender) = update_sender.as_ref() else { return };
    match outcome {
        DownloadOutcome::Cancelled => {
            let _ = sender.send(TimelineUpdate::AttachmentDownloadReset(mxc.clone()));
            SignalToUI::set_ui_signal();
        }
        DownloadOutcome::Succeeded | DownloadOutcome::Failed => {
            let result = match outcome {
                DownloadOutcome::Succeeded => Ok(()),
                _ => Err(String::new()),
            };
            let _ = sender.send(TimelineUpdate::AttachmentDownloadFinished(mxc.clone(), result));
            SignalToUI::set_ui_signal();
            // Clear the success/failure display after a short delay.
            let sender = sender.clone();
            let mxc = mxc.clone();
            crate::sliding_sync::spawn_async_task(async move {
                tokio::time::sleep(Duration::from_secs_f64(DOWNLOAD_RESULT_DURATION_SECS)).await;
                let _ = sender.send(TimelineUpdate::AttachmentDownloadReset(mxc));
                SignalToUI::set_ui_signal();
            });
        }
    }
}
