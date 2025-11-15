use makepad_widgets::*;
use matrix_sdk_ui::timeline::EventTimelineItem;
use matrix_sdk::{
    media::MediaFormat, ruma::{
        events::room::message::MessageType,
        OwnedMxcUri
    }
};
use reqwest::StatusCode;

use crate::{media_cache::{MediaCache, MediaCacheEntry}, shared::{avatar::{AvatarRef, AvatarWidgetRefExt}, image_viewer::{ImageViewerAction, ImageViewerError, LoadState}}};

/// Populates the image viewer modal with the given media content.
///
/// * If the media is already cached, it will be immediately displayed.
/// * If the media is not cached, it will be fetched from the server.
/// * If the media fetch fails, an error message will be displayed.
pub fn populate_matrix_image_modal(
    cx: &mut Cx,
    mxc_uri: OwnedMxcUri,
    media_cache: &mut MediaCache,
) {
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
                StatusCode::REQUEST_TIMEOUT => ImageViewerError::Timeout,
                _ => ImageViewerError::Unknown,
            };
            cx.action(ImageViewerAction::Show(LoadState::Error(error)));
            // Remove failed media entry from cache for MediaFormat::File so as to start all over again from loading Thumbnail.
            media_cache.remove_cache_entry(&mxc_uri, Some(MediaFormat::File));
        }
        _ => {}
    }
}

/// Extracts image name and size from an event timeline item.
pub fn extract_image_info(event_tl_item: &EventTimelineItem) -> (String, i32) {
    if let Some(message) = event_tl_item.content().as_message() {
        if let MessageType::Image(image_content) = message.msgtype() {
            let name = message.body().to_string();
            let size = image_content
                .info
                .as_ref()
                .and_then(|info| info.size)
                .map(|s| i32::try_from(s).unwrap_or_default())
                .unwrap_or(0);
            (name, size)
        } else {
            ("Unknown Image".to_string(), 0)
        }
    } else {
        ("Unknown Image".to_string(), 0)
    }
}

/// Condensed message does not have a profile, so we need to find the previous portal list item.
/// Searches backwards for a non-empty display name and avatar in previous portal list items.
/// Mutates display_name and avatar_ref.
/// Returns when first non-empty display name is found.
pub fn find_previous_profile_in_condensed_message(
    portal_list: &PortalListRef,
    mut current_index: usize,
    display_name: &mut String,
    avatar_ref: &mut AvatarRef,
) {
    // Start from the current index and work backwards
    while current_index > 0 {
        current_index -= 1;
        if let Some((_id, item_ref)) = portal_list.get_item(current_index) {
            let username = item_ref.label(ids!(content.username_view.username)).text();
            if !username.is_empty() {
                *display_name = username;
                *avatar_ref = item_ref.avatar(ids!(profile.avatar));
                return;
            }
        }
    }
}
