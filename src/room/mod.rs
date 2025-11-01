use std::{ops::Deref, sync::Arc};
use makepad_widgets::Cx;
use matrix_sdk::{room_preview::RoomPreview, ruma::OwnedRoomId, RoomDisplayName, SuccessorRoom};

use crate::utils::avatar_from_room_name;

pub mod reply_preview;
pub mod room_input_bar;
pub mod room_display_filter;
pub mod typing_notice;

pub fn live_design(cx: &mut Cx) {
    reply_preview::live_design(cx);
    room_input_bar::live_design(cx);
    typing_notice::live_design(cx);
}

/// Basic details needed to display a brief summary of a room.
///
/// You can construct this manually, but it also can be created from a
/// [`SuccessorRoom`] or a [`FetchedRoomPreview`].
#[derive(Clone, Debug)]
pub struct BasicRoomDetails {
    pub room_id: OwnedRoomId,
    pub room_name: RoomDisplayName,
    pub room_avatar: FetchedRoomAvatar,
}
impl From<&SuccessorRoom> for BasicRoomDetails {
    fn from(successor_room: &SuccessorRoom) -> Self {
        BasicRoomDetails {
            room_id: successor_room.room_id.clone(),
            room_avatar: avatar_from_room_name(None),
            room_name: RoomDisplayName::Empty,
        }
    }
}
impl From<&FetchedRoomPreview> for BasicRoomDetails {
    fn from(frp: &FetchedRoomPreview) -> Self {
        let room_name = frp.name.clone()
            .map(RoomDisplayName::Named)
            .unwrap_or(RoomDisplayName::Empty);
        BasicRoomDetails {
            room_id: frp.room_id.clone(),
            room_name,
            room_avatar: frp.room_avatar.clone(),
        }
    }
}


/// Actions related to room previews being fetched.
#[derive(Debug)]
pub enum RoomPreviewAction {
    Fetched(Result<FetchedRoomPreview, matrix_sdk::Error>),
}

/// A [`RoomPreview`] from the Matrix SDK, plus the room's fetched avatar.
#[derive(Debug)]
pub struct FetchedRoomPreview {
    pub room_preview: RoomPreview,
    pub room_avatar: FetchedRoomAvatar,
}
impl Deref for FetchedRoomPreview {
    type Target = RoomPreview;
    fn deref(&self) -> &Self::Target {
        &self.room_preview
    }
}

/// A fully-fetched room avatar ready to be displayed.
#[derive(Clone)]
pub enum FetchedRoomAvatar {
    Text(String),
    Image(Arc<[u8]>),
}
impl Default for FetchedRoomAvatar {
    fn default() -> Self {
        FetchedRoomAvatar::Text(String::from("?"))
    }
}
impl std::fmt::Debug for FetchedRoomAvatar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchedRoomAvatar::Text(text) => f.debug_tuple("Text").field(text).finish(),
            FetchedRoomAvatar::Image(_) => f.debug_tuple("Image").finish(),
        }
    }
}
