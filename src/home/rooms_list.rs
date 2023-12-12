use std::collections::HashMap;
use crossbeam_queue::SegQueue;
use makepad_widgets::*;
use matrix_sdk::ruma::{OwnedRoomId, MilliSecondsSinceUnixEpoch};
use crate::shared::clickable_view::*;
use crate::shared::stack_view_action::StackViewAction;
use crate::utils::unix_time_millis_to_datetime;

live_design! {
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

        avatar = <Image> {
            source: (IMG_DEFAULT_AVATAR),
            width: 36., height: 36.
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
                width: Fit, height: Fit
                draw_text:{
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

    RoomsList = {{RoomsList}} {
        width: Fill, height: Fill
        flow: Down
        list: <PortalList> {
            keep_invisible: true
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            room_preview = <RoomPreview> {}
            search_bar = <SearchBar> {}
        }
    }


    // temp test to see what Live requires
    RoomsList2 = {{RoomsList2}} {
        width: Fill, height: Fill
        flow: Down
        list: <PortalList> {
            keep_invisible: true
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            room_preview = <RoomPreview> {}
            search_bar = <SearchBar> {}
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


#[derive(Debug, Clone, WidgetAction)]
pub enum RoomListAction {
    /// Contains the index of the `RoomPreviewEntry` that was selected.
    /// Used to index into the `all_rooms` vector.
    Selected(RoomIndex),
    None,
}

#[derive(Clone, Debug, Default)]
pub struct RoomPreviewEntry {
    /// The matrix ID of this room.
    pub room_id: Option<OwnedRoomId>,
    /// The displayable name of this room, if known.
    pub room_name: Option<String>,
    /// The timestamp and text content of the latest message in this room.
    pub latest: Option<(MilliSecondsSinceUnixEpoch, String)>,
    
    // TODO: add room avatar image
}

#[derive(Live)]
pub struct RoomsList2 {
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[live] list: PortalList,

    // /// The list of all known rooms and their cached preview info.
    // #[rust] all_rooms: u64,
    // /// Maps the widget uid (of a room preview) to the index in the `all_rooms` vector.
    // #[rust] rooms_list_map: HashMap<u64, u64>,
}
impl LiveHook for RoomsList2 {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, RoomsList2);
    }
    fn after_new_from_doc(&mut self, _cx: &mut Cx) { }
}
impl Widget for RoomsList2 {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        unimplemented!()
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.list.redraw(cx)
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        WidgetDraw::done()
    }
}


#[derive(Live)]
pub struct RoomsList {
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[live] list: PortalList,

    /// The list of all known rooms and their cached preview info.
    #[rust] all_rooms: Vec<RoomPreviewEntry>,
    /// Maps the widget uid (of a room preview) to the index in the `all_rooms` vector.
    #[rust] rooms_list_map: HashMap<u64, usize>,
}

impl LiveHook for RoomsList {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, RoomsList2);
    }

    fn after_new_from_doc(&mut self, _cx: &mut Cx) { }
}

impl Widget for RoomsList {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        self.handle_event_with(cx, event, &mut |cx, action| {
            dispatch_action(cx, action);
        });
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.list.redraw(cx)
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.draw_walk(cx, walk);
        WidgetDraw::done()
    }
}

impl RoomsList {
    fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let mut actions = Vec::new();
        self.list
            .handle_widget_event_with(cx, event, &mut |_, action| {
                if let Some(room_index) = self.rooms_list_map.get(&action.widget_uid.0) {
                    actions.push((room_index.clone(), action));
                }
            });

        let widget_uid = self.widget_uid();
        for (room_index, action) in actions {
            if let ClickableViewAction::Click = action.action() {
                dispatch_action(
                    cx,
                    WidgetActionItem::new(RoomListAction::Selected(room_index).into(), widget_uid)
                );
                dispatch_action(
                    cx,
                    WidgetActionItem::new(StackViewAction::ShowRoom.into(), widget_uid)
                );
            }
        }
    }
}

impl RoomsList {
    pub fn draw_walk(&mut self, cx: &mut Cx2d, walk: Walk) {

        // process the list of pending updates to the room list before we display the list.
        while let Some(update) = PENDING_ROOM_UPDATES.pop() {
            match update {
                RoomListUpdate::AddRoom(room) => {
                    self.all_rooms.push(room);
                }
                RoomListUpdate::RemoveRoom(room_id) => {
                    if let Some(idx) = self.all_rooms.iter().position(|r| r.room_id == Some(room_id)) {
                        self.all_rooms.remove(idx);
                    }
                }
                RoomListUpdate::UpdateRoomMessage { room_id, timestamp, latext_message_text } => {
                    if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id == Some(room_id)) {
                        room.latest = Some((timestamp, latext_message_text));
                    }
                }
                RoomListUpdate::UpdateRoomName { room_id, room_name } => {
                    if let Some(room) = self.all_rooms.iter_mut().find(|r| r.room_id == Some(room_id)) {
                        room.room_name = Some(room_name);
                    }
                }
            }
        }

        // todo: sort list of `all_rooms` by alphabetic, most recent message, grouped by spaces, etc

        let count = self.all_rooms.len() as u64;

        cx.begin_turtle(walk, self.layout);
        self.list.set_item_range(cx, 0, count + 1);

        while self.list.draw_widget(cx).hook_widget().is_some() {
            while let Some(item_id) = self.list.next_visible_item(cx) {
                let template = match item_id {
                    0 => live_id!(search_bar),
                    _ => live_id!(room_preview),
                };

                let item = self.list.item(cx, item_id, template).unwrap();

                if item_id >= 1 && item_id < count + 1 {
                    let index_of_room = item_id as usize - 1; // -1 to account for the search bar
                    let room_info = &self.all_rooms[index_of_room];

                    self.rooms_list_map.insert(item.widget_uid().0, index_of_room);

                    if let Some(ref name) = room_info.room_name {
                        item.label(id!(preview.room_name)).set_text(name);
                    }
                    if let Some((ts, msg)) = room_info.latest {
                        if let Some(dt) = unix_time_millis_to_datetime(&ts) {
                            item.label(id!(timestamp)).set_text(
                                &format!("{} {}", dt.date(), dt.time().format("%l:%M %P"))
                            );
                        }
                        item.label(id!(preview.latest_message)).set_text(&msg);
                    }
                }

                item.draw_widget_all(cx);
            }
        }

        cx.end_turtle();
    }

}
