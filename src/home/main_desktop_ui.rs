use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use std::collections::HashMap;

use crate::{app::{AppState, AppStateAction, RoomsPanelRestoreAction, SelectedRoom}, utils::room_name_or_id};
use super::{invite_screen::InviteScreenWidgetRefExt, room_screen::RoomScreenWidgetRefExt, rooms_list::RoomsListAction};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::light_themed_dock::*;
    use crate::home::rooms_sidebar::RoomsSideBar;
    use crate::home::welcome_screen::WelcomeScreen;
    use crate::home::room_screen::RoomScreen;
    use crate::home::invite_screen::InviteScreen;

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

            // Below are the templates of widgets that can be created within dock tabs.
            rooms_sidebar = <RoomsSideBar> {}
            welcome_screen = <WelcomeScreen> {}
            room_screen = <RoomScreen> {}
            invite_screen = <InviteScreen> {}
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

    /// The order in which the rooms were opened, in chronological order
    /// from first opened (at the beginning) to last opened (at the end).
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
        self.widget_match_event(cx, event, scope); // invokes `WidgetMatchEvent` impl
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // When changing from mobile to Desktop, we need to restore the rooms panel state
        if !self.drawn_previously {
            let app_state = scope.data.get_mut::<AppState>().unwrap();
            if !app_state.saved_dock_state.open_rooms.is_empty() {
                cx.action(RoomsPanelRestoreAction::Restore(
                    app_state.saved_dock_state.clone(),
                ));
            }
            self.drawn_previously = true;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MainDesktopUI {
    /// Focuses on a room if it is already open, otherwise creates a new tab for the room.
    fn focus_or_create_tab(&mut self, cx: &mut Cx, room: SelectedRoom) {
        let dock = self.view.dock(id!(dock));

        // Do nothing if the room to select is already created and focused.
        if self.most_recently_selected_room.as_ref().is_some_and(|r| r == &room) {
            return;
        }

        // If the room is already open, select (jump to) its existing tab
        let room_id_as_live_id = LiveId::from_str(room.room_id().as_str());
        if self.open_rooms.contains_key(&room_id_as_live_id) {
            dock.select_tab(cx, room_id_as_live_id);
            self.most_recently_selected_room = Some(room);
            return;
        }

        // Create a new tab for the room
        let (tab_bar, _pos) = dock.find_tab_bar_of_tab(live_id!(home_tab)).unwrap();
        let (kind, name) = match &room {
            SelectedRoom::JoinedRoom { room_id, room_name }  => (
                live_id!(room_screen),
                room_name_or_id(room_name.as_ref(), room_id),
            ),
            SelectedRoom::InvitedRoom { room_id, room_name } => (
                live_id!(invite_screen),
                room_name_or_id(room_name.as_ref(), room_id),
            ),
        };
        let new_tab_widget = dock.create_and_select_tab(
            cx,
            tab_bar,
            room_id_as_live_id,
            kind,
            name,
            live_id!(CloseableTab),
            None, // insert the tab at the end
            // TODO: insert the tab after the most-recently-selected room
        );

        // if the tab was created, set the room screen and add the room to the room order
        if let Some(new_widget) = new_tab_widget {
            self.room_order.push(room.clone());
            match &room {
                SelectedRoom::JoinedRoom { room_id, .. }  => {
                    new_widget.as_room_screen().set_displayed_room(
                        cx,
                        room_id.clone(),
                        room.room_name().cloned(),
                    );
                }
                SelectedRoom::InvitedRoom { room_id, room_name: _ } => {
                    new_widget.as_invite_screen().set_displayed_invite(
                        cx,
                        room_id.clone(),
                    );
                }
            }
            cx.action(MainDesktopUiAction::DockSave);
        } else {
            error!("BUG: failed to create tab for {room:?}");
        }

        self.open_rooms.insert(room_id_as_live_id, room.clone());
        self.most_recently_selected_room = Some(room);
    }

    /// Closes a tab in the dock and focuses on the latest open room.
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
                                AppStateAction::RoomFocused(new_focused_room.clone()),
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
                    AppStateAction::FocusNone,
                );

                dock.select_tab(cx, live_id!(home_tab));
                self.most_recently_selected_room = None;
            }
        }

        dock.close_tab(cx, tab_id);
        self.tab_to_close = None;
        self.open_rooms.remove(&tab_id);
    }

    /// Replaces an invite with a joined room in the dock.
    fn replace_invite_with_joined_room(
        &mut self,
        cx: &mut Cx,
        scope: &mut Scope,
        room_id: OwnedRoomId,
        room_name: Option<String>,
    ) {
        let dock = self.view.dock(id!(dock));
        let Some((new_widget, true)) = dock.replace_tab(
            cx,
            LiveId::from_str(room_id.as_str()),
            live_id!(room_screen),
            Some(room_name_or_id(room_name.as_ref(), &room_id)),
            false,
        ) else {
            // Nothing we can really do here except log an error.
            error!("BUG: failed to replace InviteScreen tab with RoomScreen for {room_id}");
            return;
        };

        // Set the info to be displayed in the newly-replaced RoomScreen..
        new_widget.as_room_screen().set_displayed_room(
            cx,
            room_id.clone(),
            room_name.clone(),
        );

        // Go through all existing `SelectedRoom` instances and replace the
        // `SelectedRoom::InvitedRoom`s with `SelectedRoom::JoinedRoom`s.
        for selected_room in self.most_recently_selected_room.iter_mut()
            .chain(self.room_order.iter_mut())
            .chain(self.open_rooms.values_mut())
        {
            selected_room.upgrade_invite_to_joined(&room_id);
        }

        // Finally, emit an action to update the AppState with the new room.
        cx.widget_action(
            self.widget_uid(),
            &scope.path,
            AppStateAction::UpgradedInviteToJoinedRoom(room_id),
        );
    }
}

impl WidgetMatchEvent for MainDesktopUI {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let mut should_save_dock_action: bool = false;
        for action in actions {
            let widget_action = action.as_widget_action();

            // Handle actions emitted by the dock within the MainDesktopUI
            match widget_action.cast() { // TODO: don't we need to call `widget_uid_eq(dock.widget_uid())` here?
                // Whenever a tab (except for the home_tab) is pressed, notify the app state.
                DockAction::TabWasPressed(tab_id) => {
                    if tab_id == live_id!(home_tab) {
                        cx.widget_action(
                            self.widget_uid(),
                            &HeapLiveIdPath::default(),
                            AppStateAction::FocusNone,
                        );
                        self.most_recently_selected_room = None;
                    }
                    else if let Some(selected_room) = self.open_rooms.get(&tab_id) {
                        cx.widget_action(
                            self.widget_uid(),
                            &HeapLiveIdPath::default(),
                            AppStateAction::RoomFocused(selected_room.clone()),
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
                    self.view.dock(id!(dock)).tab_start_drag(
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
                        self.view.dock(id!(dock)).accept_drag(cx, drag_event, DragResponse::Move);
                    }
                }
                // When dropping a tab, move it to the new position
                DockAction::Drop(drop_event) => {
                    // from inside the dock, otherwise it's an external file
                    if let DragItem::FilePath {
                        internal_id: Some(internal_id),
                        ..
                    } = &drop_event.items[0] {
                        self.view.dock(id!(dock)).drop_move(cx, drop_event.abs, *internal_id);
                    }
                    should_save_dock_action = true;
                }
                _ => (),
            }

            // Handle RoomsList actions, which are updates from the rooms list.
            match widget_action.cast() {
                RoomsListAction::Selected(selected_room) => {
                    // Note that this cannot be performed within draw_walk() as the draw flow prevents from
                    // performing actions that would trigger a redraw, and the Dock internally performs (and expects)
                    // a redraw to be happening in order to draw the tab content.
                    self.focus_or_create_tab(cx, selected_room);
                }
                RoomsListAction::InviteAccepted { room_id, room_name } => {
                    self.replace_invite_with_joined_room(cx, scope, room_id, room_name);
                }
                RoomsListAction::None => { }
            }

            // Handle our own actions related to dock updates that we have previously emitted.
            match action.downcast_ref() {
                Some(MainDesktopUiAction::DockLoad) => {
                    let app_state = scope.data.get_mut::<AppState>().unwrap();
                    let dock = self.view.dock(id!(dock));
                    self.room_order = app_state.saved_dock_state.room_order.clone();
                    self.open_rooms = app_state.saved_dock_state.open_rooms.clone();
                    if app_state.saved_dock_state.dock_items.is_empty() {
                        return;
                    }

                    if let Some(mut dock) = dock.borrow_mut() {
                        dock.load_state(cx, app_state.saved_dock_state.dock_items.clone());
                        for (head_live_id, (_, widget)) in dock.items().iter() {
                            match app_state.saved_dock_state.open_rooms.get(head_live_id) {
                                Some(SelectedRoom::JoinedRoom { room_id, room_name }) => {
                                    widget.as_room_screen().set_displayed_room(
                                        cx,
                                        room_id.clone(),
                                        room_name.clone(),
                                    );
                                }
                                Some(SelectedRoom::InvitedRoom { room_id, room_name: _ }) => {
                                    widget.as_invite_screen().set_displayed_invite(cx, room_id.clone());
                                }
                                _ => { }
                            }
                        }
                    } else {
                        error!("BUG: failed to load dock state upon DockLoad action.");
                        continue;
                    }
                    // Note: the borrow of `dock` must end here *before* we call `self.focus_or_create_tab()`.

                    if let Some(ref selected_room) = &app_state.selected_room {
                        self.focus_or_create_tab(cx, selected_room.clone());
                    }
                    self.view.redraw(cx);
                }
                Some(MainDesktopUiAction::DockSave) => {
                    let app_state = scope.data.get_mut::<AppState>().unwrap();
                    let dock = self.view.dock(id!(dock));
                    if let Some(dock_items) = dock.clone_state() {
                        app_state.saved_dock_state.dock_items = dock_items;
                    }
                    app_state.saved_dock_state.open_rooms = self.open_rooms.clone();
                    app_state.saved_dock_state.room_order = self.room_order.clone();
                }
                _ => {}
            }
        }

        if should_save_dock_action {
            cx.action(MainDesktopUiAction::DockSave);
        }
    }
}

/// Actions sent to the MainDesktopUI widget for saving/restoring its dock state.
#[derive(Clone, Debug, DefaultNone)]
pub enum MainDesktopUiAction {
    /// Save the dock state from the dock to the AppState.
    DockSave,
    /// Load the room panel state from the AppState to the dock.
    DockLoad,
    None,
}
