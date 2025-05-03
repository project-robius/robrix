use std::{collections::HashMap, fmt, str::{Chars, FromStr}};

use makepad_widgets::{makepad_micro_serde::{DeRon, SerRon}, *};
use matrix_sdk::ruma::{OwnedRoomId, RoomId};

use crate::{
    home::{main_desktop_ui::MainDesktopUiAction, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_screen::MessageAction, rooms_list::RoomsListAction}, login::login_screen::LoginAction, persistent_state::{save_room_panel, save_window_state}, shared::{callout_tooltip::{CalloutTooltipOptions, CalloutTooltipWidgetRefExt, TooltipAction}, popup_list::PopupNotificationAction}, sliding_sync::current_user_id, utils::room_name_or_id, verification::VerificationAction, verification_modal::{VerificationModalAction, VerificationModalWidgetRefExt}
};
use serde::{de::{self, MapAccess, SeqAccess, Visitor}, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use serde;
use makepad_micro_serde::*;
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::home::home_screen::HomeScreen;
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
        ui: <Root>{
            main_window = <Window> {
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
            match action.downcast_ref() {
                Some(RoomsPanelRestoreAction::Restore(rooms_panel_state)) => {
                    self.app_state.saved_dock_state = rooms_panel_state.clone();
                    cx.action(MainDesktopUiAction::DockLoad);
                }
                Some(RoomsPanelRestoreAction::RestoreWindow(window_state)) => {
                    let window = self.ui.window(id!(main_window));
                    window.resize(cx, window_state.inner_size);
                    window.reposition(cx, window_state.position);
                    if window_state.is_fullscreen {                        
                        if let Some(mut window) = window.borrow_mut() {
                            //window.set_fullscreen(cx);
                        }
                    }
                }
                _ => {}
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
            }

            match action.as_widget_action().cast() {
                AppStateAction::RoomFocused(selected_room) => {
                    self.app_state.selected_room = Some(selected_room.clone());
                }
                AppStateAction::FocusNone => {
                    self.app_state.selected_room = None;
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
                }
                _ => {}
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
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // Handling Event::Shutdown is not required as Window Closed event is guaranteed when closing.
        if let Event::WindowClosed(_) = event {
            let window = self.ui.window(id!(main_window));
            let inner_size = window.get_inner_size(cx);
            // let position = window.get_position(cx);
            // let window_geom = WindowGeomState{
            //     inner_size,
            //     position,
            //     is_fullscreen: window.is_fullscreen(cx),
            // };
            // cx.spawn_thread(move || {
            //     if let Err(e) = save_window_state(window_geom) {
            //         error!("Bug! Failed to save window_state: {}", e);
            //     }
            // });
            if let Some(user_id) = current_user_id(){
                let rooms_panel = self.app_state.saved_dock_state.clone();
                let user_id = user_id.clone();
                cx.spawn_thread(move || {
                    if let Err(e) = save_room_panel(rooms_panel, user_id) {
                        error!("Bug! Failed to save room panel: {}", e);
                    }
                });
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

/// State that is shared across different parts of the Robrix app.
#[derive(Default, Debug)]
pub struct AppState {
    /// The currently-selected room, which is highlighted (selected) in the RoomsList
    /// and considered "active" in the main rooms screen.
    pub selected_room: Option<SelectedRoom>,
    /// The saved state of the dock.
    /// This is cloned from the main desktop UI's dock and saved here
    /// when transitioning from the desktop view to mobile view,
    /// and then restored from here back to the main desktop UI's dock
    /// when transitioning from the mobile view back to the desktop view.
    pub saved_dock_state: SavedDockState,
    /// Whether a user is currently logged in to Robrix or not.
    pub logged_in: bool,
}

/// A saved instance of the state of the main desktop UI's dock.
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
#[derive(Clone, Debug)]
pub enum SelectedRoom {
    JoinedRoom {
        room_id: OwnedRoomId,
        room_name: Option<String>,
    },
    InvitedRoom {
        room_id: OwnedRoomId,
        room_name: Option<String>,
    },
}
impl SerRon for SelectedRoom {
    fn ser_ron(&self, d: usize, s: &mut SerRonState) {
        match self {
            SelectedRoom::JoinedRoom { room_id, room_name } => {
                s.out.push_str("JoinedRoom"); // Variant name
                s.st_pre(); // `(\n`

                // Serialize room_id field
                s.field(d + 1, "room_id"); // `    room_id: `
                //room_id.ser_ron(d + 1, s); // Serialize the OwnedRoomId value
                room_id.to_string().ser_ron(d + 1, s);
                s.conl(); // `,\n`

                // Serialize room_name field
                s.field(d + 1, "room_name"); // `    room_name: `
                room_name.ser_ron(d + 1, s); // Serialize the Option<String> value
                s.out.push('\n'); // Newline before closing parenthesis

                s.st_post(d); // `)` with indentation
            }
            SelectedRoom::InvitedRoom { room_id, room_name } => {
                s.out.push_str("InvitedRoom"); // Variant name
                s.st_pre(); // `(\n`

                // Serialize room_id field
                s.field(d + 1, "room_id"); // `    room_id: `
                //room_id.ser_ron(d + 1, s);
                room_id.to_string().ser_ron(d + 1, s);
                s.conl(); // `,\n`

                // Serialize room_name field
                s.field(d + 1, "room_name"); // `    room_name: `
                room_name.ser_ron(d + 1, s);
                s.out.push('\n'); // Newline before closing parenthesis

                s.st_post(d); // `)` with indentation
            }
        }
    }
}
impl DeRon for SelectedRoom {
    fn de_ron(s: &mut DeRonState, i: &mut Chars) -> Result<Self, DeRonErr> {
        // Expect an identifier for the enum variant name
        if s.tok != DeRonTok::Ident {
            return Err(s.err_token("Enum identifier (JoinedRoom or InvitedRoom)"));
        }

        // Match on the variant name identifier
        let variant_name = s.identbuf.clone(); // Clone before consuming the token
        s.next_tok(i)?; // Consume the variant identifier token

        match variant_name.as_str() {
            "JoinedRoom" => {
                s.paren_open(i)?; // Expect '('

                // Placeholders for fields, using Option to detect missing ones
                let mut parsed_room_id: Option<OwnedRoomId> = None;
                let mut parsed_room_name: Option<Option<String>> = None;

                // Loop through fields within the parentheses
                while s.tok != DeRonTok::ParenClose {
                    // Expect a field name identifier
                    if s.tok != DeRonTok::Ident {
                        return Err(s.err_token("field identifier in JoinedRoom"));
                    }
                    let field_name = s.identbuf.clone();
                    s.next_tok(i)?; // Consume field identifier

                    s.colon(i)?; // Expect ':'

                    // Deserialize the field value based on the name
                    match field_name.as_str() {
                        "room_id" => {
                            if parsed_room_id.is_some() { return Err(s.err_exp(&format!("duplicate field 'room_id' in {}", variant_name))); }
                            //parsed_room_id = Some(OwnedRoomId::de_ron(s, i)?);
                            parsed_room_id = OwnedRoomId::from_str(&String::de_ron(s, i)?).ok();
                        }
                        "room_name" => {
                            if parsed_room_name.is_some() { return Err(s.err_exp(&format!("duplicate field 'room_name' in {}", variant_name))); }
                            parsed_room_name = Some(Option::<String>::de_ron(s, i)?);
                        }
                        _ => return Err(s.err_exp(&format!("unknown field '{}' in {}", field_name, variant_name))),
                    }

                    // Check for comma or closing parenthesis
                    match s.tok {
                        DeRonTok::Comma => {
                            s.next_tok(i)?; // Consume comma
                            // Allow trailing comma
                            if s.tok == DeRonTok::ParenClose { break; }
                        }
                        DeRonTok::ParenClose => break, // End of fields
                        _ => return Err(s.err_token("',' or ')' in JoinedRoom")),
                    }
                }

                s.paren_close(i)?; // Consume ')'

                // Check that all required fields were found
                let room_id = parsed_room_id.ok_or_else(|| s.err_nf(&format!("'room_id' in {}", variant_name)))?;
                let room_name = parsed_room_name.ok_or_else(|| s.err_nf(&format!("'room_name' in {}", variant_name)))?;

                Ok(SelectedRoom::JoinedRoom { room_id, room_name })
            }
            "InvitedRoom" => {
                s.paren_open(i)?; // Expect '('

                let mut parsed_room_id: Option<OwnedRoomId> = None;
                let mut parsed_room_name: Option<Option<String>> = None;

                while s.tok != DeRonTok::ParenClose {
                    if s.tok != DeRonTok::Ident { return Err(s.err_token("field identifier in InvitedRoom")); }
                    let field_name = s.identbuf.clone();
                    s.next_tok(i)?;
                    s.colon(i)?;

                    match field_name.as_str() {
                         "room_id" => {
                            if parsed_room_id.is_some() { return Err(s.err_exp(&format!("duplicate field 'room_id' in {}", variant_name))); }
                            parsed_room_id = OwnedRoomId::from_str(&String::de_ron(s, i)?).ok();
                        }
                        "room_name" => {
                            if parsed_room_name.is_some() { return Err(s.err_exp(&format!("duplicate field 'room_name' in {}", variant_name))); }
                            parsed_room_name = Some(Option::<String>::de_ron(s, i)?);
                        }
                        _ => return Err(s.err_exp(&format!("unknown field '{}' in {}", field_name, variant_name))),
                    }

                    match s.tok {
                        DeRonTok::Comma => {
                            s.next_tok(i)?;
                            if s.tok == DeRonTok::ParenClose { break; }
                        }
                        DeRonTok::ParenClose => break,
                        _ => return Err(s.err_token("',' or ')' in InvitedRoom")),
                    }
                }

                s.paren_close(i)?;

                let room_id = parsed_room_id.ok_or_else(|| s.err_nf(&format!("'room_id' in {}", variant_name)))?;
                let room_name = parsed_room_name.ok_or_else(|| s.err_nf(&format!("'room_name' in {}", variant_name)))?;

                Ok(SelectedRoom::InvitedRoom { room_id, room_name })
            }
            // Unknown variant name
            _ => Err(s.err_enum(&variant_name)),
        }
    }
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
            SelectedRoom::InvitedRoom { room_id: id, room_name } if id == room_id => {
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

/// Actions sent to the top-level App in order to update its [`AppState`].
#[derive(Clone, Debug, DefaultNone)]
pub enum AppStateAction {
    /// The given room was focused (selected).
    RoomFocused(SelectedRoom),
    /// Resets the focus to none, meaning that no room is selected.
    FocusNone,
    /// The given room has successfully been upgraded from being displayed
    /// as an InviteScreen to a RoomScreen.
    UpgradedInviteToJoinedRoom(OwnedRoomId),
    None,
}

/// Action related to restoring the main dock and RoomScreen widgets from storage.
#[derive(Debug, DefaultNone)]
pub enum RoomsPanelRestoreAction {
    /// The previously-saved state of the rooms panel & dock was loaded from storage
    /// and is now ready to be restored to the dock UI widget.
    /// This will be handled by the top-level App and by each RoomScreen in the dock.
    Restore(SavedDockState),
    /// The given room was successfully loaded from the homeserver
    /// and is known to our client.
    /// The RoomScreen for this room can now fully display the room's timeline.
    Success(OwnedRoomId),
    /// The previously-saved of the window geometry state.
    /// This will be handled by the top-level App to restore window's size and position.
    RestoreWindow(WindowGeomState),
    None,
}
#[derive(Default, Debug, Clone, PartialEq)]
/// The state of the window geometry
pub struct WindowGeomState {
    /// A tuple containing the window's width and height.
    pub inner_size: DVec2,
    /// A tuple containing the window's x and y position.
    pub position: DVec2,
    /// Maximise fullscreen if true.
    pub is_fullscreen: bool
}

impl Serialize for WindowGeomState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("WindowGeomState", 3)?;
        state.serialize_field("inner_size", &(self.inner_size.x, self.inner_size.y))?;
        state.serialize_field("position", &(self.position.x, self.position.y))?;
        state.serialize_field("is_fullscreen", &self.is_fullscreen)?;
        state.end()
    }
}
impl<'de> Deserialize<'de> for WindowGeomState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field { InnerSize, Position, IsFullscreen }

        // This part could also be generated independently by:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        //    enum Field { Secs, Nanos }
        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("Dev")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "inner_size" => Ok(Field::InnerSize),
                            "position" => Ok(Field::Position),
                            "is_fullscreen" => Ok(Field::IsFullscreen),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct WindowGeomStateVisitor;

        impl<'de> Visitor<'de> for WindowGeomStateVisitor {
            type Value = WindowGeomState;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct WindowGeomState")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<WindowGeomState, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let inner_size: (f64, f64) = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let position: (f64, f64) = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let is_fullscreen = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(WindowGeomState { inner_size: DVec2{x: inner_size.0, y: inner_size.1}, position: DVec2 { x: position.0, y: position.1 }, is_fullscreen })
            }

            fn visit_map<V>(self, mut map: V) -> Result<WindowGeomState, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut inner_size = None;
                let mut position = None;
                let mut is_fullscreen = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::InnerSize => {
                            if inner_size.is_some() {
                                return Err(de::Error::duplicate_field("inner_size"));
                            }
                            inner_size = Some(map.next_value().map(|f:(f64, f64)| DVec2 { x: f.0, y: f.1 })?);
                        }
                        Field::Position => {
                            if position.is_some() {
                                return Err(de::Error::duplicate_field("position"));
                            }
                            position = Some(map.next_value().map(|f:(f64, f64)| DVec2 { x: f.0, y: f.1 })?);
                        }
                        Field::IsFullscreen => {
                            if is_fullscreen.is_some() {
                                return Err(de::Error::duplicate_field("is_fullscreen"));
                            }
                            is_fullscreen = Some(map.next_value()?);
                        }
                    }
                }
                let inner_size = inner_size.ok_or_else(|| de::Error::missing_field("inner_size"))?;
                let position = position.ok_or_else(|| de::Error::missing_field("position"))?;
                let is_fullscreen = is_fullscreen.ok_or_else(|| de::Error::missing_field("is_fullscreen"))?;
                Ok(WindowGeomState { inner_size, position, is_fullscreen })
            }
    }

        const FIELDS: &[&str] = &["inner_size", "position", "is_fullscreen"];
        deserializer.deserialize_struct("WindowGeomState", FIELDS, WindowGeomStateVisitor)
    }
}