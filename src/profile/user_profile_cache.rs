//! A cache of user profiles and room membership info, indexed by user ID.
//!
//! The cache is only accessible from the main UI thread.

use crossbeam_queue::SegQueue;
use makepad_widgets::{warning, Cx, SignalToUI};
use matrix_sdk::{room::RoomMember, ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId}};
use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}};

use crate::{profile::user_profile::AvatarState, sliding_sync::{submit_async_request, MatrixRequest}};

use super::user_profile::UserProfile;

thread_local! {
    /// A cache of each user's profile and the rooms they are a member of, indexed by user ID.
    ///
    /// To be of any use, this cache must only be accessed by the main UI thread.
    static USER_PROFILE_CACHE: RefCell<BTreeMap<OwnedUserId, UserProfileCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}
enum UserProfileCacheEntry {
    /// A request has been issued and we're waiting for it to complete.
    Requested,
    /// The profile has been successfully loaded from the server.
    Loaded {
        user_profile: UserProfile,
        rooms: BTreeMap<OwnedRoomId, RoomMember>,
    },
}

/// The queue of user profile updates waiting to be processed by the UI thread's event handler.
static PENDING_USER_PROFILE_UPDATES: SegQueue<UserProfileUpdate> = SegQueue::new();

/// Enqueues a new user profile update and signals the UI that an update is available.
pub fn enqueue_user_profile_update(update: UserProfileUpdate) {
    PENDING_USER_PROFILE_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// A user profile update, which can include changes to a user's full profile
/// and/or room membership info.
pub enum UserProfileUpdate {
    /// A fully-fetched user profile, with info about the user's membership in a given room.
    Full {
        new_profile: UserProfile,
        room_id: OwnedRoomId,
        room_member: RoomMember,
    },
    /// An update to the user's room membership info only, without any profile changes.
    RoomMemberOnly {
        room_id: OwnedRoomId,
        room_member: RoomMember,
    },
    /// An update to the user's profile only, without changes to room membership info.
    UserProfileOnly(UserProfile),
}
impl UserProfileUpdate {
    /// Returns the user ID associated with this update.
    #[allow(unused)]
    pub fn user_id(&self) -> &UserId {
        match self {
            UserProfileUpdate::Full { new_profile, .. } => &new_profile.user_id,
            UserProfileUpdate::RoomMemberOnly { room_member, .. } => room_member.user_id(),
            UserProfileUpdate::UserProfileOnly(profile) => &profile.user_id,
        }
    }

    /// Applies this update to the given user profile info cache.
    fn apply_to_cache(self, cache: &mut BTreeMap<OwnedUserId, UserProfileCacheEntry>) {
        match self {
            UserProfileUpdate::Full { new_profile, room_id, room_member } => {
                match cache.entry(new_profile.user_id.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        e @ UserProfileCacheEntry::Requested => {
                            *e = UserProfileCacheEntry::Loaded {
                                user_profile: new_profile,
                                rooms: {
                                    let mut room_members_map = BTreeMap::new();
                                    room_members_map.insert(room_id, room_member);
                                    room_members_map
                                },
                            };
                        }
                        UserProfileCacheEntry::Loaded { user_profile, rooms } => {
                            *user_profile = new_profile;
                            rooms.insert(room_id, room_member);
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry::Loaded {
                            user_profile: new_profile,
                            rooms: {
                                let mut room_members_map = BTreeMap::new();
                                room_members_map.insert(room_id, room_member);
                                room_members_map
                            },
                        });
                    }
                }
            }
            UserProfileUpdate::RoomMemberOnly { room_id, room_member } => {
                match cache.entry(room_member.user_id().to_owned()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        e @ UserProfileCacheEntry::Requested => {
                            // This shouldn't happen, but we can still technically handle it correctly.
                            warning!("BUG: User profile cache entry was `Requested` for user {} when handling RoomMemberOnly update", room_member.user_id());
                            *e = UserProfileCacheEntry::Loaded {
                                user_profile: UserProfile {
                                    user_id: room_member.user_id().to_owned(),
                                    username: None,
                                    avatar_state: AvatarState::Known(room_member.avatar_url().map(|url| url.to_owned())),
                                },
                                rooms: {
                                    let mut room_members_map = BTreeMap::new();
                                    room_members_map.insert(room_id, room_member);
                                    room_members_map
                                },
                            };
                        }
                        UserProfileCacheEntry::Loaded { rooms, .. } => {
                            rooms.insert(room_id, room_member);
                        }
                    }
                    Entry::Vacant(entry) => {
                        // This shouldn't happen, but we can still technically handle it correctly.
                        warning!("BUG: User profile cache entry not found for user {} when handling RoomMemberOnly update", room_member.user_id());
                        entry.insert(UserProfileCacheEntry::Loaded {
                            user_profile: UserProfile {
                                user_id: room_member.user_id().to_owned(),
                                username: None,
                                avatar_state: AvatarState::Known(room_member.avatar_url().map(|url| url.to_owned())),
                            },
                            rooms: {
                                let mut room_members_map = BTreeMap::new();
                                room_members_map.insert(room_id, room_member);
                                room_members_map
                            },
                        });
                    }
                }
            }
            UserProfileUpdate::UserProfileOnly(new_profile) => {
                match cache.entry(new_profile.user_id.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        e @ UserProfileCacheEntry::Requested => {
                            *e = UserProfileCacheEntry::Loaded {
                                user_profile: new_profile,
                                rooms: BTreeMap::new(),
                            };
                        }
                        UserProfileCacheEntry::Loaded { user_profile, .. } => {
                            *user_profile = new_profile;
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry::Loaded {
                            user_profile: new_profile,
                            rooms: BTreeMap::new(),
                        });
                    }
                }
            }
        }
    }
}

/// Processes all pending user profile updates in the queue.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn process_user_profile_updates(_cx: &mut Cx) {
    USER_PROFILE_CACHE.with_borrow_mut(|cache| {
        while let Some(update) = PENDING_USER_PROFILE_UPDATES.pop() {
            // Insert the updated info into the cache
            update.apply_to_cache(cache);
        }
    });
}

/// Invokes the given closure with cached user profile info for the given user ID
/// if it exists in the cache, otherwise does nothing.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn with_user_profile<F, R>(
    _cx: &mut Cx,
    user_id: OwnedUserId,
    fetch_if_missing: bool,
    f: F,
) -> Option<R>
where
    F: FnOnce(&UserProfile, &BTreeMap<OwnedRoomId, RoomMember>) -> R,
{
    USER_PROFILE_CACHE.with_borrow_mut(|cache|
        match cache.entry(user_id) {
            Entry::Occupied(entry) => match entry.get() {
                UserProfileCacheEntry::Loaded { user_profile, rooms } => {
                    Some(f(user_profile, rooms))
                }
                UserProfileCacheEntry::Requested => {
                    // log!("User {} profile request is already in flight....", entry.key());
                    None
                }
            }
            Entry::Vacant(entry) => {
                if fetch_if_missing {
                    // log!("Did not find User {} in cache, fetching from server.", entry.key());
                    // TODO: use the extra `via` parameters from `matrix_to_uri.via()`.
                    submit_async_request(MatrixRequest::GetUserProfile {
                        user_id: entry.key().clone(),
                        room_id: None,
                        local_only: false,
                    });
                    entry.insert(UserProfileCacheEntry::Requested);
                }
                None
            }
        }
    )
}


/// Returns a clone of the cached user profile for the given user ID
/// and a clone of that user's room member info for the given room ID,
/// only if both are found in the cache.
///
/// If either the `user_id` or `room_id` wasn't found in the cache,
/// and if `fetch_if_missing` is true, then this function will submit a request
/// to asynchronously fetch the user's room membership info from the server.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_user_profile_and_room_member(
    _cx: &mut Cx,
    user_id: OwnedUserId,
    room_id: &RoomId,
    fetch_if_missing: bool,
) -> (Option<UserProfile>, Option<RoomMember>) {
    USER_PROFILE_CACHE.with_borrow_mut(|cache|
        match cache.entry(user_id) {
            Entry::Occupied(entry) => match entry.get() {
                UserProfileCacheEntry::Loaded { user_profile, rooms } => {
                    (Some(user_profile.clone()), rooms.get(room_id).cloned())
                }
                UserProfileCacheEntry::Requested => {
                    // log!("User {} Room ID {room_id} room member info request is already in flight....", entry.key());
                    (None, None)
                }
            }
            Entry::Vacant(entry) => {
                if fetch_if_missing {
                    // log!("Did not find User {} Room ID {room_id} room member info in cache, fetching from server.", entry.key());
                    // TODO: use the extra `via` parameters from `matrix_to_uri.via()`.
                    submit_async_request(MatrixRequest::GetUserProfile {
                        user_id: entry.key().clone(),
                        room_id: Some(room_id.to_owned()),
                        local_only: false,
                    });
                    entry.insert(UserProfileCacheEntry::Requested);
                }
                (None, None)
            }
        }
    )
}

/// Clears cached user profile.
/// This function requires passing in a reference to `Cx`,
/// which acts as a guarantee that these thread-local caches are cleared on the main UI thread, 
pub fn clear_user_profile_cache(_cx: &mut Cx) {
    // Clear user profile cache
    USER_PROFILE_CACHE.with_borrow_mut(|cache| {
        cache.clear();
    });
}
