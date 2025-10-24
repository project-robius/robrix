use std::sync::Arc;
use makepad_widgets::Cx;
use matrix_sdk::ruma::OwnedRoomId;

pub mod reply_preview;
pub mod room_input_bar;
pub mod room_display_filter;
pub mod typing_notice;
pub mod member_search;

pub fn live_design(cx: &mut Cx) {
    reply_preview::live_design(cx);
    room_input_bar::live_design(cx);
    typing_notice::live_design(cx);
}

/// Basic details about a room, used for displaying a preview of it.
#[derive(Clone, Debug)]
pub struct BasicRoomDetails {
    pub room_id: OwnedRoomId,
    pub room_name: Option<String>,
    pub room_avatar: RoomPreviewAvatar,
}

#[derive(Clone)]
pub enum RoomPreviewAvatar {
    Text(String),
    Image(Arc<[u8]>),
}
impl Default for RoomPreviewAvatar {
    fn default() -> Self {
        RoomPreviewAvatar::Text(String::from("?"))
    }
}
impl std::fmt::Debug for RoomPreviewAvatar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomPreviewAvatar::Text(text) => f.debug_tuple("Text").field(text).finish(),
            RoomPreviewAvatar::Image(_) => f.debug_tuple("Image").finish(),
        }
    }
}
