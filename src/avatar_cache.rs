use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}, sync::Arc};
use crossbeam_queue::SegQueue;
use makepad_widgets::{Cx, SignalToUI};
use matrix_sdk::ruma::OwnedMxcUri;

use crate::sliding_sync::{submit_async_request, MatrixRequest};


thread_local! {
    /// A cache of Avatar images, indexed by Matrix URI.
    ///
    /// To be of any use, this cache must only be accessed by the main UI thread.
    static AVATAR_NEW_CACHE: RefCell<BTreeMap<OwnedMxcUri, AvatarCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}

/// An entry in the avatar cache.
#[derive(Clone)]
pub enum AvatarCacheEntry {
    Loaded(Arc<[u8]>),
    Requested,
    Failed,
}

pub struct AvatarUpdate {
    pub mxc_uri: OwnedMxcUri,
    pub avatar_data: Result<Arc<[u8]>, matrix_sdk::Error>,
}

/// The queue of avatar updates waiting to be processed by the UI thread's event handler.
static PENDING_AVATAR_UPDATES: SegQueue<AvatarUpdate> = SegQueue::new();

/// Enqueues a new avatar update and signals the UI
/// such that the new update will be handled by the avatar sliding pane widget.
fn enqueue_avatar_update(update: AvatarUpdate) {
    PENDING_AVATAR_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// Processes all pending avatar updates in the queue.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn process_avatar_updates(_cx: &mut Cx) {
    AVATAR_NEW_CACHE.with_borrow_mut(|cache| {
        while let Some(update) = PENDING_AVATAR_UPDATES.pop() {
            cache.insert(
                update.mxc_uri,
                match update.avatar_data {
                    Ok(data) => AvatarCacheEntry::Loaded(data),
                    Err(_e) => AvatarCacheEntry::Failed,
                },
            );
        }
    });
}

/// Returns the cached avatar for the given Matrix URI if it exists,
/// or submits a request to fetch it from the server if it isn't already cached.
///
/// If a request has already been submitted, it will not re-submit a duplicate request
/// and will simply return `AvatarCacheEntry::Requested`.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_or_fetch_avatar(
    _cx: &mut Cx,
    mxc_uri: OwnedMxcUri,
) -> AvatarCacheEntry {
    AVATAR_NEW_CACHE.with_borrow_mut(|cache| {
        match cache.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                vacant.insert(AvatarCacheEntry::Requested);
            },
            Entry::Occupied(occupied) => return occupied.get().clone(),
        }
        submit_async_request(MatrixRequest::FetchAvatar {
            mxc_uri,
            on_fetched: enqueue_avatar_update,
        });
        AvatarCacheEntry::Requested
    })
}

/// Clears cached avatars.
/// This function requires passing in a reference to `Cx`,
/// which acts as a guarantee that this function must only be called by the main UI thread.
pub fn clear_avatar_cache(_cx: &mut Cx) {
    AVATAR_NEW_CACHE.with_borrow_mut(|cache| {
        cache.clear();
    });
}
