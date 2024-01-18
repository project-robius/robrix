use std::collections::HashMap;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, MilliSecondsSinceUnixEpoch};
use crate::shared::avatar::AvatarWidgetRefExt;
use crate::shared::clickable_view::*;
use crate::shared::stack_view_action::StackViewAction;
use crate::utils::{unix_time_millis_to_datetime, self};

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::view::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::search_bar::SearchBar;
    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::stack_navigation::StackNavigation;
    import crate::shared::clickable_view::ClickableView;
    import crate::shared::avatar::Avatar;

    RoomPreview = <ClickableView> {
        flow: Right, spacing: 10., padding: 10.
        width: Fill, height: Fit

        avatar = <Avatar> {}

        preview = <View> {
            width: Fill, height: Fit
            flow: Down, spacing: 7.

            room_name = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{}
                }
                text: "[Room name unknown]"
            }

            latest_message = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    wrap: Ellipsis,
                    text_style: <REGULAR_TEXT>{
                        font_size: 10.5
                    },
                }
                text: "[Latest message unknown]"
            }
        }

        timestamp = <Label> {
            width: Fit, height: Fit
            draw_text:{
                text_style: <REGULAR_TEXT>{
                    font_size: 8.
                },
            }
            text: "[Timestamp unknown]"
        }
    }

    // An empty view that takes up no space in the portal list.
    Empty = <View> { }

    StatusLabel = <View> {
        width: Fill, height: 80.0,
        align: { x: 0.5, y: 0.5 }
        draw_bg: {
            color: #f4f4f4
        }
        show_bg: true,

        label = <Label> {
            draw_text: {
                text_style: <REGULAR_TEXT>{}
            }
            text: "Loading joined rooms..."
        }
    }

    RoomsList = {{RoomsList}} {
        width: Fill, height: Fill
        flow: Down

        list = <PortalList> {
            keep_invisible: false
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            search_bar = <SearchBar> {}
            room_preview = <RoomPreview> {}
            empty = <Empty> {}
            status_label = <StatusLabel> {}
            bottom_filler = <View> {
                width: Fill,
                height: 100.0,
                draw_bg: {
                    color: #f4f4f4
                }
                show_bg: true,
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
    /// Add a new room to the list of all rooms.
    AddRoom(RoomPreviewEntry),
    /// Update the latest event content and timestamp for the given room.
    UpdateLatestEvent {
        room_id: OwnedRoomId,
        timestamp: MilliSecondsSinceUnixEpoch,
        latest_message_text: String,
    },
    /// Update the displayable name for the given room.
    UpdateRoomName {
        room_id: OwnedRoomId,
        room_name: String,
    },
    /// Update the avatar (image) for the given room.
    UpdateRoomAvatar {
        room_id: OwnedRoomId,
        avatar: RoomPreviewAvatar,
    },
    /// Remove the given room from the list of all rooms.
    #[allow(unused)]
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

#[derive(Debug, Default)]
pub struct RoomPreviewEntry {
    /// The matrix ID of this room.
    pub room_id: Option<OwnedRoomId>,
    /// The displayable name of this room, if known.
    pub room_name: Option<String>,
    /// The timestamp and text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    /// The avatar for this room: either an array of bytes holding the avatar image
    /// or a string holding the first Unicode character of the room name.
    pub avatar: RoomPreviewAvatar,

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
    #[rust] all_rooms: Vec<RoomPreviewEntry>,
    /// Maps the WidgetUid of a `RoomPreview` to that room's index in the `all_rooms` vector.
    #[rust] rooms_list_map: HashMap<u64, usize>,
    /// The latest status message that should be displayed in the bottom status label.
    #[rust] status: String,
}

impl Widget for RoomsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Currently, a Signal event is only used to tell this widget
        // that the rooms list has been updated in the background.
        if let Event::Signal = event {
            // Process all pending updates to the list of all rooms, and then redraw it.
            let mut num_updates: usize = 0;
            while let Some(update) = PENDING_ROOM_UPDATES.pop() {
                num_updates += 1;
                match update {
                    RoomsListUpdate::AddRoom(room) => {
                        self.all_rooms.push(room);
                    }
                    RoomsListUpdate::UpdateRoomAvatar { room_id, avatar } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id.as_ref() == Some(&room_id)) {
                            room.avatar = avatar;
                        } else {
                            eprintln!("Error: couldn't find room {room_id} to update avatar");
                        }
                    }
                    RoomsListUpdate::UpdateLatestEvent { room_id, timestamp, latest_message_text } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id.as_ref() == Some(&room_id)) {
                            room.latest = Some((timestamp, latest_message_text));
                        } else {
                            eprintln!("Error: couldn't find room {room_id} to update latest event");
                        }
                    }
                    RoomsListUpdate::UpdateRoomName { room_id, room_name } => {
                        if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id.as_ref() == Some(&room_id)) {
                            room.room_name = Some(room_name);
                        } else {
                            eprintln!("Error: couldn't find room {room_id} to update room name");
                        }
                    }
                    RoomsListUpdate::RemoveRoom(room_id) => {
                        if let Some(idx) = self.all_rooms.iter().position(|r| r.room_id.as_ref() == Some(&room_id)) {
                            self.all_rooms.remove(idx);
                        } else {
                            eprintln!("Error: couldn't find room {room_id} to remove room");
                        }
                    }
                    RoomsListUpdate::Status { status } => {
                        self.status = status;
                    }
                }
            }
            if num_updates > 0 {
                println!("RoomsList: processed {} updates to the list of all rooms", num_updates);
                self.redraw(cx);
            }
        }

        // Now, handle any actions on this widget, e.g., a user selecting a room.
        let widget_uid = self.widget_uid();
        for list_action in cx.capture_actions(|cx| self.view.handle_event(cx, event, scope)) {
            if let ClickableViewAction::Click = list_action.as_widget_action().cast() {
                let widget_action = list_action.as_widget_action();

                if let Some(room_index) = self.rooms_list_map
                    .iter()
                    .find(|&(&room_widget_uid, _)| widget_action.widget_uid_eq(WidgetUid(room_widget_uid)).is_some())
                    .map(|(_, &room_index)| room_index)
                {
                    let room_details = &self.all_rooms[room_index];
                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        RoomListAction::Selected {
                            room_index,
                            room_id: room_details.room_id.clone().unwrap(),
                            room_name: room_details.room_name.clone(),
                        }
                    );

                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        StackViewAction::ShowRoom,
                    );
                }
            }
        }
    }


    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {

        // TODO: sort list of `all_rooms` by alphabetic, most recent message, grouped by spaces, etc

        let count = self.all_rooms.len() as u64;
        let last_item_id = count + 1; // Add 1 for the search bar up top.

        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };
        
            // Add 1 again for the status label at the bottom.
            list.set_item_range(cx, 0, last_item_id + 1);

            while let Some(item_id) = list.next_visible_item(cx) {
                // Draw the search bar as the top entry.
                let item = if item_id == 0 {
                    list.item(cx, item_id, live_id!(search_bar)).unwrap()
                }
                // Draw the status label as the bottom entry.
                else if item_id == last_item_id {
                    let item = list.item(cx, item_id, live_id!(status_label)).unwrap();
                    if count > 0 {
                        let text = format!("Found {count} joined rooms.");
                        println!("DEBUG: writing status label {text}");
                        item.label(id!(label)).set_text(&text);
                        println!("\t DEBUG: wrote status label {text}");
                    } else {
                        println!("DEBUG: writing status label {}", self.status);
                        item.label(id!(label)).set_text(&self.status);
                        println!("\t DEBUG: wrote status label {}", self.status);
                    }
                    item
                }
                // Draw a filler entry to take up space at the bottom of the portal list.
                else if item_id > last_item_id {
                    list.item(cx, item_id, live_id!(bottom_filler)).unwrap()
                }
                // Draw actual room preview entries.
                else {
                    let item = list.item(cx, item_id, live_id!(room_preview)).unwrap();
                    let index_of_room = item_id as usize - 1; // -1 to account for the search bar
                    let room_info = &self.all_rooms[index_of_room];
    
                    self.rooms_list_map.insert(item.widget_uid().0, index_of_room);
    
                    if let Some(ref name) = room_info.room_name {
                        println!("DEBUG: writing room name {name}");
                        item.label(id!(preview.room_name)).set_text(name);
                    }
                    if let Some((ts, msg)) = room_info.latest.as_ref() {
                        if let Some(dt) = unix_time_millis_to_datetime(ts) {
                            let text = format!("{} {}", dt.date(), dt.time().format("%l:%M %P"));
                            println!("DEBUG: writing datetime {text}");
                            item.label(id!(timestamp)).set_text(&text);
                        }
                        println!("DEBUG: writing latest message {msg}");
                        item.label(id!(preview.latest_message)).set_text(msg);
                    }
                    match room_info.avatar {
                        RoomPreviewAvatar::Text(ref text) => {
                            println!("DEBUG: writing avatar text {text}");
                            item.avatar(id!(avatar)).set_text(text);
                        }
                        RoomPreviewAvatar::Image(ref img_bytes) => {
                            let _ = item.avatar(id!(avatar)).set_image(
                                |img| utils::load_png_or_jpg(&img, cx, img_bytes)
                            );
                        }
                    }

                    item
                };

                item.draw_all(cx, &mut Scope::empty());
            }
        }

        DrawStep::done()
    }

}
