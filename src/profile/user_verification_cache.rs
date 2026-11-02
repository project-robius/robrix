//! A cache of whether we've verified other users, indexed by user ID.
//!
//! The cache is only accessible from the main UI thread.

use crossbeam_queue::SegQueue;
use makepad_widgets::{Cx, SignalToUI};
use matrix_sdk::{encryption::identities::UserIdentity, ruma::{OwnedRoomId, OwnedUserId, UserId}};
use std::{cell::RefCell, collections::BTreeMap};

use crate::{home::rooms_list::RoomsListRef, sliding_sync::{submit_async_request, MatrixRequest}};

thread_local! {
    /// Whether we've verified each user that the UI has asked about.
    /// The key is a user ID,
    /// and the value is that user's verification state, or `None` while it's being fetched.
    static USER_VERIFICATION_CACHE: RefCell<BTreeMap<OwnedUserId, Option<UserVerificationState>>> = const { RefCell::new(BTreeMap::new()) };
}

/// Whether we've verified a user's cryptographic identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserVerificationState {
    /// We haven't verified this user, or they haven't set up cross-signing.
    Unverified,
    Verified,
    /// We verified this user before, but they've since reset their identity.
    VerificationViolation,
}
impl From<&UserIdentity> for UserVerificationState {
    fn from(identity: &UserIdentity) -> Self {
        if identity.is_verified() {
            Self::Verified
        } else if identity.has_verification_violation() {
            Self::VerificationViolation
        } else {
            Self::Unverified
        }
    }
}

/// Returns the given user's verification state if it's cached,
/// otherwise fetches it in the background and returns `None`.
pub fn get_or_fetch_user_verification_state(_cx: &mut Cx, user_id: &UserId) -> Option<UserVerificationState> {
    USER_VERIFICATION_CACHE.with_borrow_mut(|cache| {
        if let Some(state) = cache.get(user_id) {
            return *state;
        }
        cache.insert(user_id.to_owned(), None);
        submit_async_request(MatrixRequest::GetUserVerificationState { user_id: user_id.to_owned() });
        None
    })
}

/// Returns the verification state of the other user in the given direct room, fetching it if needed.
///
/// Returns `None` if that room isn't a direct room with just one other user, or if their state isn't known yet.
pub fn get_or_fetch_dm_user_verification_state(cx: &mut Cx, room_id: &OwnedRoomId) -> Option<UserVerificationState> {
    let dm_user_id = cx.has_global::<RoomsListRef>()
        .then(|| cx.get_global::<RoomsListRef>().get_dm_user_id(room_id))
        .flatten()?;
    get_or_fetch_user_verification_state(cx, &dm_user_id)
}

/// The queue of verification state updates waiting to be processed by the UI thread's event handler.
static PENDING_USER_VERIFICATION_UPDATES: SegQueue<UserVerificationUpdate> = SegQueue::new();

/// Enqueues a new verification state update and signals the UI that an update is available.
pub fn enqueue_user_verification_update(update: UserVerificationUpdate) {
    PENDING_USER_VERIFICATION_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

pub enum UserVerificationUpdate {
    /// The latest verification states of these users.
    States(Vec<(OwnedUserId, UserVerificationState)>),
    /// Our own identity changed, which can change whether we've verified anyone else.
    OwnIdentityChanged,
}

/// Emitted after users' verification states were updated in the cache,
/// so that every widget showing those users can redraw them.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub struct UserVerificationStatesUpdated;

/// Processes all pending verification state updates,
/// emitting [`UserVerificationStatesUpdated`] if there were any.
///
/// The App calls this upon every Signal, before any widget handles that Signal.
pub fn process_user_verification_updates(cx: &mut Cx) {
    let mut has_updates = false;
    USER_VERIFICATION_CACHE.with_borrow_mut(|cache| {
        while let Some(update) = PENDING_USER_VERIFICATION_UPDATES.pop() {
            has_updates = true;
            match update {
                UserVerificationUpdate::States(states) => cache.extend(
                    states.into_iter().map(|(user_id, state)| (user_id, Some(state)))
                ),
                // Any cached state could be stale now, so we re-fetch each one when it's shown again.
                UserVerificationUpdate::OwnIdentityChanged => cache.clear(),
            }
        }
    });
    if has_updates {
        cx.action(UserVerificationStatesUpdated);
    }
}

/// Clears the cache of users' verification states.
pub fn clear_user_verification_cache(_cx: &mut Cx) {
    USER_VERIFICATION_CACHE.with_borrow_mut(|cache| cache.clear());
}
