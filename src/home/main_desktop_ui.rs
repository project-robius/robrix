use makepad_widgets::*;
use ruma::OwnedRoomId;
use tokio::sync::Notify;
use std::{collections::{HashMap, HashSet}, sync::Arc};

use crate::{app::{AppState, AppStateAction, SavedDockState, SelectedRoom}, home::{navigation_tab_bar::{NavigationBarAction, SelectedTab}, rooms_list::RoomsListRef, space_lobby::SpaceLobbyScreenWidgetRefExt}, utils::RoomNameId};
use super::{invite_screen::InviteScreenWidgetRefExt, room_screen::RoomScreenWidgetRefExt, rooms_list::RoomsListAction};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.MainDesktopUI = #(MainDesktopUI::register_widget(vm)) {
        dock := mod.widgets.RobrixDock {
            width: Fill,
            height: Fill,
            padding: 0,
            spacing: 0,
            margin: 0

            tab_bar +: {
                CloseableTab := mod.widgets.RobrixTab { closeable: true }
                PermanentTab := mod.widgets.RobrixTab { closeable: false }
            }


            root := DockSplitter {
                axis: SplitterAxis.Horizontal
                align: SplitterAlign.FromA(300.0)
                a: @rooms_sidebar_tabs
                b: @main_tabs
            }

            // This is a "fixed" tab with no header that cannot be closed.
            rooms_sidebar_tabs := DockTabs{
                tabs: [@rooms_sidebar_tab]
                selected: 0
                hide_tab_bar: true
            }

            main_tabs := DockTabs{
                tabs: [@home_tab]
                selected: 0
            }

            rooms_sidebar_tab := DockTab {
                kind: @rooms_sidebar // this template is defined below.
                template: @PermanentTab
            }

            home_tab := DockTab{
                name: "Home"
                kind: @welcome_screen
                template: @PermanentTab
            }

            // Below are the templates of widgets that can be created within dock tabs.
            rooms_sidebar := mod.widgets.RoomsSideBar {}
            welcome_screen := mod.widgets.WelcomeScreen {}
            room_screen := mod.widgets.RoomScreen {}
            invite_screen := mod.widgets.InviteScreen {}
            space_lobby_screen := mod.widgets.SpaceLobbyScreen {}
        }
    }
}

#[derive(Script, Widget)]
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

    /// The order in which room/thread tabs were last viewed,
    /// from oldest at the front to most recent at the end.
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

impl ScriptHook for MainDesktopUI {
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.default_layout = self.save_dock_state(cx);
        });
    }
}
impl Widget for MainDesktopUI {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.widget_match_event(cx, event, scope); // invokes `WidgetMatchEvent` impl
        self.view.handle_event(cx, event, scope);

        // For convenience, we support go-back gestures when viewing a thread's tab
        // to easily go back to the most recent room.
        if let Some(sr @ SelectedRoom::Thread { .. }) = self.most_recently_selected_room.as_ref()
            && (event.back_pressed() || matches!(event, Event::MouseUp(e) if e.button.is_back()))
        {
            self.close_tab(cx, sr.tab_id());
            self.redraw(cx);
            cx.action(MainDesktopUiAction::SaveDockIntoAppState);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.drawn_previously && cx.has_global::<RoomsListRef>() {
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
    /// Moves or adds the given room the the end of the `room_order`, the most recent spot.
    fn mark_room_as_recent(&mut self, room: &SelectedRoom) {
        self.room_order.retain(|sr| sr != room);
        self.room_order.push(room.clone());
    }

    /// Selects the tab for the given `room`, or the home tab if `None`.
    ///
    /// Updates the room order, current selection, and app state.
    fn select_room(&mut self, cx: &mut Cx, room: Option<SelectedRoom>) {
        let dock = self.view.dock(cx, ids!(dock));
        if let Some(room) = room {
            dock.select_tab(cx, room.tab_id());
            self.mark_room_as_recent(&room);
            cx.action(AppStateAction::RoomFocused(room.clone()));
            self.most_recently_selected_room = Some(room);
        } else {
            dock.select_tab(cx, id!(home_tab));
            cx.action(AppStateAction::FocusNone);
            self.most_recently_selected_room = None;
        }
    }

    /// Focuses on a room if it is already open, otherwise creates a new tab for the room.
    fn focus_or_create_tab(&mut self, cx: &mut Cx, room: SelectedRoom) {
        // Do nothing if the room to select is already created and focused.
        if self.most_recently_selected_room.as_ref().is_some_and(|sr| sr == &room) {
            return;
        }

        let dock = self.view.dock(cx, ids!(dock));

        // If the room is already open, select (jump to) its existing tab
        let room_tab_id = room.tab_id();
        if self.open_rooms.contains_key(&room_tab_id) {
            self.select_room(cx, Some(room));
            // Lazily initialize the tab's widget if it was deferred during dock restoration.
            self.init_tab_if_needed(cx, room_tab_id);
            cx.action(MainDesktopUiAction::SaveDockIntoAppState);
            return;
        }

        // Create a new tab for the room
        let kind = room.dock_kind();

        // Insert the tab after the currently-selected room's tab, if possible.
        // Otherwise, insert it after the home tab, which should always exist.
        let (tab_bar, insert_after) = self.most_recently_selected_room.as_ref()
            .and_then(|curr_room| dock.find_tab_bar_of_tab(curr_room.tab_id()))
            .unwrap_or_else(|| dock.find_tab_bar_of_tab(id!(home_tab)).unwrap());

        let new_tab_widget = dock.create_and_select_tab(
            cx,
            tab_bar,
            room_tab_id,
            kind,
            room.display_name(),
            id!(CloseableTab),
            Some(insert_after),
        );

        // if the tab was created, set the room screen
        if let Some(new_widget) = new_tab_widget {
            match &room {
                SelectedRoom::JoinedRoom { room_name_id }  => {
                    new_widget.as_room_screen().set_displayed_room(
                        cx,
                        room_name_id,
                        None,
                    );
                }
                SelectedRoom::Thread { room_name_id, thread_root_event_id } => {
                    new_widget.as_room_screen().set_displayed_room(
                        cx,
                        room_name_id,
                        Some(thread_root_event_id.clone()),
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
            self.open_rooms.insert(room_tab_id, room.clone());
            self.select_room(cx, Some(room));
        } else {
            error!("BUG: failed to create tab for {room:?}");
        }
    }

    /// Closes a tab in the dock and selects the next most recently viewed tab.
    fn close_tab(&mut self, cx: &mut Cx, tab_id: LiveId) {
        let dock = self.view.dock(cx, ids!(dock));

        let Some(room_being_closed) = self.open_rooms.get(&tab_id).cloned() else {
            // This shouldn't happen (the tab should always be in the set of open rooms),
            // but we still need to handle it gracefully.
            dock.close_tab(cx, tab_id);
            self.init_all_visible_tabs(cx);
            return;
        };
        // If we're closing a thread timeline, free up its resources & bkgd async tasks.
        room_being_closed.close_thread_timeline(cx);
        self.room_order.retain(|sr| sr != &room_being_closed);

        let is_active_tab = self.most_recently_selected_room.as_ref() == Some(&room_being_closed);
        let room_to_select = if is_active_tab {
            self.room_order.last().cloned()
        } else {
            self.most_recently_selected_room.clone()
        };

        dock.close_tab(cx, tab_id);
        self.open_rooms.remove(&tab_id);

        // Makepad's dock chooses an adjacent tab by itself, so we have to override that.
        self.select_room(cx, room_to_select);

        self.init_all_visible_tabs(cx);
    }

    /// Closes all tabs
    pub fn close_all_tabs(&mut self, cx: &mut Cx) {
        let dock = self.view.dock(cx, ids!(dock));
        for (tab_id, room) in self.open_rooms.iter() {
            room.close_thread_timeline(cx);
            dock.close_tab(cx, *tab_id);
        }

        // Clear tab-related dock UI state.
        self.open_rooms.clear();
        self.room_order.clear();
        self.select_room(cx, None);
        cx.action(MainDesktopUiAction::SaveDockIntoAppState);
    }

    /// Replaces an invite with a joined room in the dock.
    fn replace_invite_with_joined_room(
        &mut self,
        cx: &mut Cx,
        _scope: &mut Scope,
        room_name_id: &RoomNameId,
    ) {
        let dock = self.view.dock(cx, ids!(dock));
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
            .set_displayed_room(cx, room_name_id, None);

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
    fn save_dock_state_to(&mut self, cx: &mut Cx, app_state: &mut AppState) {
        let saved_dock_state = self.save_dock_state(cx);
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
    fn save_dock_state(&self, cx: &mut Cx) -> SavedDockState {
        let dock = self.view.dock(cx, ids!(dock));
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
        let dock = self.view.dock(cx, ids!(dock));

        let saved_ref: Option<&SavedDockState> = if let Some(ss) = self.selected_space.as_ref() {
            app_state.saved_dock_state_per_space.get(ss)
        } else {
            Some(&app_state.saved_dock_state_home)
        };
        let space_label = self.selected_space.as_ref()
            .map(|s| format!("space {s}"))
            .unwrap_or_else(|| "home".to_string());
        let (to_restore, recreate_from_room_order): (SavedDockState, Option<Vec<SelectedRoom>>) =
            match saved_ref {
                None => (self.default_layout.clone(), None),
                Some(sds) if sds.open_rooms.is_empty()
                    && sds.room_order.is_empty()
                    && sds.selected_room.is_none() => (self.default_layout.clone(), None),
                Some(sds) => {
                    let mut candidate = sds.clone();
                    match dock_state_repair::validate_and_repair_dock_state(&mut candidate) {
                        Ok(false) => (candidate, None),
                        Ok(true) => {
                            log!("Repaired corrupt saved dock state for {space_label}.");
                            // Update the app state with the repaired dock state to ensure that
                            // the next save operation will persist valid state to storage.
                            if let Some(ss) = self.selected_space.as_ref() {
                                app_state.saved_dock_state_per_space.insert(ss.clone(), candidate.clone());
                            } else {
                                app_state.saved_dock_state_home = candidate.clone();
                            }
                            (candidate, None)
                        }
                        Err(reason) => {
                            error!(
                                "Saved dock state for {space_label} is unrepairable ({reason}); \
                                 falling back to default layout and re-adding tabs from saved room_order."
                            );
                            let original_selected = sds.selected_room.clone();
                            let original_order = sds.room_order.clone();
                            let mut fallback = self.default_layout.clone();
                            fallback.selected_room = original_selected;
                            (fallback, Some(original_order))
                        }
                    }
                }
            };

        let SavedDockState { dock_items, open_rooms, room_order, selected_room } = to_restore;

        self.room_order = room_order;
        self.open_rooms = open_rooms;

        dock.load_state(cx, dock_items);
        // Lazily populate the dock content to avoid initializing tabs that aren't visible.
        self.init_all_visible_tabs(cx);

        // If using the fallback, we re-add each tab from room_order so the user's opened rooms aren't lost.
        if let Some(orig_room_order) = recreate_from_room_order {
            for room in orig_room_order {
                self.focus_or_create_tab(cx, room);
            }
        }

        // Now that we've loaded the dock content, we can re-select the selected room.
        let selected_room = selected_room.clone();
        if let Some(selected_room) = selected_room.clone() {
            self.focus_or_create_tab(cx, selected_room);
        }
        app_state.selected_room = selected_room;
        self.redraw(cx);
    }

    /// Initializes a single dock tab's widget content based on the room it represents.
    ///
    /// This is extracted as a helper so it can be called both during dock restoration
    /// (for the selected tab only) and lazily when the user clicks on an uninitialized tab.
    fn init_tab_widget(
        cx: &mut Cx,
        open_rooms: &HashMap<LiveId, SelectedRoom>,
        tab_live_id: &LiveId,
        widget: &WidgetRef,
    ) {
        match open_rooms.get(tab_live_id) {
            Some(SelectedRoom::JoinedRoom { room_name_id }) => {
                widget.as_room_screen().set_displayed_room(
                    cx,
                    room_name_id,
                    None,
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
            Some(SelectedRoom::Thread { room_name_id, thread_root_event_id }) => {
                widget.as_room_screen().set_displayed_room(
                    cx,
                    room_name_id,
                    Some(thread_root_event_id.clone()),
                );
            }
            None => { }
        }
    }

    /// Lazily initializes a tab's widget if it hasn't been initialized yet.
    ///
    /// This is called when a tab becomes visible (e.g., via user click or sidebar selection)
    /// that was restored from saved state but whose widget content was deferred
    /// to avoid blocking the UI thread.
    ///
    /// It is safe to call this on an already-initialized tab, as the underlying
    /// `set_displayed_*` methods short-circuit when the content is already set.
    fn init_tab_if_needed(&self, cx: &mut Cx, tab_id: LiveId) {
        if !self.open_rooms.contains_key(&tab_id) {
            return;
        }
        let widget = self.view.dock(cx, ids!(dock)).item(tab_id);
        if !widget.is_empty() {
            Self::init_tab_widget(cx, &self.open_rooms, &tab_id, &widget);
        }
    }

    /// Initializes all currently-visible (selected-in-their-pane) tabs that
    /// haven't been initialized yet.
    ///
    /// This is useful after operations that may change which tabs are visible
    /// without going through explicit tab selection (e.g., closing a tab causes
    /// the dock to auto-select an adjacent tab).
    fn init_all_visible_tabs(&self, cx: &mut Cx) {
        let dock = self.view.dock(cx, ids!(dock));
        let Some(mut dock) = dock.borrow_mut() else { return };
        for (tab_id, widget) in dock.visible_items() {
            Self::init_tab_widget(cx, &self.open_rooms, &tab_id, &widget);
        }
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
                self.save_dock_state_to(cx, app_state);
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
                        self.select_room(cx, None);
                    }
                    else if let Some(selected_room) = self.open_rooms.get(&tab_id).cloned() {
                        self.select_room(cx, Some(selected_room));
                    }
                    // Lazily initialize this tab's widget if it was deferred during dock restoration.
                    self.init_tab_if_needed(cx, tab_id);
                    should_save_dock_action = true;
                }
                DockAction::TabCloseWasPressed(tab_id) => {
                    self.close_tab(cx, tab_id);
                    self.redraw(cx);
                    should_save_dock_action = true;
                }
                // When dragging a tab, allow it to be dragged
                DockAction::ShouldTabStartDrag(tab_id) => {
                    self.view.dock(cx, ids!(dock)).tab_start_drag(
                        cx,
                        tab_id,
                        DragItem::FilePath {
                            path: "".to_string(),
                            internal_id: Some(tab_id),
                        },
                    );
                }
                // When dragging a tab, allow it to be dragged
                DockAction::Drag(drag_event) if drag_event.items.len() == 1 => {
                    self.view.dock(cx, ids!(dock)).accept_drag(cx, drag_event, DragResponse::Move);
                }
                // When dropping a tab, move it to the new position
                DockAction::Drop(drop_event) => {
                    // from inside the dock, otherwise it's an external file
                    if let DragItem::FilePath {
                        internal_id: Some(internal_id),
                        ..
                    } = &drop_event.items[0] {
                        self.view.dock(cx, ids!(dock)).drop_move(cx, drop_event.abs, *internal_id);
                    }
                    // A drag-drop may create a new split pane, revealing an
                    // uninitialized tab that was deferred during dock restoration.
                    self.init_all_visible_tabs(cx);
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
                RoomsListAction::OpenRoomContextMenu { .. } => {}
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
                    self.save_dock_state_to(cx, app_state);
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

/// Saved dock state validation, repair, and helpers for translating
/// `SelectedRoom`s into dock items.
mod dock_state_repair {
    use super::*;

    /// Checks the given saved dock state and makes repairs in place.
    ///
    /// Removes orphaned, duplicated, and dangling dock item references while preserving
    /// all open rooms that can still be reconstructed.
    ///
    /// * Returns `Ok` if we successfully repaired the dock state (or it was already good),
    /// * Returns `Err` is the state is unrepairable, meaning we should fall back to the default.
    pub(super) fn validate_and_repair_dock_state(saved: &mut SavedDockState) -> Result<bool, &'static str> {
        let root = id!(root);
        let home_tab = id!(home_tab);
        let main_tabs_dsl = id!(main_tabs);
        let rooms_sidebar_tab = id!(rooms_sidebar_tab);
        let rooms_sidebar_tabs = id!(rooms_sidebar_tabs);

        let mut repaired = false;

        repaired |= ensure_fixed_tab_item(
            saved,
            home_tab,
            DockItem::Tab {
                name: "Home".to_string(),
                template: id!(PermanentTab),
                kind: id!(welcome_screen),
            },
        )?;
        repaired |= ensure_fixed_tab_item(
            saved,
            rooms_sidebar_tab,
            DockItem::Tab {
                name: "Rooms".to_string(),
                template: id!(PermanentTab),
                kind: id!(rooms_sidebar),
            },
        )?;
        if !any_tabs_contains(&saved.dock_items, home_tab) {
            repaired |= ensure_tab_ref_in_tabs_container(saved, main_tabs_dsl, home_tab)?;
        }
        repaired |= ensure_tab_ref_in_tabs_container(saved, rooms_sidebar_tabs, rooms_sidebar_tab)?;

        // Rebuild missing bookkeeping from the more redundant fields. Older corrupt
        // snapshots often have a valid room_order but an incomplete open_rooms map.
        let known_rooms: Vec<SelectedRoom> = saved.room_order.iter()
            .chain(saved.selected_room.iter())
            .cloned()
            .collect();
        for room in known_rooms {
            let tab_id = room.tab_id();
            if saved.open_rooms.get(&tab_id) != Some(&room) {
                saved.open_rooms.insert(tab_id, room);
                repaired = true;
            }
        }
        if let Some(sr) = saved.selected_room.clone()
            && !saved.room_order.iter().any(|r| r == &sr)
        {
            saved.room_order.push(sr);
            repaired = true;
        }

        // If an open room is still known semantically but its DockItem::Tab was lost,
        // recreate that tab item so it can be rescued into the main tab bar.
        for (&tab_id, room) in saved.open_rooms.iter() {
            match saved.dock_items.get(&tab_id) {
                Some(DockItem::Tab { .. }) => {}
                None => {
                    saved.dock_items.insert(tab_id, dock_tab_for_selected_room(room));
                    repaired = true;
                }
                Some(_) => return Err("open room tab id collides with a dock container"),
            }
        }

        let (mut tree_order, mut reachable) = walk_dock_tree(&saved.dock_items)?;

        // The main-tabs container is the first reachable `Tabs` (in DFS order) that
        // holds `home_tab`.
        let Some(main_tabs_id) = find_tabs_containing(&saved.dock_items, &tree_order, home_tab) else {
            return Err("no reachable main-tabs container (holding home_tab) was found");
        };
        if main_tabs_id == root {
            promote_root_tabs_to_default_split(
                saved,
                main_tabs_dsl,
                rooms_sidebar_tabs,
                rooms_sidebar_tab,
            )?;
            repaired = true;
            (tree_order, _) = walk_dock_tree(&saved.dock_items)?;
        } else if main_tabs_id != main_tabs_dsl {
            // Normalize the home-holding tab container to id!(main_tabs). If id!(main_tabs)
            // is already a reachable different container, swap the two IDs instead of
            // deleting the reachable container's content.
            normalize_main_tabs_id(saved, main_tabs_id, main_tabs_dsl, &reachable)?;
            repaired = true;
            (tree_order, _) = walk_dock_tree(&saved.dock_items)?;
        }

        // Dedupe in tree (DFS) order so the root-most reference wins. Otherwise
        // find_tab_bar_of_tab picks an arbitrary HashMap entry and new tabs can
        // land in the wrong container.
        let mut referenced_tabs = HashSet::new();
        repaired |= remove_bad_and_duplicate_tab_refs(saved, &tree_order, &mut referenced_tabs);

        // Rescue open room tabs that are no longer referenced by any reachable Tabs
        // container. Recreated missing DockItem::Tab entries from above are handled here.
        repaired |= append_unreferenced_open_rooms_to_main_tabs(saved, main_tabs_dsl, &mut referenced_tabs);
        (tree_order, reachable) = walk_dock_tree(&saved.dock_items)?;

        // Collapse reachable empty Tabs out of the tree. Dedupe can strip the only
        // tab from a container; if we leave it in place, Makepad renders it as an
        // empty pane. main_tabs and the fixed sidebar tabs container are preserved.
        loop {
            let empty_id = tree_order.iter()
                .find(|id| **id != root
                    && **id != main_tabs_dsl
                    && **id != rooms_sidebar_tabs
                    && matches!(
                        saved.dock_items.get(*id),
                        Some(DockItem::Tabs { tabs, .. }) if tabs.is_empty()
                    )
                )
                .copied();
            let Some(empty_id) = empty_id else { break };
            collapse_tabs_container(saved, empty_id, main_tabs_dsl, rooms_sidebar_tabs)?;
            repaired = true;
            (tree_order, reachable) = walk_dock_tree(&saved.dock_items)?;
        }

        // Drop anything still unreachable, such as orphaned Tabs/Splitter items.
        let len_before = saved.dock_items.len();
        saved.dock_items.retain(|id, _| reachable.contains(id));
        if saved.dock_items.len() != len_before {
            repaired = true;
            (tree_order, _) = walk_dock_tree(&saved.dock_items)?;
        }

        if find_tabs_containing(&saved.dock_items, &tree_order, home_tab) != Some(main_tabs_dsl) {
            return Err("home_tab is not in the reachable main-tabs container");
        }
        if find_tabs_containing(&saved.dock_items, &tree_order, rooms_sidebar_tab).is_none() {
            return Err("rooms sidebar tab is not reachable");
        }

        // Collect the post-cleanup set of tab IDs reachable from the tree, used to
        // prune open_rooms below. No new dedupe needed; collapse/drop only remove
        // containers, they can't introduce duplicate tab refs.
        referenced_tabs.clear();
        for &id in &tree_order {
            if let Some(DockItem::Tabs { tabs, .. }) = saved.dock_items.get(&id) {
                referenced_tabs.extend(tabs.iter().copied());
            }
        }

        // Adjust the indices for the selected room.
        if let Some(sel_id) = saved.selected_room.as_ref().map(|sr| sr.tab_id())
            && let Some(DockItem::Tabs { tabs, selected, .. }) =
                saved.dock_items.get_mut(&main_tabs_dsl)
            && let Some(idx) = tabs.iter().position(|t| *t == sel_id)
            && *selected != idx
        {
            *selected = idx;
            repaired = true;
        }

        let len_before = saved.open_rooms.len();
        saved.open_rooms.retain(|id, _| referenced_tabs.contains(id));
        repaired |= saved.open_rooms.len() != len_before;

        // Drop room_order entries whose tab is gone or duplicated, then append any
        // open_rooms entries that aren't in the order yet (sorted by tab id for stability).
        let mut seen_room_tabs = HashSet::with_capacity(saved.room_order.len());
        let len_before = saved.room_order.len();
        saved.room_order.retain(|sr| {
            saved.open_rooms.contains_key(&sr.tab_id()) && seen_room_tabs.insert(sr.tab_id())
        });
        let mut missing: Vec<SelectedRoom> = saved.open_rooms.values()
            .filter(|r| !seen_room_tabs.contains(&r.tab_id()))
            .cloned()
            .collect();
        missing.sort_by_key(|r| r.tab_id().0);
        repaired |= saved.room_order.len() != len_before || !missing.is_empty();
        saved.room_order.extend(missing);

        repaired |= saved.selected_room
            .take_if(|sr| !saved.open_rooms.contains_key(&sr.tab_id()))
            .is_some();

        Ok(repaired)
    }

    fn dock_tab_for_selected_room(room: &SelectedRoom) -> DockItem {
        DockItem::Tab {
            name: room.display_name(),
            template: id!(CloseableTab),
            kind: room.dock_kind(),
        }
    }

    fn ensure_fixed_tab_item(
        saved: &mut SavedDockState,
        tab_id: LiveId,
        tab_item: DockItem,
    ) -> Result<bool, &'static str> {
        match saved.dock_items.get(&tab_id) {
            Some(DockItem::Tab { .. }) => Ok(false),
            None => {
                saved.dock_items.insert(tab_id, tab_item);
                Ok(true)
            }
            Some(_) => Err("fixed tab id collides with a dock container"),
        }
    }

    fn any_tabs_contains(dock_items: &HashMap<LiveId, DockItem>, tab_id: LiveId) -> bool {
        dock_items.values().any(|item| {
            matches!(item, DockItem::Tabs { tabs, .. } if tabs.contains(&tab_id))
        })
    }

    fn ensure_tab_ref_in_tabs_container(
        saved: &mut SavedDockState,
        tabs_id: LiveId,
        tab_id: LiveId,
    ) -> Result<bool, &'static str> {
        match saved.dock_items.get_mut(&tabs_id) {
            Some(DockItem::Tabs { tabs, selected, .. }) => {
                if tabs.contains(&tab_id) {
                    return Ok(false);
                }
                if !tabs.is_empty() {
                    *selected = selected.saturating_add(1);
                }
                tabs.insert(0, tab_id);
                Ok(true)
            }
            Some(DockItem::Tab { .. }) | Some(DockItem::Splitter { .. }) => {
                Err("fixed tabs container id collides with another dock item")
            }
            None => Ok(false),
        }
    }

    fn promote_root_tabs_to_default_split(
        saved: &mut SavedDockState,
        main_tabs_id: LiveId,
        rooms_sidebar_tabs_id: LiveId,
        rooms_sidebar_tab_id: LiveId,
    ) -> Result<(), &'static str> {
        let root = id!(root);
        let Some(root_tabs) = saved.dock_items.remove(&root) else {
            return Err("root dock item is missing");
        };
        if !matches!(root_tabs, DockItem::Tabs { .. }) {
            saved.dock_items.insert(root, root_tabs);
            return Err("root dock item is not a tabs container");
        }

        saved.dock_items.insert(main_tabs_id, root_tabs);
        saved.dock_items.insert(
            rooms_sidebar_tabs_id,
            DockItem::Tabs {
                tabs: vec![rooms_sidebar_tab_id],
                selected: 0,
                closable: true,
                hide_tab_bar: true,
            },
        );
        saved.dock_items.insert(
            root,
            DockItem::Splitter {
                axis: SplitterAxis::Horizontal,
                align: SplitterAlign::FromA(300.0),
                a: rooms_sidebar_tabs_id,
                b: main_tabs_id,
            },
        );
        Ok(())
    }

    fn walk_dock_tree(
        dock_items: &HashMap<LiveId, DockItem>,
    ) -> Result<(Vec<LiveId>, HashSet<LiveId>), &'static str> {
        fn visit(
            id: LiveId,
            dock_items: &HashMap<LiveId, DockItem>,
            order: &mut Vec<LiveId>,
            reachable: &mut HashSet<LiveId>,
            visiting: &mut HashSet<LiveId>,
        ) -> Result<(), &'static str> {
            let Some(item) = dock_items.get(&id) else {
                return Err("dock item reference is missing");
            };
            if visiting.contains(&id) {
                return Err("dock item graph contains a cycle");
            }
            if reachable.contains(&id) {
                return if matches!(item, DockItem::Tab { .. }) {
                    Ok(())
                } else {
                    Err("dock container is referenced more than once")
                };
            }

            reachable.insert(id);
            visiting.insert(id);
            order.push(id);
            match item {
                DockItem::Splitter { a, b, .. } => {
                    if a == b {
                        return Err("splitter references the same child twice");
                    }
                    visit(*a, dock_items, order, reachable, visiting)?;
                    visit(*b, dock_items, order, reachable, visiting)?;
                }
                DockItem::Tabs { tabs, .. } => {
                    for tab_id in tabs {
                        if matches!(dock_items.get(tab_id), Some(DockItem::Tab { .. })) {
                            visit(*tab_id, dock_items, order, reachable, visiting)?;
                        }
                    }
                }
                DockItem::Tab { .. } => {}
            }
            visiting.remove(&id);
            Ok(())
        }

        if !dock_items.contains_key(&id!(root)) {
            return Err("root dock item is missing");
        }

        let mut order = Vec::with_capacity(dock_items.len());
        let mut reachable = HashSet::with_capacity(dock_items.len());
        let mut visiting = HashSet::with_capacity(dock_items.len());
        visit(id!(root), dock_items, &mut order, &mut reachable, &mut visiting)?;
        Ok((order, reachable))
    }

    fn find_tabs_containing(
        dock_items: &HashMap<LiveId, DockItem>,
        tree_order: &[LiveId],
        tab_id: LiveId,
    ) -> Option<LiveId> {
        tree_order.iter().find_map(|id| {
            if let Some(DockItem::Tabs { tabs, .. }) = dock_items.get(id) {
                if tabs.contains(&tab_id) {
                    return Some(*id);
                }
            }
            None
        })
    }

    fn normalize_main_tabs_id(
        saved: &mut SavedDockState,
        current_main_tabs_id: LiveId,
        main_tabs_dsl: LiveId,
        reachable: &HashSet<LiveId>,
    ) -> Result<(), &'static str> {
        if current_main_tabs_id == id!(root) {
            return Err("main-tabs container is root");
        }
        let Some(current_main_tabs) = saved.dock_items.remove(&current_main_tabs_id) else {
            return Err("main-tabs container is missing");
        };
        let existing_dsl_item = saved.dock_items.remove(&main_tabs_dsl);
        let existing_dsl_was_reachable = existing_dsl_item.as_ref().is_some_and(|item| {
            reachable.contains(&main_tabs_dsl) && !matches!(item, DockItem::Tab { .. })
        });

        saved.dock_items.insert(main_tabs_dsl, current_main_tabs);
        if existing_dsl_was_reachable
            && let Some(existing_dsl_item) = existing_dsl_item
        {
            saved.dock_items.insert(current_main_tabs_id, existing_dsl_item);
        }

        // Rewrite splitter refs: when the existing dsl entry was reachable we swap
        // the two IDs everywhere; otherwise we just point current -> dsl.
        let remap = |id: &mut LiveId| {
            if *id == current_main_tabs_id {
                *id = main_tabs_dsl;
            } else if existing_dsl_was_reachable && *id == main_tabs_dsl {
                *id = current_main_tabs_id;
            }
        };
        for item in saved.dock_items.values_mut() {
            if let DockItem::Splitter { a, b, .. } = item {
                remap(a);
                remap(b);
            }
        }

        Ok(())
    }

    fn remove_bad_and_duplicate_tab_refs(
        saved: &mut SavedDockState,
        tree_order: &[LiveId],
        referenced_tabs: &mut HashSet<LiveId>,
    ) -> bool {
        let valid_tabs: HashSet<LiveId> = saved.dock_items.iter()
            .filter_map(|(id, item)| matches!(item, DockItem::Tab { .. }).then_some(*id))
            .collect();
        let mut seen = HashSet::with_capacity(valid_tabs.len());
        let mut repaired = false;
        for &id in tree_order {
            if let Some(DockItem::Tabs { tabs, selected, .. }) = saved.dock_items.get_mut(&id) {
                let original_len = tabs.len();
                tabs.retain(|tab_id| {
                    valid_tabs.contains(tab_id)
                        && seen.insert(*tab_id)
                        && {
                            referenced_tabs.insert(*tab_id);
                            true
                        }
                });
                if tabs.len() != original_len {
                    repaired = true;
                }
                let new_selected = if tabs.is_empty() {
                    0
                } else if *selected >= tabs.len() {
                    tabs.len() - 1
                } else {
                    *selected
                };
                if *selected != new_selected {
                    *selected = new_selected;
                    repaired = true;
                }
            }
        }
        repaired
    }

    fn append_unreferenced_open_rooms_to_main_tabs(
        saved: &mut SavedDockState,
        main_tabs_id: LiveId,
        referenced_tabs: &mut HashSet<LiveId>,
    ) -> bool {
        let mut repaired = false;
        let mut candidates = Vec::with_capacity(saved.room_order.len() + saved.open_rooms.len());
        let mut candidate_ids = HashSet::with_capacity(saved.room_order.len() + saved.open_rooms.len());
        for room in saved.room_order.iter() {
            let tab_id = room.tab_id();
            if candidate_ids.insert(tab_id) {
                candidates.push(tab_id);
            }
        }
        let mut unordered: Vec<LiveId> = saved.open_rooms.keys()
            .filter(|tab_id| candidate_ids.insert(**tab_id))
            .copied()
            .collect();
        unordered.sort_by_key(|tab_id| tab_id.0);
        candidates.extend(unordered);

        let candidates: Vec<LiveId> = candidates.into_iter()
            .filter(|tab_id| matches!(saved.dock_items.get(tab_id), Some(DockItem::Tab { .. })))
            .collect();
        if let Some(DockItem::Tabs { tabs, selected, .. }) = saved.dock_items.get_mut(&main_tabs_id) {
            for tab_id in candidates {
                if referenced_tabs.contains(&tab_id) {
                    continue;
                }
                tabs.push(tab_id);
                referenced_tabs.insert(tab_id);
                repaired = true;
            }
            if *selected >= tabs.len() && !tabs.is_empty() {
                *selected = tabs.len() - 1;
                repaired = true;
            }
        }
        repaired
    }

    fn collapse_tabs_container(
        saved: &mut SavedDockState,
        tabs_id: LiveId,
        main_tabs_id: LiveId,
        rooms_sidebar_tabs_id: LiveId,
    ) -> Result<(), &'static str> {
        let root = id!(root);
        let Some((parent_split, sibling)) = saved.dock_items.iter().find_map(|(sid, item)| {
            if let DockItem::Splitter { a, b, .. } = item {
                if *a == tabs_id {
                    return Some((*sid, *b));
                }
                if *b == tabs_id {
                    return Some((*sid, *a));
                }
            }
            None
        }) else {
            saved.dock_items.remove(&tabs_id);
            return Ok(());
        };

        if parent_split == root {
            if sibling == main_tabs_id || sibling == rooms_sidebar_tabs_id {
                return Err("empty pane collapse would remove a required dock container id");
            }
            let Some(sibling_content) = saved.dock_items.remove(&sibling) else {
                return Err("empty pane sibling is missing");
            };
            saved.dock_items.insert(root, sibling_content);
        } else {
            if !saved.dock_items.contains_key(&sibling) {
                return Err("empty pane sibling is missing");
            }
            if !replace_splitter_refs(&mut saved.dock_items, parent_split, sibling) {
                return Err("empty pane parent splitter is detached");
            }
            saved.dock_items.remove(&parent_split);
        }
        saved.dock_items.remove(&tabs_id);
        Ok(())
    }

    fn replace_splitter_refs(
        dock_items: &mut HashMap<LiveId, DockItem>,
        from: LiveId,
        to: LiveId,
    ) -> bool {
        let mut replaced = false;
        for item in dock_items.values_mut() {
            if let DockItem::Splitter { a, b, .. } = item {
                if *a == from { *a = to; replaced = true; }
                if *b == from { *b = to; replaced = true; }
            }
        }
        replaced
    }

}
