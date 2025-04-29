use std::collections::HashMap;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{ruma::{events::tag::Tags, MilliSecondsSinceUnixEpoch, OwnedRoomAliasId, OwnedRoomId, OwnedUserId}, Room};
use crate::{
    app::AppState,
    room::room_display_filter::{FilterableRoom, RoomDisplayFilter, RoomDisplayFilterBuilder, RoomFilterCriteria, SortFn},
    shared::{collapsible_header::{CollapsibleHeaderAction, CollapsibleHeaderWidgetRefExt, HeaderCategory},
    jump_to_bottom_button::UnreadMessageCount},
    sliding_sync::{submit_async_request, MatrixRequest, PaginationDirection},
};

use super::{room_preview::RoomPreviewAction, rooms_sidebar::RoomsViewAction};

/// Whether to pre-paginate visible rooms at least once in order to
/// be able to display the latest message in the room preview,
/// and to have something to immediately show when a user first opens a room.
const PREPAGINATE_VISIBLE_ROOMS: bool = true;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::search_bar::SearchBar;
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
    /// Remove the given room from the list of all rooms.
    RemoveRoom(OwnedRoomId),
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


#[derive(Debug, Clone, DefaultNone)]
pub enum RoomsListAction {
    Selected {
        room_id: OwnedRoomId,
        room_name: Option<String>,
    },
    None,
}

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
}

/// A room that the user has been invited to.
pub struct InvitedRoomInfo {
    pub room: Room,
    /// The avatar for this room: either an array of bytes holding the avatar image
    /// or a string holding the first Unicode character of the room name.
    pub room_avatar: RoomPreviewAvatar,
    /// Info about the user who invited us to this room, if available.
    pub inviter_info: Option<InviterInfo>,
    /// The timestamp and Html text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    /// Whether this room is currently selected in the UI.
    pub is_selected: bool,
}

/// Info about the user who invited us to a room.
pub struct InviterInfo {
    pub user_id: OwnedUserId,
    pub display_name: Option<String>,
    pub avatar: Option<Vec<u8>>,
}

#[derive(Debug)]
pub enum RoomPreviewAvatar {
    Text(String),
    Image(Vec<u8>),
}
impl Default for RoomPreviewAvatar {
    fn default() -> Self {
        RoomPreviewAvatar::Text(String::new())
    }
}


#[derive(Live, LiveHook, Widget)]
pub struct RoomsList {
    #[deref] view: View,

    /// The list of all rooms that the user has been invited to.
    #[rust] invited_rooms: HashMap<OwnedRoomId, InvitedRoomInfo>,

    /// The set of all joined rooms and their cached preview info.
    #[rust] all_joined_rooms: HashMap<OwnedRoomId, JoinedRoomInfo>,

    /// The currently-active filter function for the list of rooms.
    ///
    /// Note: for performance reasons, this does not get automatically applied
    /// when its value changes. Instead, you must manually invoke it on the set of `all_joined_rooms`
    /// in order to update the set of `displayed_rooms` accordingly.
    #[rust] display_filter: RoomDisplayFilter,

    /// The list of invited rooms currently displayed in the UI, in order from top to bottom.
    /// This is a strict subset of the rooms present in `all_invited_rooms`, and should be determined
    /// by applying the `display_filter` to the set of `all_invited_rooms`.
    #[rust] displayed_invited_rooms: Vec<OwnedRoomId>,
    #[rust(true)] is_invited_rooms_header_expanded: bool,

    /// The list of joined rooms currently displayed in the UI, in order from top to bottom.
    /// This is a strict subset of the rooms present in `all_joined_rooms`, and should be determined
    /// by applying the `display_filter` to the set of `all_joined_rooms`.
    #[rust] displayed_joined_rooms: Vec<OwnedRoomId>,
    #[rust(true)] is_joined_rooms_header_expanded: bool,

    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,
    /// The ID of the currently-selected room.
    #[rust] current_active_room: Option<OwnedRoomId>,
    /// The maximum number of rooms that will ever be loaded.
    #[rust] max_known_rooms: Option<u32>,
}

impl RoomsList {

    /// Determines if all known rooms have been loaded.
    ///
    /// Returns `true` if the number of rooms in `all_joined_rooms` and `invited_rooms` equals or exceeds
    /// `max_known_rooms`, or `false` if `max_known_rooms` is `None`.
    pub fn all_known_rooms_loaded(&self) -> bool {
        self.max_known_rooms.is_some_and(|max_rooms| self.all_joined_rooms.len() + self.invited_rooms.len() >= max_rooms as usize)
    }
    /// Returns `true` if the given `room_id` is already in the `all_joined_rooms` and `invited_rooms` lists.
    /// and `false` if it is not.
    pub fn is_room_loaded(&self, room_id: &OwnedRoomId) -> bool {
        self.all_joined_rooms.contains_key(room_id) || self.invited_rooms.contains_key(room_id)
    }
    /// Handle all pending updates to the list of all rooms.
    fn handle_rooms_list_updates(&mut self, cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        let mut num_updates: usize = 0;
        while let Some(update) = PENDING_ROOM_UPDATES.pop() {
            num_updates += 1;
            match update {
                RoomsListUpdate::AddInvitedRoom(invited_room) => {
                    let room_id = invited_room.room.room_id().to_owned();
                    let should_display = (self.display_filter)(&invited_room);
                    let _replaced = self.invited_rooms.insert(room_id.clone(), invited_room);
                    if let Some(_old_room) = _replaced {
                        error!("BUG: Added invited room {room_id} that already existed");
                    } else {
                        if should_display {
                            self.displayed_invited_rooms.push(room_id);
                        }
                    }
                    self.update_status_rooms_count();
                }
                RoomsListUpdate::AddJoinedRoom(joined_room) => {
                    let room_id = joined_room.room_id.clone();
                    let should_display = (self.display_filter)(&joined_room);
                    let _replaced = self.all_joined_rooms.insert(room_id.clone(), joined_room);
                    if let Some(_old_room) = _replaced {
                        error!("BUG: Added joined room {room_id} that already existed");
                    } else {
                        if should_display {
                            self.displayed_joined_rooms.push(room_id);
                        }
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
                        error!("Error: couldn't find room {} to update unread messages count", room_id);
                    }
                }
                RoomsListUpdate::UpdateRoomName { room_id, new_room_name } => {
                    if let Some(room) = self.all_joined_rooms.get_mut(&room_id) {
                        let was_displayed = (self.display_filter)(room);
                        room.room_name = Some(new_room_name);
                        let should_display = (self.display_filter)(room);
                        match (was_displayed, should_display) {
                            (true, true) | (false, false) => {
                                // No need to update the displayed rooms list.
                            }
                            (true, false) => {
                                // Room was displayed but should no longer be displayed.
                                self.displayed_joined_rooms.retain(|r| r != &room_id);
                            }
                            (false, true) => {
                                // Room was not displayed but should now be displayed.
                                self.displayed_joined_rooms.push(room_id);
                            }
                        }
                    } else {
                        error!("Error: couldn't find room {room_id} to update room name");
                    }
                }
                RoomsListUpdate::RemoveRoom(room_id) => {
                    self.all_joined_rooms
                        .remove(&room_id)
                        .and_then(|_removed|
                            self.displayed_joined_rooms.iter().position(|r| r == &room_id)
                        )
                        .map(|index_to_remove| {
                            // Remove the room from the list of displayed rooms.
                            self.displayed_joined_rooms.remove(index_to_remove);
                        })
                        .unwrap_or_else(|| {
                            error!("Error: couldn't find room {room_id} to remove room");
                        });

                    self.update_status_rooms_count();

                    // TODO: send an action to the RoomScreen to hide this room
                    //       if it is currently being displayed,
                    //       and also ensure that the room's TimelineUIState is preserved
                    //       and saved (if the room has not been left),
                    //       and also that it's MediaCache instance is put into a special state
                    //       where its internal update sender gets replaced upon next usage
                    //       (that is, upon the next time that same room is opened by the user).
                }
                RoomsListUpdate::ClearRooms => {
                    self.all_joined_rooms.clear();
                    self.displayed_joined_rooms.clear();
                    self.invited_rooms.clear();
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
                    } else {
                        error!("Error: couldn't find room {room_id} to update tags");
                    }
                }
                RoomsListUpdate::Status { status } => {
                    self.status = status;
                }
            }
        }
        if num_updates > 0 {
            log!("RoomsList: processed {} updates to the list of all rooms", num_updates);
            self.redraw(cx);
        }
    }

    /// Updates the status message to show how many rooms have been loaded.
    fn update_status_rooms_count(&mut self) {
        self.status = if let Some(max_rooms) = self.max_known_rooms {
            format!("Loaded {} of {} total rooms.", self.all_joined_rooms.len(), max_rooms)
        } else {
            format!("Loaded {} rooms.", self.all_joined_rooms.len())
        };
    }

    /// Updates the status message to show how many rooms are currently displayed
    /// that match the current search filter.
    fn update_status_matching_rooms(&mut self) {
        let total = self.displayed_invited_rooms.len() + self.displayed_joined_rooms.len();
        self.status = match total {
            0 => "No matching rooms found.".to_string(),
            1 => "Found 1 matching room.".to_string(),
            n => format!("Found {} matching rooms.", n),
        }
    }

    /// Returns true if the given room is contained in any of the displayed room sets,
    /// i.e., either the invited rooms or the joined rooms.
    fn is_room_displayable(&self, room: &OwnedRoomId) -> bool {
        self.displayed_invited_rooms.contains(room)
        || self.displayed_joined_rooms.contains(room)
    }

    /// Updates the lists of displayed rooms based on the current search filter
    /// and redraws the RoomsList.
    fn update_displayed_rooms(&mut self, cx: &mut Cx, keywords: &str) {
        let portal_list = self.view.portal_list(id!(list));
        if keywords.is_empty() {
            // Reset the displayed rooms list to show all rooms.
            self.display_filter = RoomDisplayFilter::default();
            self.displayed_joined_rooms = self.all_joined_rooms.keys().cloned().collect();
            self.displayed_invited_rooms = self.invited_rooms.keys().cloned().collect();
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

        /// An inner function that generates a sorted, filtered list of rooms to display.
        fn generate_displayed_rooms<FR: FilterableRoom>(
            rooms_map: &HashMap<OwnedRoomId, FR>,
            display_filter: &RoomDisplayFilter,
            sort_fn: Option<&SortFn>,
        ) -> Vec<OwnedRoomId> {
            if let Some(sort_fn) = sort_fn {
                let mut filtered_rooms: Vec<_> = rooms_map
                    .iter()
                    .filter(|(_, room)| display_filter(*room))
                    .collect();
                filtered_rooms.sort_by(
                    |(_, room_a), (_, room_b)| sort_fn(*room_a, *room_b)
                );
                filtered_rooms
                    .into_iter()
                    .map(|(room_id, _)| room_id.clone())
                    .collect()
            } else {
                rooms_map
                    .iter()
                    .filter(|(_, room)| display_filter(*room))
                    .map(|(room_id, _)| room_id.clone())
                    .collect()
            }
        }

        // Update the displayed rooms list and redraw it.
        self.displayed_joined_rooms = generate_displayed_rooms(&self.all_joined_rooms, &self.display_filter, sort_fn.as_deref());
        self.displayed_invited_rooms = generate_displayed_rooms(&self.invited_rooms, &self.display_filter, sort_fn.as_deref());
        self.update_status_matching_rooms();
        portal_list.set_first_id_and_scroll(0, 0.0);
        self.redraw(cx);
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
                let Some(room_details) = self.all_joined_rooms.get(&clicked_room_id) else {
                    error!("BUG: couldn't get room details for room {clicked_room_id}");
                    continue;
                };

                self.current_active_room = Some(clicked_room_id.clone());
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    RoomsListAction::Selected {
                        room_id: room_details.room_id.to_owned(),
                        room_name: room_details.room_name.clone(),
                    }
                );
                self.redraw(cx);
            }
            else if let CollapsibleHeaderAction::Toggled { category } = list_action.as_widget_action().cast() {
                match category {
                    HeaderCategory::Invites => {
                        self.is_invited_rooms_header_expanded = !self.is_invited_rooms_header_expanded;
                    }
                    HeaderCategory::JoinedRooms => {
                        self.is_joined_rooms_header_expanded = !self.is_joined_rooms_header_expanded;
                    }
                    _todo => todo!("Handle other header categories"),
                }
                self.redraw(cx);
            }
        }

        if let Event::Actions(actions) = event {
            for action in actions {
                if let RoomsViewAction::Search(search_text) = action.as_widget_action().cast() {
                    self.update_displayed_rooms(cx, &search_text);
                }
            }
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Update the currently-selected room from the AppState data.
        self.current_active_room = app_state.rooms_panel.selected_room.as_ref()
            .map(|sel_room| sel_room.room_id.clone())
            .filter(|room_id| self.is_room_displayable(room_id));

        // Based on the various displayed room lists and is_expanded state of each room header,
        // calculate the indices in the PortalList where the headers and rooms should be drawn.
        let should_show_invited_rooms_header = !self.displayed_invited_rooms.is_empty();
        let should_show_joined_rooms_header = !self.displayed_joined_rooms.is_empty();

        let index_of_invited_rooms_header = should_show_invited_rooms_header.then_some(0);
        let index_of_first_invited_room = should_show_invited_rooms_header as usize;
        let index_after_invited_rooms = index_of_first_invited_room
            + if self.is_invited_rooms_header_expanded { self.displayed_invited_rooms.len() } else { 0 };
        let index_of_joined_rooms_header = should_show_joined_rooms_header.then_some(index_after_invited_rooms);
        let index_of_first_joined_room = index_after_invited_rooms + should_show_joined_rooms_header as usize;
        let index_after_joined_rooms = index_of_first_joined_room
            + if self.is_joined_rooms_header_expanded { self.displayed_joined_rooms.len() } else { 0 };
        let status_label_id = index_after_joined_rooms;
        let total_count = status_label_id + 1; // +1 for the status label

        let get_invited_room_id = |portal_list_index: usize| {
            let index = portal_list_index - index_of_first_invited_room;
            self.is_invited_rooms_header_expanded.then(||
                self.displayed_invited_rooms.get(index)
            )
            .flatten()
        };
        let get_joined_room_id = |portal_list_index: usize| {
            let index = portal_list_index - index_of_first_joined_room;
            self.is_joined_rooms_header_expanded.then(||
                self.displayed_joined_rooms.get(index)
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

                if index_of_invited_rooms_header == Some(portal_list_index) {
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
                    if let Some(invited_room) = self.invited_rooms.get_mut(invited_room_id)  {
                        let item = list.item(cx, portal_list_index, live_id!(room_preview));
                        invited_room.is_selected = self.current_active_room.as_deref() == Some(invited_room_id);
                        // Pass the room info down to the RoomPreview widget via Scope.
                        scope = Scope::with_props(&*invited_room);
                        item.draw_all(cx, &mut scope);
                    } else {
                        list.item(cx, portal_list_index, live_id!(empty)).draw_all(cx, &mut scope);
                    }
                }
                else if index_of_joined_rooms_header == Some(portal_list_index) {
                    let item = list.item(cx, portal_list_index, live_id!(collapsible_header));
                    item.as_collapsible_header().set_details(
                        cx,
                        self.is_joined_rooms_header_expanded,
                        HeaderCategory::JoinedRooms,
                        0, // TODO: sum up all the unread mentions in all displayed joined rooms
                        // NOTE: this might be really slow, so we should maintain a running total of mentions in this struct
                    );
                    item.draw_all(cx, &mut scope);
                }
                else if let Some(joined_room_id) = get_joined_room_id(portal_list_index) {
                    if let Some(joined_room) = self.all_joined_rooms.get_mut(joined_room_id) {
                        let item = list.item(cx, portal_list_index, live_id!(room_preview));
                        joined_room.is_selected = self.current_active_room.as_ref() == Some(joined_room_id);

                        // Paginate the room if it hasn't been paginated yet.
                        if PREPAGINATE_VISIBLE_ROOMS && !joined_room.has_been_paginated {
                            joined_room.has_been_paginated = true;
                            submit_async_request(MatrixRequest::PaginateRoomTimeline {
                                room_id: joined_room.room_id.clone(),
                                num_events: 50,
                                direction: PaginationDirection::Backwards,
                            });
                        }
                        // Pass the room info down to the RoomPreview widget via Scope.
                        scope = Scope::with_props(&*joined_room);
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
    pub fn all_known_rooms_loaded(
        &self,
    ) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.all_known_rooms_loaded()
    }
    /// See [`RoomsList::is_room_loaded()`].
    pub fn is_room_loaded(&self, room_id: &OwnedRoomId) -> bool {
        let Some(inner) = self.borrow() else { return false };
        inner.is_room_loaded(room_id)
    }
}
pub struct RoomsListScopeProps {
    /// Whether the RoomsList's inner PortalList was scrolling
    /// when the latest finger down event occurred.
    pub was_scrolling: bool,
}
