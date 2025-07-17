use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{ruma::{events::tag::Tags, MilliSecondsSinceUnixEpoch, OwnedRoomAliasId, OwnedRoomId, OwnedUserId}, RoomState};
use crate::{
    app::{AppState, SelectedRoom},
    room::{room_display_filter::{RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria, SortFn}, RoomPreviewAvatar},
    shared::{collapsible_header::{CollapsibleHeaderAction, CollapsibleHeaderWidgetRefExt, HeaderCategory},
    jump_to_bottom_button::UnreadMessageCount, room_filter_input_bar::RoomFilterAction},
    sliding_sync::{submit_async_request, MatrixRequest, PaginationDirection},
};
use super::room_preview::RoomPreviewAction;

/// Whether to pre-paginate visible rooms at least once in order to
/// be able to display the latest message in the room preview,
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


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::helpers::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::html_or_plaintext::HtmlOrPlaintext;
    use crate::shared::collapsible_header::*;
    use crate::home::room_preview::*;

    // An empty view that takes up no space in the portal list.
    Empty = <View> { }

    StatusLabel = <View> {
        width: Fill, height: Fit,
        align: { x: 0.5, y: 0.5 }
        padding: 15.0,

        label = <Label> {
            padding: 0
            width: Fill,
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

        list = <PortalList> {
            keep_invisible: false,
            auto_tail: false,
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            collapsible_header = <CollapsibleHeader> {}
            room_preview = <RoomPreview> {}
            empty = <Empty> {}
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
    /// Update the number of unread messages for the given room.
    UpdateNumUnreadMessages {
        room_id: OwnedRoomId,
        count: UnreadMessageCount,
        unread_mentions: u64,
    },
    /// Update the displayable name for the given room.
    UpdateRoomName {
        room_id: OwnedRoomId,
        new_room_name: String,
    },
    /// Update the avatar (image) for the given room.
    UpdateRoomAvatar {
        room_id: OwnedRoomId,
        avatar: RoomPreviewAvatar,
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
    /// A new room was selected.
    Selected(SelectedRoom),
    /// A new room was joined from an accepted invite,
    /// meaning that the existing `InviteScreen` should be converted
    /// to a `RoomScreen` to display now-joined room.
    InviteAccepted {
        room_id: OwnedRoomId,
        room_name: Option<String>,
    },
    None,
}


/// UI-related info about a joined room.
///
/// This includes info needed display a preview of that room in the RoomsList
/// and to filter the list of rooms based on the current search filter.
#[derive(Debug)]
pub struct JoinedRoomInfo {
    /// The matrix ID of this room.
    pub room_id: OwnedRoomId,
    /// The displayable name of this room, if known.
    pub room_name: Option<String>,
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
    pub avatar: RoomPreviewAvatar,
    /// Whether this room has been paginated at least once.
    /// We pre-paginate visible rooms at least once in order to
    /// be able to display the latest message in the room preview,
    /// and to have something to immediately show when a user first opens a room.
    pub has_been_paginated: bool,
    /// Whether this room is currently selected in the UI.
    pub is_selected: bool,
    /// Whether this a direct room.
    pub is_direct: bool,
}

/// UI-related info about a room that the user has been invited to.
///
/// This includes info needed display a preview of that room in the RoomsList
/// and to filter the list of rooms based on the current search filter.
pub struct InvitedRoomInfo {
    /// The matrix ID of this room.
    pub room_id: OwnedRoomId,
    /// The displayable name of this room, if known.
    pub room_name: Option<String>,
    /// The canonical alias for this room, if any.
    pub canonical_alias: Option<OwnedRoomAliasId>,
    /// The alternative aliases for this room, if any.
    pub alt_aliases: Vec<OwnedRoomAliasId>,
    /// The avatar for this room: either an array of bytes holding the avatar image
    /// or a string holding the first Unicode character of the room name.
    pub room_avatar: RoomPreviewAvatar,
    /// Info about the user who invited us to this room, if available.
    pub inviter_info: Option<InviterInfo>,
    /// The timestamp and Html text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    /// The state of this how this invite is being handled by the client backend
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


#[derive(Live, Widget)]
pub struct RoomsList {
    #[deref] view: View,

    /// The list of all rooms that the user has been invited to.
    ///
    /// This is a shared reference to the thread-local [`ALL_INVITED_ROOMS`] variable.
    #[rust] invited_rooms: Rc<RefCell<HashMap<OwnedRoomId, InvitedRoomInfo>>>,

    /// The set of all joined rooms and their cached preview info.
    /// This includes both direct rooms and regular rooms.
    #[rust] all_joined_rooms: HashMap<OwnedRoomId, JoinedRoomInfo>,

    /// The currently-active filter function for the list of rooms.
    ///
    /// Note: for performance reasons, this does not get automatically applied
    /// when its value changes. Instead, you must manually invoke it on the set of `all_joined_rooms`
    /// in order to update the set of `displayed_rooms` accordingly.
    #[rust] display_filter: RoomDisplayFilter,

    /// The list of invited rooms currently displayed in the UI, in order from top to bottom.
    /// This is a strict subset of the rooms in `all_invited_rooms`, and should be determined
    /// by applying the `display_filter` to the set of `all_invited_rooms`.
    #[rust] displayed_invited_rooms: Vec<OwnedRoomId>,
    #[rust(false)] is_invited_rooms_header_expanded: bool,

    /// The list of direct rooms currently displayed in the UI, in order from top to bottom.
    /// This is a strict subset of the rooms present in `all_joined_rooms`,
    /// and should be determined by applying the `display_filter && is_direct`
    /// to the set of `all_joined_rooms`.
    #[rust] displayed_direct_rooms: Vec<OwnedRoomId>,
    #[rust(false)] is_direct_rooms_header_expanded: bool,

    /// The list of regular (non-direct) joined rooms currently displayed in the UI,
    /// in order from top to bottom.
    /// This is a strict subset of the rooms in `all_joined_rooms`,
    /// and should be determined by applying the `display_filter && !is_direct`
    /// to the set of `all_joined_rooms`.
    ///
    /// **Direct rooms are excluded** from this; they are in `displayed_direct_rooms`.
    #[rust] displayed_regular_rooms: Vec<OwnedRoomId>,
    #[rust(true)] is_regular_rooms_header_expanded: bool,

    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,
    /// The ID of the currently-selected room.
    #[rust] current_active_room: Option<OwnedRoomId>,
    /// The maximum number of rooms that will ever be loaded.
    #[rust] max_known_rooms: Option<u32>,
}

impl LiveHook for RoomsList {
    fn after_new_from_doc(&mut self, _: &mut Cx) {
        self.invited_rooms = ALL_INVITED_ROOMS.with(Rc::clone);
    }
}

impl RoomsList {
    /// Determines if all known rooms have been loaded from the homeserver.
    ///
    /// Returns `true` if the number of rooms in `all_joined_rooms` and `invited_rooms` equals or exceeds
    /// `max_known_rooms`.
    /// Returns `false` if `max_known_rooms` is `None`.
    pub fn all_known_rooms_loaded(&self) -> bool {
        self.max_known_rooms.is_some_and(|max_rooms| {
            self.all_joined_rooms.len() + self.invited_rooms.borrow().len() >= max_rooms as usize
        })
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
                    let room_id = invited_room.room_id.clone();
                    let should_display = (self.display_filter)(&invited_room);
                    let _replaced = self.invited_rooms.borrow_mut().insert(room_id.clone(), invited_room);
                    if let Some(_old_room) = _replaced {
                        error!("BUG: Added invited room {room_id} that already existed");
                    } else {
                        if should_display {
                            self.displayed_invited_rooms.push(room_id);
                        }
                    }
                    self.update_status_rooms_count();
                    // Signal the UI to update the RoomScreen
                    SignalToUI::set_ui_signal();
                }
                RoomsListUpdate::AddJoinedRoom(joined_room) => {
                    let room_id = joined_room.room_id.clone();
                    let is_direct = joined_room.is_direct;
                    let room_name = joined_room.room_name.clone();
                    let should_display = (self.display_filter)(&joined_room);
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
                        cx.widget_action(
                            self.widget_uid(),
                            &scope.path,
                            RoomsListAction::InviteAccepted { room_id, room_name }
                        );
                    }
                    self.update_status_rooms_count();
                    // Signal the UI to update the RoomScreen
                    SignalToUI::set_ui_signal();
                }
                RoomsListUpdate::UpdateRoomAvatar { room_id, avatar } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        room.avatar = avatar;
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
                RoomsListUpdate::UpdateNumUnreadMessages { room_id, count , unread_mentions} => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        (room.num_unread_messages, room.num_unread_mentions) = match count {
                            UnreadMessageCount::Unknown => (0, 0),
                            UnreadMessageCount::Known(count) => (count, unread_mentions),
                        };
                    } else {
                        warning!("Warning: couldn't find room {} to update unread messages count", room_id);
                    }
                }
                RoomsListUpdate::UpdateRoomName { room_id, new_room_name } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        let was_displayed = (self.display_filter)(room);
                        room.room_name = Some(new_room_name);
                        let should_display = (self.display_filter)(room);
                        match (was_displayed, should_display) {
                            // No need to update the displayed rooms list.
                            (true, true) | (false, false) => { }
                            // Room was displayed but should no longer be displayed.
                            (true, false) => {
                                if room.is_direct {
                                    self.displayed_direct_rooms.iter()
                                        .position(|r| r == &room_id)
                                        .map(|index| self.displayed_direct_rooms.remove(index));
                                } else {
                                    self.displayed_regular_rooms.iter()
                                        .position(|r| r == &room_id)
                                        .map(|index| self.displayed_regular_rooms.remove(index));
                                }
                            }
                            // Room was not displayed but should now be displayed.
                            (false, true) => {
                                if room.is_direct {
                                    self.displayed_direct_rooms.push(room_id);
                                } else {
                                    self.displayed_regular_rooms.push(room_id);
                                }
                            }
                        }
                    } else {
                        error!("Error: couldn't find room {room_id} to update room name");
                    }
                }
                RoomsListUpdate::RemoveRoom { room_id, new_state: _ } => {
                    if let Some(removed) = self.all_joined_rooms.remove(&room_id) {
                        log!("Removed room {room_id} from the list of all joined rooms");
                        if removed.is_direct {
                            self.displayed_direct_rooms.iter()
                                .position(|r| r == &room_id)
                                .map(|index| self.displayed_direct_rooms.remove(index));
                        } else {
                            self.displayed_regular_rooms.iter()
                                .position(|r| r == &room_id)
                                .map(|index| self.displayed_regular_rooms.remove(index));
                        }
                    }
                    else if let Some(_removed) = self.invited_rooms.borrow_mut().remove(&room_id) {
                        log!("Removed room {room_id} from the list of all invited rooms");
                        self.displayed_invited_rooms.iter()
                            .position(|r| r == &room_id)
                            .map(|index| self.displayed_invited_rooms.remove(index));
                    }

                    self.update_status_rooms_count();
                }
                RoomsListUpdate::ClearRooms => {
                    self.all_joined_rooms.clear();
                    self.displayed_direct_rooms.clear();
                    self.displayed_regular_rooms.clear();
                    self.invited_rooms.borrow_mut().clear();
                    self.displayed_invited_rooms.clear();
                    self.update_status_rooms_count();
                }
                RoomsListUpdate::NotLoaded => {
                    self.status = "Loading rooms (waiting for homeserver)...".to_string();
                }
                RoomsListUpdate::LoadedRooms { max_rooms } => {
                    self.max_known_rooms = max_rooms;
                    self.update_status_rooms_count();
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
            }
        }
        if num_updates > 0 {
            // log!("RoomsList: processed {} updates to the list of all rooms", num_updates);
            self.redraw(cx);
        }
    }

    /// Updates the status message to show how many rooms have been loaded.
    fn update_status_rooms_count(&mut self) {
        let num_rooms = self.all_joined_rooms.len() + self.invited_rooms.borrow().len();
        self.status = if let Some(max_rooms) = self.max_known_rooms {
            format!("Loaded {num_rooms} of {max_rooms} total rooms.")
        } else {
            format!("Loaded {num_rooms} rooms.")
        };
    }

    /// Updates the status message to show how many rooms are currently displayed
    /// that match the current search filter.
    fn update_status_matching_rooms(&mut self) {
        let num_rooms = self.displayed_invited_rooms.len()
            + self.displayed_direct_rooms.len()
            + self.displayed_regular_rooms.len();
        self.status = match num_rooms {
            0 => "No matching rooms found.".to_string(),
            1 => "Found 1 matching room.".to_string(),
            n => format!("Found {} matching rooms.", n),
        }
    }

    /// Returns true if the given room is contained in any of the displayed room sets,
    /// i.e., either the invited rooms or the joined rooms.
    fn is_room_displayable(&self, room: &OwnedRoomId) -> bool {
        self.displayed_invited_rooms.contains(room)
        || self.displayed_direct_rooms.contains(room)
        || self.displayed_regular_rooms.contains(room)
    }

    /// Updates the lists of displayed rooms based on the current search filter
    /// and redraws the RoomsList.
    fn update_displayed_rooms(&mut self, cx: &mut Cx, keywords: &str) {
        let portal_list = self.view.portal_list(id!(list));
        if keywords.is_empty() {
            // Reset each of the displayed_* lists to show all rooms.
            self.display_filter = RoomDisplayFilter::default();
            self.displayed_invited_rooms = self.invited_rooms.borrow().keys().cloned().collect();

            self.displayed_direct_rooms.clear();
            self.displayed_regular_rooms.clear();
            for (id, jr) in &self.all_joined_rooms {
                if jr.is_direct {
                    self.displayed_direct_rooms.push(id.clone());
                } else {
                    self.displayed_regular_rooms.push(id.clone());
                }
            }
            self.update_status_rooms_count();
            portal_list.set_first_id_and_scroll(0, 0.0);
            self.redraw(cx);
            return;
        }

        // Create a new filter function based on the given keywords
        // and store it in this RoomsList such that we can apply it to newly-added rooms.
        let (filter, sort_fn) = RoomDisplayFilterBuilder::new()
            .set_keywords(keywords.to_owned())
            .set_filter_criteria(RoomFilterCriteria::All)
            .build();
        self.display_filter = filter;

        self.displayed_invited_rooms = self.generate_displayed_invited_rooms(sort_fn.as_deref());

        let (new_displayed_regular_rooms, new_displayed_direct_rooms) =
            self.generate_displayed_joined_rooms(sort_fn.as_deref());

        self.displayed_regular_rooms = new_displayed_regular_rooms;
        self.displayed_direct_rooms = new_displayed_direct_rooms;

        self.update_status_matching_rooms();
        portal_list.set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
    }

    /// Generates the list of displayed invited rooms based on the current filter
    /// and the given sort function.
    fn generate_displayed_invited_rooms(&self, sort_fn: Option<&SortFn>) -> Vec<OwnedRoomId> {
        let invited_rooms_ref = self.invited_rooms.borrow();
        let filtered_invited_rooms_iter = invited_rooms_ref
            .iter()
            .filter(|(_, room)| (self.display_filter)(*room));

        if let Some(sort_fn) = sort_fn {
            let mut filtered_invited_rooms = filtered_invited_rooms_iter
                .collect::<Vec<_>>();
            filtered_invited_rooms.sort_by(|(_, room_a), (_, room_b)| sort_fn(*room_a, *room_b));
            filtered_invited_rooms
                .into_iter()
                .map(|(room_id, _)| room_id.clone()).collect()
        } else {
            filtered_invited_rooms_iter.map(|(room_id, _)| room_id.clone()).collect()
        }
    }

    /// Generates the lists of displayed direct rooms and displayed regular rooms
    /// based on the current filter and the given sort function.
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

        let filtered_joined_rooms_iter = self.all_joined_rooms.iter()
            .filter(|(_, room)| (self.display_filter)(*room));

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
            was_scrolling: self.view.portal_list(id!(list)).was_scrolling(),
        };
        let list_actions = cx.capture_actions(
            |cx| self.view.handle_event(cx, event, &mut Scope::with_props(&props))
        );
        for list_action in list_actions {
            if let RoomPreviewAction::Clicked(clicked_room_id) = list_action.as_widget_action().cast() {
                let new_selected_room = if let Some(jr) = self.all_joined_rooms.get(&clicked_room_id) {
                    SelectedRoom::JoinedRoom {
                        room_id: jr.room_id.clone().into(),
                        room_name: jr.room_name.clone(),
                    }
                } else if let Some(ir) = self.invited_rooms.borrow().get(&clicked_room_id) {
                    SelectedRoom::InvitedRoom {
                        room_id: ir.room_id.to_owned().into(),
                        room_name: ir.room_name.clone(),
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
            else if let CollapsibleHeaderAction::Toggled { category } = list_action.as_widget_action().cast() {
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
                    self.update_displayed_rooms(cx, &keywords);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Update the currently-selected room from the AppState data.
        self.current_active_room = app_state.selected_room.as_ref()
            .map(|sel_room| sel_room.room_id().clone())
            .filter(|room_id| self.is_room_displayable(room_id));

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
                    let item = list.item(cx, portal_list_index, live_id!(collapsible_header));
                    item.as_collapsible_header().set_details(
                        cx,
                        self.is_invited_rooms_header_expanded,
                        HeaderCategory::Invites,
                        self.displayed_invited_rooms.len() as u64,
                    );
                    item.draw_all(cx, &mut scope);
                }
                else if let Some(invited_room_id) = get_invited_room_id(portal_list_index) {
                    if let Some(invited_room) =
                        self.invited_rooms.borrow_mut().get_mut(invited_room_id)
                    {
                        let item = list.item(cx, portal_list_index, live_id!(room_preview));
                        invited_room.is_selected =
                            self.current_active_room.as_deref() == Some(invited_room_id);
                        // Pass the room info down to the RoomPreview widget via Scope.
                        scope = Scope::with_props(&*invited_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, live_id!(empty))
                            .draw_all(cx, &mut scope);
                    }
                }
                else if direct_rooms_indexes.header_index == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, live_id!(collapsible_header));
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
                        let item = list.item(cx, portal_list_index, live_id!(room_preview));
                        direct_room.is_selected =
                            self.current_active_room.as_ref() == Some(direct_room_id);

                        // Paginate the room if it hasn't been paginated yet.
                        if PREPAGINATE_VISIBLE_ROOMS && !direct_room.has_been_paginated {
                            direct_room.has_been_paginated = true;
                            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                                room_id: direct_room.room_id.clone(),
                                num_events: 50,
                                direction: PaginationDirection::Backwards,
                            });
                        }
                        // Pass the room info down to the RoomPreview widget via Scope.
                        scope = Scope::with_props(&*direct_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, live_id!(empty))
                            .draw_all(cx, &mut scope);
                    }
                }
                else if regular_rooms_indexes.header_index == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, live_id!(collapsible_header));
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
                        let item = list.item(cx, portal_list_index, live_id!(room_preview));
                        regular_room.is_selected =
                            self.current_active_room.as_ref() == Some(regular_room_id);

                        // Paginate the room if it hasn't been paginated yet.
                        if PREPAGINATE_VISIBLE_ROOMS && !regular_room.has_been_paginated {
                            regular_room.has_been_paginated = true;
                            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                                room_id: regular_room.room_id.clone(),
                                num_events: 50,
                                direction: PaginationDirection::Backwards,
                            });
                        }
                        // Pass the room info down to the RoomPreview widget via Scope.
                        scope = Scope::with_props(&*regular_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, live_id!(empty)).draw_all(cx, &mut scope);
                    }
                }
                // Draw the status label as the bottom entry.
                else if portal_list_index == status_label_id {
                    let item = list.item(cx, portal_list_index, live_id!(status_label));
                    item.as_view().apply_over(cx, live!{
                        height: Fit,
                        label = { text: (&self.status) }
                    });
                    item.draw_all(cx, &mut scope);
                }
                // Draw a filler entry to take up space at the bottom of the portal list.
                else {
                    list.item(cx, portal_list_index, live_id!(bottom_filler))
                        .draw_all(cx, &mut scope);
                }
            }
        }

        DrawStep::done()
    }
}

impl RoomsListRef {
    /// See [`RoomsList::all_known_rooms_loaded()`].
    pub fn all_known_rooms_loaded(&self) -> bool {
        let Some(inner) = self.borrow() else {
            return false;
        };
        inner.all_known_rooms_loaded()
    }

    /// See [`RoomsList::is_room_loaded()`].
    pub fn is_room_loaded(&self, room_id: &OwnedRoomId) -> bool {
        let Some(inner) = self.borrow() else {
            return false;
        };
        inner.is_room_loaded(room_id)
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
