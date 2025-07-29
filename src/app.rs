//! The top-level application content.
//!
//! See `handle_startup()` for the first code that runs on app startup.

// Ignore clippy warnings in `DeRon` macro derive bodies.
#![allow(clippy::question_mark)]

use std::collections::HashMap;
use makepad_widgets::{makepad_micro_serde::*, *};
use matrix_sdk::ruma::{OwnedRoomId, RoomId};
use crate::{
    home::{
        main_desktop_ui::MainDesktopUiAction,
        new_message_context_menu::NewMessageContextMenuWidgetRefExt,
        room_screen::MessageAction,
        rooms_list::RoomsListAction,
    },
    join_leave_room_modal::{
        JoinLeaveRoomModalAction,
        JoinLeaveRoomModalWidgetRefExt,
    },
    login::login_screen::LoginAction,
    persistence,
    shared::callout_tooltip::{
        CalloutTooltipOptions,
        CalloutTooltipWidgetRefExt,
        TooltipAction,
    },
    sliding_sync::current_user_id,
    utils::{
        room_name_or_id,
        OwnedRoomIdRon,
    },
    verification::VerificationAction,
    verification_modal::{
        VerificationModalAction,
        VerificationModalWidgetRefExt,
    },
};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::home_screen::HomeScreen;
    use crate::verification_modal::VerificationModal;
    use crate::join_leave_room_modal::JoinLeaveRoomModal;
    use crate::login::login_screen::LoginScreen;
    use crate::shared::popup_list::PopupList;
    use crate::home::new_message_context_menu::*;
    use crate::shared::callout_tooltip::CalloutTooltip;


    App = {{App}} {
        ui: <Root>{
            main_window = <Window> {
                window: {inner_size: vec2(1280, 800), title: "Robrix"},
                pass: {clear_color: #FFFFFF00}
                caption_bar = {
                    caption_label = {
                        label = {
                            margin: {left: 65},
                            align: {x: 0.5},
                            text: "Robrix",
                            draw_text: {color: (COLOR_TEXT)}
                        }
                    }
                    windows_buttons = {
                        // Note: these are the background colors of the buttons used in Windows:
                        // * idle: Clear, for all three buttons.
                        // * hover: #E9E9E9 for minimize and maximize, #E81123 for close.
                        // * down: either darker (on light mode) or lighter (on dark mode).
                        //
                        // However, the DesktopButton widget doesn't support drawing a background color yet,
                        // so these colors are the colors of the icon itself, not the background highlight.
                        // When it supports that, we will keep the icon color always black,
                        // and change the background color instead based on the above colors.
                        min   = { draw_bg: {color: #0, color_hover: #9, color_down: #3} }
                        max   = { draw_bg: {color: #0, color_hover: #9, color_down: #3} }
                        close = { draw_bg: {color: #0, color_hover: #E81123, color_down: #FF0015} }
                    }
                    draw_bg: {color: #F3F3F3},
                }
            

                body = {
                    padding: 0,

                    <View> {
                        width: Fill, height: Fill,
                        flow: Overlay,

                        home_screen_view = <View> {
                            visible: false
                            home_screen = <HomeScreen> {}
                        }
                        join_leave_modal = <Modal> {
                            content: {
                                join_leave_modal_inner = <JoinLeaveRoomModal> {}
                            }
                        }
                        login_screen_view = <View> {
                            visible: true
                            login_screen = <LoginScreen> {}
                        }
                        <PopupList> {}
                        
                        // Context menus should be shown in front of other UI elements,
                        // but behind the verification modal.
                        new_message_context_menu = <NewMessageContextMenu> { }

                        // We want the verification modal to always show up on top of
                        // all other elements when an incoming verification request is received.
                        verification_modal = <Modal> {
                            content: {
                                verification_modal_inner = <VerificationModal> {}
                            }
                        }

                        // Tooltips must be shown in front of all other UI elements,
                        // since they can be shown as a hover atop any other widget.
                        app_tooltip = <CalloutTooltip> {}
                    }
                } // end of body
            }
        }
    }
}

app_main!(App);

#[derive(Live)]
pub struct App {
    #[live]
    ui: WidgetRef,

    #[rust]
    app_state: AppState,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        // Order matters here, as some widget definitions depend on others.
        // `makepad_widgets` must be registered first,
        // then `shared`` widgets (in which styles are defined),
        // then other modules widgets.
        makepad_widgets::live_design(cx);
        crate::shared::live_design(cx);
        #[cfg(feature = "tsp")]
        crate::tsp::live_design(cx);
        crate::settings::live_design(cx);
        crate::room::live_design(cx);
        crate::join_leave_room_modal::live_design(cx);
        crate::verification_modal::live_design(cx);
        crate::home::live_design(cx);
        crate::profile::live_design(cx);
        crate::login::live_design(cx);
    }
}

impl LiveHook for App {
    fn after_update_from_doc(&mut self, cx: &mut Cx) {
        self.update_login_visibility(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // Initialize the project directory here from the main UI thread
        // such that background threads/tasks will be able to can access it.
        let _app_data_dir = crate::app_data_dir();
        log!("App::Startup: app_data_dir: {:?}", _app_data_dir);

        if let Err(e) = persistence::load_window_state(self.ui.window(id!(main_window)), cx) {
            error!("Failed to load window state: {}", e);
        }

        self.update_login_visibility(cx);

        log!("App::Startup: starting matrix sdk loop");
        let tokio_rt = crate::sliding_sync::start_matrix_tokio().unwrap();

        #[cfg(feature = "tsp")] {
            log!("App::Startup: initializing TSP (Trust Spanning Protocol) module.");
            crate::tsp::tsp_init(tokio_rt).unwrap();
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                log!("Received LoginAction::LoginSuccess, hiding login view.");
                self.app_state.logged_in = true;
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // Handle an action requesting to open the new message context menu.
            if let MessageAction::OpenMessageContextMenu { details, abs_pos } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(id!(app_tooltip)).hide(cx);
                let new_message_context_menu = self.ui.new_message_context_menu(id!(new_message_context_menu));
                let expected_dimensions = new_message_context_menu.show(cx, details);
                // Ensure the context menu does not spill over the window's bounds.
                let rect = self.ui.window(id!(main_window)).area().rect(cx);
                let pos_x = min(abs_pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(abs_pos.y, rect.size.y - expected_dimensions.y);
                new_message_context_menu.apply_over(cx, live! {
                    main_content = { margin: { left: (pos_x), top: (pos_y) } }
                });
                self.ui.redraw(cx);
                continue;
            }

            if let Some(AppStateAction::RestoreAppStateFromPersistentState(app_state)) = action.downcast_ref() {
                // Ignore the `logged_in` state that was stored persistently.
                let logged_in_actual = self.app_state.logged_in;
                self.app_state = app_state.clone();
                self.app_state.logged_in = logged_in_actual;
                cx.action(MainDesktopUiAction::LoadDockFromAppState);
            }

            if let RoomsListAction::Selected(selected_room) = action.as_widget_action().cast() {
                // A room has been selected, update the app state and navigate to the main content view.
                let display_name = room_name_or_id(selected_room.room_name(), selected_room.room_id());
                self.app_state.selected_room = Some(selected_room);
                // Set the Stack Navigation header to show the name of the newly-selected room.
                self.ui
                    .label(id!(main_content_view.header.content.title_container.title))
                    .set_text(cx, &display_name);

                // Navigate to the main content view
                cx.widget_action(
                    self.ui.widget_uid(),
                    &Scope::default().path,
                    StackNavigationAction::NavigateTo(live_id!(main_content_view))
                );
                self.ui.redraw(cx);
                continue;
            }

            // Handle actions that instruct us to update the top-level app state.
            match action.as_widget_action().cast() {
                AppStateAction::RoomFocused(selected_room) => {
                    self.app_state.selected_room = Some(selected_room.clone());
                    continue;
                }
                AppStateAction::FocusNone => {
                    self.app_state.selected_room = None;
                    continue;
                }
                AppStateAction::UpgradedInviteToJoinedRoom(room_id) => {
                    if let Some(selected_room) = self.app_state.selected_room.as_mut() {
                        let did_upgrade = selected_room.upgrade_invite_to_joined(&room_id);
                        // Updating the AppState's selected room and issuing a redraw
                        // will cause the MainMobileUI to redraw the newly-joined room.
                        if did_upgrade {
                            self.ui.redraw(cx);
                        }
                    }
                    continue;
                }
                _ => {}
            }

            // Handle actions for showing or hiding the tooltip.
            match action.as_widget_action().cast() {
                TooltipAction::HoverIn {
                    widget_rect,
                    text,
                    text_color,
                    bg_color,
                } => {
                    // Don't show any tooltips if the message context menu is currently shown.
                    if self.ui.new_message_context_menu(id!(new_message_context_menu)).is_currently_shown(cx) {
                        self.ui.callout_tooltip(id!(app_tooltip)).hide(cx);
                    }
                    else {
                        self.ui.callout_tooltip(id!(app_tooltip)).show_with_options(
                            cx,
                            &text,
                            CalloutTooltipOptions {
                                widget_rect,
                                text_color,
                                bg_color,
                            },
                        );
                    }
                    continue;
                }
                TooltipAction::HoverOut => {
                    self.ui.callout_tooltip(id!(app_tooltip)).hide(cx);
                    continue;
                }
                _ => {}
            }

            // Handle actions needed to open/close the join/leave room modal.
            match action.downcast_ref() {
                Some(JoinLeaveRoomModalAction::Open(kind)) => {
                    self.ui.join_leave_room_modal(id!(join_leave_modal_inner)).set_kind(cx, kind.clone());
                    self.ui.modal(id!(join_leave_modal)).open(cx);
                    continue;
                }
                Some(JoinLeaveRoomModalAction::Close { was_internal, .. }) => {
                    if *was_internal {
                        self.ui.modal(id!(join_leave_modal)).close(cx);
                    }
                    continue;
                }
                _ => {}
            }

            // `VerificationAction`s come from a background thread, so they are NOT widget actions.
            // Therefore, we cannot use `as_widget_action().cast()` to match them.
            //
            // Note: other verification actions are handled by the verification modal itself.
            if let Some(VerificationAction::RequestReceived(state)) = action.downcast_ref() {
                self.ui.verification_modal(id!(verification_modal_inner))
                    .initialize_with_data(cx, state.clone());
                self.ui.modal(id!(verification_modal)).open(cx);
                continue;
            }
            if let Some(VerificationModalAction::Close) = action.downcast_ref() {
                self.ui.modal(id!(verification_modal)).close(cx);
                continue;
            }

            // // message source modal handling.
            // match action.as_widget_action().cast() {
            //     MessageAction::MessageSourceModalOpen { room_id: _, event_id: _, original_json: _ } => {
            //        // self.ui.message_source(id!(message_source_modal_inner)).initialize_with_data(room_id, event_id, original_json);
            //        // self.ui.modal(id!(message_source_modal)).open(cx);
            //     }
            //     MessageAction::MessageSourceModalClose => {
            //         self.ui.modal(id!(message_source_modal)).close(cx);
            //     }
            //     _ => {}
            // }
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // if let Event::WindowGeomChange(geom) = event {
        //     log!("App::handle_event(): Window geometry changed: {:?}", geom);
        // }

        if let Event::Shutdown = event {
            let window_ref = self.ui.window(id!(main_window));
            if let Err(e) = persistence::save_window_state(window_ref, cx) {
                error!("Failed to save window state. Error: {e}");
            }
            if let Some(user_id) = current_user_id() {
                let app_state = self.app_state.clone();
                if let Err(e) = persistence::save_app_state(app_state, user_id) {
                    error!("Failed to save app state. Error: {e}");
                }
            }
            #[cfg(feature = "tsp")] {
                // Save the TSP wallet state, if it exists, with a 3-second timeout.
                let tsp_state = std::mem::take(&mut *crate::tsp::tsp_state_ref().lock().unwrap());
                if tsp_state.has_content() {
                    let res = crate::sliding_sync::block_on_async_with_timeout(
                        Some(std::time::Duration::from_secs(3)),
                        async move {
                            match tsp_state.close_and_serialize().await {
                                Ok(saved_state) => match persistence::save_tsp_state_async(saved_state).await {
                                    Ok(_) => log!("Successfully saved TSP wallet state to persistent storage."),
                                    Err(e) => error!("Failed to save TSP wallet state. Error: {e}"),
                                }
                                Err(e) => error!("Failed to close and serialize TSP wallet state. Error: {e}"),
                            }
                        },
                    );
                    if let Err(_e) = res {
                        error!("Failed to save TSP wallet state before app shutdown. Error: Timed Out.");
                    }
                }
            }
        }
        
        // Forward events to the MatchEvent trait implementation.
        self.match_event(cx, event);
        let scope = &mut Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, scope);

        /*
         * TODO: I'd like for this to work, but it doesn't behave as expected.
         *       The context menu fails to draw properly when a draw event is passed to it.
         *       Also, once we do get this to work, we should remove the
         *       Hit::FingerScroll event handler in the new_message_context_menu widget.
         *
        // We only forward "interactive hit" events to the underlying UI view
        // if none of the various overlay views are visible.
        // Currently, the only overlay view that captures interactive events is
        // the new message context menu.
        // We always forward "non-interactive hit" events to the inner UI view.
        // We check which overlay views are visible in the order of those views' z-ordering,
        // such that the top-most views get a chance to handle the event first.

        let new_message_context_menu = self.ui.new_message_context_menu(id!(new_message_context_menu));
        let is_interactive_hit = utils::is_interactive_hit_event(event);
        let is_pane_shown: bool;
        if new_message_context_menu.is_currently_shown(cx) {
            is_pane_shown = true;
            new_message_context_menu.handle_event(cx, event, scope);
        }
        else {
            is_pane_shown = false;
        }

        if !is_pane_shown || !is_interactive_hit {
            // Forward the event to the inner UI view.
            self.ui.handle_event(cx, event, scope);
        }
         *
         */
    }
}

impl App {
    fn update_login_visibility(&self, cx: &mut Cx) {
        let show_login = !self.app_state.logged_in;
        if !show_login {
            self.ui
                .modal(id!(login_screen_view.login_screen.login_status_modal))
                .close(cx);
        }
        self.ui.view(id!(login_screen_view)).set_visible(cx, show_login);
        self.ui.view(id!(home_screen_view)).set_visible(cx, !show_login);
    }
}

/// App-wide state that is stored persistently across multiple app runs
/// and shared/updated across various parts of the app.
#[derive(Clone, Default, Debug, DeRon, SerRon)]
pub struct AppState {
    /// The currently-selected room, which is highlighted (selected) in the RoomsList
    /// and considered "active" in the main rooms screen.
    pub selected_room: Option<SelectedRoom>,
    /// A saved "snapshot" of the dock's UI state.
    pub saved_dock_state: SavedDockState,
    /// Whether a user is currently logged in to Robrix or not.
    pub logged_in: bool,
}

/// A snapshot of the main dock: all state needed to restore the dock tabs/layout.
#[derive(Clone, Default, Debug, DeRon, SerRon)]
pub struct SavedDockState {
    /// All items contained in the dock, keyed by their LiveId.
    pub dock_items: HashMap<LiveId, DockItem>,
    /// The rooms that are currently open, keyed by the LiveId of their tab.
    pub open_rooms: HashMap<LiveId, SelectedRoom>,
    /// The order in which the rooms were opened, in chronological order
    /// from first opened (at the beginning) to last opened (at the end).
    pub room_order: Vec<SelectedRoom>,
}

/// Represents a room currently or previously selected by the user.
///
/// One `SelectedRoom` is considered equal to another if their `room_id`s are equal.
#[derive(Clone, Debug, SerRon, DeRon)]
pub enum SelectedRoom {
    JoinedRoom {
        room_id: OwnedRoomIdRon,
        room_name: Option<String>,
    },
    InvitedRoom {
        room_id: OwnedRoomIdRon,
        room_name: Option<String>,
    },
}

impl SelectedRoom {
    pub fn room_id(&self) -> &OwnedRoomId {
        match self {
            SelectedRoom::JoinedRoom { room_id, .. } => room_id,
            SelectedRoom::InvitedRoom { room_id, .. } => room_id,
        }
    }

    pub fn room_name(&self) -> Option<&String> {
        match self {
            SelectedRoom::JoinedRoom { room_name, .. } => room_name.as_ref(),
            SelectedRoom::InvitedRoom { room_name, .. } => room_name.as_ref(),
        }
    }

    /// Upgrades this room from an invite to a joined room
    /// if its `room_id` matches the given `room_id`.
    ///
    /// Returns `true` if the room was an `InvitedRoom` with the same `room_id`
    /// that was successfully upgraded to a `JoinedRoom`;
    /// otherwise, returns `false`.
    pub fn upgrade_invite_to_joined(&mut self, room_id: &RoomId) -> bool {
        match self {
            SelectedRoom::InvitedRoom { room_id: id, room_name } if id.0 == room_id => {
                let name = room_name.take();
                *self = SelectedRoom::JoinedRoom {
                    room_id: id.clone(),
                    room_name: name,
                };
                true
            }
            _ => false,
        }
    }
}
impl PartialEq for SelectedRoom {
    fn eq(&self, other: &Self) -> bool {
        self.room_id() == other.room_id()
    }
}
impl Eq for SelectedRoom {}

/// Actions sent to the top-level App in order to update / restore its [`AppState`].
#[derive(Clone, Debug, DefaultNone)]
pub enum AppStateAction {
    /// The given room was focused (selected).
    RoomFocused(SelectedRoom),
    /// Resets the focus to none, meaning that no room is selected.
    FocusNone,
    /// The given room has successfully been upgraded from being displayed
    /// as an InviteScreen to a RoomScreen.
    UpgradedInviteToJoinedRoom(OwnedRoomId),
    /// The app state was restored from persistent storage.
    RestoreAppStateFromPersistentState(AppState),
    /// The given room was successfully loaded from the homeserver
    /// and is now known to our client.
    ///
    /// The RoomScreen for this room can now fully display the room's timeline.
    RoomLoadedSuccessfully(OwnedRoomId),
    None,
}
