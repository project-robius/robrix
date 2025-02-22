use std::{cell::RefCell, collections::BTreeMap};

use crossbeam_queue::SegQueue;
use makepad_widgets::Cx;
use matrix_sdk::{room_preview::RoomPreview, ruma::OwnedRoomOrAliasId, OwnedServerName};

use crate::sliding_sync::{submit_async_request, MatrixRequest};

thread_local! {
    static ROOM_PREVIEW_CACHE: RefCell<BTreeMap<OwnedRoomOrAliasId, RoomPreviewCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}

enum RoomPreviewCacheEntry {
    Requested,
    Loaded(RoomPreview),
}

pub struct RoomPreviewUpdate {
    pub room_id: OwnedRoomOrAliasId,
    pub room_preview: RoomPreview,
}

static PENDING_AVATAR_UPDATES: SegQueue<RoomPreviewUpdate> = SegQueue::new();

pub fn enqueue_room_preview_update(update: RoomPreviewUpdate) {
    PENDING_AVATAR_UPDATES.push(update);
}

pub fn process_room_preview_updates(_cx: &mut Cx) {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        while let Some(update) = PENDING_AVATAR_UPDATES.pop() {
            cache.insert(
                update.room_id.clone(),
                RoomPreviewCacheEntry::Loaded(update.room_preview),
            );
        }
    });
}

pub fn with_room_preview<F, R>(
    _cx: &mut Cx,
    room_or_alias_id: OwnedRoomOrAliasId,
    via: Vec<OwnedServerName>,
    fetch_if_missing: bool,
    f: F,
) -> Option<R>
where
    F: FnOnce(&RoomPreview) -> R,
{
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        match cache.get(&room_or_alias_id) {
            Some(RoomPreviewCacheEntry::Loaded(preview)) => Some(f(preview)),
            Some(RoomPreviewCacheEntry::Requested) => None,
            None => {
                if fetch_if_missing {
                    cache.insert(room_or_alias_id.clone(), RoomPreviewCacheEntry::Requested);
                    submit_async_request(MatrixRequest::GetRoomPreview {
                        room_or_alias_id,
                        via,
                    });
                }
                None
            }
        }
    })
}