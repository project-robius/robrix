//! Download a Matrix media attachment and save it to storage.

use std::{sync::Arc, time::Duration};
use makepad_widgets::SignalToUI;
use matrix_sdk::ruma::{OwnedMxcUri, events::room::MediaSource};
use crate::{
    home::room_screen::TimelineUpdate,
    shared::popup_list::{PopupKind, enqueue_popup_notification},
    sliding_sync::{MatrixRequest, submit_async_request},
};

type TimelineUpdateSenderOption = Option<crossbeam_channel::Sender<TimelineUpdate>>;

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
    pub fn button_text(&self) -> &'static str {
        match self {
            Self::File => "Download File",
            Self::Audio => "Download Audio",
            Self::Video => "Download Video",
            Self::Image => "Download Image",
        }
    }
}

/// Fetches the attachment bytes via the matrix worker, then shows the native
/// save picker dialog with those bytes.
///
/// This *does* use the matrix SDK's media cache, so there's a good chance
/// that an attachment, especially small ones, will be instantly served from the cache.
///
/// The download indicator stays in the "in-progress" state until everything is done.
/// We transition to "success" only if the file is saved, "idle" if the user cancels,
/// and "failure" if the download fails or write to storage fails.
pub fn start_attachment_download(
    info: DownloadableAttachment,
    update_sender: TimelineUpdateSenderOption,
) {
    let mxc = media_source_mxc(&info.media_source).clone();
    let filename = info.filename.clone();
    submit_async_request(MatrixRequest::DownloadMedia {
        media_source: info.media_source,
        filename: info.filename,
        on_download_result: Box::new(move |result| match result {
            MediaDownloadResult::Downloaded(bytes) => {
                show_save_dialog(filename, bytes, Some(mxc), update_sender)
            }
            MediaDownloadResult::Failed(e) => {
                enqueue_popup_notification(
                    format!("Failed to download \"{filename}\": {e}"),
                    PopupKind::Error,
                    None,
                );
                finish_download_indicator(&update_sender, Some(&mxc), DownloadOutcome::Failed);
            }
            MediaDownloadResult::Cancelled => {
                finish_download_indicator(&update_sender, Some(&mxc), DownloadOutcome::Cancelled);
            }
        }),
    });
}

/// Saves an attachment already in memory directly to storage, without showing any dialog.
pub fn save_loaded_attachment(info: DownloadableAttachment, bytes: Arc<[u8]>) {
    show_save_dialog(info.filename, bytes, None, None);
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
    let Some(sender) = update_sender.as_ref() else { return };
    let Some(mxc) = mxc else { return };
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
