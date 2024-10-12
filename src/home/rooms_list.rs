use std::collections::HashMap;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::{room, ruma::{api::client::filter, MilliSecondsSinceUnixEpoch, OwnedRoomAliasId, OwnedRoomId}};
use crate::{app::AppState, sliding_sync::{submit_async_request, MatrixRequest, PaginationDirection}};

use super::{room_preview::RoomPreviewAction, rooms_sidebar::{RoomsSideBarFilter, RoomsSideBarAction}};

/// Whether to pre-paginate visible rooms at least once in order to
/// be able to display the latest message in the room preview,
/// and to have something to immediately show when a user first opens a room.
const PREPAGINATE_VISIBLE_ROOMS: bool = true;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::search_bar::SearchBar;
    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::avatar::Avatar;
    import crate::shared::html_or_plaintext::HtmlOrPlaintext;
    
    import crate::home::room_preview::*;

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

    RoomsList = {{RoomsList}} {
        width: Fill, height: Fill
        flow: Down

        list = <PortalList> {
            keep_invisible: false
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

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
    /// Add a new room to the list of all rooms.
    AddRoom(RoomPreviewEntry),
    /// Clear all rooms in the list of all rooms.
    ClearRooms,
    /// Update the latest event content and timestamp for the given room.
    UpdateLatestEvent {
        room_id: OwnedRoomId,
        timestamp: MilliSecondsSinceUnixEpoch,
        /// The Html-formatted text preview of the latest message.
        latest_message_text: String,
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

pub type RoomIndex = usize;


#[derive(Debug, Clone, DefaultNone)]
pub enum RoomListAction {
    Selected {
        /// The index (into the `all_rooms` vector) of the selected `RoomPreviewEntry`.
        room_index: RoomIndex,
        room_id: OwnedRoomId,
        room_name: Option<String>,
    },
    None,
}

#[derive(Debug)]
pub struct RoomPreviewEntry {
    /// The matrix ID of this room.
    pub room_id: OwnedRoomId,
    /// The displayable name of this room, if known.
    pub room_name: Option<String>,
    /// The main alias for this room, if known.
    pub cannonical_alias: Option<OwnedRoomAliasId>,
    /// A list of alternative aliases for this room.
    pub alt_aliases: Vec<OwnedRoomAliasId>,
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

    /// The list of all known rooms and their cached preview info.
    //
    // TODO: change this into a hashmap keyed by room ID.
    #[rust] all_rooms: Vec<RoomPreviewEntry>,
    /// Maps the WidgetUid of a `RoomPreview` to that room's index in the `all_rooms` vector.
    #[rust] rooms_list_map: HashMap<u64, usize>,
    /// Maps the OwnedRoomId to the index of the room in the `all_rooms` vector.
    #[rust] rooms_list_owned_room_id_map: HashMap<OwnedRoomId, usize>,
    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,
    /// The index of the currently selected room
    #[rust] current_active_room_index: Option<usize>,
    /// The list of indices of the currently filtered rooms.
    #[rust] current_filtered_rooms_indices: Vec<usize>,
    /// The maximum number of rooms that will ever be loaded.
    #[rust] max_known_rooms: Option<u32>,
}

#[derive(Debug)]
pub enum RoomsFilterCondition {
    All,
    /// Filter by room name.
    RoomName,
    /// Filter by room alias, either the cannonical alias or any of the alternative aliases.
    RoomAlias,
}


impl Widget for RoomsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Process all pending updates to the list of all rooms, and then redraw it.
        {
            let mut num_updates: usize = 0;
            while let Some(update) = PENDING_ROOM_UPDATES.pop() {
                num_updates += 1;
                match update {
                    RoomsListUpdate::AddRoom(room) => {
                        let room_index = self.all_rooms.len();
                        self.rooms_list_owned_room_id_map.insert(room.room_id.clone(), room_index);
                        self.all_rooms.push(room);
                    }
                    RoomsListUpdate::UpdateRoomAvatar { room_id, avatar } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| &r.room_id == &room_id) {
                            room.avatar = avatar;
                        } else {
                            error!("Error: couldn't find room {room_id} to update avatar");
                        }
                    }
                    RoomsListUpdate::UpdateLatestEvent { room_id, timestamp, latest_message_text } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| &r.room_id == &room_id) {
                            room.latest = Some((timestamp, latest_message_text));
                        } else {
                            error!("Error: couldn't find room {room_id} to update latest event");
                        }
                    }
                    RoomsListUpdate::UpdateRoomName { room_id, new_room_name } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| &r.room_id == &room_id) {
                            room.room_name = Some(new_room_name);
                        } else {
                            error!("Error: couldn't find room {room_id} to update room name");
                        }
                    }
                    RoomsListUpdate::RemoveRoom(room_id) => {
                        if let Some(idx) = self.all_rooms.iter().position(|r| &r.room_id == &room_id) {
                            self.all_rooms.remove(idx);
                        } else {
                            error!("Error: couldn't find room {room_id} to remove room");
                        }
                    }
                    RoomsListUpdate::ClearRooms => {
                        self.all_rooms.clear();
                    }
                    RoomsListUpdate::NotLoaded => {
                        self.status = "Loading rooms (waiting for homeserver)...".to_string();
                    }
                    RoomsListUpdate::LoadedRooms { max_rooms } => {
                        self.max_known_rooms = max_rooms;
                        self.update_status_rooms_count();
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

        // Now, handle any actions on this widget, e.g., a user selecting a room.
        let widget_uid = self.widget_uid();
        for list_action in cx.capture_actions(|cx| self.view.handle_event(cx, event, scope)) {
            if let RoomPreviewAction::Click = list_action.as_widget_action().cast() {
                let widget_action = list_action.as_widget_action();

                if let Some(room_index) = self.rooms_list_map
                    .iter()
                    .find(|&(&room_widget_uid, _)| widget_action.widget_uid_eq(WidgetUid(room_widget_uid)).is_some())
                    .map(|(_, &room_index)| room_index)
                {
                    let room_details = &self.all_rooms[room_index];
                    self.current_active_room_index = Some(room_index);
                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        RoomListAction::Selected {
                            room_index,
                            room_id: room_details.room_id.to_owned(),
                            room_name: room_details.room_name.clone(),
                        }
                    );
                    self.redraw(cx);
                }
            }
        }

        self.widget_match_event(cx, event, scope);
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // TODO: sort list of `all_rooms` by alphabetic, most recent message, grouped by spaces, etc
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Override the current active room index if the app state has a different selected room
        if let Some(room) = app_state.rooms_panel.selected_room.as_ref() {
            if let Some(room_index) = self.rooms_list_owned_room_id_map.get(&room.id) {
                self.current_active_room_index = Some(*room_index);
            }
        } else {
            self.current_active_room_index = None;
        }

        let count = self.all_rooms.len();
        // if !self.current_filtered_rooms_indices.is_empty() {
        //     count = self.current_filtered_rooms_indices.len();
        // }

        let status_label_id = count;

        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };

            // Add 1 for the status label at the bottom.
            list.set_item_range(cx, 0, count + 1);

            while let Some(item_id) = list.next_visible_item(cx) {
                let mut scope = Scope::empty();

                // Draw the room preview for each room.
                let item = if let Some(room_info) = self.all_rooms.get_mut(item_id) {
                    let item = list.item(cx, item_id, live_id!(room_preview));
                    self.rooms_list_map.insert(item.widget_uid().0, item_id);
                    room_info.is_selected = self.current_active_room_index == Some(item_id);

                    // Paginate the room if it hasn't been paginated yet.
                    if PREPAGINATE_VISIBLE_ROOMS && !room_info.has_been_paginated {
                        room_info.has_been_paginated = true;
                        submit_async_request(MatrixRequest::PaginateRoomTimeline {
                            room_id: room_info.room_id.clone(),
                            num_events: 50,
                            direction: PaginationDirection::Backwards,
                        });
                    }

                    // Pass the room info through Scope down to the RoomPreview widget.
                    scope = Scope::with_props(&*room_info);

                    item
                }
                // Draw the status label as the bottom entry.
                else if item_id == status_label_id {
                    let item = list.item(cx, item_id, live_id!(status_label));
                    item.as_view().apply_over(cx, live!{
                        height: Fit,
                        label = { text: (&self.status) }
                    });
                    item
                }
                // Draw a filler entry to take up space at the bottom of the portal list.
                else {
                    list.item(cx, item_id, live_id!(bottom_filler))
                };

                item.draw_all(cx, &mut scope);
            }
        }

        DrawStep::done()
    }

}

impl WidgetMatchEvent for RoomsList {

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        // Handle any actions on this widget, e.g., a user filtering the list of rooms.
        for action in actions {
            // Handle the filter action for the rooms view.
            if let RoomsSideBarAction::Filter {
                value,
                filter_type
            } = action.as_widget_action().cast() {
                // we only handle the filter action for the rooms view
                if let RoomsSideBarFilter::Rooms = filter_type {
                    if value.is_empty() {
                        self.current_filtered_rooms_indices = (0..self.all_rooms.len()).collect();
                    } else {
                        let value = value.trim().to_lowercase();
                        // We only filter by room name for now.
                        let filtered_rooms_index = self.filter_rooms(&value, RoomsFilterCondition::RoomName);
                        log!("Filtering rooms by room name: {:?}", filtered_rooms_index);
                        self.current_filtered_rooms_indices = filtered_rooms_index;
                    }

                    self.redraw(cx);
                }
            }
        }
    }
}

impl RoomsList {
    fn update_status_rooms_count(&mut self) {
        self.status = if let Some(max_rooms) = self.max_known_rooms {
            format!("Loaded {} of {} total rooms.", self.all_rooms.len(), max_rooms)
        } else {
            format!("Loaded {} rooms.", self.all_rooms.len())
        };
    }

    pub fn filter_rooms(&self, keywords: &str, filter_condition: RoomsFilterCondition) -> Vec<usize> {

        let filtered_rooms_indices = match filter_condition {
            RoomsFilterCondition::All => todo!(),
            RoomsFilterCondition::RoomName => self.filter_rooms_by_room_name(keywords),
            RoomsFilterCondition::RoomAlias => self.filter_rooms_by_room_alias(keywords),
        };

        filtered_rooms_indices
    }

    /// Filter rooms by room name.
    pub fn filter_rooms_by_room_name(&self, keywords: &str) -> Vec<usize> {
        let mut filtered_rooms_indices = Vec::new();

        self
        .all_rooms
        .iter()
        .enumerate()
        .for_each(|(index, room_info)| {
            if let Some(room_name) = &room_info.room_name {
                if room_name.to_lowercase().contains(&keywords) {
                    filtered_rooms_indices.push(index);
                }
            }
        });

        filtered_rooms_indices
    }

    /// Filter rooms by room alias, either the cannonical alias or any of the alternative aliases.
    pub fn filter_rooms_by_room_alias(&self, keywords: &str) -> Vec<usize> {
        let mut filtered_rooms_indices = Vec::new();

        self
        .all_rooms
        .iter()
        .enumerate()
        .for_each(|(index, room_info)| {

            if let Some(cannonical_alias) = &room_info.cannonical_alias {
                if cannonical_alias.to_string().to_lowercase().contains(&keywords) {
                    filtered_rooms_indices.push(index);
                }
            } else {
                room_info.alt_aliases.iter().for_each(|alias| {
                    if alias.to_string().to_lowercase().contains(&keywords) {
                        filtered_rooms_indices.push(index);
                    }
                });
            }

        });

        filtered_rooms_indices
    }
}