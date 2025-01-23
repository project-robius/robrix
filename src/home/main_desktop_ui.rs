use makepad_widgets::*;
use std::collections::HashMap;

use crate::app::SelectedRoom;

use super::room_screen::RoomScreenWidgetRefExt;
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
}

impl Widget for MainDesktopUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
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
                }
                DockAction::TabCloseWasPressed(tab_id) => {
                    self.tab_to_close = Some(tab_id);
                    self.close_tab(cx, tab_id);
                    self.redraw(cx);
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
                }
                _ => (),
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
                self.focus_or_create_tab(cx, SelectedRoom {
                    room_id: room_id.clone(),
                    room_name: room_name.clone(),
                });
            }
        }
    }
}

#[derive(Clone, DefaultNone, Debug)]
pub enum RoomsPanelAction {
    None,
    /// Notifies that a room was focused
    RoomFocused(SelectedRoom),
    /// Resets the focus on the rooms panel
    FocusNone,
}
