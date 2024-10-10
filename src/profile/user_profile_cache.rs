use crossbeam_queue::SegQueue;
use makepad_widgets::{log, warning, Cx, SignalToUI};
use matrix_sdk::{room::RoomMember, ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId}};
use matrix_sdk_ui::timeline::Profile;
use std::{cell::RefCell, collections::{btree_map::Entry, BTreeMap}};

use crate::profile::user_profile::AvatarState;

use super::user_profile::{UserProfile, UserProfilePaneInfo};

thread_local! {
    /// A cache of each user's profile and the rooms they are a member of, indexed by user ID.
    ///
    /// To be of any use, this cache must only be accessed by the main UI thread.
    static USER_PROFILE_CACHE: RefCell<BTreeMap<OwnedUserId, UserProfileCacheEntry>> = RefCell::new(BTreeMap::new());
}
struct UserProfileCacheEntry {
    user_profile: UserProfile,
    room_members: BTreeMap<OwnedRoomId, RoomMember>,
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
            UserProfileUpdate::RoomMemberOnly { room_member, .. } => &room_member.user_id(),
            UserProfileUpdate::UserProfileOnly(profile) => &profile.user_id,
        }
    }

    /// Applies this update to the given user profile pane info,
    /// only if the user_id and room_id match that of the update.
    ///
    /// Returns `true` if the update resulted in any actual content changes.
    #[allow(unused)]
    fn apply_to_current_pane(&self, info: &mut UserProfilePaneInfo) -> bool {
        match self {
            UserProfileUpdate::Full { new_profile, room_id, room_member } => {
                if info.user_id == new_profile.user_id {
                    info.profile_and_room_id.user_profile = new_profile.clone();
                    if &info.room_id == room_id {
                        info.room_member = Some(room_member.clone());
                    }
                    return true;
                }
            }
            UserProfileUpdate::RoomMemberOnly { room_id, room_member } => {
                log!("Applying RoomMemberOnly update to user profile pane info: user_id={}, room_id={}, ignored={}",
                    room_member.user_id(), room_id, room_member.is_ignored(),
                );
                if info.user_id == room_member.user_id() && &info.room_id == room_id {
                    log!("RoomMemberOnly update matches user profile pane info, updating room member.");
                    info.room_member = Some(room_member.clone());
                    return true;
                }
            }
            UserProfileUpdate::UserProfileOnly(new_profile) => {
                if info.user_id == new_profile.user_id {
                    info.profile_and_room_id.user_profile = new_profile.clone();
                    return true;
                }
            }
        }
        false
    }

    /// Applies this update to the given user profile info cache.
    fn apply_to_cache(self, cache: &mut BTreeMap<OwnedUserId, UserProfileCacheEntry>) {
        match self {
            UserProfileUpdate::Full { new_profile, room_id, room_member } => {
                match cache.entry(new_profile.user_id.to_owned()) {
                    Entry::Occupied(mut entry) => {
                        let entry_mut = entry.get_mut();
                        entry_mut.user_profile = new_profile;
                        entry_mut.room_members.insert(room_id, room_member);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry {
                            user_profile: new_profile,
                            room_members: {
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
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().room_members.insert(room_id, room_member);
                    }
                    Entry::Vacant(entry) => {
                        // This shouldn't happen, but we can still technically handle it correctly.
                        warning!("BUG: User profile cache entry not found for user {} when handling RoomMemberOnly update", room_member.user_id());
                        entry.insert(UserProfileCacheEntry {
                            user_profile: UserProfile {
                                user_id: room_member.user_id().to_owned(),
                                username: None,
                                avatar_state: AvatarState::Known(room_member.avatar_url().map(|url| url.to_owned())),
                            },
                            room_members: {
                                let mut room_members_map = BTreeMap::new();
                                room_members_map.insert(room_id, room_member);
                                room_members_map
                            },
                        });
                    }
                }
            }
            UserProfileUpdate::UserProfileOnly(new_profile) => {
                match cache.entry(new_profile.user_id.to_owned()) {
                    Entry::Occupied(mut entry) => entry.get_mut().user_profile = new_profile,
                    Entry::Vacant(entry) => {
                        entry.insert(UserProfileCacheEntry {
                            user_profile: new_profile,
                            room_members: BTreeMap::new(),
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
#[allow(unused)]
pub fn with_user_profile<F, R>(_cx: &mut Cx, user_id: &UserId, f: F) -> Option<R>
where
    F: FnOnce(&UserProfile, &BTreeMap<OwnedRoomId, RoomMember>) -> R,
{
    USER_PROFILE_CACHE.with_borrow(|cache| {
        if let Some(entry) = cache.get(user_id) {
            Some(f(&entry.user_profile, &entry.room_members))
        } else {
            None
        }
    })
}

/// Returns a clone of the cached user profile for the given user ID, if it exists.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
#[allow(unused)]
pub fn get_user_profile(_cx: &mut Cx, user_id: &UserId) -> Option<UserProfile> {
    USER_PROFILE_CACHE.with_borrow(|cache| {
        cache.get(user_id).map(|entry| entry.user_profile.clone())
    })
}
/// Set user profile into cache only if cache does not have user profile
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
#[allow(unused)]
pub fn set_user_profile(
    _cx: &mut Cx,
    user_id: &UserId,
    username: String,
    avatar_state: AvatarState,
) {
    USER_PROFILE_CACHE.with_borrow_mut(|cache| {
        cache
            .entry(user_id.to_owned())
            .or_insert(UserProfileCacheEntry {
                user_profile: UserProfile {
                    user_id: user_id.to_owned(),
                    username: Some(username),
                    avatar_state,
                },
                room_members: BTreeMap::new(),
            });
    });
}

/// Returns a clone of the cached user profile for the given user ID
/// and a clone of that user's room member info for the given room ID.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_user_profile_and_room_member(
    _cx: &mut Cx,
    user_id: &UserId,
    room_id: &RoomId,
) -> (Option<UserProfile>, Option<RoomMember>) {
    USER_PROFILE_CACHE.with_borrow(|cache|
        if let Some(entry) = cache.get(user_id) {
            (
                Some(entry.user_profile.clone()),
                entry.room_members.get(room_id).cloned(),
            )
        } else {
            (None, None)
        }
    )
}

/// Returns a clone of the cached room member info for the given user ID and room ID,
/// only if both are found in the cache.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_user_room_member_info(
    _cx: &mut Cx,
    user_id: &UserId,
    room_id: &RoomId,
) -> Option<RoomMember> {
    USER_PROFILE_CACHE.with_borrow(|cache|
        cache.get(user_id)
            .and_then(|entry| entry.room_members.get(room_id).cloned())
    )
}
