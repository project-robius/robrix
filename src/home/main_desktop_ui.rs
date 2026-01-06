use makepad_widgets::*;
use ruma::OwnedRoomId;
use tokio::sync::Notify;
use std::{collections::HashMap, sync::Arc};

use crate::{app::{AppState, AppStateAction, SavedDockState, SelectedRoom}, home::{navigation_tab_bar::{NavigationBarAction, SelectedTab}, rooms_list::RoomsListRef, space_lobby::SpaceLobbyScreenWidgetRefExt}, utils::RoomNameId};
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
    use crate::home::space_lobby::SpaceLobbyScreen;

    pub MainDesktopUI = {{MainDesktopUI}} {
        dock = <Dock> {
            width: Fill,
            height: Fill,
            padding: 0,
            spacing: 0,
            // Align the dock with the RoomFilterInputBar. Not sure why we need this...
            margin: {left: 1.75}


            root = Splitter {
                axis: Horizontal,
                align: FromA(300.0),
                a: rooms_sidebar_tab,
                b: main
            }

            // This is a "fixed" tab with no header that cannot be closed.
            rooms_sidebar_tab = Tab {
                name: "" // show no tab header
                kind: rooms_sidebar // this template is defined below.
            }

            main = Tabs {
                tabs: [home_tab],
                selected: 0,
            }

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
            space_lobby_screen = <SpaceLobbyScreen> {}
        }
    }
}

#[derive(Live, Widget)]
pub struct MainDesktopUI {
    #[deref]
    view: View,

    /// The default layout that should be loaded into the dock
    /// when there is no previously-saved content to restore.
    /// This is a Rust-level instance of the dock content defined in the above live DSL.
    #[rust] default_layout: SavedDockState,

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

    /// The ID of the currently-selected space, if any.
    ///
    /// This determines which set of rooms this dock is currently showing.
    /// If `None`, we're displaying the main home view of all rooms from any space.
    #[rust] selected_space: Option<OwnedRoomId>,

    /// Boolean to indicate if we've drawn the MainDesktopUi previously in the desktop view.
    ///
    /// When switching mobile view to desktop, we need to restore the saved app state to the UI.
    /// * If false, this widget emits an action to load the dock from the saved dock state.
    /// * If true, this widget proceeds to draw the desktop UI as normal.
    #[rust]
    drawn_previously: bool,
}

impl LiveHook for MainDesktopUI {
    fn after_new_from_doc(&mut self, _: &mut Cx) {
        self.default_layout = self.save_dock_state();
    }
}

impl Widget for MainDesktopUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.widget_match_event(cx, event, scope); // invokes `WidgetMatchEvent` impl
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.drawn_previously {
            // When changing from Mobile to Desktop view mode, we need to restore the state
            // of this widget, which we get from the `AppState` passed down via `scope`.
            // This includes the currently selected space, which we get from the RoomsList widget.
            // We must set `selected_space` first before the load operation occurs, in order for
            // the proper space-specific instance of the saved dock UI layout/state to be selected.
            self.selected_space = cx.get_global::<RoomsListRef>().get_selected_space_id();
            cx.action(MainDesktopUiAction::LoadDockFromAppState);
            self.drawn_previously = true;
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MainDesktopUI {
    /// Focuses on a room if it is already open, otherwise creates a new tab for the room.
    fn focus_or_create_tab(&mut self, cx: &mut Cx, room: SelectedRoom) {
        // Do nothing if the room to select is already created and focused.
        if self.most_recently_selected_room.as_ref().is_some_and(|r| r == &room) {
            return;
        }

        let dock = self.view.dock(ids!(dock));

        // If the room is already open, select (jump to) its existing tab
        let room_id_as_live_id = LiveId::from_str(room.room_id().as_str());
        if self.open_rooms.contains_key(&room_id_as_live_id) {
            dock.select_tab(cx, room_id_as_live_id);
            self.most_recently_selected_room = Some(room);
            return;
        }

        // Create a new tab for the room
        let (tab_bar, _pos) = dock.find_tab_bar_of_tab(id!(home_tab)).unwrap();
        let (kind, name) = match &room {
            SelectedRoom::JoinedRoom { room_name_id }  => (
                id!(room_screen),
                room_name_id.to_string(),
            ),
            SelectedRoom::InvitedRoom { room_name_id } => (
                id!(invite_screen),
                room_name_id.to_string(),
            ),
            SelectedRoom::Space { space_name_id } => (
                id!(space_lobby_screen),
                space_name_id.to_string(),
            ),
        };
        let new_tab_widget = dock.create_and_select_tab(
            cx,
            tab_bar,
            room_id_as_live_id,
            kind,
            name,
            id!(CloseableTab),
            None, // insert the tab at the end
            // TODO: insert the tab after the most-recently-selected room
        );

        // if the tab was created, set the room screen and add the room to the room order
        if let Some(new_widget) = new_tab_widget {
            self.room_order.push(room.clone());
            match &room {
                SelectedRoom::JoinedRoom { room_name_id }  => {
                    new_widget.as_room_screen().set_displayed_room(
                        cx,
                        room_name_id,
                    );
                }
                SelectedRoom::InvitedRoom { room_name_id } => {
                    new_widget.as_invite_screen().set_displayed_invite(
                        cx,
                        room_name_id,
                    );
                }
                SelectedRoom::Space { space_name_id } => {
                    new_widget.as_space_lobby_screen().set_displayed_space(
                        cx,
                        space_name_id,
                    );
                }
            }
            cx.action(MainDesktopUiAction::SaveDockIntoAppState);
        } else {
            error!("BUG: failed to create tab for {room:?}");
        }

        self.open_rooms.insert(room_id_as_live_id, room.clone());
        self.most_recently_selected_room = Some(room);
    }

    /// Closes a tab in the dock and focuses on the latest open room.
    fn close_tab(&mut self, cx: &mut Cx, tab_id: LiveId) {
        let dock = self.view.dock(ids!(dock));
        if let Some(room_being_closed) = self.open_rooms.get(&tab_id) {
            self.room_order.retain(|sr| sr != room_being_closed);

            if self.open_rooms.len() > 1 {
                // If the closing tab is the active one, then focus the next room
                let active_room = self.most_recently_selected_room.as_ref();
                if let Some(active_room) = active_room {
                    if active_room == room_being_closed {
                        if let Some(new_focused_room) = self.room_order.last() {
                            // notify the app state about the new focused room
                            cx.action(AppStateAction::RoomFocused(new_focused_room.clone()));

                            // Set the new selected room to be used in the current draw
                            self.most_recently_selected_room = Some(new_focused_room.clone());
                        }
                    }
                }
            } else {
                // If there is no room to focus, notify app to reset the selected room in the app state
                cx.action(AppStateAction::FocusNone);
                dock.select_tab(cx, id!(home_tab));
                self.most_recently_selected_room = None;
            }
        }

        dock.close_tab(cx, tab_id);
        self.tab_to_close = None;
        self.open_rooms.remove(&tab_id);
    }

    /// Closes all tabs
    pub fn close_all_tabs(&mut self, cx: &mut Cx) {
        let dock = self.view.dock(ids!(dock));
        for tab_id in self.open_rooms.keys() {        
            dock.close_tab(cx, *tab_id);
        }

        dock.select_tab(cx, id!(home_tab));
        cx.action(AppStateAction::FocusNone);

        // Clear tab-related dock UI state.
        self.open_rooms.clear();
        self.tab_to_close = None;
        self.room_order.clear();
        self.most_recently_selected_room = None;
    }

    /// Replaces an invite with a joined room in the dock.
    fn replace_invite_with_joined_room(
        &mut self,
        cx: &mut Cx,
        _scope: &mut Scope,
        room_name_id: &RoomNameId,
    ) {
        let dock = self.view.dock(ids!(dock));
        let Some((new_widget, true)) = dock.replace_tab(
            cx,
            LiveId::from_str(room_name_id.room_id().as_str()),
            id!(room_screen),
            Some(room_name_id.to_string()),
            false,
        ) else {
            // Nothing we can really do here except log an error.
            error!("BUG: failed to replace InviteScreen tab with RoomScreen for {room_name_id}");
            return;
        };

        // Set the info to be displayed in the newly-replaced RoomScreen..
        new_widget
            .as_room_screen()
            .set_displayed_room(cx, room_name_id);

        // Go through all existing `SelectedRoom` instances and replace the
        // `SelectedRoom::InvitedRoom`s with `SelectedRoom::JoinedRoom`s.
        for selected_room in self.most_recently_selected_room.iter_mut()
            .chain(self.room_order.iter_mut())
            .chain(self.open_rooms.values_mut())
        {
            selected_room.upgrade_invite_to_joined(room_name_id.room_id());
        }

        // Finally, emit an action to update the AppState with the new room.
        cx.action(AppStateAction::UpgradedInviteToJoinedRoom(room_name_id.room_id().clone()));
    }

    /// Saves a copy of the current UI state of the dock into the given app state,
    /// properly accounting for which space is currently selected.
    fn save_dock_state_to(&mut self, app_state: &mut AppState) {
        if self.open_rooms.is_empty() {
            return;
        } 
        let saved_dock_state = self.save_dock_state();
        if let Some(space_id) = self.selected_space.as_ref() {
            app_state.saved_dock_state_per_space.insert(
                space_id.clone(),
                saved_dock_state,
            );
        } else {
            app_state.saved_dock_state_home = saved_dock_state;
        }
    }

    /// An inner function that creates a `SavedDockState` from the current contents of this widget. 
    fn save_dock_state(&self) -> SavedDockState {
        let dock = self.view.dock(ids!(dock));
        SavedDockState {
            dock_items: dock.clone_state().unwrap_or_default(),
            open_rooms: self.open_rooms.clone(),
            room_order: self.room_order.clone(),
            selected_room: self.most_recently_selected_room.clone(),
        }
    }

    /// Loads and populates the dock from the saved dock state for the currently-selected space.
    ///
    /// If the saved state is empty (has no open rooms), we use the default dock layout
    /// defined in the DSL: one splitter with the RoomsList on the left and a Welcome tab on the right.
    fn load_dock_state_from(&mut self, cx: &mut Cx, app_state: &mut AppState) {
        let dock = self.view.dock(ids!(dock));
        let to_restore_opt = if let Some(ss) = self.selected_space.as_ref() {
            app_state.saved_dock_state_per_space.get(ss)
        } else {
            Some(&app_state.saved_dock_state_home)
        };
        let to_restore = match to_restore_opt {
            None => &self.default_layout,
            Some(sds) if sds.open_rooms.is_empty() => &self.default_layout,
            Some(sds) => sds,
        };
        let SavedDockState { dock_items, open_rooms, room_order, selected_room } = to_restore;

        self.room_order = room_order.clone();
        self.open_rooms = open_rooms.clone();

        if let Some(mut dock) = dock.borrow_mut() {
            dock.load_state(cx, dock_items.clone());
            // Populate the content within each restored dock tab.
            if !self.open_rooms.is_empty() {
                for (head_live_id, (_, widget)) in dock.items().iter() {
                    match self.open_rooms.get(head_live_id) {
                        Some(SelectedRoom::JoinedRoom { room_name_id }) => {
                            widget.as_room_screen().set_displayed_room(
                                cx,
                                room_name_id,
                            );
                        }
                        Some(SelectedRoom::InvitedRoom { room_name_id }) => {
                            widget.as_invite_screen().set_displayed_invite(
                                cx,
                                room_name_id,
                            );
                        }
                        Some(SelectedRoom::Space { space_name_id }) => {
                            widget.as_space_lobby_screen().set_displayed_space(
                                cx,
                                space_name_id,
                            );
                        }
                        None => { }
                    }
                }
            }
        } else {
            error!("BUG: failed to borrow dock widget to restore state upon LoadDockFromAppState action.");
            return;
        }
        // Note: the borrow of `dock` must end here *before* we call `self.focus_or_create_tab()`.

        // Now that we've loaded the dock content, we can re-select the selected room.
        let selected_room = selected_room.clone();
        if let Some(selected_room) = selected_room.clone() {
            self.focus_or_create_tab(cx, selected_room);
        }
        app_state.selected_room = selected_room;
        self.redraw(cx);
    }
}

impl WidgetMatchEvent for MainDesktopUI {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, scope: &mut Scope) {
        let mut should_save_dock_action: bool = false;
        for action in actions {
            let widget_action = action.as_widget_action();

            if let Some(MainDesktopUiAction::CloseAllTabs { on_close_all }) = action.downcast_ref() {
                self.close_all_tabs(cx);
                on_close_all.notify_one();
                continue;
            }

            // If the currently-selected space has been changed, we must handle that
            // by switching the dock to show the layout for another space.
            if let Some(NavigationBarAction::TabSelected(tab)) = action.downcast_ref() {
                let new_space = match (tab, self.selected_space.as_ref()) {
                    (SelectedTab::Space { space_name_id }, space_id_opt)
                        if space_id_opt.is_none_or(|id| id != space_name_id.room_id()) => 
                    {
                        Some(space_name_id.room_id().clone())
                    }
                    (SelectedTab::Home, Some(_)) => None,
                    _ => continue,
                };
                let app_state = scope.data.get_mut::<AppState>().unwrap();
                self.save_dock_state_to(app_state);
                self.selected_space = new_space;
                self.load_dock_state_from(cx, app_state);
                self.redraw(cx);
                continue;
            }

            // Handle actions emitted by the dock within the MainDesktopUI
            match widget_action.cast() {
                // Whenever a tab (except for the home_tab) is pressed, notify the app state.
                DockAction::TabWasPressed(tab_id) => {
                    if tab_id == id!(home_tab) {
                        cx.action(AppStateAction::FocusNone);
                        self.most_recently_selected_room = None;
                    }
                    else if let Some(selected_room) = self.open_rooms.get(&tab_id) {
                        cx.action(AppStateAction::RoomFocused(selected_room.clone()));
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
                    self.view.dock(ids!(dock)).tab_start_drag(
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
                        self.view.dock(ids!(dock)).accept_drag(cx, drag_event, DragResponse::Move);
                    }
                }
                // When dropping a tab, move it to the new position
                DockAction::Drop(drop_event) => {
                    // from inside the dock, otherwise it's an external file
                    if let DragItem::FilePath {
                        internal_id: Some(internal_id),
                        ..
                    } = &drop_event.items[0] {
                        self.view.dock(ids!(dock)).drop_move(cx, drop_event.abs, *internal_id);
                    }
                    should_save_dock_action = true;
                }
                _ => (),
            }

            // Handle RoomsList actions, which are updates from the rooms list.
            match widget_action.cast_ref() {
                RoomsListAction::Selected(selected_room) => {
                    // Note that this cannot be performed within draw_walk() as the draw flow prevents from
                    // performing actions that would trigger a redraw, and the Dock internally performs (and expects)
                    // a redraw to be happening in order to draw the tab content.
                    self.focus_or_create_tab(cx, selected_room.clone());
                }
                RoomsListAction::InviteAccepted { room_name_id } => {
                    self.replace_invite_with_joined_room(cx, scope, room_name_id);
                }
                RoomsListAction::None => { }
            }

            // Handle our own actions related to dock updates that we have previously emitted.
            match action.downcast_ref() {
                Some(MainDesktopUiAction::LoadDockFromAppState) => {
                    let app_state = scope.data.get_mut::<AppState>().unwrap();
                    self.load_dock_state_from(cx, app_state);
                }
                Some(MainDesktopUiAction::SaveDockIntoAppState) => {
                    let app_state = scope.data.get_mut::<AppState>().unwrap();
                    self.save_dock_state_to(app_state);
                }
                _ => {}
            }
        }

        if should_save_dock_action {
            cx.action(MainDesktopUiAction::SaveDockIntoAppState);
        }
    }
}

/// Actions sent to the MainDesktopUI widget for saving/restoring its dock state.
#[derive(Debug)]
pub enum MainDesktopUiAction {
    /// Save the state of the dock into the AppState.
    SaveDockIntoAppState,
    /// Load the room panel state from the AppState to the dock.
    LoadDockFromAppState,
    /// Close all tabs; see [`MainDesktopUI::close_all_tabs()`]
    CloseAllTabs {
        on_close_all: Arc<Notify>,
    },
}
