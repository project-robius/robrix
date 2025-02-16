use matrix_sdk::ruma::OwnedRoomId;
use std::sync::Arc;
use matrix_sdk_ui::timeline::TimelineItem;

/// Search result updates sent from async worker to UI.
pub enum SearchUpdate {
    // A new batch of search results has been received.
    NewResults {
        room_id: OwnedRoomId,
        results: Vec<Arc<TimelineItem>>, // List of matching search results
        next_batch: Option<String>,      // Token for pagination
    },
}
