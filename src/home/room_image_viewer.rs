use makepad_widgets::*;
use matrix_sdk_ui::timeline::EventTimelineItem;
use matrix_sdk::{
    media::MediaFormat,
    ruma::events::room::{message::MessageType, MediaSource},
};
use reqwest::StatusCode;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, shared::image_viewer::{ImageViewerAction, ImageViewerError, LoadState}};

/// Populates the image viewer modal with the given media content.
///
/// * If the media is already cached, it will be immediately displayed.
/// * If the media is not cached, it will be fetched from the server.
/// * If the media fetch fails, an error message will be displayed.
pub fn populate_matrix_image_modal(
    cx: &mut Cx,
    media_source: MediaSource,
    media_cache: &mut MediaCache,
) {
    let MediaSource::Plain(mxc_uri) = media_source else {
        return;
    };
    // Try to get media from cache or trigger fetch
    let media_entry = media_cache.try_get_media_or_fetch(mxc_uri.clone(), MediaFormat::File);

    // Handle the different media states
    match media_entry {
        (MediaCacheEntry::Loaded(data), MediaFormat::File) => {
            cx.action(ImageViewerAction::Show(LoadState::Loaded(data)));
        }
        (MediaCacheEntry::Failed(status_code), MediaFormat::File) => {
            let error = match status_code {
                StatusCode::NOT_FOUND => ImageViewerError::NotFound,
                StatusCode::INTERNAL_SERVER_ERROR => ImageViewerError::ConnectionFailed,
                StatusCode::PARTIAL_CONTENT => ImageViewerError::BadData,
                StatusCode::UNAUTHORIZED => ImageViewerError::Unauthorized,
                _ => ImageViewerError::Unknown,
            };
            cx.action(ImageViewerAction::Show(LoadState::Error(error)));
            // Remove failed media entry from cache for MediaFormat::File so as to start all over again from loading Thumbnail.
            media_cache.remove_cache_entry(&mxc_uri, Some(MediaFormat::File));
        }
        _ => {}
    }
}

/// Gets image name and file size in bytes from an event timeline item.
pub fn get_image_name_and_filesize(event_tl_item: &EventTimelineItem) -> (String, u64) {
    if let Some(message) = event_tl_item.content().as_message() {
        if let MessageType::Image(image_content) = message.msgtype() {
            let name = message.body().to_string();
            let size = image_content
                .info
                .as_ref()
                .and_then(|info| info.size)
                .map(u64::from)
                .unwrap_or(0);
            return (name, size);
        }
    }
    ("Unknown Image".to_string(), 0)
}
