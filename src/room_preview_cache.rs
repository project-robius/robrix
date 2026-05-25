//! A cache of resolved room previews, keyed on the room ID or alias used to
//! reach the room.
//!
//! The cache stores a thinned-down [`FetchedRoomPreview`] — currently just
//! the room's [`RoomNameId`] (ID + display name) and an [`AvatarState`] —
//! which is the subset needed by `RobrixHtmlLink` widgets to draw room /
//! alias / event link pills without re-querying the server each time. All
//! three of `MatrixId::Room`, `MatrixId::RoomAlias`, and `MatrixId::Event`
//! resolve to room-level metadata via the same `client.get_room_preview()`
//! call, so they share entries here. User-link pills are served by the user
//! profile cache and don't go through this cache.
//!
//! Entries are good for 24 hours; on read, expired `Loaded` entries are
//! refetched. The cache is only accessible from the main UI thread.

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

/// How long a `Loaded` entry stays valid before being refetched on next read.
const CACHE_ENTRY_TTL: Duration = Duration::from_secs(24 * 60 * 60);

thread_local! {
    /// A cache of resolved room previews, indexed by room-or-alias ID.
    ///
    /// To be of any use, this cache must only be accessed by the main UI thread.
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

/// What the caller of [`get_or_fetch_room_preview`] learns about a room.
#[derive(Clone, Debug)]
pub enum CachedRoomPreview {
    /// Resolved info; safe to display the pill with this name and avatar.
    Loaded {
        room_name_id: RoomNameId,
        room_avatar: AvatarState,
    },
    /// A fetch is in flight; the widget should show a fallback in the meantime.
    Requested,
}

/// An update sent from the matrix worker thread once a room preview fetch completes.
///
/// Carries the full [`FetchedRoomPreview`] for forward-compatibility; the
/// cache thins it down to [`RoomNameId`] + [`AvatarState`] on insert.
pub struct RoomPreviewUpdate {
    pub room_or_alias_id: OwnedRoomOrAliasId,
    pub fetched: FetchedRoomPreview,
}

/// The queue of room preview updates waiting to be processed by the UI thread.
static PENDING_ROOM_PREVIEW_UPDATES: SegQueue<RoomPreviewUpdate> = SegQueue::new();

/// Enqueues a new room preview update and signals the UI that an update is available.
pub fn enqueue_room_preview_update(update: RoomPreviewUpdate) {
    PENDING_ROOM_PREVIEW_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// Drains all pending room preview updates into the cache.
///
/// This function requires passing in a reference to `Cx`, which isn't used,
/// but acts as a guarantee that this function must only be called by the
/// main UI thread.
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

/// Maps a [`FetchedRoomAvatar`] (the form returned by the worker, which has
/// already attempted to fetch any avatar bytes) into an [`AvatarState`] for
/// the cache. `Image(bytes)` becomes `Loaded(bytes)`; `Text(_)` becomes
/// `Known(None)` — at this layer we don't distinguish "no avatar set" from
/// "avatar fetch failed", since the consuming widget renders the same
/// text fallback either way.
fn fetched_room_avatar_to_avatar_state(avatar: FetchedRoomAvatar) -> AvatarState {
    match avatar {
        FetchedRoomAvatar::Image(bytes) => AvatarState::Loaded(bytes),
        FetchedRoomAvatar::Text(_) => AvatarState::Known(None),
    }
}

/// Returns the cached preview for the given room link, or kicks off a fetch
/// if absent or expired.
///
/// This function requires passing in a reference to `Cx`, which isn't used,
/// but acts as a guarantee that this function must only be called by the
/// main UI thread.
pub fn get_or_fetch_room_preview(
    _cx: &mut Cx,
    room_or_alias_id: &RoomOrAliasId,
    via: &[OwnedServerName],
) -> CachedRoomPreview {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        // `raw_entry_mut().from_key(&borrowed)` looks up by reference, so we
        // only allocate an owned key on insert (vacant or stale-overwrite).
        match cache.raw_entry_mut().from_key(room_or_alias_id) {
            RawEntryMut::Occupied(mut occupied) => {
                // Fast path: a fresh `Loaded` entry is returned as-is.
                if let CacheEntryState::Loaded { room_name_id, room_avatar } = &occupied.get().state {
                    if occupied.get().loaded_at.elapsed() < CACHE_ENTRY_TTL {
                        return CachedRoomPreview::Loaded {
                            room_name_id: room_name_id.clone(),
                            room_avatar: room_avatar.clone(),
                        };
                    }
                }
                // Otherwise: a stale `Loaded` (refetch + overwrite) or an
                // already in-flight `Requested` (no-op; prior fetch will land).
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

/// Removes all `Requested` entries so they can be re-fetched after a network
/// recovery. Mirrors the same affordance on `user_profile_cache` and
/// `avatar_cache`: in-flight requests submitted while offline likely failed
/// silently, leaving stale `Requested` entries that would otherwise block
/// re-fetching forever.
pub fn clear_all_pending_requests() {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| {
        cache.retain(|_, entry| !matches!(entry.state, CacheEntryState::Requested));
    });
}

/// Clears the entire cache. Called on logout.
///
/// This function requires passing in a reference to `Cx`, which acts as a
/// guarantee that this thread-local cache is cleared on the main UI thread.
pub fn clear_room_preview_cache(_cx: &mut Cx) {
    ROOM_PREVIEW_CACHE.with_borrow_mut(|cache| cache.clear());
}
