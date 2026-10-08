use std::sync::{Arc, Mutex};

use makepad_widgets::*;
use matrix_sdk_ui::timeline::EventTimelineItem;
use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters},
    ruma::events::room::{message::MessageType, MediaSource},
};
use matrix_sdk::reqwest::StatusCode;

use crate::{home::room_screen::TimelineUpdate, media_cache::{error_to_media_cache_entry, MediaCacheEntry}, shared::image_viewer::ImageViewerError, sliding_sync::{submit_async_request, MatrixRequest}};

/// Fetches the full-size image for the image viewer.
///
/// The image viewer is already showing a thumbnail, so this returns nothing.
/// [`show_fetched_image_in_viewer`] will be called when the full image arrives.
pub fn fetch_full_image_for_viewer(media_source: MediaSource) {
    submit_async_request(MatrixRequest::FetchMedia {
        media_request: MediaRequestParameters {
            source: media_source,
            format: MediaFormat::File,
        },
        on_fetched: show_fetched_image_in_viewer,
        // this isn't used
        destination: Arc::new(Mutex::new(MediaCacheEntry::Requested)),
        update_sender: None,
    });
}

/// Details about the file of an image message or a sticker.
pub struct ImageFileDetails {
    pub name: String,
    pub caption: Option<String>,
    /// The image's format, e.g., "PNG", from its mimetype or else its file extension.
    pub format: Option<String>,
    pub size_in_bytes: Option<u64>,
}

/// Gets the details of the file of an image message or a sticker from an event timeline item.
pub fn get_image_file_details(event_tl_item: &EventTimelineItem) -> ImageFileDetails {
    let content = event_tl_item.content();
    let (name, caption, info) = if let Some(message) = content.as_message()
        && let MessageType::Image(image_content) = message.msgtype()
    {
        (image_content.filename(), image_content.caption(), image_content.info.as_deref())
    } else if let Some(sticker) = content.as_sticker() {
        // A sticker has no file name or caption, just a description in its body.
        (sticker.content().body.as_str(), None, Some(&sticker.content().info))
    } else {
        return ImageFileDetails {
            name: "Unknown Image".to_string(),
            caption: None,
            format: None,
            size_in_bytes: None,
        };
    };
    let format = info
        .and_then(|info| info.mimetype.as_deref())
        .and_then(|mimetype| mimetype.strip_prefix("image/"))
        // e.g., "svg+xml" is SVG and "x-icon" is ICON.
        .map(|subtype| subtype.split('+').next().unwrap_or(subtype).trim_start_matches("x-"))
        .or_else(|| name.rsplit_once('.').map(|(_, extension)| extension))
        .filter(|format| !format.is_empty())
        .map(str::to_uppercase);
    ImageFileDetails {
        name: name.to_string(),
        caption: caption.map(str::to_string),
        format,
        size_in_bytes: info.and_then(|info| info.size).map(u64::from).filter(|&size| size > 0),
    }
}

/// The result of the image viewer's request to fetch a full-size image.
#[derive(Clone, Debug)]
pub enum ImageViewerFetchAction {
    Loaded(Arc<[u8]>),
    Failed(ImageViewerError),
}

fn show_fetched_image_in_viewer(
    _destination: &Mutex<MediaCacheEntry>,
    request: MediaRequestParameters,
    data: matrix_sdk::Result<Vec<u8>>,
    _update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let action = match data {
        Ok(data) => ImageViewerFetchAction::Loaded(data.into()),
        Err(e) => ImageViewerFetchAction::Failed(
            match error_to_media_cache_entry(e, &request) {
                MediaCacheEntry::Failed(StatusCode::NOT_FOUND) => ImageViewerError::NotFound,
                MediaCacheEntry::Failed(StatusCode::INTERNAL_SERVER_ERROR) => ImageViewerError::ConnectionFailed,
                MediaCacheEntry::Failed(StatusCode::PARTIAL_CONTENT) => ImageViewerError::BadData,
                MediaCacheEntry::Failed(StatusCode::UNAUTHORIZED) => ImageViewerError::Unauthorized,
                _ => ImageViewerError::Unknown,
            }
        ),
    };
    Cx::post_action(action);
}
