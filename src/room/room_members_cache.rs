//! A cache of room members, indexed by room ID.
//!
//! Similar to user_profile_cache, this cache is only accessible from the main UI thread.

use crossbeam_queue::SegQueue;
use makepad_widgets::{log, Cx, SignalToUI};
use matrix_sdk::{room::RoomMember, ruma::OwnedRoomId};
use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

thread_local! {
    static ROOM_MEMBERS_CACHE: RefCell<BTreeMap<OwnedRoomId, RoomMembersCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}

enum RoomMembersCacheEntry {
    Requested,
    Loaded(Vec<RoomMember>),
}

static PENDING_ROOM_MEMBERS_UPDATES: SegQueue<RoomMembersUpdate> = SegQueue::new();

pub struct RoomMembersUpdate {
    pub room_id: OwnedRoomId,
    pub members: Vec<RoomMember>,
}

pub fn enqueue_room_members_update(update: RoomMembersUpdate) {
    PENDING_ROOM_MEMBERS_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

pub fn process_room_members_updates(_cx: &mut Cx) {
    log!("Processing room members updates...");
    ROOM_MEMBERS_CACHE.with_borrow_mut(|cache| {
        let mut processed = 0;
        while let Some(update) = PENDING_ROOM_MEMBERS_UPDATES.pop() {
            processed += 1;
            log!("Processing update {} - room {} with {} members",
                processed, update.room_id, update.members.len());
            match cache.entry(update.room_id) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = RoomMembersCacheEntry::Loaded(update.members);
                }
                Entry::Vacant(entry) => {
                    entry.insert(RoomMembersCacheEntry::Loaded(update.members));
                }
            }
        }
        if processed > 0 {
            log!("Processed {} room members updates", processed);
        }
    });
}

pub fn get_room_members(
    _cx: &mut Cx,
    room_id: OwnedRoomId,
    fetch_if_missing: bool,
) -> Option<Vec<RoomMember>> {
    ROOM_MEMBERS_CACHE.with_borrow_mut(|cache|
        match cache.entry(room_id) {
            Entry::Occupied(entry) => match entry.get() {
                RoomMembersCacheEntry::Loaded(members) => Some(members.clone()),
                RoomMembersCacheEntry::Requested => None,
            }
            Entry::Vacant(entry) => {
                if fetch_if_missing {
                    log!("Room members not found in cache for room {}, fetching from server.", entry.key());
                    submit_async_request(MatrixRequest::FetchRoomMembers {
                        room_id: entry.key().clone(),
                    });
                    entry.insert(RoomMembersCacheEntry::Requested);
                }
                None
            }
        }
    )
}
