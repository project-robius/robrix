use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use std::collections::HashMap;

use crate::{app::{AppState, SelectedRoom}, sliding_sync::{check_room_in_all_room_info, submit_async_request}};

use super::room_screen::{RoomScreenPrompt, RoomScreenWidgetRefExt};
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::light_themed_dock::*;
    use crate::home::welcome_screen::WelcomeScreen;
    use crate::home::rooms_sidebar::RoomsSideBar;
    use crate::home::room_screen::RoomScreen;

   pub MainDesktopUI = {{MainDesktopUI}} {
        dock = <Dock> {
            width: Fill,
            height: Fill,
            padding: 0,
            spacing: 0,

            root = Splitter {
                axis: Horizontal,
                align: FromA(300.0),
                a: rooms_sidebar_tab,
                b: main
            }

            // Not really a tab, but it needs to be one to be used in the dock
            rooms_sidebar_tab = Tab {
                name: "" // show no tab header
                kind: rooms_sidebar
            }

            main = Tabs{tabs:[home_tab], selected:0}

            home_tab = Tab {
                name: "Home"
                kind: welcome_screen
                template: PermanentTab
            }

            rooms_sidebar = <RoomsSideBar> {}
            welcome_screen = <WelcomeScreen> {}
            room_screen = <RoomScreen> {}
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct MainDesktopUI {
    #[deref]
    view: View,

    /// The rooms that are currently open, keyed by the LiveId of their tab.
    #[rust]
    open_rooms: HashMap<LiveId, SelectedRoom>,

    /// The tab that should be closed in the next draw event
    #[rust]
    tab_to_close: Option<LiveId>,

    /// The order in which the rooms were opened
    #[rust]
    room_order: Vec<SelectedRoom>,

    /// The most recently selected room, used to prevent re-selecting the same room in Dock
    /// which would trigger redraw of whole Widget.
    #[rust]
    most_recently_selected_room: Option<SelectedRoom>,

    /// Boolean to indicate if we've drawn the MainDesktopUi previously in the desktop view.
    ///
    /// When switching mobile view to desktop, we need to restore the rooms panel state.
    /// If it is false, we will post an action to load the dock from the saved dock state.
    /// If it is true, we will do nothing.
    #[rust]
    drawn_previously: bool,
}

impl Widget for MainDesktopUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let dock = self.view.dock(id!(dock));
        if let Event::Actions(actions) = event {
            for action in actions {
                match action.downcast_ref() {
                    Some(RoomsPanelAction::DockLoadAll) => {
                        let app_state = scope.data.get_mut::<AppState>().unwrap();

                        self.room_order = app_state.rooms_panel.room_order.clone();
                        self.open_rooms = app_state.rooms_panel.open_rooms.clone();
                        if app_state.rooms_panel.dock_state.is_empty() {
                            return;
                        }

                        if let Some(mut dock) = dock.borrow_mut() {
                            dock.load_state(cx, app_state.rooms_panel.dock_state.clone());
                            dock.items().iter().for_each(|(head_liveid, (_, widget))| {
                                if let Some(room) =
                                    app_state.rooms_panel.open_rooms.get(head_liveid)
                                {
                                    if check_room_in_all_room_info(&room.room_id) {
                                        widget.as_room_screen().set_displayed_room(
                                            cx,
                                            room.room_id.clone(),
                                            room.room_name.clone().unwrap_or_default(),
                                        );
                                    } else {
                                        submit_async_request(crate::sliding_sync::MatrixRequest::DockWaitForRoomReady { room_id: room.room_id.clone() });
                                    }
                                }
                            });
                        } else {
                            return;
                        }

                        if let Some(ref selected_room) = &app_state.rooms_panel.selected_room {
                            self.focus_or_create_tab(cx, selected_room.clone());
                        }
                    }
                    Some(RoomsPanelAction::DockPending(room_id)) => {
                        let app_state = scope.data.get_mut::<AppState>().unwrap();
                        if let Some(mut dock) = dock.borrow_mut() {
                            for (head_liveid, (_, widget)) in dock.items().iter() {
                                if let Some(room) =
                                    app_state.rooms_panel.open_rooms.get(head_liveid)
                                {
                                    if &room.room_id == room_id {
                                        widget
                                            .as_room_screen()
                                            .set_prompt(cx, RoomScreenPrompt::Pending);
                                    }
                                }
                            }
                        } else {
                            return;
                        }
                    }
                    Some(RoomsPanelAction::DockSave) => {
                        let app_state = scope.data.get_mut::<AppState>().unwrap();
                        if let Some(dock_state) = dock.clone_state() {
                            app_state.rooms_panel.dock_state = dock_state;
                        }
                        app_state.rooms_panel.open_rooms = self.open_rooms.clone();
                        app_state.rooms_panel.room_order = self.room_order.clone();
                    }
                    Some(RoomsPanelAction::DockSuccess(room_id)) => {
                        let app_state = scope.data.get_mut::<AppState>().unwrap();
                        if let Some(mut dock) = dock.borrow_mut() {
                            for (head_liveid, (_, widget)) in dock.items().iter() {
                                if let Some(room) =
                                    app_state.rooms_panel.open_rooms.get(head_liveid)
                                {
                                    if &room.room_id == room_id {
                                        widget.as_room_screen().set_displayed_room(
                                            cx,
                                            room.room_id.clone(),
                                            room.room_name.clone().unwrap_or_default(),
                                        );
                                        break;
                                    }
                                }
                            }
                        } else {
                            return;
                        }
                    }
                    Some(RoomsPanelAction::DockTimeout(room_id)) => {
                        let app_state = scope.data.get_mut::<AppState>().unwrap();
                        if let Some(mut dock) = dock.borrow_mut() {
                            for (head_liveid, (_, widget)) in dock.items().iter() {
                                if let Some(room) =
                                    app_state.rooms_panel.open_rooms.get(head_liveid)
                                {
                                    if &room.room_id == room_id {
                                        widget
                                            .as_room_screen()
                                            .set_prompt(cx, RoomScreenPrompt::Timeout);
                                    }
                                }
                            }
                        } else {
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // When changing from mobile to Desktop, we need to restore the rooms panel state
        if !self.drawn_previously {
            let app_state = scope.data.get_mut::<AppState>().unwrap();
            if !app_state.rooms_panel.open_rooms.is_empty() {
                cx.action(RoomsPanelAction::DockLoadAll);
            }
            self.drawn_previously = true;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MainDesktopUI {
    /// Focuses on a room if it is already open, otherwise creates a new tab for the room
    fn focus_or_create_tab(&mut self, cx: &mut Cx, room: SelectedRoom) {
        let dock = self.view.dock(id!(dock));

        // Do nothing if the room to select is already created and focused.
        if self.most_recently_selected_room.as_ref().is_some_and(|r| r == &room) {
            return;
        }

        // If the room is already open, select (jump to) its existing tab
        let room_id_as_live_id = LiveId::from_str(room.room_id.as_str());
        if self.open_rooms.contains_key(&room_id_as_live_id) {
            dock.select_tab(cx, room_id_as_live_id);
            self.most_recently_selected_room = Some(room);
            return;
        }

        self.open_rooms.insert(room_id_as_live_id, room.clone());

        let displayed_room_name = room.room_name.clone()
            .unwrap_or_else(|| format!("Room ID {}", &room.room_id));

        // create a new tab for the room
        let (tab_bar, _pos) = dock.find_tab_bar_of_tab(live_id!(home_tab)).unwrap();
        let kind = live_id!(room_screen);

        let result = dock.create_and_select_tab(
            cx,
            tab_bar,
            room_id_as_live_id,
            kind,
            displayed_room_name.clone(),
            live_id!(CloseableTab),
            // `None` will insert the tab at the end
            None,
        );

        // if the tab was created, set the room screen and add the room to the room order
        if let Some(widget) = result {
            self.room_order.push(room.clone());
            widget.as_room_screen().set_displayed_room(
                cx,
                room.room_id.clone(),
                displayed_room_name,
            );
            cx.action(RoomsPanelAction::DockSave);
        } else {
            error!("Failed to create tab for room {}, {:?}", room.room_id, room.room_name);
        }

        self.most_recently_selected_room = Some(room);
    }

    /// Closes a tab in the dock and focuses in the latest open room
    fn close_tab(&mut self, cx: &mut Cx, tab_id: LiveId) {
        let dock = self.view.dock(id!(dock));
        if let Some(room_being_closed) = self.open_rooms.get(&tab_id) {
            self.room_order.retain(|sr| sr != room_being_closed);

            if self.open_rooms.len() > 1 {
                // If the closing tab is the active one, then focus the next room
                let active_room = self.most_recently_selected_room.as_ref();
                if let Some(active_room) = active_room {
                    if active_room == room_being_closed {
                        if let Some(new_focused_room) = self.room_order.last() {
                            // notify the app state about the new focused room
                            cx.widget_action(
                                self.widget_uid(),
                                &HeapLiveIdPath::default(),
                                RoomsPanelAction::RoomFocused(new_focused_room.clone()),
                            );

                            // Set the new selected room to be used in the current draw
                            self.most_recently_selected_room = Some(new_focused_room.clone());
                        }
                    }
                }
            } else {
                // If there is no room to focus, notify app to reset the selected room in the app state
                cx.widget_action(
                    self.widget_uid(),
                    &HeapLiveIdPath::default(),
                    RoomsPanelAction::FocusNone,
                );

                dock.select_tab(cx, live_id!(home_tab));
                self.most_recently_selected_room = None;
            }
        }

        dock.close_tab(cx, tab_id);
        self.tab_to_close = None;
        self.open_rooms.remove(&tab_id);
    }
}

impl MatchEvent for MainDesktopUI {
    fn handle_action(&mut self, cx: &mut Cx, action: &Action) {
        let dock = self.view.dock(id!(dock));

        if let Some(action) = action.as_widget_action() {
            // Handle Dock actions
            let mut should_save_dock_action: bool = false;
            match action.cast() {
                // Whenever a tab (except for the home_tab) is pressed, notify the app state.
                DockAction::TabWasPressed(tab_id) => {
                    if tab_id == live_id!(home_tab) {
                        cx.widget_action(
                            self.widget_uid(),
                            &HeapLiveIdPath::default(),
                            RoomsPanelAction::FocusNone,
                        );
                        self.most_recently_selected_room = None;
                    } else if let Some(selected_room) = self.open_rooms.get(&tab_id) {
                        cx.widget_action(
                            self.widget_uid(),
                            &HeapLiveIdPath::default(),
                            RoomsPanelAction::RoomFocused(selected_room.clone()),
                        );
                        self.most_recently_selected_room = Some(selected_room.clone());
                    }
                    should_save_dock_action = true;
                }
                DockAction::TabCloseWasPressed(tab_id) => {
                    self.tab_to_close = Some(tab_id);
                    self.close_tab(cx, tab_id);
                    self.redraw(cx);
                    should_save_dock_action = true;
                }
                // When dragging a tab, allow it to be dragged
                DockAction::ShouldTabStartDrag(tab_id) => {
                    dock.tab_start_drag(
                        cx,
                        tab_id,
                        DragItem::FilePath {
                            path: "".to_string(),
                            internal_id: Some(tab_id),
                        },
                    );
                }
                // When dragging a tab, allow it to be dragged
                DockAction::Drag(drag_event) => {
                    if drag_event.items.len() == 1 {
                        dock.accept_drag(cx, drag_event, DragResponse::Move);
                    }
                }
                // When dropping a tab, move it to the new position
                DockAction::Drop(drop_event) => {
                    // from inside the dock, otherwise it's an external file
                    if let DragItem::FilePath {
                        internal_id: Some(internal_id),
                        ..
                    } = &drop_event.items[0] {
                        dock.drop_move(cx, drop_event.abs, *internal_id);
                    }
                    should_save_dock_action = true;
                }
                DockAction::SplitPanelChanged { panel_id: _, axis: _, align: _ } => {
                    should_save_dock_action = true;
                }
                _ => (),
            }
            if should_save_dock_action {
                cx.action(RoomsPanelAction::DockSave);
            }
            // Handle RoomsList actions
            if let super::rooms_list::RoomsListAction::Selected {
                room_id,
                room_index: _,
                room_name,
            } = action.cast() {
                // Note that this cannot be performed within draw_walk() as the draw flow prevents from
                // performing actions that would trigger a redraw, and the Dock internally performs (and expects)
                // a redraw to be happening in order to draw the tab content.
                self.focus_or_create_tab(cx, SelectedRoom { room_id, room_name });
            }
        }
    }
}

/// Actions sent to/from the rooms panel that affect the RoomsList
/// or one of the RoomScreen widgets.
#[derive(Clone, DefaultNone, Debug)]
pub enum RoomsPanelAction {
    None,
    /// Notifies that a room was focused.
    RoomFocused(SelectedRoom),
    /// Resets the focus to none, meaning that no room has focus.
    FocusNone,
    /// Save the dock state from the dock to the AppState.
    DockSave,
    /// Load the room panel state from the AppState to the dock.
    DockLoadAll,
    /// Display a spinner widget to show the room screen is loading.
    DockPending(OwnedRoomId),
    /// Display timeline in the room screen
    DockSuccess(OwnedRoomId),
    /// Display timeout message after waiting 20 seconds for the room screen to load.
    DockTimeout(OwnedRoomId)
}
