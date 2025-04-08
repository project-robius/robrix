use std::collections::HashMap;

use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;
use crate::{
    home::{main_desktop_ui::{MainDesktopUIDockActions, RoomsPanelAction}, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_screen::MessageAction, rooms_list::RoomsListAction}, login::login_screen::LoginAction, persistent_state::save_room_panel, shared::{callout_tooltip::{CalloutTooltipOptions, CalloutTooltipWidgetRefExt, TooltipAction}, popup_list::PopupNotificationAction}, sliding_sync::current_user_id, utils::DVec2Wrapper, verification::VerificationAction, verification_modal::{VerificationModalAction, VerificationModalWidgetRefExt}
};
use serde::{Deserialize, Serialize};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::home_screen::HomeScreen;
    use crate::profile::my_profile_screen::MyProfileScreen;
    use crate::verification_modal::VerificationModal;
    use crate::login::login_screen::LoginScreen;
    use crate::shared::popup_list::PopupList;
    use crate::home::new_message_context_menu::*;
    use crate::shared::callout_tooltip::CalloutTooltip;


    APP_TAB_COLOR = #344054
    APP_TAB_COLOR_HOVER = #636e82
    APP_TAB_COLOR_ACTIVE = #091

    AppTab = <RadioButton> {
        width: Fit,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},

        icon_walk: {width: 20, height: 20, margin: 0.0}
        label_walk: {margin: 0.0}

        draw_bg: {
            radio_type: Tab,

            // Draws a horizontal line under the tab when selected or hovered.
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(
                    20.0,
                    self.rect_size.y - 2.5,
                    self.rect_size.x - 40,
                    self.rect_size.y - 4,
                    0.5
                );
                sdf.fill(
                    mix(
                        mix(
                            #0000,
                            (APP_TAB_COLOR_HOVER),
                            self.hover
                        ),
                        (APP_TAB_COLOR_ACTIVE),
                        self.active
                    )
                );
                return sdf.result;
            }
        }

        draw_text: {
            color: (APP_TAB_COLOR)
            color_hover: (APP_TAB_COLOR_HOVER)
            color_active: (APP_TAB_COLOR_ACTIVE)

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.active
                )
            }
        }

        draw_icon: {
            instance color: (APP_TAB_COLOR)
            instance color_hover: (APP_TAB_COLOR_HOVER)
            instance color_active: (APP_TAB_COLOR_ACTIVE)
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color,
                        self.color_hover,
                        self.hover
                    ),
                    self.color_active,
                    self.selected
                )
            }
        }
    }

    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(1280, 800), title: "Robrix"},
            caption_bar = {caption_label = {label = {text: "Robrix"}}}
            pass: {clear_color: #2A}

            body = {
                // A wrapper view for showing top-level app modals/dialogs/popups
                <View> {
                    width: Fill, height: Fill,
                    flow: Overlay,

                    home_screen_view = <View> {
                        visible: false
                        home_screen = <HomeScreen> {}
                    }
                    login_screen_view = <View> {
                        visible: true
                        login_screen = <LoginScreen> {}
                    }
                    app_tooltip = <CalloutTooltip> {}
                    popup = <PopupNotification> {
                        margin: {top: 45, right: 13},
                        content: {
                            <PopupList> {}
                        }
                    }

                    // Context menus should be shown above other UI elements,
                    // but beneath the verification modal.
                    new_message_context_menu = <NewMessageContextMenu> { }

                    // message_source_modal = <Modal> {
                    //     content: {
                    //         message_source_modal_inner = <MessageSourceModal> {}
                    //     }
                    // }

                    // We want the verification modal to always show up on top of
                    // all other elements when an incoming verification request is received.
                    verification_modal = <Modal> {
                        content: {
                            verification_modal_inner = <VerificationModal> {}
                        }
                    }
                }
            } // end of body
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
        crate::room::live_design(cx);
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
        log!("App::handle_startup(): app_data_dir: {:?}", _app_data_dir);

        self.update_login_visibility(cx);

        log!("App::handle_startup(): starting matrix sdk loop");
        crate::sliding_sync::start_matrix_tokio().unwrap();
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                log!("Received LoginAction::LoginSuccess, hiding login view.");
                self.app_state.logged_in = true;
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
            }

            // Handle an action requesting to open the new message context menu.
            if let MessageAction::OpenMessageContextMenu { details, abs_pos } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(id!(app_tooltip)).hide(cx);
                let new_message_context_menu = self.ui.new_message_context_menu(id!(new_message_context_menu));
                let expected_dimensions = new_message_context_menu.show(cx, details);
                // Ensure the context menu does not spill over the window's bounds.
                let rect = self.ui.area().rect(cx);
                let pos_x = min(abs_pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(abs_pos.y, rect.size.y - expected_dimensions.y);
                new_message_context_menu.apply_over(cx, live! {
                    main_content = { margin: { left: (pos_x), top: (pos_y) } }
                });
                self.ui.redraw(cx);
            }

            match action.downcast_ref() {
                Some(PopupNotificationAction::Open) => {
                    self.ui.popup_notification(id!(popup)).open(cx);
                }
                Some(PopupNotificationAction::Close) => {
                    self.ui.popup_notification(id!(popup)).close(cx);
                }
                _ => {}
            }
            if let Some(AppRestoreDockAction::Restore(rooms_panel_state)) = action.downcast_ref() {
                self.app_state.rooms_panel = rooms_panel_state.clone();
                cx.action(MainDesktopUIDockActions::DockRestore);
                cx.push_unique_platform_op(CxOsOp::ResizeWindow(CxWindowPool::id_zero(), rooms_panel_state.window_size.0));
                cx.push_unique_platform_op(CxOsOp::RepositionWindow(CxWindowPool::id_zero(), rooms_panel_state.window_position.0));
                if rooms_panel_state.window_is_fullscreen {
                    cx.push_unique_platform_op(CxOsOp::MaximizeWindow(CxWindowPool::id_zero()));
                }
            }

            match action.as_widget_action().cast() {
                // A room has been selected, update the app state and navigate to the main content view.
                RoomsListAction::Selected { room_id, room_index: _, room_name } => {
                    self.app_state.rooms_panel.selected_room = Some(SelectedRoom {
                        room_id: room_id.clone(),
                        room_name: room_name.clone(),
                    });

                    let widget_uid = self.ui.widget_uid();
                    // Navigate to the main content view
                    cx.widget_action(
                        widget_uid,
                        &Scope::default().path,
                        StackNavigationAction::NavigateTo(live_id!(main_content_view))
                    );
                    // Update the Stack Navigation header with the room name
                    self.ui.label(id!(main_content_view.header.content.title_container.title))
                        .set_text(cx, &room_name.unwrap_or_else(|| format!("Room ID {}", &room_id)));
                    self.ui.redraw(cx);
                }
                RoomsListAction::None => { }
            }

            match action.as_widget_action().cast() {
                RoomsPanelAction::RoomFocused(selected_room) => {
                    self.app_state.rooms_panel.selected_room = Some(selected_room.clone());
                }
                RoomsPanelAction::FocusNone => {
                    self.app_state.rooms_panel.selected_room = None;
                }
                RoomsPanelAction::None => { }
            }

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
                }
                TooltipAction::HoverOut => {
                    self.ui.callout_tooltip(id!(app_tooltip)).hide(cx);
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
            }
            if let VerificationModalAction::Close = action.as_widget_action().cast() {
                self.ui.modal(id!(verification_modal)).close(cx);
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

    /*
    fn handle_shutdown(&mut self, _cx: &mut Cx) {
        log!("App::handle_shutdown()");
    }
    fn handle_foreground(&mut self, _cx: &mut Cx) {
        log!("App::handle_foreground()");
    }
    fn handle_background(&mut self, _cx: &mut Cx) {
        log!("App::handle_background()");
    }
    fn handle_pause(&mut self, _cx: &mut Cx) {
        log!("App::handle_pause()");
    }
    fn handle_resume(&mut self, _cx: &mut Cx) {
        log!("App::handle_resume()");
    }
    fn handle_app_got_focus(&mut self, _cx: &mut Cx) {
        log!("App::handle_app_got_focus()");
    }
    fn handle_app_lost_focus(&mut self, _cx: &mut Cx) {
        log!("App::handle_app_lost_focus()");
    }
    */
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::WindowGeomChange(window_geom_change_event) = event {
            self.app_state.window_geom = Some(window_geom_change_event.new_geom.clone());
            self.app_state.rooms_panel.window_position = DVec2Wrapper(window_geom_change_event.new_geom.position);
            self.app_state.rooms_panel.window_size = DVec2Wrapper(window_geom_change_event.new_geom.inner_size);
            self.app_state.rooms_panel.window_is_fullscreen = window_geom_change_event.new_geom.is_fullscreen;
        }
        if let (Event::WindowClosed(_), Some(user_id)) = (event, current_user_id()) {
            if let Err(e) = save_room_panel(&self.app_state.rooms_panel, &user_id) {
                log!("Bug! Failed to save save_room_panel: {}", e);
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

#[derive(Default, Debug)]
pub struct AppState {
    pub rooms_panel: RoomsPanelState,
    pub logged_in: bool,
    /// The current window geometry.
    pub window_geom: Option<event::WindowGeom>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
/// The state of the rooms panel
pub struct RoomsPanelState {
    /// The most-recently selected room, which is highlighted in the rooms list panel.
    pub selected_room: Option<SelectedRoom>,
    /// The order in which the rooms were opened.
    pub room_order: Vec<SelectedRoom>,
    /// The saved dock state created by makepad's dock widget.
    #[serde(skip_serializing, skip_deserializing)]
    pub dock_state: HashMap<LiveId, DockItem>,
    /// The rooms that are currently open, keyed by the LiveId of their tab.
    pub open_rooms: HashMap<u64, SelectedRoom>,
    /// A tuple containing the window's width and height.
    pub window_size: DVec2Wrapper,
    /// A tuple containing the window's x and y position.
    pub window_position: DVec2Wrapper,
    /// Maximise fullscreen if true
    pub window_is_fullscreen: bool
}
/// Represents a room currently or previously selected by the user.
///
/// One `SelectedRoom` is considered equal to another if their `room_id`s are equal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedRoom {
    pub room_id: OwnedRoomId,
    pub room_name: Option<String>,
}
impl PartialEq for SelectedRoom {
    fn eq(&self, other: &Self) -> bool {
        self.room_id == other.room_id
    }
}
impl Eq for SelectedRoom {}

/// The possible actions that can result in updates to the dock of rooms tabs.
#[derive(DefaultNone, Clone, Debug)]
pub enum AppRestoreDockAction {
    /// Load the previously-saved dock state and restore it to the dock.
    /// This will be handled by the top-level App and by each RoomScreen in the dock.
    Restore(RoomsPanelState),
    /// The given room has not yet been loaded from the homeserver
    /// and is waiting to be known by our client so that it can be displayed.
    /// Each RoomScreen widget will handle and update its own status
    /// to be pending, and should thus display a loading spinner / notice.
    Pending(OwnedRoomId),
    /// The given room was successfully loaded from the homeserver
    /// and is known to our client.
    /// The RoomScreen for this room can now fully display the room's timeline.
    Success(OwnedRoomId),
    /// Informs all room screens that all known rooms have been loaded.
    /// Automatically fails all pending rooms.
    LoadingCompleted,
    None
}
