use std::{collections::HashMap, ops::Deref};
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::ruma::{MilliSecondsSinceUnixEpoch, OwnedRoomId};

use crate::{app::AppState, sliding_sync::{submit_async_request, MatrixRequest, PaginationDirection}};

use super::{room_preview::RoomPreviewAction, rooms_sidebar::{RoomsSideBarAction, RoomsSideBarFilter}};

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
            auto_tail: false
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

/// A filter function that is called for each room to determine whether it should be displayed.
///
/// If the function returns `true`, the room is displayed; otherwise, it is not shown.
/// The default value is a filter function that always returns `true`.
///
/// ## Example
/// The following example shows how to create and apply a filter function
/// that only displays rooms that have a displayable name starting with the letter "M":
/// ```rust,norun
/// rooms_list.display_filter = RoomDisplayFilter(Box::new(
///     |room| room.room_name.as_ref().is_some_and(|n| n.starts_with("M"))
/// ));
/// rooms_list.displayed_rooms = rooms_list.all_rooms.iter()
///    .filter(|(_, room)| (rooms_list.display_filter)(room))
///    .collect();
/// // Then redraw the rooms_list widget.
/// ```
pub struct RoomDisplayFilter(Box<dyn Fn(&RoomPreviewEntry) -> bool>);
impl Default for RoomDisplayFilter {
    fn default() -> Self {
        RoomDisplayFilter(Box::new(|_| true))
    }
}
impl Deref for RoomDisplayFilter {
    type Target = Box<dyn Fn(&RoomPreviewEntry) -> bool>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct RoomsList {
    #[deref] view: View,

    /// The single set of all known rooms and their cached preview info.
    #[rust] all_rooms: HashMap<OwnedRoomId, RoomPreviewEntry>,

    /// The currently-active filter function for the list of rooms.
    ///
    /// Note: for performance reasons, this does not get automatically applied
    /// when its value changes. Instead, you must manually invoke it on the set of `all_rooms`
    /// in order to update the set of `displayed_rooms` accordingly.
    #[rust] display_filter: RoomDisplayFilter,
    
    /// The list of rooms currently displayed in the UI, in order from top to bottom.
    /// This must be a strict subset of the rooms present in `all_rooms`, and should be determined
    /// by applying the `display_filter` to the set of `all_rooms``.
    #[rust] displayed_rooms: Vec<OwnedRoomId>,

    /// Maps the WidgetUid of a `RoomPreview` to that room's index in the `displayed_rooms` vector.
    ///
    /// NOTE: this should only be modified by the draw routine, not anything else.
    #[rust] displayed_rooms_map: HashMap<WidgetUid, usize>,

    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,
    /// The index of the currently-selected room.
    #[rust] current_active_room_index: Option<usize>,
    /// The maximum number of rooms that will ever be loaded.
    #[rust] max_known_rooms: Option<u32>,
}

impl RoomsList {
    fn update_status_rooms_count(&mut self) {
        self.status = if let Some(max_rooms) = self.max_known_rooms {
            format!("Loaded {} of {} total rooms.", self.all_rooms.len(), max_rooms)
        } else {
            format!("Loaded {} rooms.", self.all_rooms.len())
        };
    }
    fn filter_rooms(&mut self, keywords: &str) {
        let keywords = keywords.to_lowercase();

        if keywords.is_empty() {
            self.display_filter = RoomDisplayFilter::default();
            self.displayed_rooms = self.all_rooms.keys().cloned().collect();
            return;
        }

        self.display_filter = RoomDisplayFilter(Box::new(move |room| {
            let room_name = room.room_name.as_ref().map(|n| n.to_lowercase());
            room_name.as_ref().map_or(false, |n| n.contains(&keywords))
        }));

        self.displayed_rooms = self.all_rooms.iter()
            .filter(|(_, room)| (self.display_filter)(&room))
            .map(|(room_id, _)| room_id.clone())
            .collect();
    }
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
                        let room_id = room.room_id.clone();
                        let should_display = (self.display_filter)(&room);
                        let _replaced = self.all_rooms.insert(room_id.clone(), room);
                        if let Some(_old_room) = _replaced {
                            error!("BUG: Added room {room_id} that already existed");
                        } else {
                            if should_display {
                                self.displayed_rooms.push(room_id);
                            }
                        }
                    }
                    RoomsListUpdate::UpdateRoomAvatar { room_id, avatar } => {
                        if let Some(room) = self.all_rooms.get_mut(&room_id) {
                            room.avatar = avatar;
                        } else {
                            error!("Error: couldn't find room {room_id} to update avatar");
                        }
                    }
                    RoomsListUpdate::UpdateLatestEvent { room_id, timestamp, latest_message_text } => {
                        if let Some(room) = self.all_rooms.get_mut(&room_id) {
                            room.latest = Some((timestamp, latest_message_text));
                        } else {
                            error!("Error: couldn't find room {room_id} to update latest event");
                        }
                    }
                    RoomsListUpdate::UpdateRoomName { room_id, new_room_name } => {
                        if let Some(room) = self.all_rooms.get_mut(&room_id) {
                            let was_displayed = (self.display_filter)(&room);
                            room.room_name = Some(new_room_name);
                            let should_display = (self.display_filter)(&room);
                            match (was_displayed, should_display) {
                                (true, true) | (false, false) => {
                                    // No need to update the displayed rooms list.
                                }
                                (true, false) => {
                                    // Room was displayed but should no longer be displayed.
                                    self.displayed_rooms.retain(|r| r != &room_id);
                                }
                                (false, true) => {
                                    // Room was not displayed but should now be displayed.
                                    self.displayed_rooms.push(room_id);
                                }
                            }
                        } else {
                            error!("Error: couldn't find room {room_id} to update room name");
                        }
                    }
                    RoomsListUpdate::RemoveRoom(room_id) => {
                        self.all_rooms
                            .remove(&room_id)
                            .and_then(|_removed|
                                self.displayed_rooms.iter().position(|r| r == &room_id)
                            )
                            .map(|index_to_remove| {
                                // Remove the room from the list of displayed rooms.
                                self.displayed_rooms.remove(index_to_remove);
                            })
                            .unwrap_or_else(|| {
                                error!("Error: couldn't find room {room_id} to remove room");
                            });

                        // TODO: send an action to the RoomScreen to hide this room
                        //       if it is currently being displayed,
                        //       and also ensure that the room's TimelineUIState is preserved
                        //       and saved (if the room has not been left),
                        //       and also that it's MediaCache instance is put into a special state
                        //       where its internal update sender gets replaced upon next usage
                        //       (that is, upon the next time that same room is opened by the user).
                    }
                    RoomsListUpdate::ClearRooms => {
                        self.all_rooms.clear();
                        self.displayed_rooms.clear();
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

                let Some(displayed_room_index) = self.displayed_rooms_map
                    .iter()
                    .find(|&(&room_widget_uid, _)| widget_action.widget_uid_eq(room_widget_uid).is_some())
                    .map(|(_, &room_index)| room_index)
                else {
                    error!("BUG: couldn't find displayed index of clicked room for widget action {widget_action:?}");
                    continue;
                };
                let Some(room_details) = self.displayed_rooms
                    .get(displayed_room_index)
                    .and_then(|room_id| self.all_rooms.get(room_id))
                else {
                    error!("BUG: couldn't get room details for room at displayed index {displayed_room_index}");
                    continue;
                };

                self.current_active_room_index = Some(displayed_room_index);
                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    RoomListAction::Selected {
                        room_index: displayed_room_index,
                        room_id: room_details.room_id.to_owned(),
                        room_name: room_details.room_name.clone(),
                    }
                );
                self.redraw(cx);
            }
        }
        self.widget_match_event(cx, event, scope);
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Override the current active room index if the app state has a different selected room
        if let Some(room) = app_state.rooms_panel.selected_room.as_ref() {
            if let Some(room_index) = self.displayed_rooms.iter().position(|r| r == &room.id) {
                self.current_active_room_index = Some(room_index);
            }
        } else {
            self.current_active_room_index = None;
        }

        let count = self.displayed_rooms.len();
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
                // Draw the room preview for each room in the `displayed_rooms` list.
                let room_to_draw = self.displayed_rooms
                    .get(item_id)
                    .and_then(|room_id| self.all_rooms.get_mut(room_id));
                let item = if let Some(room_info) = room_to_draw {
                    let item = list.item(cx, item_id, live_id!(room_preview));
                    self.displayed_rooms_map.insert(item.widget_uid(), item_id);
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

                    // Pass the room info down to the RoomPreview widget via Scope.
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
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions, _scope: &mut Scope) {
        for action in actions {
            if let RoomsSideBarAction::Filter { keywords, filter } = action.as_widget_action().cast() {
                if filter == RoomsSideBarFilter::Rooms {
                    self.filter_rooms(&keywords);
                }
            }
        }
    }
}