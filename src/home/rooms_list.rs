//! The `RoomsList` widget displays a filterable list of rooms
//! that can be clicked on to open the room timeline (`RoomScreen`).
//!
//! It is responsible for receiving room-related updates from the background task
//! that runs the room list service.
//! It also receives space-related updates from the background task that runs
//! the space sync service(s).
//!
//! Generally, it maintains several key states:
//! * The set of all joined rooms, which is displayed separately as "direct" rooms
//!   and non-direct regular rooms.
//! * The set of invited rooms, which have not yet been joined.
//! * The map of spaces and their child rooms and nested subspaces.
//!
//! This widget is a global singleton and is thus accessible via `Cx::get_global()`,
//! so you can use it from other widgets or functions on the main UI thread
//! that need to query basic info about a particular room or space.

use std::{cell::RefCell, collections::{HashMap, HashSet, hash_map::Entry}, rc::Rc, sync::Arc};
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState;
use tokio::sync::mpsc::UnboundedSender;
use matrix_sdk::{RoomState, ruma::{events::tag::Tags, MilliSecondsSinceUnixEpoch, OwnedRoomAliasId, OwnedRoomId, OwnedUserId}};
use crate::{
    app::{AppState, SelectedRoom},
    home::{
        navigation_tab_bar::{NavigationBarAction, SelectedTab},
        space_lobby::{SpaceLobbyAction, SpaceLobbyEntryWidgetExt},
    },
    room::{
        FetchedRoomAvatar,
        room_display_filter::{RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria, SortFn},
    },
    shared::{
        collapsible_header::{CollapsibleHeaderAction, CollapsibleHeaderWidgetRefExt, HeaderCategory},
        jump_to_bottom_button::UnreadMessageCount,
        popup_list::{PopupItem, PopupKind, enqueue_popup_notification},
        room_filter_input_bar::RoomFilterAction,
    },
    sliding_sync::{MatrixRequest, PaginationDirection, submit_async_request},
    space_service_sync::{ParentChain, SpaceRequest, SpaceRoomListAction}, utils::RoomNameId,
};
use super::rooms_list_entry::RoomsListEntryAction;

/// Whether to pre-paginate visible rooms at least once in order to
/// be able to display the latest message in a room's RoomsListEntry,
/// and to have something to immediately show when a user first opens a room.
const PREPAGINATE_VISIBLE_ROOMS: bool = true;

thread_local! {
    /// The list of all invited rooms, which is only tracked here
    /// because the backend doesn't need to track any info about them.
    ///
    /// This must only be accessed by the main UI thread.
    static ALL_INVITED_ROOMS: Rc<RefCell<HashMap<OwnedRoomId, InvitedRoomInfo>>> = Rc::new(RefCell::new(HashMap::new()));
}

/// Returns a reference to the list of all invited rooms.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn get_invited_rooms(_cx: &mut Cx) -> Rc<RefCell<HashMap<OwnedRoomId, InvitedRoomInfo>>> {
    ALL_INVITED_ROOMS.with(Rc::clone)
}

/// Clears all invited rooms
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn clear_all_invited_rooms(_cx: &mut Cx) {
    ALL_INVITED_ROOMS.with(|rooms| {
       rooms.borrow_mut().clear();
    });
}


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::html_or_plaintext::HtmlOrPlaintext;
    use crate::shared::collapsible_header::*;
    use crate::home::rooms_list_entry::*;
    use crate::home::space_lobby::*;

    StatusLabel = <View> {
        width: Fill, height: Fit,
        flow: Right,
        align: { x: 0.5, y: 0.5 }
        padding: 15.0,

        loading_spinner = <LoadingSpinner> {
            visible: false,
            width: 20,
            height: 20,
            draw_bg: {
                color: (COLOR_ACTIVE_PRIMARY)
                border_size: 3.0,
            }
        }

        label = <Label> {
            padding: 0
            width: Fill,
            flow: RightWrap,
            align: { x: 0.5, y: 0.5 }
            draw_text: {
                wrap: Word,
                color: (MESSAGE_TEXT_COLOR),
                text_style: <REGULAR_TEXT>{}
            }
            text: "Loading rooms..."
        }
    }

    pub RoomsList = {{RoomsList}} {
        width: Fill, height: Fill
        flow: Down
        cursor: Default,

        space_lobby_entry = <SpaceLobbyEntry> {}

        list = <PortalList> {
            keep_invisible: false,
            auto_tail: false,
            width: Fill, height: Fill
            flow: Down,
            spacing: 0.0

            collapsible_header = <CollapsibleHeader> {}
            rooms_list_entry = <RoomsListEntry> {}
            empty = <View> {}
            status_label = <StatusLabel> {}
            bottom_filler = <View> {
                width: Fill,
                height: 100.0,
            }
        }
    }
}


/// The possible updates that should be displayed by the single list of all rooms.
///
/// These updates are enqueued by the `enqueue_rooms_list_update` function
/// (which is called from background async tasks that receive updates from the matrix server),
/// and then dequeued by the `RoomsList` widget's `handle_event` function.
pub enum RoomsListUpdate {
    /// No rooms have been loaded yet.
    NotLoaded,
    /// Some rooms were loaded, and the server optionally told us
    /// the max number of rooms that will ever be loaded.
    LoadedRooms{ max_rooms: Option<u32> },
    /// Add a new room to the list of rooms the user has been invited to.
    /// This will be maintained and displayed separately from joined rooms.
    AddInvitedRoom(InvitedRoomInfo),
    /// Add a new room to the list of all rooms that the user has joined.
    AddJoinedRoom(JoinedRoomInfo),
    /// Clear all rooms in the list of all rooms.
    ClearRooms,
    /// Update the latest event content and timestamp for the given room.
    UpdateLatestEvent {
        room_id: OwnedRoomId,
        timestamp: MilliSecondsSinceUnixEpoch,
        /// The Html-formatted text preview of the latest message.
        latest_message_text: String,
    },
    /// Update the number of unread messages and mentions for the given room.
    UpdateNumUnreadMessages {
        room_id: OwnedRoomId,
        unread_messages: UnreadMessageCount,
        unread_mentions: u64,
    },
    /// Update the displayable name for the given room.
    UpdateRoomName {
        new_room_name: RoomNameId,
    },
    /// Update the avatar (image) for the given room.
    UpdateRoomAvatar {
        room_id: OwnedRoomId,
        room_avatar: FetchedRoomAvatar,
    },
    /// Update whether the given room is a direct room.
    UpdateIsDirect {
        room_id: OwnedRoomId,
        is_direct: bool,
    },
    /// Remove the given room from the rooms list
    RemoveRoom {
        room_id: OwnedRoomId,
        /// The new state of the room (which caused its removal).
        new_state: RoomState,
    },
    /// Update the tags for the given room.
    Tags {
        room_id: OwnedRoomId,
        new_tags: Tags,
    },
    /// Update the status label at the bottom of the list of all rooms.
    Status {
        status: String,
    },
    /// Mark the given room as tombstoned.
    TombstonedRoom {
        room_id: OwnedRoomId
    },
    /// Hide the given room from being displayed.
    ///
    /// This is useful for temporarily preventing a room from being shown,
    /// e.g., after a room has been left but before the homeserver has registered
    /// that we left it and removed it via the RoomListService.
    HideRoom {
        room_id: OwnedRoomId,
    },
    /// Scroll to the given room.
    ScrollToRoom(OwnedRoomId),
    /// The background space service is now listening for requests,
    /// and the sender-side channel endpoint is included.
    SpaceRequestSender(UnboundedSender<SpaceRequest>),
}

static PENDING_ROOM_UPDATES: SegQueue<RoomsListUpdate> = SegQueue::new();

/// Enqueue a new room update for the list of all rooms
/// and signals the UI that a new update is available to be handled.
pub fn enqueue_rooms_list_update(update: RoomsListUpdate) {
    PENDING_ROOM_UPDATES.push(update);
    SignalToUI::set_ui_signal();
}

/// Actions emitted by the RoomsList widget.
#[derive(Debug, Clone, DefaultNone)]
pub enum RoomsListAction {
    /// A new room or space was selected.
    Selected(SelectedRoom),
    /// A new room was joined from an accepted invite,
    /// meaning that the existing `InviteScreen` should be converted
    /// to a `RoomScreen` to display now-joined room.
    InviteAccepted {
        room_name_id: RoomNameId,
    },
    None,
}


/// UI-related info about a joined room.
///
/// This includes info needed display a preview of that room in the RoomsList
/// and to filter the list of rooms based on the current search filter.
#[derive(Debug)]
pub struct JoinedRoomInfo {
    /// The displayable name of this room (includes room ID for fallback).
    pub room_name_id: RoomNameId,
    /// The number of unread messages in this room.
    pub num_unread_messages: u64,
    /// The number of unread mentions in this room.
    pub num_unread_mentions: u64,
    /// The canonical alias for this room, if any.
    pub canonical_alias: Option<OwnedRoomAliasId>,
    /// The alternative aliases for this room, if any.
    pub alt_aliases: Vec<OwnedRoomAliasId>,
    /// The tags associated with this room, if any.
    /// This includes things like is_favourite, is_low_priority,
    /// whether the room is a server notice room, etc.
    pub tags: Tags,
    /// The timestamp and Html text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    /// The avatar for this room: either an array of bytes holding the avatar image
    /// or a string holding the first Unicode character of the room name.
    pub room_avatar: FetchedRoomAvatar,
    /// Whether this room has been paginated at least once.
    /// We pre-paginate visible rooms at least once in order to
    /// be able to display the latest message in the RoomsListEntry
    /// and to have something to immediately show when a user first opens a room.
    pub has_been_paginated: bool,
    /// Whether this room is currently selected in the UI.
    pub is_selected: bool,
    /// Whether this a direct room.
    pub is_direct: bool,
    /// Whether this room is tombstoned (shut down and replaced with a successor room).
    pub is_tombstoned: bool,

    // TODO: we could store the parent chain(s) of this room, i.e., which spaces
    //       they are children of. One room can be in multiple spaces.
}

/// UI-related info about a room that the user has been invited to.
///
/// This includes info needed display a preview of that room in the RoomsList
/// and to filter the list of rooms based on the current search filter.
pub struct InvitedRoomInfo {
    /// The displayable name of this room (includes room ID for fallback).
    pub room_name_id: RoomNameId,
    /// The canonical alias for this room, if any.
    pub canonical_alias: Option<OwnedRoomAliasId>,
    /// The alternative aliases for this room, if any.
    pub alt_aliases: Vec<OwnedRoomAliasId>,
    /// The avatar for this room: either an array of bytes holding the avatar image
    /// or a string holding the first Unicode character of the room name.
    pub room_avatar: FetchedRoomAvatar,
    /// Info about the user who invited us to this room, if available.
    pub inviter_info: Option<InviterInfo>,
    /// The timestamp and Html text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    /// The state of how this invite is being handled by the client backend
    /// and what should be shown in the UI.
    ///
    /// We maintain this state here instead of in the `InviteScreen`
    /// because we need the state to persist even if the `InviteScreen` is closed.
    pub invite_state: InviteState,
    /// Whether this room is currently selected in the UI.
    pub is_selected: bool,
    /// Whether this is an invite to a direct room.
    pub is_direct: bool,
}

/// Info about the user who invited us to a room.
#[derive(Clone)]
pub struct InviterInfo {
    pub user_id: OwnedUserId,
    pub display_name: Option<String>,
    pub avatar: Option<Arc<[u8]>>,
}
impl std::fmt::Debug for InviterInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InviterInfo")
            .field("user_id", &self.user_id)
            .field("display_name", &self.display_name)
            .field("avatar?", &self.avatar.is_some())
            .finish()
    }
}

/// The state of a pending invite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InviteState {
    /// Waiting for the user to accept or decline the invite.
    #[default]
    WaitingOnUserInput,
    /// Waiting for the server to respond to the user's "join room" action.
    WaitingForJoinResult,
    /// Waiting for the server to respond to the user's "leave room" action.
    WaitingForLeaveResult,
    /// The invite was accepted and the room was successfully joined.
    /// We're now waiting for our client to receive the joined room from the homeserver.
    WaitingForJoinedRoom,
    /// The invite was declined and the room was successfully left.
    /// This should result in the InviteScreen being closed.
    RoomLeft,
}

/// The value in the RoomsList's `space_map` that contains info about a space.
#[derive(Default)]
struct SpaceMapValue { 
    /// Whether this space is fully paginated, meaning that our client has obtained
    /// the full list of direct children within this space.
    ///
    /// Note that it *does not* mean that all nested/subspaces within this space
    /// have been fully paginated themselves.
    is_fully_paginated: bool,
    /// The set of rooms that are direct children of this space, excluding subspaces.
    direct_child_rooms: Arc<HashSet<OwnedRoomId>>,
    /// The nested subspaces (only spaces) that are direct children of this space.
    direct_subspaces: Arc<HashSet<OwnedRoomId>>,
    /// The chain of parents that this space has, ordered from highest to lowest level.
    ///
    /// That is, the first element is this space's top-level ancestor space,
    /// while the last element is this space's immediate parent.
    parent_chain: ParentChain,
}

#[derive(Live, Widget)]
pub struct RoomsList {
    #[deref] view: View,

    /// The list of all rooms that the user has been invited to.
    ///
    /// This is a shared reference to the thread-local [`ALL_INVITED_ROOMS`] variable.
    #[rust] invited_rooms: Rc<RefCell<HashMap<OwnedRoomId, InvitedRoomInfo>>>,

    /// The set of all joined rooms and their cached info.
    /// This includes both direct rooms and regular rooms, but not invited rooms.
    #[rust] all_joined_rooms: HashMap<OwnedRoomId, JoinedRoomInfo>,

    /// The space that is currently selected as a display filter for the rooms list, if any.
    /// * If `None` (default), no space is selected, and all rooms can be shown.
    /// * If `Some`, the rooms list is in "space" mode. A special "Space Lobby" entry
    ///   is shown at the top, and only child rooms within this space will be displayed.
    #[rust] selected_space: Option<RoomNameId>,

    /// The sender used to send Space-related requests to the background service.
    #[rust] space_request_sender: Option<UnboundedSender<SpaceRequest>>,

    /// A flattened map of all spaces known to the client.
    ///
    /// The key is a Space ID, and the value contains a list of all regular rooms
    /// and nested subspaces *directly* within that space.
    ///
    /// This can include both joined and non-joined spaces.
    #[rust] space_map: HashMap<OwnedRoomId, SpaceMapValue>,

    /// Rooms that are explicitly hidden and should never be shown in the rooms list.
    #[rust] hidden_rooms: HashSet<OwnedRoomId>,

    /// The currently-active filter function for the list of rooms.
    ///
    /// ## Important Notes
    /// 1. Do not use this directly. Instead, use the `should_display_room!()` macro.
    /// 2. This does *not* get auto-applied when it changes, for performance reasons.
    #[rust] display_filter: RoomDisplayFilter,

    /// The latest keywords (trimmed) entered into the `RoomFilterInputBar`.
    ///
    /// If empty, there are no filter keywords in use, and all rooms/spaces should be shown.
    #[rust] filter_keywords: String,

    /// The list of invited rooms currently displayed in the UI.
    #[rust] displayed_invited_rooms: Vec<OwnedRoomId>,
    #[rust(false)] is_invited_rooms_header_expanded: bool,

    /// The list of direct rooms currently displayed in the UI.
    #[rust] displayed_direct_rooms: Vec<OwnedRoomId>,
    #[rust(false)] is_direct_rooms_header_expanded: bool,

    /// The list of regular (non-direct) joined rooms currently displayed in the UI.
    ///
    /// **Direct rooms are excluded** from this; they are in `displayed_direct_rooms`.
    #[rust] displayed_regular_rooms: Vec<OwnedRoomId>,
    #[rust(true)] is_regular_rooms_header_expanded: bool,

    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,

    /// The ID of the currently-selected room.
    #[rust] current_active_room: Option<OwnedRoomId>,

    /// The maximum number of rooms that will ever be loaded.
    ///
    /// This should not be used to determine whether all requested rooms have been loaded,
    /// because we will likely never receive this many rooms due to the room list service
    /// excluding rooms that we have filtered out (e.g., left or tombstoned rooms, spaces, etc).
    #[rust] max_known_rooms: Option<u32>,
    // /// Whether the room list service has loaded all requested rooms from the homeserver.
    // #[rust] all_rooms_loaded: bool,
}

impl LiveHook for RoomsList {
    fn after_new_from_doc(&mut self, _: &mut Cx) {
        self.invited_rooms = ALL_INVITED_ROOMS.with(Rc::clone);
    }
}

/// A macro that returns whether a given Room should be displayed in the RoomsList.
/// This is only intended for usage within RoomsList methods.
///
/// ## Arguments
/// 1. `self: &RoomsList`: an immutable reference to the `RoomsList` widget struct.
/// 2. `room_id: &OwnedRoomId`: an immutable reference to the room's ID.
/// 3. `room: &dyn impl FilterableRoom`: an immutable reference to the room info,
///     which must implement the [`FilterableRoom`] trait.
macro_rules! should_display_room {
    ($self:expr, $room_id:expr, $room:expr) => {
        !$self.hidden_rooms.contains($room_id)
            && ($self.display_filter)($room)
            && $self.selected_space.as_ref()
                .is_none_or(|space| $self.is_room_indirectly_in_space(space.room_id(), $room_id))
    };
}


impl RoomsList {
    /// Returns whether the homeserver has finished syncing all of the rooms
    /// that should be synced to our client based on the currently-specified room list filter. 
    pub fn all_rooms_loaded(&self) -> bool {
        // TODO: fix this: figure out a way to determine if
        //       all requested rooms have been received from the homeserver.
        false
        // self.all_rooms_loaded
    }

    /// Returns `true` if the given `room_id` is in the `all_joined_rooms` or `invited_rooms` list.
    pub fn is_room_loaded(&self, room_id: &OwnedRoomId) -> bool {
        self.all_joined_rooms.contains_key(room_id)
            || self.invited_rooms.borrow().contains_key(room_id)
    }

    /// Handle all pending updates to the list of all rooms.
    fn handle_rooms_list_updates(&mut self, cx: &mut Cx, _event: &Event, scope: &mut Scope) {
        let mut num_updates: usize = 0;
        while let Some(update) = PENDING_ROOM_UPDATES.pop() {
            num_updates += 1;
            match update {
                RoomsListUpdate::AddInvitedRoom(invited_room) => {
                    let room_id = invited_room.room_name_id.room_id().clone();
                    let should_display = should_display_room!(self, &room_id, &invited_room);
                    let _replaced = self.invited_rooms.borrow_mut().insert(room_id.clone(), invited_room);
                    if let Some(_old_room) = _replaced {
                        error!("BUG: Added invited room {room_id} that already existed");
                    } else {
                        if should_display {
                            self.displayed_invited_rooms.push(room_id);
                        }
                    }
                    self.update_status();
                    // Signal the UI to update the RoomScreen
                    SignalToUI::set_ui_signal();
                }
                RoomsListUpdate::AddJoinedRoom(joined_room) => {
                    let room_id = joined_room.room_name_id.room_id().clone();
                    let is_direct = joined_room.is_direct;
                    let should_display = should_display_room!(self, &room_id, &joined_room);
                    let replaced = self.all_joined_rooms.insert(room_id.clone(), joined_room);
                    if replaced.is_none() {
                        if should_display {
                            if is_direct {
                                self.displayed_direct_rooms.push(room_id.clone());
                            } else {
                                self.displayed_regular_rooms.push(room_id.clone());
                            }
                        }
                    } else {
                        error!("BUG: Added joined room {room_id} that already existed");
                    }

                    // If this room was added as a result of accepting an invite, we must:
                    // 1. Remove the room from the list of invited rooms.
                    // 2. Update the displayed invited rooms list to remove this room.
                    // 3. Emit an action to inform other widgets that the InviteScreen
                    //    displaying the invite to this room should be converted to a
                    //    RoomScreen displaying the now-joined room.
                    if let Some(_accepted_invite) = self.invited_rooms.borrow_mut().remove(&room_id) {
                        log!("Removed room {room_id} from the list of invited rooms");
                        self.displayed_invited_rooms.iter()
                            .position(|r| r == &room_id)
                            .map(|index| self.displayed_invited_rooms.remove(index));
                        if let Some(room) = self.all_joined_rooms.get(&room_id) {
                            cx.widget_action(
                                self.widget_uid(),
                                &scope.path,
                                RoomsListAction::InviteAccepted {
                                    room_name_id: room.room_name_id.clone(),
                                }
                            );
                        }
                    }
                    self.update_status();
                    // Signal the UI to update the RoomScreen
                    SignalToUI::set_ui_signal();
                }
                RoomsListUpdate::UpdateRoomAvatar { room_id, room_avatar } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.room_avatar = room_avatar;
                    } else {
                        error!("Error: couldn't find room {room_id} to update avatar");
                    }
                }
                RoomsListUpdate::UpdateLatestEvent { room_id, timestamp, latest_message_text } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.latest = Some((timestamp, latest_message_text));
                    } else {
                        error!("Error: couldn't find room {room_id} to update latest event");
                    }
                }
                RoomsListUpdate::UpdateNumUnreadMessages { room_id, unread_messages, unread_mentions } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        (room.num_unread_messages, room.num_unread_mentions) = match unread_messages {
                            UnreadMessageCount::Unknown => (0, 0),
                            UnreadMessageCount::Known(count) => (count, unread_mentions),
                        };
                    } else {
                        warning!("Warning: couldn't find room {} to update unread messages count", room_id);
                    }
                }
                RoomsListUpdate::UpdateRoomName { new_room_name } => {

                    // TODO: broadcast a new AppState action to ensure that this room's or space's new name
                    //       gets updated in all of the `SelectedRoom` instances throughout Robrix,
                    //       e.g., the name of the room in the Dock Tab or the StackNav header.

                    let room_id = new_room_name.room_id().clone();
                    // Try to update joined room first
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.room_name_id = new_room_name;
                        let is_direct = room.is_direct;
                        let should_display = should_display_room!(self, &room_id, room);
                        let (pos_in_list, displayed_list) = if is_direct {
                            (
                                self.displayed_direct_rooms.iter().position(|r| r == &room_id),
                                &mut self.displayed_direct_rooms,
                            )
                        } else {
                            (
                                self.displayed_regular_rooms.iter().position(|r| r == &room_id),
                                &mut self.displayed_regular_rooms,
                            )
                        };
                        if should_display {
                            if pos_in_list.is_none() {
                                displayed_list.push(room_id);
                            }
                        } else {
                            pos_in_list.map(|i| displayed_list.remove(i));
                        }
                    }
                    // If not a joined room, try to update invited room
                    else {
                        let mut invited_rooms = self.invited_rooms.borrow_mut();
                        if let Some(invited_room) = invited_rooms.get_mut(&room_id) {
                            invited_room.room_name_id = new_room_name;
                            let should_display = should_display_room!(self, &room_id, invited_room);
                            let pos_in_list = self.displayed_invited_rooms.iter()
                                .position(|r| r == &room_id);
                            if should_display {
                                if pos_in_list.is_none() {
                                    self.displayed_invited_rooms.push(room_id);
                                }
                            } else {
                                pos_in_list.map(|i| self.displayed_invited_rooms.remove(i));
                            }
                        } else {
                            warning!("Warning: couldn't find room {new_room_name} to update its name.");
                        }
                    }
                }
                RoomsListUpdate::UpdateIsDirect { room_id, is_direct } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        let was_direct = room.is_direct;
                        if was_direct == is_direct {
                            continue;
                        }
                        enqueue_popup_notification(PopupItem {
                            message: format!("{} was changed from {} to {}.",
                                room.room_name_id,
                                if room.is_direct { "direct" } else { "regular" },
                                if is_direct { "direct" } else { "regular" }
                            ),
                            auto_dismissal_duration: Some(5.0),
                            kind: PopupKind::Info,
                        });

                        // Remove the room from the previous list (direct or regular).
                        let list_to_remove_from = if was_direct {
                            &mut self.displayed_direct_rooms
                        } else {
                            &mut self.displayed_regular_rooms
                        };
                        list_to_remove_from.iter()
                            .position(|r| r == &room_id)
                            .map(|index| list_to_remove_from.remove(index));

                        // Update the room. If it should be displayed, add it to the proper list.
                        room.is_direct = is_direct;
                        if should_display_room!(self, &room_id, room) {
                            if is_direct {
                                self.displayed_direct_rooms.push(room_id);
                            } else {
                                self.displayed_regular_rooms.push(room_id);
                            }
                        }
                    } else {
                        error!("Error: couldn't find room {room_id} to update is_direct");
                    }
                }
                RoomsListUpdate::RemoveRoom { room_id, new_state: _ } => {
                    if let Some(removed) = self.all_joined_rooms.remove(&room_id) {
                        log!("Removed room {room_id} from the list of all joined rooms");
                        let list_to_remove_from = if removed.is_direct {
                            &mut self.displayed_direct_rooms
                        } else {
                            &mut self.displayed_regular_rooms
                        };
                        list_to_remove_from.iter()
                            .position(|r| r == &room_id)
                            .map(|index| list_to_remove_from.remove(index));
                    }
                    else if let Some(_removed) = self.invited_rooms.borrow_mut().remove(&room_id) {
                        log!("Removed room {room_id} from the list of all invited rooms");
                        self.displayed_invited_rooms.iter()
                            .position(|r| r == &room_id)
                            .map(|index| self.displayed_invited_rooms.remove(index));
                    }

                    self.hidden_rooms.remove(&room_id);
                    self.update_status();
                }
                RoomsListUpdate::ClearRooms => {
                    self.all_joined_rooms.clear();
                    self.displayed_direct_rooms.clear();
                    self.displayed_regular_rooms.clear();
                    self.invited_rooms.borrow_mut().clear();
                    self.displayed_invited_rooms.clear();
                    self.update_status();
                }
                RoomsListUpdate::NotLoaded => {
                    self.status = "Loading rooms (waiting for homeserver)...".to_string();
                }
                RoomsListUpdate::LoadedRooms { max_rooms } => {
                    self.max_known_rooms = max_rooms;
                },
                RoomsListUpdate::Tags { room_id, new_tags } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.tags = new_tags;
                    } else if let Some(_room) = self.invited_rooms.borrow().get(&room_id) {
                        log!("Ignoring updated tags update for invited room {room_id}");
                    } else {
                        warning!("Warning: skipping updated Tags for unknown room {room_id}.");
                    }
                }
                RoomsListUpdate::Status { status } => {
                    self.status = status;
                }
                RoomsListUpdate::TombstonedRoom { room_id } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.is_tombstoned = true;
                        let is_direct = room.is_direct;
                        let should_display = should_display_room!(self, &room_id, room);
                        let (pos_in_list, displayed_list) = if is_direct {
                            (
                                self.displayed_direct_rooms.iter().position(|r| r == &room_id),
                                &mut self.displayed_direct_rooms,
                            )
                        } else {
                            (
                                self.displayed_regular_rooms.iter().position(|r| r == &room_id),
                                &mut self.displayed_regular_rooms,
                            )
                        };
                        if should_display {
                            if pos_in_list.is_none() {
                                displayed_list.push(room_id);
                            }
                        } else {
                            pos_in_list.map(|i| displayed_list.remove(i));
                        }
                    } else {
                        warning!("Warning: couldn't find room {room_id} to update the tombstone status");
                    }
                }
                RoomsListUpdate::HideRoom { room_id } => {
                    self.hidden_rooms.insert(room_id.clone());
                    // Hiding a regular room is the most common case (e.g., after its successor is joined),
                    // so we check that list first.
                    if let Some(i) = self.displayed_regular_rooms.iter().position(|r| r == &room_id) {
                        self.displayed_regular_rooms.remove(i);
                    }
                    else if let Some(i) = self.displayed_direct_rooms.iter().position(|r| r == &room_id) {
                        self.displayed_direct_rooms.remove(i);
                    }
                    else if let Some(i) = self.displayed_invited_rooms.iter().position(|r| r == &room_id) {
                        self.displayed_invited_rooms.remove(i);
                        continue;
                    }
                }
                RoomsListUpdate::ScrollToRoom(room_id) => {
                    let portal_list = self.view.portal_list(ids!(list));
                    let speed = 50.0;
                    let portal_list_index = if let Some(direct_index) = self.displayed_direct_rooms.iter().position(|r| r == &room_id) {
                        let (_, direct_rooms_indexes, _) = self.calculate_indexes();
                        direct_rooms_indexes.first_room_index + direct_index
                    }
                    else if let Some(regular_index) = self.displayed_regular_rooms.iter().position(|r| r == &room_id) {
                        let (_, _, regular_rooms_indexes) = self.calculate_indexes();
                        regular_rooms_indexes.first_room_index + regular_index
                    }
                    else { continue };
                    // Scroll to just above the room to make it more obviously visible.
                    portal_list.smooth_scroll_to(cx, portal_list_index.saturating_sub(1), speed, Some(15));
                }
                RoomsListUpdate::SpaceRequestSender(sender) => {
                    self.space_request_sender = Some(sender);
                    num_updates -= 1;  // this does not require a redraw.
                }
            }
        }
        if num_updates > 0 {
            self.redraw(cx);
        }
    }

    /// Updates the status message to show how many rooms have been loaded
    /// or how many rooms match the current room filter keywords.
    ///
    /// Note: this *does not* actually redraw the status message or rooms list;
    ///       that must be done separately.
    fn update_status(&mut self) {
        let num_rooms = self.displayed_invited_rooms.len()
            + self.displayed_direct_rooms.len()
            + self.displayed_regular_rooms.len();

        let mut text = match (self.filter_keywords.is_empty(), num_rooms) {
            (true, 0)  => "No joined rooms found".to_string(),
            (true, 1)  => "Loaded 1 room".to_string(),
            (true, n)  => format!("Loaded {n} rooms"),
            (false, 0) => "No matching rooms found".to_string(),
            (false, 1) => "Found 1 matching room".to_string(),
            (false, n) => format!("Found {n} matching rooms"),
        };
        match self.selected_space.is_some() {
            true => text.push_str(" in this space."),
            false => text.push('.'),
        };
        self.status = text;
    }

    /// Updates and redraws the lists of displayed rooms in the RoomsList.
    /// 
    /// This will update the display filter based on the current filter keywords
    /// and the currently-selected space (if any).
    fn update_displayed_rooms(&mut self, cx: &mut Cx) {
        // Determine and set the filter function and sort function.
        let (display_fn, sort_fn) = if self.filter_keywords.is_empty() {
            (RoomDisplayFilter::default(), None)
        } else {
            // Create a new filter function based on the given keywords.
            RoomDisplayFilterBuilder::new()
                .set_keywords(self.filter_keywords.clone())
                .set_filter_criteria(RoomFilterCriteria::All)
                .build()
        };
        self.display_filter = display_fn;

        self.displayed_invited_rooms = self.generate_displayed_invited_rooms(sort_fn.as_deref());
        let (regular, direct) = self.generate_displayed_joined_rooms(sort_fn.as_deref());
        self.displayed_regular_rooms = regular;
        self.displayed_direct_rooms = direct;

        self.update_status();
        self.view.portal_list(ids!(list)).set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    /// Generates the list of displayed invited rooms based on the current filter
    /// and the given sort function.
    fn generate_displayed_invited_rooms(&self, sort_fn: Option<&SortFn>) -> Vec<OwnedRoomId> {
        let invited_rooms_ref = self.invited_rooms.borrow();
        let filtered_invited_rooms_iter = invited_rooms_ref
            .iter()
            .filter(|&(room_id, room)| should_display_room!(self, room_id, room));

        if let Some(sort_fn) = sort_fn {
            let mut filtered_invited_rooms = filtered_invited_rooms_iter
                .collect::<Vec<_>>();
            filtered_invited_rooms.sort_by(|(_, room_a), (_, room_b)| sort_fn(*room_a, *room_b));
            filtered_invited_rooms
                .into_iter()
                .map(|(room_id, _)| room_id.clone())
                .collect()
        } else {
            filtered_invited_rooms_iter
                .map(|(room_id, _)| room_id.clone())
                .collect()
        }
    }

    /// Generates a tuple of (displayed regular rooms, displayed direct rooms)
    /// based on the current `display_filter` function and the given sort function.
    fn generate_displayed_joined_rooms(&self, sort_fn: Option<&SortFn>) -> (Vec<OwnedRoomId>, Vec<OwnedRoomId>) {
        let mut new_displayed_regular_rooms = Vec::new();
        let mut new_displayed_direct_rooms = Vec::new();
        let mut push_room = |room_id: &OwnedRoomId, jr: &JoinedRoomInfo| {
            let room_id = room_id.clone();
            if jr.is_direct {
                new_displayed_direct_rooms.push(room_id);
            } else {
                new_displayed_regular_rooms.push(room_id);
            }
        };

        let filtered_joined_rooms_iter = self.all_joined_rooms
            .iter()
            .filter(|&(room_id, room)| should_display_room!(self, room_id, room));

        if let Some(sort_fn) = sort_fn {
            let mut filtered_rooms = filtered_joined_rooms_iter.collect::<Vec<_>>();
            filtered_rooms.sort_by(|(_, room_a), (_, room_b)| sort_fn(*room_a, *room_b));
            for (room_id, jr) in filtered_rooms.into_iter() {
                push_room(room_id, jr)
            }
        } else {
            for (room_id, jr) in filtered_joined_rooms_iter {
                push_room(room_id, jr)
            }
        };

        (new_displayed_regular_rooms, new_displayed_direct_rooms)
    }

    /// Calculate the indices in the PortalList where the headers and rooms should be drawn.
    ///
    /// Returns a tuple of:
    /// 1. The indexes for the invited rooms,
    /// 2. The indexes for the direct rooms (DMs / People),
    /// 3. The indexes for the regular non-direct joined rooms.
    fn calculate_indexes(&self) -> (RoomCategoryIndexes, RoomCategoryIndexes, RoomCategoryIndexes) {
        // Based on the various displayed room lists and is_expanded state of each room header,
        // calculate the indices in the PortalList where the headers and rooms should be drawn.
        let should_show_invited_rooms_header = !self.displayed_invited_rooms.is_empty();
        let should_show_direct_rooms_header = !self.displayed_direct_rooms.is_empty();
        let should_show_regular_rooms_header = !self.displayed_regular_rooms.is_empty();

        let index_of_invited_rooms_header = should_show_invited_rooms_header.then_some(0);
        let index_of_first_invited_room = should_show_invited_rooms_header as usize;
        let index_after_invited_rooms = index_of_first_invited_room +
            if self.is_invited_rooms_header_expanded {
                self.displayed_invited_rooms.len()
            } else {
                0
            };

        let index_of_direct_rooms_header = should_show_direct_rooms_header
            .then_some(index_after_invited_rooms);
        let index_of_first_direct_room = index_after_invited_rooms +
            should_show_direct_rooms_header as usize;
        let index_after_direct_rooms = index_of_first_direct_room +
            if self.is_direct_rooms_header_expanded {
                self.displayed_direct_rooms.len()
            } else {
                0
            };

        let index_of_regular_rooms_header = should_show_regular_rooms_header
            .then_some(index_after_direct_rooms);
        let index_of_first_regular_room = index_after_direct_rooms +
            should_show_regular_rooms_header as usize;
        let index_after_regular_rooms = index_of_first_regular_room +
            if self.is_regular_rooms_header_expanded {
                self.displayed_regular_rooms.len()
            } else {
                0
            };

        let invited_rooms_indexes = RoomCategoryIndexes {
            header_index: index_of_invited_rooms_header,
            first_room_index: index_of_first_invited_room,
            after_rooms_index: index_after_invited_rooms,
        };
        let direct_rooms_indexes = RoomCategoryIndexes {
            header_index: index_of_direct_rooms_header,
            first_room_index: index_of_first_direct_room,
            after_rooms_index: index_after_direct_rooms,
        };
        let regular_rooms_indexes = RoomCategoryIndexes {
            header_index: index_of_regular_rooms_header,
            first_room_index: index_of_first_regular_room,
            after_rooms_index: index_after_regular_rooms,
        };
        (
            invited_rooms_indexes,
            direct_rooms_indexes,
            regular_rooms_indexes,
        )
    }

    /// Handle any incoming updates to spaces' room lists and pagination state.
    fn handle_space_room_list_action(&mut self, cx: &mut Cx, action: &SpaceRoomListAction) {
        match action {
            SpaceRoomListAction::UpdatedChildren { space_id, parent_chain, direct_child_rooms, direct_subspaces } => {
                match self.space_map.entry(space_id.clone()) {
                    Entry::Occupied(mut occ) => {
                        let occ_mut = occ.get_mut();
                        occ_mut.parent_chain = parent_chain.clone();
                        occ_mut.direct_child_rooms = Arc::clone(direct_child_rooms);
                        occ_mut.direct_subspaces   = Arc::clone(direct_subspaces);
                    }
                    Entry::Vacant(vac) => {
                        vac.insert_entry(SpaceMapValue {
                            is_fully_paginated: false,
                            parent_chain: parent_chain.clone(),
                            direct_child_rooms: Arc::clone(direct_child_rooms),
                            direct_subspaces:   Arc::clone(direct_subspaces),
                        });
                    }
                }
                if self.selected_space.as_ref().is_some_and(|sel_space|
                    sel_space.room_id() == space_id
                    || parent_chain.contains(sel_space.room_id())
                ) {
                    self.update_displayed_rooms(cx);
                }
            }
            SpaceRoomListAction::PaginationState { space_id, parent_chain, state } => {
                let is_fully_paginated = matches!(state, SpaceRoomListPaginationState::Idle { end_reached: true });
                // Only re-fetch the list of rooms in this space if it was not already fully paginated.
                let should_fetch_rooms: bool;
                match self.space_map.entry(space_id.clone()) {
                    Entry::Occupied(mut occ) => {
                        let value_mut = occ.get_mut();
                        should_fetch_rooms = !value_mut.is_fully_paginated;
                        value_mut.is_fully_paginated = is_fully_paginated;
                    }
                    Entry::Vacant(vac) => {
                        vac.insert_entry(SpaceMapValue {
                            is_fully_paginated,
                            parent_chain: parent_chain.clone(),
                            ..Default::default()
                        });
                        should_fetch_rooms = true;
                    }
                }
                let Some(sender) = self.space_request_sender.as_ref() else {
                    error!("BUG: RoomsList: no space request sender was available after pagination state update.");
                    return;
                };
                if should_fetch_rooms {
                    if sender.send(SpaceRequest::GetChildren {
                        space_id: space_id.clone(),
                        parent_chain: parent_chain.clone(),
                    }).is_err() {
                        error!("BUG: RoomsList: failed to send GetRooms request for space {space_id}.");
                    }
                }

                // In order to determine which rooms are in a given top-level space,
                // we also must know all of the rooms within that space's subspaces.
                // Thus, we must continue paginating this space until we fully fetch
                // all of its children, such that we can see if any of them are subspaces,
                // and then we'll paaginate those as well.
                if !is_fully_paginated {
                    if sender.send(SpaceRequest::PaginateSpaceRoomList {
                        space_id: space_id.clone(),
                        parent_chain: parent_chain.clone(),
                    }).is_err() {
                        error!("BUG: RoomsList: failed to send pagination request for space {space_id}.");
                    }
                }
            }
            SpaceRoomListAction::PaginationError { space_id, error } => {
                error!("RoomsList: failed to paginate rooms in space {space_id}: {error:?}");
                enqueue_popup_notification(PopupItem {
                    message: "Failed to fetch more rooms in this space. Try again later.".to_string(),
                    auto_dismissal_duration: None,
                    kind: PopupKind::Error,
                });
            }
        }
    }

    /// Returns whether the given target room or space is indirectly within the given parent space.
    ///
    /// This will recursively search all nested spaces within the given `parent_space`.
    fn is_room_indirectly_in_space(&self, parent_space: &OwnedRoomId, target: &OwnedRoomId) -> bool {
        if let Some(smv) = self.space_map.get(parent_space) {
            if smv.direct_child_rooms.contains(target) {
                return true;
            }
            for subspace in smv.direct_subspaces.iter() {
                if self.is_room_indirectly_in_space(subspace, target) {
                    return true;
                }
            }
        }
        false
    }
}

impl Widget for RoomsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Process all pending updates to the list of all rooms, and then redraw it.
        if matches!(event, Event::Signal) {
            self.handle_rooms_list_updates(cx, event, scope);
        }

        // Now, handle any actions on this widget, e.g., a user selecting a room.
        // We use Scope `props` to pass down the current scrolling state of the PortalList.
        let props = RoomsListScopeProps {
            was_scrolling: self.view.portal_list(ids!(list)).was_scrolling(),
        };
        let rooms_list_actions = cx.capture_actions(
            |cx| self.view.handle_event(cx, event, &mut Scope::with_props(&props))
        );
        for action in rooms_list_actions {
            // Handle a regular room (joined or invited) being clicked.
            if let RoomsListEntryAction::Clicked(clicked_room_id) = action.as_widget_action().cast() {
                let new_selected_room = if let Some(jr) = self.all_joined_rooms.get(&clicked_room_id) {
                    SelectedRoom::JoinedRoom {
                        room_name_id: jr.room_name_id.clone(),
                    }
                } else if let Some(ir) = self.invited_rooms.borrow().get(&clicked_room_id) {
                    SelectedRoom::InvitedRoom {
                        room_name_id: ir.room_name_id.clone(),
                    }
                } else {
                    error!("BUG: couldn't find clicked room details for room {clicked_room_id}");
                    continue;
                };

                self.current_active_room = Some(clicked_room_id.clone());
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    RoomsListAction::Selected(new_selected_room),
                );
                self.redraw(cx);
            }
            // Handle the space lobby being clicked.
            else if let Some(SpaceLobbyAction::SpaceLobbyEntryClicked) = action.downcast_ref() {
                let Some(space_name_id) = self.selected_space.clone() else { continue };
                self.current_active_room = Some(space_name_id.room_id().clone());
                let new_selected_space = SelectedRoom::Space { space_name_id };
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    RoomsListAction::Selected(new_selected_space),
                );
                self.redraw(cx);
            }
            // Handle a collapsible header being clicked.
            else if let CollapsibleHeaderAction::Toggled { category } = action.as_widget_action().cast() {
                match category {
                    HeaderCategory::Invites => {
                        self.is_invited_rooms_header_expanded = !self.is_invited_rooms_header_expanded;
                    }
                    HeaderCategory::RegularRooms => {
                        self.is_regular_rooms_header_expanded = !self.is_regular_rooms_header_expanded;
                    }
                    HeaderCategory::DirectRooms => {
                        self.is_direct_rooms_header_expanded =
                            !self.is_direct_rooms_header_expanded;
                    }
                    _todo => todo!("Handle other header categories"),
                }
                self.redraw(cx);
            }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                if let RoomFilterAction::Changed(keywords) = action.as_widget_action().cast() {
                    self.filter_keywords = keywords;
                    self.update_displayed_rooms(cx);
                    continue;
                }

                // Handle a space navigation tab being selected or de-selected.
                if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                    match tab {
                        SelectedTab::Space { space_name_id } => {
                            if self.selected_space.as_ref().is_some_and(|s| s.room_id() == space_name_id.room_id()) {
                                continue;
                            }

                            self.selected_space = Some(space_name_id.clone());
                            self.view.space_lobby_entry(ids!(space_lobby_entry)).set_visible(cx, true);

                            // If we don't have the full list of children in this newly-selected space, then fetch it.
                            let (is_fully_paginated, parent_chain) = self.space_map
                                .get(space_name_id.room_id())
                                .map(|smv| (smv.is_fully_paginated, smv.parent_chain.clone()))
                                .unwrap_or_default();
                            if !is_fully_paginated {
                                let Some(sender) = self.space_request_sender.as_ref() else {
                                    error!("BUG: RoomsList: no space request sender was available.");
                                    continue;
                                };

                                if sender.send(SpaceRequest::SubscribeToSpaceRoomList {
                                    space_id: space_name_id.room_id().clone(),
                                    parent_chain: parent_chain.clone(),
                                }).is_err() {
                                    error!("BUG: RoomsList: failed to send SubscribeToSpaceRoomList request for space {space_name_id}.");
                                }
                                if sender.send(SpaceRequest::PaginateSpaceRoomList {
                                    space_id: space_name_id.room_id().clone(),
                                    parent_chain: parent_chain.clone(),
                                }).is_err() {
                                    error!("BUG: RoomsList: failed to send PaginateSpaceRoomList request for space {space_name_id}.");
                                }
                                if sender.send(SpaceRequest::GetChildren {
                                    space_id: space_name_id.room_id().clone(),
                                    parent_chain,
                                }).is_err() {
                                    error!("BUG: RoomsList: failed to send GetRooms request for space {space_name_id}.");
                                }
                            }
                        }
                        _ => {
                            self.selected_space = None;
                            self.view.space_lobby_entry(ids!(space_lobby_entry)).set_visible(cx, false);
                        }
                    }

                    self.update_displayed_rooms(cx);
                    continue;
                }

                if let Some(space_room_list_action) = action.downcast_ref() {
                    self.handle_space_room_list_action(cx, space_room_list_action);
                    continue;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Update the currently-selected room from the AppState data.
        self.current_active_room = app_state.selected_room.as_ref()
            .map(|sel_room| sel_room.room_id().clone());

        // Based on the various displayed room lists and is_expanded state of each room header,
        // calculate the indices in the PortalList where the headers and rooms should be drawn.
        let (invited_rooms_indexes, direct_rooms_indexes, regular_rooms_indexes) =
            self.calculate_indexes();

        let status_label_id = regular_rooms_indexes.after_rooms_index;
        // Add one for the status label
        let total_count = status_label_id + 1;

        let get_invited_room_id = |portal_list_index: usize| {
            portal_list_index.checked_sub(invited_rooms_indexes.first_room_index)
                .and_then(|index| self.is_invited_rooms_header_expanded
                    .then(|| self.displayed_invited_rooms.get(index))
                )
                .flatten()
        };

        let get_direct_room_id = |portal_list_index: usize| {
            portal_list_index.checked_sub(direct_rooms_indexes.first_room_index)
                .and_then(|index| self.is_direct_rooms_header_expanded
                    .then(|| self.displayed_direct_rooms.get(index))
                )
                .flatten()
        };
        let get_regular_room_id = |portal_list_index: usize| {
            portal_list_index.checked_sub(regular_rooms_indexes.first_room_index)
                .and_then(|index| self.is_regular_rooms_header_expanded
                    .then(|| self.displayed_regular_rooms.get(index))
                )
                .flatten()
        };

        // Start the actual drawing procedure.
        while let Some(widget_to_draw) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = widget_to_draw.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            list.set_item_range(cx, 0, total_count);

            while let Some(portal_list_index) = list.next_visible_item(cx) {
                let mut scope = Scope::empty();

                if invited_rooms_indexes.header_index == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, id!(collapsible_header));
                    item.as_collapsible_header().set_details(
                        cx,
                        self.is_invited_rooms_header_expanded,
                        HeaderCategory::Invites,
                        self.displayed_invited_rooms.len() as u64,
                    );
                    item.draw_all(cx, &mut scope);
                }
                else if let Some(invited_room_id) = get_invited_room_id(portal_list_index) {
                    let mut invited_rooms_mut = self.invited_rooms.borrow_mut();
                    if let Some(invited_room) = invited_rooms_mut.get_mut(invited_room_id) {
                        let item = list.item(cx, portal_list_index, id!(rooms_list_entry));
                        invited_room.is_selected =
                            self.current_active_room.as_deref() == Some(invited_room_id);
                        // Pass the room info down to the RoomsListEntry widget via Scope.
                        scope = Scope::with_props(&*invited_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, id!(empty))
                            .draw_all(cx, &mut scope);
                    }
                }
                else if direct_rooms_indexes.header_index == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, id!(collapsible_header));
                    item.as_collapsible_header().set_details(
                        cx,
                        self.is_direct_rooms_header_expanded,
                        HeaderCategory::DirectRooms,
                        0,
                        // TODO: sum up all the unread mentions in rooms
                        // NOTE: this might be really slow, so we should maintain a running total of mentions in this struct
                    );
                    item.draw_all(cx, &mut scope);
                }
                else if let Some(direct_room_id) = get_direct_room_id(portal_list_index) {
                    if let Some(direct_room) = self.all_joined_rooms.get_mut(direct_room_id) {
                        let item = list.item(cx, portal_list_index, id!(rooms_list_entry));
                        direct_room.is_selected =
                            self.current_active_room.as_ref() == Some(direct_room_id);

                        // Paginate the room if it hasn't been paginated yet.
                        if PREPAGINATE_VISIBLE_ROOMS && !direct_room.has_been_paginated {
                            direct_room.has_been_paginated = true;
                            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                                room_id: direct_room.room_name_id.room_id().clone(),
                                num_events: 50,
                                direction: PaginationDirection::Backwards,
                            });
                        }
                        // Pass the room info down to the RoomsListEntry widget via Scope.
                        scope = Scope::with_props(&*direct_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, id!(empty))
                            .draw_all(cx, &mut scope);
                    }
                }
                else if regular_rooms_indexes.header_index == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, id!(collapsible_header));
                    item.as_collapsible_header().set_details(
                        cx,
                        self.is_regular_rooms_header_expanded,
                        HeaderCategory::RegularRooms,
                        0,
                        // TODO: sum up all the unread mentions in rooms.
                        // NOTE: this might be really slow, so we should maintain a running total of mentions in this struct
                    );
                    item.draw_all(cx, &mut scope);
                }
                else if let Some(regular_room_id) = get_regular_room_id(portal_list_index) {
                    if let Some(regular_room) = self.all_joined_rooms.get_mut(regular_room_id) {
                        let item = list.item(cx, portal_list_index, id!(rooms_list_entry));
                        regular_room.is_selected =
                            self.current_active_room.as_ref() == Some(regular_room_id);

                        // Paginate the room if it hasn't been paginated yet.
                        if PREPAGINATE_VISIBLE_ROOMS && !regular_room.has_been_paginated {
                            regular_room.has_been_paginated = true;
                            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                                room_id: regular_room.room_name_id.room_id().clone(),
                                num_events: 50,
                                direction: PaginationDirection::Backwards,
                            });
                        }
                        // Pass the room info down to the RoomsListEntry widget via Scope.
                        scope = Scope::with_props(&*regular_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, id!(empty)).draw_all(cx, &mut scope);
                    }
                }
                // Draw the status label as the bottom entry.
                else if portal_list_index == status_label_id {
                    let item = list.item(cx, portal_list_index, id!(status_label));
                    item.as_view().apply_over(cx, live!{
                        height: Fit,
                        label = { text: (&self.status) }
                    });
                    item.draw_all(cx, &mut scope);
                }
                // Draw a filler entry to take up space at the bottom of the portal list.
                else {
                    list.item(cx, portal_list_index, id!(bottom_filler))
                        .draw_all(cx, &mut scope);
                }
            }
        }

        DrawStep::done()
    }
}

impl RoomsListRef {
    /// See [`RoomsList::all_rooms_loaded()`].
    pub fn all_rooms_loaded(&self) -> bool {
        let Some(inner) = self.borrow() else { return false; };
        inner.all_rooms_loaded()
    }

    /// See [`RoomsList::is_room_loaded()`].
    pub fn is_room_loaded(&self, room_id: &OwnedRoomId) -> bool {
        let Some(inner) = self.borrow() else { return false; };
        inner.is_room_loaded(room_id)
    }

    /// Returns the name of the given room, if it is known and loaded.
    pub fn get_room_name(&self, room_id: &OwnedRoomId) -> Option<RoomNameId> {
        let inner = self.borrow()?;
        inner.all_joined_rooms
            .get(room_id)
            .map(|jr| jr.room_name_id.clone())
            .or_else(||
                inner.invited_rooms.borrow()
                    .get(room_id)
                    .map(|ir| ir.room_name_id.clone())
            )
    }

    /// Returns the currently-selected space (the one selected in the SpacesBar).
    pub fn get_selected_space(&self) -> Option<RoomNameId> {
        self.borrow()?.selected_space.clone()
    }

    /// Same as [`Self::get_selected_space()`], but only returns the space ID.
    pub fn get_selected_space_id(&self) -> Option<OwnedRoomId> {
        self.borrow()?.selected_space.as_ref().map(|ss| ss.room_id().clone())
    }
}

pub struct RoomsListScopeProps {
    /// Whether the RoomsList's inner PortalList was scrolling
    /// when the latest finger down event occurred.
    pub was_scrolling: bool,
}

/// The set of indexes for each room category in the the RoomsList's PortalList.
///
/// Each category's room count should be `after_rooms_index - first_room_index`.
#[derive(Debug, Clone, Copy)]
struct RoomCategoryIndexes {
    /// The index of this room category's header, at which a `<CollapsibleHeader>` widget is displayed.
    ///
    /// This is an `Option` because the header is only shown if there are some rooms in this category.
    /// This is `Some` if the header should be shown (meaning there *are* rooms in this category),
    /// and `None` if the header should *not* be shown (meaning there are no rooms in this category).
    header_index: Option<usize>,
    /// The index of the first room in this category that appears immediately after the header.
    first_room_index: usize,
    /// The index after the last room in this category, which is where the next category should start.
    after_rooms_index: usize,
}
