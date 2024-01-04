use std::collections::HashMap;
use std::time::SystemTime;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, MilliSecondsSinceUnixEpoch};
use crate::shared::clickable_view::*;
use crate::shared::stack_view_action::StackViewAction;
use crate::utils::unix_time_millis_to_datetime;

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

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")

    RoomPreview = <ClickableView> {
        flow: Right, spacing: 10., padding: 10.
        width: Fill, height: Fit

        // The avatar image for this room.
        // By default, this is the first character of the room's name,
        // but is replaced by the image once it is obtained from the server.
        avatar = <View> {
            width: 36., height: 36.
            align: { x: 0.5, y: 0.5 }
            flow: Overlay

            text_view = <RoundedView> {
                visible: true,
                align: { x: 0.5, y: 0.5 }
                draw_bg: {
                    instance radius: 4.0,
                    instance border_width: 1.0,
                    // instance border_color: #ddd,
                    color: #dfd
                }
                
                text = <Label> {
                    width: Fit, height: Fit
                    draw_text: {
                        text_style: <TITLE_TEXT>{ font_size: 16. }
                    }
                    text: ""
                }
            }

            img_view = <View> {
                visible: false,
                img = <Image> {
                    width: Fill, height: Fill,
                    source: (IMG_DEFAULT_AVATAR),
                }
            }
        }

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

    RoomsCount = <View> {
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
            text: "Found 0 joined rooms."
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
            rooms_count = <RoomsCount> {}
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


pub enum RoomListUpdate {
    AddRoom(RoomPreviewEntry),
    UpdateRoomMessage {
        room_id: OwnedRoomId,
        timestamp: MilliSecondsSinceUnixEpoch,
        latext_message_text: String,
    },
    UpdateRoomName {
        room_id: OwnedRoomId,
        room_name: String,
    },
    RemoveRoom(OwnedRoomId),
}

static PENDING_ROOM_UPDATES: SegQueue<RoomListUpdate> = SegQueue::new();

/// Enqueue a new room update for the list of all rooms.
pub fn update_rooms_list(update: RoomListUpdate) {
    PENDING_ROOM_UPDATES.push(update);
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
    /// The avatar for this room, which is either array of bytes holding the avatar image
    /// or a string holding the first character of the room name.
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


#[derive(Live, Widget)]
pub struct RoomsList {
    #[deref] view: View,

    /// The list of all known rooms and their cached preview info.
    #[rust] all_rooms: Vec<RoomPreviewEntry>,
    /// Maps the WidgetUid of a `RoomPreview` to that room's index in the `all_rooms` vector.
    #[rust] rooms_list_map: HashMap<u64, usize>,
}

impl LiveHook for RoomsList {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) { }
}


impl Widget for RoomsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Currently, a Signal event is only used to tell this widget
        // that the rooms list has been updated in the background.
        if let Event::Signal = event {
            self.redraw(cx);
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
                    println!("------------- User clicked room index {room_index}, {}, {} ------------------", room_details.room_id.clone().unwrap(), room_details.room_name.clone().unwrap());
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

        // process the list of pending updates to the room list before we display the list.
        while let Some(update) = PENDING_ROOM_UPDATES.pop() {
            match update {
                RoomListUpdate::AddRoom(room) => {
                    self.all_rooms.push(room);
                }
                RoomListUpdate::RemoveRoom(room_id) => {
                    if let Some(idx) = self.all_rooms.iter().position(|r| r.room_id.as_ref() == Some(&room_id)) {
                        self.all_rooms.remove(idx);
                    }
                }
                RoomListUpdate::UpdateRoomMessage { room_id, timestamp, latext_message_text } => {
                    if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id.as_ref() == Some(&room_id)) {
                        room.latest = Some((timestamp, latext_message_text));
                    }
                }
                RoomListUpdate::UpdateRoomName { room_id, room_name } => {
                    if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id.as_ref() == Some(&room_id)) {
                        room.room_name = Some(room_name);
                    }
                }
            }
        }

        // TODO: sort list of `all_rooms` by alphabetic, most recent message, grouped by spaces, etc

        let count = self.all_rooms.len() as u64;
        let last_item_id = count + 1; // Add 1 for the search bar up top.

        // Start the actual drawing procedure.
        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step() {
            // We only care about drawing the portal list.
            let portal_list_ref = list_item.as_portal_list();
            let Some(mut list) = portal_list_ref.borrow_mut() else { continue };
        
            // Add 1 again for the rooms count label at the bottom.
            list.set_item_range(cx, 0, last_item_id + 1);

            while let Some(item_id) = list.next_visible_item(cx) {
                // Draw the search bar as the top entry.
                let item = if item_id == 0 {
                    list.item(cx, item_id, live_id!(search_bar)).unwrap()
                }
                // Draw the rooms count as the bottom entry.
                else if item_id == last_item_id {
                    let item = list.item(cx, item_id, live_id!(rooms_count)).unwrap();
                    item.label(id!(label)).set_text(&format!("Found {count} joined rooms."));
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
                        item.label(id!(preview.room_name)).set_text(name);
                    }
                    if let Some((ts, msg)) = room_info.latest.as_ref() {
                        if let Some(dt) = unix_time_millis_to_datetime(ts) {
                            item.label(id!(timestamp)).set_text(
                                &format!("{} {}", dt.date(), dt.time().format("%l:%M %P"))
                            );
                        }
                        item.label(id!(preview.latest_message)).set_text(msg);
                    }
                    match room_info.avatar {
                        RoomPreviewAvatar::Text(ref text) => {
                            item.view(id!(avatar.img_view)).set_visible(false);
                            item.view(id!(avatar.text_view)).set_visible(true);
                            item.label(id!(avatar.text_view.text)).set_text(text);
                        }
                        RoomPreviewAvatar::Image(ref img_bytes) => {
                            item.view(id!(avatar.img_view)).set_visible(true);
                            item.view(id!(avatar.text_view)).set_visible(false);
                            item.image(id!(avatar.img_view.img)).load_png_from_data(cx, img_bytes);

                            // debugging: dump out the avatar image to disk
                            if false {
                                let mut path = crate::temp_storage::get_temp_dir_path().clone();
                                let filename = format!("{}_{}",
                                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                                    room_info.room_name.as_ref().unwrap(),
                                );
                                path.push(filename);
                                path.set_extension("png");
                                println!("Writing avatar image to disk: {:?}", path);
                                std::fs::write(path, img_bytes)
                                    .expect("Failed to write avatar image to disk");
                            }
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
