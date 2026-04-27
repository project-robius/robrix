//! A cache of room previews keyed by room ID or alias.
//!
//! The cache currently just stores part of the room preview info:
//! the room's ID, display name, and avatar.
//!
//! Currently we treat cache entries as stale after 24 hours, but this
//! is a placeholder for proper invalidation based on subscribing to
//! updates for any rooms in the cache.

use crossbeam_queue::SegQueue;
use hashbrown::hash_map::{HashMap, RawEntryMut};
use makepad_widgets::{Cx, SignalToUI};
use matrix_sdk::OwnedServerName;
use ruma::{OwnedRoomOrAliasId, RoomOrAliasId};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use crate::{
    room::{FetchedRoomAvatar, FetchedRoomPreview},
    shared::avatar::AvatarState,
    sliding_sync::{submit_async_request, MatrixRequest, RoomPreviewResponseMode},
    utils::RoomNameId,
};

const CACHE_ENTRY_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

thread_local! {
    /// A cache of resolved room previews, indexed by room-or-alias ID.
    ///
    /// This cache must only be accessed by the main UI thread.
    static ROOM_PREVIEW_CACHE: RefCell<HashMap<OwnedRoomOrAliasId, CacheEntry>>
        = RefCell::new(HashMap::new());
}

struct CacheEntry {
    state: CacheEntryState,
    /// When this entry was inserted into the cache.
    loaded_at: Instant,
}

enum CacheEntryState {
    /// A fetch has been issued and we're waiting for it to complete.
    Requested,
    /// The room preview has been successfully loaded.
    Loaded {
        room_name_id: RoomNameId,
        room_avatar: AvatarState,
    },
}

/// A room preview entry as returned to the caller of [`get_or_fetch_room_preview`].
#[derive(Clone, Debug)]
pub enum CachedRoomPreview {
    /// The preview is loaded and ready to display.
    Loaded {
        room_name_id: RoomNameId,
        room_avatar: AvatarState,
    },
    /// A fetch is in flight; show a fallback while waiting.
    Requested,
}

/// An update sent from the matrix worker thread once a room preview fetch completes.
///
/// Carries the full [`FetchedRoomPreview`] so we can later widen what the
/// cache stores without changing the worker side.
pub struct RoomPreviewUpdate {
    pub room_or_alias_id: OwnedRoomOrAliasId,
    pub fetched: FetchedRoomPreview,
}

/// The queue of room preview updates waiting to be processed by the UI thread's event handler.
static PENDING_ROOM_PREVIEW_UPDATES: SegQueue<RoomPreviewUpdate> = SegQueue::new();

/// Enqueues a new room preview update and signals the UI that an update is available.
pub fn enqueue_room_preview_update(update: RoomPreviewUpdate) {
    PENDING_ROOM_PREVIEW_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// Processes all pending room preview updates in the queue.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn process_room_preview_updates(_cx: &mut Cx) {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        while let Some(update) = PENDING_ROOM_PREVIEW_UPDATES.pop() {
            let RoomPreviewUpdate { room_or_alias_id, fetched } = update;
            cache.insert(
                room_or_alias_id,
                CacheEntry {
                    state: CacheEntryState::Loaded {
                        room_name_id: fetched.room_name_id,
                        room_avatar: fetched_room_avatar_to_avatar_state(fetched.room_avatar),
                    },
                    loaded_at: Instant::now(),
                },
            );
        }
    });
}

/// Maps a [`FetchedRoomAvatar`] (which has already attempted to fetch any
/// avatar bytes on the worker side) into an [`AvatarState`] for the cache.
///
/// `Image(bytes)` becomes `Loaded(bytes)`. `Text(_)` becomes `Known(None)`:
/// at this layer we don't distinguish "no avatar set" from "avatar fetch
/// failed", since the consuming widget renders the same text fallback either way.
fn fetched_room_avatar_to_avatar_state(avatar: FetchedRoomAvatar) -> AvatarState {
    match avatar {
        FetchedRoomAvatar::Image(bytes) => AvatarState::Loaded(bytes),
        FetchedRoomAvatar::Text(_) => AvatarState::Known(None),
    }
}

/// Returns the cached preview for the given room ID or alias if it exists,
/// or submits a request to fetch it from the server if it isn't already cached.
///
/// If a request has already been submitted, it will not re-submit a duplicate request
/// and will simply return `CachedRoomPreview::Requested`. If the cached entry is
/// older than `CACHE_ENTRY_LIFETIME`, a fresh fetch is submitted and the entry is
/// reset to `Requested`.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_or_fetch_room_preview(
    _cx: &mut Cx,
    room_or_alias_id: &RoomOrAliasId,
    via: &[OwnedServerName],
) -> CachedRoomPreview {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        // raw_entry_mut lets us look up by `&RoomOrAliasId` without cloning;
        // we only allocate an owned key on insert.
        match cache.raw_entry_mut().from_key(room_or_alias_id) {
            RawEntryMut::Occupied(mut occupied) => {
                // Fast path: a fresh `Loaded` entry is returned as-is.
                if let CacheEntryState::Loaded { room_name_id, room_avatar } = &occupied.get().state {
                    if occupied.get().loaded_at.elapsed() < CACHE_ENTRY_LIFETIME {
                        return CachedRoomPreview::Loaded {
                            room_name_id: room_name_id.clone(),
                            room_avatar: room_avatar.clone(),
                        };
                    }
                }
                // Otherwise it's a stale `Loaded` (refetch and overwrite) or an
                // already-in-flight `Requested` (do nothing; prior fetch will land).
                if matches!(occupied.get().state, CacheEntryState::Loaded { .. }) {
                    submit_async_request(MatrixRequest::GetRoomPreview {
                        room_or_alias_id: room_or_alias_id.to_owned(),
                        via: via.to_vec(),
                        response_mode: RoomPreviewResponseMode::RoomPreviewCache,
                    });
                    occupied.insert(CacheEntry {
                        state: CacheEntryState::Requested,
                        loaded_at: Instant::now(),
                    });
                }
                CachedRoomPreview::Requested
            }
            RawEntryMut::Vacant(vacant) => {
                submit_async_request(MatrixRequest::GetRoomPreview {
                    room_or_alias_id: room_or_alias_id.to_owned(),
                    via: via.to_vec(),
                    response_mode: RoomPreviewResponseMode::RoomPreviewCache,
                });
                vacant.insert(
                    room_or_alias_id.to_owned(),
                    CacheEntry {
                        state: CacheEntryState::Requested,
                        loaded_at: Instant::now(),
                    },
                );
                CachedRoomPreview::Requested
            }
        }
    })
}

/// Removes all `Requested` entries from the room preview cache,
/// allowing them to be re-fetched.
///
/// This should be called when the app transitions from offline back to online,
/// because any in-flight requests that were submitted while offline have likely
/// failed, leaving stale entries that permanently block re-fetching.
pub fn clear_all_pending_requests() {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        cache.retain(|_, entry| !matches!(entry.state, CacheEntryState::Requested));
    });
}

/// Clears the room preview cache.
/// This function requires passing in a reference to `Cx`,
/// which acts as a guarantee that this thread-local cache is cleared on the main UI thread.
pub fn clear_room_preview_cache(_cx: &mut Cx) {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| cache.clear());
}
