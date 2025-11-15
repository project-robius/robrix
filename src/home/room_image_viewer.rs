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

/// Gets image name and file size in bytes from an event timeline item.
pub fn get_image_name_and_filesize(event_tl_item: &EventTimelineItem) -> (String, u64) {
    if let Some(message) = event_tl_item.content().as_message() {
        if let MessageType::Image(image_content) = message.msgtype() {
            let name = message.body().to_string();
            let size = image_content
                .info
                .as_ref()
                .and_then(|info| info.size)
                .map(|s| u64::try_from(s).unwrap_or_default())
                .unwrap_or(0);
            return (name, size);
        }
    }
    ("Unknown Image".to_string(), 0)
}

/// Finds the most recent non-empty profile in a condensed message by searching backwards.
///
/// Condensed messages don't have their own profile, so this function searches previous
/// portal list items to find the most recent non-empty display name and avatar.
///
/// The search starts from `current_index - 1` and moves backwards through the portal list.
/// Stops and returns when the first non-empty display name is found.
///
/// # Mutates
///
/// * `display_name` - Updated with the found non-empty display name
/// * `avatar_ref` - Updated with the corresponding avatar reference
///
/// # Parameters
///
/// * `portal_list` - Reference to the portal list to search through
/// * `current_index` - Starting index (searches backwards from this position)
/// * `display_name` - Output parameter for the found display name
/// * `avatar_ref` - Output parameter for the found avatar reference
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
