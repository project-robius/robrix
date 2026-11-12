//! A cache of the current user's membership in rooms.
//!
//! The rooms list only has the rooms we're in or invited to, so a room that's missing from it
//! could be still loading or gone for good. This cache tells a screen which one it is,
//! e.g., that we left a room or got kicked or banned from it.
//!
//! The backend sends an update whenever our membership in any room changes,
//! and whenever [`get_or_fetch_room_membership()`] asks about a room that isn't cached.

use std::{cell::RefCell, collections::HashMap};

use crossbeam_queue::SegQueue;
use makepad_widgets::{Cx, SignalToUI};
use matrix_sdk::{OwnedServerName, RoomState, ruma::{OwnedRoomId, OwnedUserId, RoomId}};

use crate::sliding_sync::{MatrixRequest, submit_async_request};

/// The current user's membership in a room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomMembership {
    pub state: RoomState,
    pub is_space: bool,
    /// Who kicked or banned us from this room, if anyone did.
    pub removed_by: Option<RemovedBy>,
    /// The reason they gave for kicking or banning us.
    pub reason: Option<String>,
    /// How we could get back into this room.
    pub join_method: JoinMethod,
    /// Servers that are likely in this room, which we need if our homeserver isn't in it anymore.
    pub via: Vec<OwnedServerName>,
}

/// The user who kicked or banned the current user from a room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedBy {
    pub user_id: OwnedUserId,
    pub display_name: Option<String>,
}

impl std::fmt::Display for RemovedBy {
    /// Shows their name and user ID, or just their user ID if we don't know their name.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.display_name {
            Some(name) => write!(f, "{name} ({})", self.user_id),
            None => write!(f, "{}", self.user_id),
        }
    }
}

/// How the current user can join a room they aren't in, based on its join rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinMethod {
    /// We can join right away.
    Join,
    /// We can knock, i.e., ask to be invited.
    Knock,
    /// We need an invite, or to be in another room that this room's join rule allows.
    InviteOrOtherRoom,
    /// We need an invite.
    Invite,
}

/// An update sent from the backend with our current membership in a room.
pub struct RoomMembershipUpdate {
    pub room_id: OwnedRoomId,
    /// `None` if our client doesn't know this room at all.
    pub membership: Option<RoomMembership>,
}

/// An action emitted when our cached membership in the given room has changed.
///
/// This is *NOT* a widget action.
#[derive(Clone, Debug)]
pub struct RoomMembershipChanged {
    pub room_id: OwnedRoomId,
}

thread_local! {
    /// Our membership in each room that changed or that we asked about.
    /// A `None` value means we asked but haven't been told anything yet.
    ///
    /// This must only be accessed by the main UI thread.
    static ROOM_MEMBERSHIPS: RefCell<HashMap<OwnedRoomId, Option<RoomMembership>>> = RefCell::new(HashMap::new());
}

static PENDING_MEMBERSHIP_UPDATES: SegQueue<RoomMembershipUpdate> = SegQueue::new();

/// Enqueues a new room membership update and signals the UI to handle it.
pub fn enqueue_room_membership_update(update: RoomMembershipUpdate) {
    PENDING_MEMBERSHIP_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// Applies all pending room membership updates to the cache,
/// and emits a [`RoomMembershipChanged`] action for each one that changed something.
///
/// The App calls this upon every Signal, before any widget handles that Signal.
pub fn process_room_membership_updates(cx: &mut Cx) {
    while let Some(RoomMembershipUpdate { room_id, membership }) = PENDING_MEMBERSHIP_UPDATES.pop() {
        let previous = ROOM_MEMBERSHIPS.with_borrow_mut(|cache| cache.insert(room_id.clone(), membership.clone()));
        if previous.flatten() != membership {
            cx.action(RoomMembershipChanged { room_id });
        }
    }
}

/// Returns our cached membership in the given room, if we know it.
///
/// The `Cx` isn't used, it just ensures that only the main UI thread can call this.
pub fn get_room_membership(_cx: &mut Cx, room_id: &RoomId) -> Option<RoomMembership> {
    ROOM_MEMBERSHIPS.with_borrow(|cache| cache.get(room_id).cloned().flatten())
}

/// Returns our cached membership in the given room, or asks the backend for it if it isn't cached.
///
/// A [`RoomMembershipChanged`] action is emitted once the backend's answer arrives.
pub fn get_or_fetch_room_membership(_cx: &mut Cx, room_id: &OwnedRoomId) -> Option<RoomMembership> {
    ROOM_MEMBERSHIPS.with_borrow_mut(|cache| {
        if let Some(cached) = cache.get(room_id) {
            return cached.clone();
        }
        cache.insert(room_id.clone(), None);
        submit_async_request(MatrixRequest::GetRoomMembership { room_id: room_id.clone() });
        None
    })
}

/// Clears the cache and any pending updates, e.g., upon logout.
pub fn clear_room_membership_cache(_cx: &mut Cx) {
    while PENDING_MEMBERSHIP_UPDATES.pop().is_some() { }
    ROOM_MEMBERSHIPS.with_borrow_mut(HashMap::clear);
}
