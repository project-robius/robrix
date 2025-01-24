use makepad_widgets::*;
use matrix_sdk::ruma::OwnedRoomId;

use crate::{
    home::{main_desktop_ui::RoomsPanelAction, room_screen::MessageAction, rooms_list::RoomsListAction}, login::login_screen::LoginAction, shared::popup_list::PopupNotificationAction, verification::VerificationAction, verification_modal::{VerificationModalAction, VerificationModalWidgetRefExt}
};

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
    
    ICON_CHAT = dep("crate://self/resources/icons/chat.svg")
    ICON_CONTACTS = dep("crate://self/resources/icons/contacts.svg")
    ICON_DISCOVER = dep("crate://self/resources/icons/discover.svg")
    ICON_ME = dep("crate://self/resources/icons/me.svg")


    APP_TAB_COLOR = #344054
    APP_TAB_COLOR_HOVER = #636e82
    APP_TAB_COLOR_SELECTED = #091

    AppTab = <RadioButton> {
        width: Fit,
        height: Fill,
        flow: Down,
        align: {x: 0.5, y: 0.5},

        icon_walk: {width: 20, height: 20, margin: 0.0}
        label_walk: {margin: 0.0}

        draw_radio: {
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
                        (APP_TAB_COLOR_SELECTED),
                        self.selected
                    )
                );
                return sdf.result;
            }
        }

        draw_text: {
            color_unselected: (APP_TAB_COLOR)
            color_unselected_hover: (APP_TAB_COLOR_HOVER)
            color_selected: (APP_TAB_COLOR_SELECTED)

            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }

        draw_icon: {
            instance color_unselected: (APP_TAB_COLOR)
            instance color_unselected_hover: (APP_TAB_COLOR_HOVER)
            instance color_selected: (APP_TAB_COLOR_SELECTED)
            fn get_color(self) -> vec4 {
                return mix(
                    mix(
                        self.color_unselected,
                        self.color_unselected_hover,
                        self.hover
                    ),
                    self.color_selected,
                    self.selected
                )
            }
        }
    }

    App = {{App}} {
        ui: <Window> {
            window: {inner_size: vec2(1280, 800)},
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
                    popup = <PopupNotification> {
                        margin: {top: 45, right: 13},
                        content: {
                            <PopupList> {}
                        }
                    }
                    verification_modal = <Modal> {
                        content: {
                            verification_modal_inner = <VerificationModal> {}
                        }
                    }
                    
                    // message_source_modal = <Modal> {
                    //     content: {
                    //         message_source_modal_inner = <MessageSourceModal> {}
                    //     }
                    // }
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
            match action.downcast_ref() {
                Some(PopupNotificationAction::Open) => {
                    self.ui.popup_notification(id!(popup)).open(cx);
                }
                Some(PopupNotificationAction::Close) => {
                    self.ui.popup_notification(id!(popup)).close(cx);
                }
                _ => {}
            }
            match action.as_widget_action().cast() {
                // A room has been selected, update the app state and navigate to the main content view.
                RoomsListAction::Selected {
                    room_id,
                    room_index: _,
                    room_name,
                } => {

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

            // message source modal handling.
            match action.as_widget_action().cast() {
                MessageAction::MessageSourceModalOpen { room_id: _, event_id: _, original_json: _ } => {
                   // self.ui.message_source(id!(message_source_modal_inner)).initialize_with_data(room_id, event_id, original_json);
                   // self.ui.modal(id!(message_source_modal)).open(cx);
                }
                MessageAction::MessageSourceModalClose => {
                    self.ui.modal(id!(message_source_modal)).close(cx);
                }
                _ => {}
            }
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
        }
        // Forward events to the MatchEvent trait impl, and then to the App's UI element.
        self.match_event(cx, event);
        let scope = &mut Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, scope);
    }
}

impl App {
    fn update_login_visibility(&self, cx: &mut Cx) {
        let show_login = !self.app_state.logged_in;
        self.ui.view(id!(login_screen_view)).set_visible(cx, show_login);
        self.ui.view(id!(home_screen_view)).set_visible(cx, !show_login);
        if !show_login {
            self.ui
                .modal(id!(login_screen_view.login_screen.login_status_modal))
                .close(cx);
        }
    }
}

#[derive(Default, Debug)]
pub struct AppState {
    pub rooms_panel: RoomsPanelState,
    pub logged_in: bool,
    /// The current window geometry.
    pub window_geom: Option<event::WindowGeom>,
}

#[derive(Default, Debug)]
pub struct RoomsPanelState {
    pub selected_room: Option<SelectedRoom>,
}

/// Represents a room currently or previously selected by the user.
///
/// One `SelectedRoom` is considered equal to another if their `room_id`s are equal.
#[derive(Clone, Debug)]
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
