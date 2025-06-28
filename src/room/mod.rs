use std::sync::Arc;
use makepad_widgets::{Cx, WidgetUid};
use matrix_sdk::{ruma::{OwnedRoomAliasId, OwnedRoomId}, OwnedServerName};

pub mod room_input_bar;
pub mod room_member_manager;
pub mod room_display_filter;

pub fn live_design(cx: &mut Cx) {
    room_input_bar::live_design(cx)
}

/// Actions sent from the backend task as a result of a [`MatrixRequest::ResolveRoomAlias`].
#[derive(Debug)]
pub enum ResolveRoomAliasAction {
    Resolved {
        requester_uid: WidgetUid,
        room_alias: OwnedRoomAliasId,
        room_id: OwnedRoomId,
        servers: Vec<OwnedServerName>,
    },
    Failed {
        requester_uid: WidgetUid,
        room_alias: OwnedRoomAliasId,
        error: matrix_sdk::Error,
    }
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
        RoomPreviewAvatar::Text(String::new())
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
