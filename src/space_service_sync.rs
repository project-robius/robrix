//! Background tasks that subscribe to the Matrix SpaceService in order to
//! track changes to the user's joined spaces and send updates the UI.

use std::{collections::{HashMap, HashSet, hash_map::Entry}, iter::Peekable, sync::Arc};
use eyeball_im::VectorDiff;
use futures_util::StreamExt;
use imbl::Vector;
use makepad_widgets::*;
use matrix_sdk::{Client, RoomState, media::MediaRequestParameters};
use matrix_sdk_ui::spaces::{SpaceRoom, SpaceRoomList, SpaceService, room_list::SpaceRoomListPaginationState};
use ruma::{OwnedMxcUri, OwnedRoomId, events::room::MediaSource};
use tokio::{runtime::Handle, sync::mpsc::{UnboundedReceiver, UnboundedSender}, task::JoinHandle};
use crate::{home::{rooms_list::{RoomsListUpdate, enqueue_rooms_list_update}, spaces_bar::{JoinedSpaceInfo, SpacesListUpdate, enqueue_spaces_list_update}}, room::FetchedRoomAvatar, utils};

/// Whether to enable verbose logging of all spaces service diff updates.
const LOG_SPACE_SERVICE_DIFFS: bool = cfg!(feature = "log_space_service_diffs");

/// Requests related to obtaining info about Spaces, via the background space service.
pub enum SpaceRequest {
    /// Start obtaining the list of rooms in the given space from the homeserver,
    /// and listen for ongoing updates to that list.
    SubscribeToSpaceRoomList {
        space_id: OwnedRoomId,
    },
    /// Stop listening to updates for the list of rooms in the given space.
    ///
    /// Note: the Matrix SDK offers no way to unsubscribe from a space room list,
    /// so this just stops the async background task that runs the subscriber loop.
    UnsubscribeFromSpaceRoomList {
        space_id: OwnedRoomId,
    },
    /// Paginate the given space's room list, i.e., fetch the next batch of rooms in the list.
    PaginateSpaceRoomList {
        space_id: OwnedRoomId,
    },
    /// Get a copy of the currently-known children (rooms and spaces) in the given space.
    GetChildren {
        space_id: OwnedRoomId,
    }
}

/// Internal requests sent from the [`space_service_loop`] to a specific space's [`space_room_list_loop`].
enum SpaceRoomListRequest {
    /// Get a copy of the currently-known chil.ren (rooms & spaces) in this space.
    GetChildren,
    /// Paginate this space to get info about more of its children.
    Paginate,
    Shutdown,
}


/// The main async loop task that listens for changes to all spaces.
pub async fn space_service_loop(space_service: SpaceService, client: Client) -> anyhow::Result<()> {
    // Create a channel for sending space-related requests from the main UI thread to this background worker.
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<SpaceRequest>();
    // Give the request sender channel endpoint to the RoomsList widget.
    enqueue_rooms_list_update(RoomsListUpdate::SpaceRequestSender(sender));

    // The set of async tasks that are handling room list requests for each space,
    // along with a sender to send `SpaceRoomListRequest`s to those tasks.
    let mut space_room_list_tasks = HashMap::new();
    // A closure to make it easier to use/spawn a `space_room_list_loop` task.
    let get_or_spawn_space_room_list = async |
        space_room_list_tasks: &mut HashMap<OwnedRoomId, (UnboundedSender<SpaceRoomListRequest>, JoinHandle<()>)>,
        space_id: &OwnedRoomId,
    | -> UnboundedSender<SpaceRoomListRequest> {
        match space_room_list_tasks.entry(space_id.clone()) {
            Entry::Occupied(occ) => occ.get().0.clone(),
            Entry::Vacant(vac) => {
                let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<SpaceRoomListRequest>();
                let srl = space_service.space_room_list(space_id.clone()).await;
                let join_handle = Handle::current().spawn(space_room_list_loop(space_id.clone(), receiver, srl));
                vac.insert((sender, join_handle))
                    .0.clone()
            }
        }
    };

    // Get the set of top-level (root) spaces that the user has joined.
    let (initial_spaces, mut spaces_diff_stream) = space_service.subscribe_to_joined_spaces().await;
    for space in &initial_spaces {
        add_new_space(space, &client).await;
    }
    let mut all_joined_spaces: Vector<SpaceRoom> = initial_spaces;
    if LOG_SPACE_SERVICE_DIFFS { log!("space_service: initial set: {all_joined_spaces:?}"); }


    loop { tokio::select! {
        // Handle new space requests.
        request_opt = receiver.recv() => {
            let Some(request) = request_opt else { break };
            match request {
                SpaceRequest::GetChildren { space_id } => {
                    let sender = get_or_spawn_space_room_list(&mut space_room_list_tasks, &space_id).await;
                    if sender.send(SpaceRoomListRequest::GetChildren).is_err() {
                        error!("BUG: failed to send GetRooms request to space room list loop for space {space_id}");
                    }
                }
                SpaceRequest::SubscribeToSpaceRoomList { space_id } => {
                    let _sender = get_or_spawn_space_room_list(&mut space_room_list_tasks, &space_id).await;
                }
                SpaceRequest::PaginateSpaceRoomList { space_id } => {
                    let sender = get_or_spawn_space_room_list(&mut space_room_list_tasks, &space_id).await;
                    if sender.send(SpaceRoomListRequest::Paginate).is_err() {
                        error!("BUG: failed to send paginate request to space room list loop for space {space_id}");
                    }
                }
                SpaceRequest::UnsubscribeFromSpaceRoomList { space_id } => {
                    if let Some((sender, join_handle)) = space_room_list_tasks.remove(&space_id) {
                        let _ = sender.send(SpaceRoomListRequest::Shutdown);
                        let _ = join_handle.await;
                    }
                }
            }
        }

        // Handle updates to the list of spaces.
        batch_opt = spaces_diff_stream.next() => {
            let Some(batch) = batch_opt else { break };
            let mut peekable_diffs = batch.into_iter().peekable();
            while let Some(diff) = peekable_diffs.next() {
                match diff {
                    VectorDiff::Append { values: new_spaces } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Append {}", new_spaces.len()); }
                        for new_space in new_spaces {
                            add_new_space(&new_space, &client).await;
                            all_joined_spaces.push_back(new_space);
                        }
                    }
                    VectorDiff::Clear => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Clear"); }
                        all_joined_spaces.clear();
                        enqueue_spaces_list_update(SpacesListUpdate::ClearSpaces);
                    }
                    VectorDiff::PushFront { value: new_space } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff PushFront"); }
                        add_new_space(&new_space, &client).await;
                        all_joined_spaces.push_front(new_space);
                    }
                    VectorDiff::PushBack { value: new_space } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff PushBack"); }
                        add_new_space(&new_space, &client).await;
                        all_joined_spaces.push_back(new_space);
                    }
                    remove_diff @ VectorDiff::PopFront => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff PopFront"); }
                        if let Some(space) = all_joined_spaces.pop_front() {
                            optimize_remove_then_add_into_update(
                                remove_diff,
                                space,
                                &mut peekable_diffs,
                                &mut all_joined_spaces,
                                &client,
                            ).await;
                        }
                    }
                    remove_diff @ VectorDiff::PopBack => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff PopBack"); }
                        if let Some(space) = all_joined_spaces.pop_back() {
                            optimize_remove_then_add_into_update(
                                remove_diff,
                                space,
                                &mut peekable_diffs,
                                &mut all_joined_spaces,
                                &client,
                            ).await;
                        }
                    }
                    VectorDiff::Insert { index, value: new_space } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Insert at {index}"); }
                        add_new_space(&new_space, &client).await;
                        all_joined_spaces.insert(index, new_space);
                    }
                    VectorDiff::Set { index, value: changed_space } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Set at {index}"); }
                        if let Some(old_space) = all_joined_spaces.get(index) {
                            update_space(old_space, &changed_space, &client).await;
                        } else {
                            error!("BUG: space_service diff: Set index {index} was out of bounds.");
                        }
                        all_joined_spaces.set(index, changed_space);
                    }
                    remove_diff @ VectorDiff::Remove { index: remove_index } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Remove at {remove_index}"); }
                        if remove_index < all_joined_spaces.len() {
                            let space = all_joined_spaces.remove(remove_index);
                            optimize_remove_then_add_into_update(
                                remove_diff,
                                space,
                                &mut peekable_diffs,
                                &mut all_joined_spaces,
                                &client,
                            ).await;
                        } else {
                            error!("BUG: space_service: diff Remove index {remove_index} out of bounds, len {}", all_joined_spaces.len());
                        }
                    }
                    VectorDiff::Truncate { length } => {
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Truncate to {length}"); }
                        // Iterate manually so we can know which spaces are being removed.
                        while all_joined_spaces.len() > length {
                            if let Some(space) = all_joined_spaces.pop_back() {
                                remove_space(&space);
                            }
                        }
                        all_joined_spaces.truncate(length); // sanity check
                    }
                    VectorDiff::Reset { values: new_spaces } => {
                        // We implement this by clearing all spaces and then adding back the new values.
                        if LOG_SPACE_SERVICE_DIFFS { log!("space_service: diff Reset, old length {}, new length {}", all_joined_spaces.len(), new_spaces.len()); }
                        // Iterate manually so we can know which spaces are being removed.
                        while let Some(space) = all_joined_spaces.pop_back() {
                            remove_space(&space);
                        }
                        enqueue_spaces_list_update(SpacesListUpdate::ClearSpaces);
                        for new_space in &new_spaces {
                            add_new_space(new_space, &client).await;
                        }
                        all_joined_spaces = new_spaces;
                    }
                }
            }
            if LOG_SPACE_SERVICE_DIFFS { log!("space_service: after batch diff: {all_joined_spaces:?}"); }
        }

        else => {
            break;
        }
    } }

    anyhow::bail!("Space service sync loop ended unexpectedly")
}


async fn add_new_space(space: &SpaceRoom, client: &Client) {
    let space_avatar_opt = if let Some(url) = &space.avatar_url {
        fetch_space_avatar(url.clone(), client)
            .await
            .inspect_err(|e| error!("Failed to fetch avatar for new space {:?} ({}): {e}", space.display_name, space.room_id))
            .ok()
    } else { None };
    let space_avatar = space_avatar_opt.unwrap_or_else(
        || utils::avatar_from_room_name(Some(&space.display_name))
    );

    let jsi = JoinedSpaceInfo {
        space_id: space.room_id.clone(),
        canonical_alias: space.canonical_alias.clone(),
        display_name: space.display_name.clone(),
        topic: space.topic.clone(),
        space_avatar,
        num_joined_members: space.num_joined_members,
        join_rule: space.join_rule.clone(),
        world_readable: space.world_readable,
        guest_can_join: space.guest_can_join,
        children_count: space.children_count,
    };
    enqueue_spaces_list_update(SpacesListUpdate::AddJoinedSpace(jsi));
}


/// Attempts to optimize a common SpaceService operation of remove + add.
///
/// If a `Remove` diff (or `PopBack` or `PopFront`) is immediately followed by
/// an `Insert` diff (or `PushFront` or `PushBack`) for the same space,
/// we can treat it as a simple `Set` operation, in which we call `update_space()`.
/// This is much more efficient than removing the space and then adding it back.
///
/// This tends to happen frequently in order to change the space's state
/// or to "sort" the space list by changing its positional order.
async fn optimize_remove_then_add_into_update(
    remove_diff: VectorDiff<SpaceRoom>,
    space: SpaceRoom,
    peekable_diffs: &mut Peekable<impl Iterator<Item = VectorDiff<SpaceRoom>>>,
    all_joined_spaces: &mut Vector<SpaceRoom>,
    client: &Client,
) {
    let next_diff_was_handled: bool;
    match peekable_diffs.peek() {
        Some(VectorDiff::Insert { index: insert_index, value: new_space })
            if space.room_id == new_space.room_id =>
        {
            if LOG_SPACE_SERVICE_DIFFS {
                log!("Optimizing {remove_diff:?} + Insert({insert_index}) into Update for space {}", space.room_id);
            }
            update_space(&space, new_space, client).await;
            all_joined_spaces.insert(*insert_index, new_space.clone());
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushFront { value: new_space })
            if space.room_id == new_space.room_id =>
        {
            if LOG_SPACE_SERVICE_DIFFS {
                log!("Optimizing {remove_diff:?} + PushFront into Update for space {}", space.room_id);
            }
            update_space(&space, new_space, client).await;
            all_joined_spaces.push_front(new_space.clone());
            next_diff_was_handled = true;
        }
        Some(VectorDiff::PushBack { value: new_space })
            if space.room_id == new_space.room_id =>
        {
            if LOG_SPACE_SERVICE_DIFFS {
                log!("Optimizing {remove_diff:?} + PushBack into Update for space {}", space.room_id);
            }
            update_space(&space, new_space, client).await;
            all_joined_spaces.push_back(new_space.clone());
            next_diff_was_handled = true;
        }
        _ => next_diff_was_handled = false,
    }
    if next_diff_was_handled {
        peekable_diffs.next(); // consume the next diff
    } else {
        remove_space(&space);
    }
}


/// Invoked when the space service has received an update that changes an existing space.
async fn update_space(
    old_space: &SpaceRoom,
    new_space: &SpaceRoom,
    client: &Client,
) {
    let new_space_id = new_space.room_id.clone();
    if old_space.room_id == new_space_id {
        // Handle state transitions for a space.
        if LOG_SPACE_SERVICE_DIFFS {
            log!("Space {:?} ({new_space_id}) state went from {:?} --> {:?}", new_space.display_name, old_space.state, new_space.state);
        }
        if old_space.state != new_space.state {
            match new_space.state {
                Some(RoomState::Banned) => {
                    // TODO: handle spaces that this user has been banned from.
                    log!("Removing Banned space: {:?} ({new_space_id})", new_space.display_name);
                    remove_space(new_space);
                    return;
                }
                Some(RoomState::Left) => {
                    log!("Removing Left space: {:?} ({new_space_id})", new_space.display_name);
                    // TODO: instead of removing this, we could optionally add it to
                    //       a separate list of left space, which would be collapsed by default.
                    //       Upon clicking a left space, we could show a splash page
                    //       that prompts the user to rejoin the space or forget it permanently.
                    //       Currently, we just remove it and do not show left spaces at all.
                    remove_space(new_space);
                    return;
                }
                Some(RoomState::Joined) => {
                    log!("update_space(): adding new Joined space: {:?} ({new_space_id})", new_space.display_name);
                    add_new_space(new_space, client).await;
                    return;
                }
                Some(RoomState::Invited) => {
                    log!("update_space(): adding new Invited space: {:?} ({new_space_id})", new_space.display_name);
                    add_new_space(new_space, client).await;
                    return;
                }
                Some(RoomState::Knocked) => {
                    // TODO: handle Knocked spaces (e.g., can you re-knock? or cancel a prior knock?)
                    return;
                }
                None => {
                    error!("WARNING: UNTESTED: new space {} ({}) RoomState is None", new_space.display_name, new_space.room_id);
                }
            }
        }

        if old_space.canonical_alias != new_space.canonical_alias {
            log!("Updating space {} alias: {:?} --> {:?}", new_space_id, old_space.canonical_alias, new_space.canonical_alias);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateCanonicalAlias {
                space_id: new_space_id.clone(),
                new_canonical_alias: new_space.canonical_alias.clone(),
            });
        }

        if old_space.display_name != new_space.display_name {
            log!("Updating space {} name: {:?} --> {:?}", new_space_id, old_space.display_name, new_space.display_name);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateSpaceName {
                space_id: new_space_id.clone(),
                new_space_name: new_space.display_name.clone(),
            });
        }

        if old_space.topic != new_space.topic {
            log!("Updating space {} topic:\n    {:?}\n  -->\n    {:?}", new_space_id, old_space.topic, new_space.topic);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateSpaceTopic {
                space_id: new_space_id.clone(),
                topic: new_space.topic.clone(),
            });
        }

        // Here, we need to check each of the space's states to determine what has changed.
        if old_space.avatar_url != new_space.avatar_url {
            log!("Updating avatar for space {}", new_space_id);
            let space_id = new_space_id.clone();
            let space_display_name = new_space.display_name.clone();
            let url_opt = new_space.avatar_url.clone();
            let client2 = client.clone();
            // Spawn a new task to fetch the space's new avatar in the background.
            Handle::current().spawn(async move {
                let space_avatar_opt = if let Some(url) = url_opt {
                    fetch_space_avatar(url, &client2)
                        .await
                        .inspect_err(|e| error!("Failed to fetch avatar for space {:?} ({}): {e}", space_display_name, space_id))
                        .ok()
                } else { None };
                let avatar = space_avatar_opt.unwrap_or_else(
                    || utils::avatar_from_room_name(Some(&space_display_name))
                );
                enqueue_spaces_list_update(SpacesListUpdate::UpdateSpaceAvatar { space_id, avatar });
            });
        }

        if old_space.num_joined_members != new_space.num_joined_members {
            log!("Updating space {} joined members: {} --> {}", new_space_id, old_space.num_joined_members, new_space.num_joined_members);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateNumJoinedMembers {
                space_id: new_space_id.clone(),
                num_joined_members: new_space.num_joined_members,
            });
        }

        if old_space.join_rule != new_space.join_rule {
            log!("Updating space {} join rule: {:?} --> {:?}", new_space_id, old_space.join_rule, new_space.join_rule);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateJoinRule {
                space_id: new_space_id.clone(),
                join_rule: new_space.join_rule.clone(),
            });
        }

        if old_space.world_readable != new_space.world_readable {
            log!("Updating space {} world readable: {:?} --> {:?}", new_space_id, old_space.world_readable, new_space.world_readable);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateWorldReadable {
                space_id: new_space_id.clone(),
                world_readable: new_space.world_readable,
            });
        }

        if old_space.guest_can_join != new_space.guest_can_join {
            log!("Updating space {} guest can join: {:?} --> {:?}", new_space_id, old_space.guest_can_join, new_space.guest_can_join);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateGuestCanJoin {
                space_id: new_space_id.clone(),
                guest_can_join: new_space.guest_can_join,
            });
        }

        if old_space.children_count != new_space.children_count {
            log!("Updating space {} children count: {:?} --> {:?}", new_space_id, old_space.children_count, new_space.children_count);
            enqueue_spaces_list_update(SpacesListUpdate::UpdateChildrenCount {
                space_id: new_space_id.clone(),
                children_count: new_space.children_count,
            });
        }
    }
    else {
        warning!("UNTESTED SCENARIO: update_space(): removing old room {}, replacing with new room {}",
            old_space.room_id, new_space_id,
        );
        remove_space(old_space);
        add_new_space(new_space, client).await;
    }
}


/// Invoked when the space service has received an update to remove an existing space.
fn remove_space(space: &SpaceRoom) {
    enqueue_spaces_list_update(SpacesListUpdate::RemoveSpace {
        space_id: space.room_id.clone(),
        new_state: space.state,
    });
}


/// Fetches the avatar for the space at the given URL.
///
/// Returns `Some` if the avatar image was successfully fetched.
async fn fetch_space_avatar(url: OwnedMxcUri, client: &Client) -> matrix_sdk::Result<FetchedRoomAvatar> {
    let request = MediaRequestParameters {
        source: MediaSource::Plain(url),
        format: utils::AVATAR_THUMBNAIL_FORMAT.into(),
    };
    client.media()
        .get_media_content(&request, true)
        .await
        .map(|img_data| FetchedRoomAvatar::Image(img_data.into()))
}



/// A loop that listens for changes to the set of rooms in a given space.
async fn space_room_list_loop(
    space_id: OwnedRoomId,
    mut receiver: UnboundedReceiver<SpaceRoomListRequest>,
    space_room_list: SpaceRoomList,
) {
    // Define a closure that calls `paginate()` and handles broadcasting the result.
    let paginate_once = async || match space_room_list.paginate().await {
        Ok(()) => {
            Cx::post_action(SpaceRoomListAction::PaginationState {
                space_id: space_id.clone(),
                state: space_room_list.pagination_state(),
            });
        }
        Err(error) => Cx::post_action(SpaceRoomListAction::PaginationError {
            space_id: space_id.clone(),
            error,
        }),
    };

    /// An inner function that creates a HashSet from the set of all rooms in this space.
    fn to_hash_set(all_rooms_in_space: &Vector<SpaceRoom>) -> Arc<HashSet<OwnedRoomId>> {
        Arc::new(HashSet::from_iter(all_rooms_in_space.iter().map(|c| c.room_id.clone())))
    }


    // First, we paginate the space once to get at least *some* child rooms.    
    paginate_once().await;

    let (mut all_rooms_in_space, mut room_stream) = space_room_list.subscribe_to_room_updates();

    loop { tokio::select! {
        // Handle new requests.
        request_opt = receiver.recv() => {
            let Some(request) = request_opt else { break };
            match request {
                SpaceRoomListRequest::GetChildren => {
                    Cx::post_action(SpaceRoomListAction::UpdatedChildren {
                        space_id: space_id.clone(),
                        direct_children: to_hash_set(&all_rooms_in_space),
                    });
                }
                SpaceRoomListRequest::Paginate => paginate_once().await,
                SpaceRoomListRequest::Shutdown => return,
            }
        }

        // Handle updates to the list of rooms in this space.
        batch_opt = room_stream.next() => {
            let Some(batch) = batch_opt else { break };
            for diff in batch {
                diff.apply(&mut all_rooms_in_space);
            }
            Cx::post_action(SpaceRoomListAction::UpdatedChildren {
                space_id: space_id.clone(),
                direct_children: to_hash_set(&all_rooms_in_space),
            });
        }
    } }
}


/// Actions emitted from the SpaceRoomList for a given space.
pub enum SpaceRoomListAction {
    /// The list of rooms & spaces that are direct children of the given space has changed.
    UpdatedChildren {
        space_id: OwnedRoomId,
        /// The *direct* children (both rooms and nested spaces) of this space.
        direct_children: Arc<HashSet<OwnedRoomId>>,
    },
    /// The state of the background pagination process that was fetching the list
    /// of rooms in the given space has changed.
    PaginationState {
        space_id: OwnedRoomId,
        state: SpaceRoomListPaginationState,
    },
    /// There was an error in the background pagination process that was fetching
    /// the list of rooms in the given space.
    PaginationError {
        space_id: OwnedRoomId,
        error: matrix_sdk::Error,
    },
}
impl std::fmt::Debug for SpaceRoomListAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpaceRoomListAction::UpdatedChildren { space_id, direct_children } => {
                f.debug_struct("SpaceRoomListAction::UpdatedChildren")
                    .field("space_id", space_id)
                    .field("direct_children_len", &direct_children.len())
                    .finish()
            }
            SpaceRoomListAction::PaginationState { space_id, state } => {
                f.debug_struct("SpaceRoomListAction::PaginationState")
                    .field("space_id", space_id)
                    .field("state", state)
                    .finish()
            }
            SpaceRoomListAction::PaginationError { space_id, error } => {
                f.debug_struct("SpaceRoomListAction::PaginationError")
                    .field("space_id", space_id)
                    .field("error", error)
                    .finish()
            }
        }
    }
}
