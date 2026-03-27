//! The top-level application content.
//!
//! See `handle_startup()` for the first code that runs on app startup.

use std::{cell::RefCell, collections::HashMap};
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, RoomId, UserId}};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use crate::{
    avatar_cache::clear_avatar_cache, home::{
        event_source_modal::{EventSourceModalAction, EventSourceModalWidgetRefExt}, invite_modal::{InviteModalAction, InviteModalWidgetRefExt}, invite_screen::InviteScreenWidgetRefExt, main_desktop_ui::MainDesktopUiAction, navigation_tab_bar::{NavigationBarAction, SelectedTab}, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::RoomContextMenuWidgetRefExt, room_screen::{InviteAction, MessageAction, RoomScreenWidgetRefExt, clear_timeline_states}, rooms_list::{RoomsListAction, RoomsListRef, RoomsListUpdate, clear_all_invited_rooms, enqueue_rooms_list_update}, space_lobby::SpaceLobbyScreenWidgetRefExt
    }, join_leave_room_modal::{
        JoinLeaveModalKind, JoinLeaveRoomModalAction, JoinLeaveRoomModalWidgetRefExt
    }, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction, LogoutConfirmModalWidgetRefExt}, persistence, profile::user_profile_cache::clear_user_profile_cache, room::BasicRoomDetails, shared::{confirmation_modal::{ConfirmationModalContent, ConfirmationModalWidgetRefExt}, image_viewer::{ImageViewerAction, LoadState}, popup_list::{PopupKind, enqueue_popup_notification}}, sliding_sync::{DirectMessageRoomAction, MatrixRequest, current_user_id, get_sync_service, submit_async_request}, utils::RoomNameId, verification::VerificationAction, verification_modal::{
        VerificationModalAction,
        VerificationModalWidgetRefExt,
    }
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    load_all_resources() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.inner_size: vec2(1280, 800)
                window.title: "Robrix"
                pass.clear_color: #FFFFFF00
                caption_bar +: {
                    draw_bg.color: #F3F3F3
                    caption_label +: {
                        label +: {
                            align: Align{x: 0.5},
                            draw_text +: { color: #0 }
                            text: "Robrix"
                        }
                    }
                    windows_buttons +: {
                         // Note: these are the background colors of the buttons used in Windows:
                        // * idle: Clear, for all three buttons.
                        // * hover: #E9E9E9 for minimize and maximize, #E81123 for close.
                        // * down: either darker (on light mode) or lighter (on dark mode).
                        //
                        // However, the DesktopButton widget doesn't support drawing a background color yet,
                        // so these colors are the colors of the icon itself, not the background highlight.
                        // When it supports that, we will keep the icon color always black,
                        // and change the background color instead based on the above colors.
                        min +: { draw_bg +: {color: #0, color_hover: #9, color_down: #3} }
                        max +: { draw_bg +: {color: #0, color_hover: #9, color_down: #3} }
                        close +: { draw_bg +: {color: #0, color_hover: #E81123, color_down: #FF0015} }
                    }
                }
            

                body +: {
                    padding: 0,

                    View {
                        width: Fill, height: Fill,
                        flow: Overlay,

                        home_screen_view := View {
                            visible: false
                            home_screen := HomeScreen {}
                        }
                        join_leave_modal := Modal {
                            content +: {
                                join_leave_modal_inner := JoinLeaveRoomModal {}
                            }
                        }
                        login_screen_view := View {
                            visible: true
                            login_screen := LoginScreen {}
                        }

                        image_viewer_modal := Modal {
                            content +: {
                                width: Fill, height: Fill,
                                image_viewer_modal_inner := ImageViewer {}
                            }
                        }
                        
                        // Context menus should be shown in front of other UI elements,
                        // but behind verification modals.
                        new_message_context_menu := NewMessageContextMenu { }
                        room_context_menu := RoomContextMenu { }

                        // A modal to confirm sending out an invite to a room.
                        invite_confirmation_modal := Modal {
                            content +: {
                                invite_confirmation_modal_inner := PositiveConfirmationModal {
                                    wrapper +: { buttons_view +: { accept_button +: {
                                        draw_icon +: { svg: (ICON_INVITE) }
                                        icon_walk: Walk{width: 28, height: Fit, margin: Inset{left: -10, right: 2} }
                                    } } }
                                }
                            }
                        }

                        // A modal to invite a user to a room.
                        invite_modal := Modal {
                            content +: {
                                invite_modal_inner := InviteModal {}
                            }
                        }

                        // Show the logout confirmation modal.
                        logout_confirm_modal := Modal {
                            content +: {
                                logout_confirm_modal_inner := LogoutConfirmModal {}
                            }
                        }

                        // Show the event source modal (View Source for messages).
                        event_source_modal := Modal {
                            content +: {
                                height: Fill,
                                width: Fill,
                                align: Align{x: 0.5, y: 0.5},
                                event_source_modal_inner := EventSourceModal {}
                            }
                        }

                        // Show incoming verification requests in front of the aforementioned UI elements.
                        verification_modal := Modal {
                            content +: {
                                verification_modal_inner := VerificationModal {}
                            }
                        }
                        tsp_verification_modal := Modal {
                            content +: {
                                tsp_verification_modal_inner := TspVerificationModal {}
                            }
                        }

                        // A generic modal to confirm any positive action.
                        positive_confirmation_modal := Modal {
                            content +: {
                                positive_confirmation_modal_inner := PositiveConfirmationModal { }
                            }
                        }

                        // A modal to confirm any deletion/removal action.
                        delete_confirmation_modal := Modal {
                            content +: {
                                delete_confirmation_modal_inner := NegativeConfirmationModal { }
                            }
                        }

                        PopupList {}

                        // Tooltips must be shown in front of all other UI elements,
                        // since they can be shown as a hover atop any other widget.
                        app_tooltip := CalloutTooltip {}
                    }
                } // end of body
            }
        }
    }
}

app_main!(App);

#[derive(Script)]
pub struct App {
    #[live] ui: WidgetRef,
    /// The top-level app state, shared across various parts of the app.
    #[rust] app_state: AppState,
    /// The details of a room we're waiting on to be loaded so that we can navigate to it.
    /// This can be either a room we're waiting to join, or one we're waiting to be invited to.
    /// Also includes an optional room ID to be closed once the awaited room has been loaded.
    #[rust] waiting_to_navigate_to_room: Option<(BasicRoomDetails, Option<OwnedRoomId>)>,
    /// A stack of previously-selected rooms for mobile navigation.
    /// When a view is popped off the stack, the previous `selected_room` is restored from here.
    #[rust] mobile_room_nav_stack: Vec<SelectedRoom>,
}

impl ScriptHook for App {
    /// After a hot-reload update, refresh the login/home screen visibility.
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.update_login_visibility(cx);
        });
    }

    /// After initial creation, set the global singleton for the PopupList widget.
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            crate::shared::popup_list::set_global_popup_list(cx, &self.ui);
        });
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // only init logging/tracing once
        let _ = tracing_subscriber::fmt::try_init();

        // Override Makepad's new default-JSON logger. We just want regular formatting.
        fn regular_log(file_name: &str, line_start: u32, column_start: u32, _line_end: u32, _column_end: u32, message: String, level: LogLevel) {
            let l = match level {
                LogLevel::Panic   => "[!]",
                LogLevel::Error   => "[E]",
                LogLevel::Warning => "[W]",
                LogLevel::Log     => "[I]",
                LogLevel::Wait    => "[.]",
            };
            println!("{l} {file_name}:{}:{}: {message}", line_start + 1, column_start + 1);
        }
        *LOG_WITH_LEVEL.write().unwrap() = regular_log;

        // Initialize the project directory here from the main UI thread
        // such that background threads/tasks will be able to can access it.
        let _app_data_dir = crate::app_data_dir();
        log!("App::handle_startup(): app_data_dir: {:?}", _app_data_dir);

        if let Err(e) = persistence::load_window_state(self.ui.window(cx, ids!(main_window)), cx) {
            error!("Failed to load window state: {}", e);
        }

        // Hide the caption bar on macOS and Linux, which use native window chrome.
        // On Windows (with custom chrome), the caption bar is needed.
        if matches!(cx.os_type(), OsType::Macos | OsType::LinuxWindow(_) | OsType::LinuxDirect) {
            let mut window = self.ui.window(cx, ids!(main_window));
            script_apply_eval!(cx, window, {
                show_caption_bar: false
            });
        }

        self.update_login_visibility(cx);

        log!("App::Startup: starting matrix sdk loop");
        let _tokio_rt_handle = crate::sliding_sync::start_matrix_tokio().unwrap();

        #[cfg(feature = "tsp")] {
            log!("App::Startup: initializing TSP (Trust Spanning Protocol) module.");
            crate::tsp::tsp_init(_tokio_rt_handle).unwrap();
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let invite_confirmation_modal_inner = self.ui.confirmation_modal(cx, ids!(invite_confirmation_modal_inner));
        if let Some(_accepted) = invite_confirmation_modal_inner.closed(actions) {
            self.ui.modal(cx, ids!(invite_confirmation_modal)).close(cx);
        }

        let delete_confirmation_modal_inner = self.ui.confirmation_modal(cx, ids!(delete_confirmation_modal_inner));
        if let Some(_accepted) = delete_confirmation_modal_inner.closed(actions) {
            self.ui.modal(cx, ids!(delete_confirmation_modal)).close(cx);
        }

        let positive_confirmation_modal_inner = self.ui.confirmation_modal(cx, ids!(positive_confirmation_modal_inner));
        if let Some(_accepted) = positive_confirmation_modal_inner.closed(actions) {
            self.ui.modal(cx, ids!(positive_confirmation_modal)).close(cx);
        }

        for action in actions {
            match action.downcast_ref() {
                Some(LogoutConfirmModalAction::Open) => {
                    self.ui.logout_confirm_modal(cx, ids!(logout_confirm_modal_inner)).reset_state(cx);
                    self.ui.modal(cx, ids!(logout_confirm_modal)).open(cx);
                    continue;
                },
                Some(LogoutConfirmModalAction::Close { was_internal, .. }) => {
                    if *was_internal {
                        self.ui.modal(cx, ids!(logout_confirm_modal)).close(cx);
                    }
                    continue;
                },
                _ => {}
            }

            match action.downcast_ref() {
                Some(LogoutAction::LogoutSuccess) => {
                    self.app_state.logged_in = false;
                    self.ui.modal(cx, ids!(logout_confirm_modal)).close(cx);
                    self.update_login_visibility(cx);
                    self.ui.redraw(cx);
                    continue;
                }
                Some(LogoutAction::ClearAppState { on_clear_appstate }) =>  {
                    // Clear user profile cache, invited_rooms timeline states 
                    clear_all_app_state(cx);
                    // Reset all app state to its default.
                    self.app_state = Default::default();
                    on_clear_appstate.notify_one();
                    continue;
                }
                _ => {}
            }

            if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                log!("Received LoginAction::LoginSuccess, hiding login view.");
                self.app_state.logged_in = true;
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // If a login failure occurs mid-session (e.g., an expired/revoked token detected
            // by `handle_session_changes`), navigate back to the login screen.
            // When not yet logged in, the login_screen widget handles displaying the failure modal.
            if let Some(LoginAction::LoginFailure(_)) = action.downcast_ref() {
                if self.app_state.logged_in {
                    log!("Received LoginAction::LoginFailure while logged in; showing login screen.");
                    self.app_state.logged_in = false;
                    self.update_login_visibility(cx);
                    self.ui.redraw(cx);
                }
                continue;
            }

            // Handle an action requesting to open the new message context menu.
            if let MessageAction::OpenMessageContextMenu { details, abs_pos } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                let new_message_context_menu = self.ui.new_message_context_menu(cx, ids!(new_message_context_menu));
                let expected_dimensions = new_message_context_menu.show(cx, details);
                // Ensure the context menu does not spill over the window's bounds.
                let rect = self.ui.window(cx, ids!(main_window)).area().rect(cx);
                let pos_x = min(abs_pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(abs_pos.y, rect.size.y - expected_dimensions.y);
                let margin = Inset {
                    left: pos_x as f64,
                    top: pos_y as f64,
                    right: 0.0,
                    bottom: 0.0,
                };
                let mut main_content_view = new_message_context_menu.view(cx, ids!(main_content));
                script_apply_eval!(cx, main_content_view, {
                    margin: #(margin)
                });
                self.ui.redraw(cx);
                continue;
            }

            // Handle an action requesting to open the room context menu.
            if let RoomsListAction::OpenRoomContextMenu { details, pos } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                let room_context_menu = self.ui.room_context_menu(cx, ids!(room_context_menu));
                let expected_dimensions = room_context_menu.show(cx, details);
                // Ensure the context menu does not spill over the window's bounds.
                let rect = self.ui.window(cx, ids!(main_window)).area().rect(cx);
                let pos_x = min(pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(pos.y, rect.size.y - expected_dimensions.y);
                let margin = Inset {
                    left: pos_x as f64,
                    top: pos_y as f64,
                    right: 0.0,
                    bottom: 0.0,
                };
                let mut main_content_view = room_context_menu.view(cx, ids!(main_content));
                script_apply_eval!(cx, main_content_view, {
                    margin: #(margin)
                });
                self.ui.redraw(cx);
                continue;
            }

            // A new room has been selected; push the appropriate view onto the mobile
            // StackNavigation and update the app state.
            // In Desktop mode, MainDesktopUI also handles this action to manage dock tabs;
            // the mobile push is harmless there (the view isn't drawn).
            match action.as_widget_action().cast() {
                RoomsListAction::Selected(selected_room) => {
                    self.push_selected_room_view(cx, selected_room);
                    continue;
                }
                // An invite was accepted; upgrade the selected room from invite to joined.
                // In Desktop mode, MainDesktopUI also handles this (harmless duplicate).
                RoomsListAction::InviteAccepted { room_name_id } => {
                    cx.action(AppStateAction::UpgradedInviteToJoinedRoom(room_name_id.room_id().clone()));
                    continue;
                }
                _ => {}
            }

            // When a stack navigation pop is initiated (back button pressed),
            // pop the mobile nav stack so it stays in sync with StackNavigation.
            if let StackNavigationAction::Pop = action.as_widget_action().cast() {
                if self.app_state.selected_room.is_some() {
                    self.app_state.selected_room = self.mobile_room_nav_stack.pop();
                }
                // Don't `continue` — let StackNavigation also process this Pop.
            }

            // Handle actions that instruct us to update the top-level app state.
            match action.downcast_ref() {
                Some(AppStateAction::RoomFocused(selected_room)) => {
                    self.app_state.selected_room = Some(selected_room.clone());
                    continue;
                }
                Some(AppStateAction::FocusNone) => {
                    self.app_state.selected_room = None;
                    continue;
                }
                Some(AppStateAction::UpgradedInviteToJoinedRoom(room_id)) => {
                    if let Some(selected_room) = self.app_state.selected_room.as_mut() {
                        let did_upgrade = selected_room.upgrade_invite_to_joined(room_id);
                        // Updating the AppState's selected room and issuing a redraw
                        // will cause the MainMobileUI to redraw the newly-joined room.
                        if did_upgrade {
                            self.ui.redraw(cx);
                        }
                    }
                    continue;
                }
                Some(AppStateAction::RestoreAppStateFromPersistentState(app_state)) => {
                    // Ignore the `logged_in` state that was stored persistently.
                    let logged_in_actual = self.app_state.logged_in;
                    self.app_state = app_state.clone();
                    self.app_state.logged_in = logged_in_actual;
                    cx.action(MainDesktopUiAction::LoadDockFromAppState);
                    continue;
                }
                Some(AppStateAction::BotRoomBindingUpdated {
                    room_id,
                    bound,
                    bot_user_id,
                    warning,
                }) => {
                    self.app_state.bot_settings.set_room_bound(
                        room_id.clone(),
                        bot_user_id.clone(),
                        *bound,
                    );
                    if let Some(user_id) = current_user_id() {
                        if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                            error!("Failed to persist app state after updating BotFather room binding. Error: {e}");
                        }
                    }
                    let kind = if warning.is_some() {
                        PopupKind::Warning
                    } else {
                        PopupKind::Success
                    };
                    let message = match (*bound, bot_user_id.as_ref(), warning.as_deref()) {
                        (true, Some(bot_user_id), Some(warning)) => {
                            format!("BotFather {bot_user_id} is available for room {room_id}, but inviting it reported a warning: {warning}")
                        }
                        (true, Some(bot_user_id), None) => {
                            format!("Bound room {room_id} to BotFather {bot_user_id}.")
                        }
                        (false, Some(bot_user_id), Some(warning)) => {
                            format!("Unbound BotFather {bot_user_id} from room {room_id}, with warning: {warning}")
                        }
                        (false, Some(bot_user_id), None) => {
                            format!("Unbound BotFather {bot_user_id} from room {room_id}.")
                        }
                        (false, None, Some(warning)) => {
                            format!("Unbound room {room_id} from BotFather, with warning: {warning}")
                        }
                        (false, None, None) => {
                            format!("Unbound room {room_id} from BotFather.")
                        }
                        (true, None, Some(warning)) => {
                            format!("BotFather is available for room {room_id}, with warning: {warning}")
                        }
                        (true, None, None) => {
                            format!("Bound room {room_id} to BotFather.")
                        }
                    };
                    enqueue_popup_notification(message, kind, Some(5.0));
                    self.ui.redraw(cx);
                    continue;
                }
                Some(AppStateAction::BotRoomBindingDetected {
                    room_id,
                    bot_user_id,
                }) => {
                    if self
                        .app_state
                        .bot_settings
                        .bound_bot_user_id(room_id.as_ref())
                        .is_some_and(|existing_bot_user_id| existing_bot_user_id.as_str() == bot_user_id.as_str())
                    {
                        continue;
                    }
                    self.app_state.bot_settings.set_room_bound(
                        room_id.clone(),
                        Some(bot_user_id.clone()),
                        true,
                    );
                    if let Some(user_id) = current_user_id() {
                        if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                            error!("Failed to persist detected BotFather room binding. Error: {e}");
                        }
                    }
                    self.ui.redraw(cx);
                    continue;
                }
                Some(AppStateAction::NavigateToRoom { room_to_close, destination_room }) => {
                    self.navigate_to_room(cx, room_to_close.as_ref(), destination_room);
                    continue;
                }
                // If we successfully loaded a room that we were waiting on,
                // we can now navigate to it and optionally close a previous room.
                Some(AppStateAction::RoomLoadedSuccessfully { room_name_id, .. }) if
                    self.waiting_to_navigate_to_room.as_ref()
                        .is_some_and(|(dr, _)| dr.room_id() == room_name_id.room_id()) =>
                {
                    log!("Loaded awaited room {room_name_id:?}, navigating to it now...");
                    if let Some((dest_room, room_to_close)) = self.waiting_to_navigate_to_room.take() {
                        self.navigate_to_room(cx, room_to_close.as_ref(), &dest_room);
                    }
                    continue;
                }
                _ => {}
            }

            // Handle actions for showing or hiding the tooltip.
            match action.as_widget_action().cast() {
                TooltipAction::HoverIn { text, widget_rect, options } => {
                    // Don't show any tooltips if the message context menu is currently shown.
                    if self.ui.new_message_context_menu(cx, ids!(new_message_context_menu)).is_currently_shown(cx) {
                        self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                    }
                    else {
                        self.ui.callout_tooltip(cx, ids!(app_tooltip)).show_with_options(
                            cx,
                            &text,
                            widget_rect,
                            options,
                        );
                    }
                    continue;
                }
                TooltipAction::HoverOut => {
                    self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                    continue;
                }
                _ => {}
            }

            // Handle actions needed to open/close the join/leave room modal.
            match action.downcast_ref() {
                Some(JoinLeaveRoomModalAction::Open { kind, show_tip }) => {
                    self.ui
                        .join_leave_room_modal(cx, ids!(join_leave_modal_inner))
                        .set_kind(cx, kind.clone(), *show_tip);
                    self.ui.modal(cx, ids!(join_leave_modal)).open(cx);
                    continue;
                }
                Some(JoinLeaveRoomModalAction::Close { was_internal, .. }) => {
                    if *was_internal {
                        self.ui.modal(cx, ids!(join_leave_modal)).close(cx);
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
                self.ui.verification_modal(cx, ids!(verification_modal_inner))
                    .initialize_with_data(cx, state.clone());
                self.ui.modal(cx, ids!(verification_modal)).open(cx);
                continue;
            }
            if let Some(VerificationModalAction::Close) = action.downcast_ref() {
                self.ui.modal(cx, ids!(verification_modal)).close(cx);
                continue;
            }
            match action.downcast_ref() {
                Some(ImageViewerAction::Show(LoadState::Loading(_, _))) => {
                    self.ui.modal(cx, ids!(image_viewer_modal)).open(cx);
                    continue;
                }
                Some(ImageViewerAction::Hide) => {
                    self.ui.modal(cx, ids!(image_viewer_modal)).close(cx);
                    continue;
                }
                _ => {}
            }
            // Handle actions to open/close the TSP verification modal.
            #[cfg(feature = "tsp")] {
                use std::ops::Deref;
                use crate::tsp::{tsp_verification_modal::{TspVerificationModalAction, TspVerificationModalWidgetRefExt}, TspIdentityAction};

                if let Some(TspIdentityAction::ReceivedDidAssociationRequest { details, wallet_db }) = action.downcast_ref() {
                    self.ui.tsp_verification_modal(cx, ids!(tsp_verification_modal_inner))
                        .initialize_with_details(cx, details.clone(), wallet_db.deref().clone());
                    self.ui.modal(cx, ids!(tsp_verification_modal)).open(cx);
                    continue;
                }
                if let Some(TspVerificationModalAction::Close) = action.downcast_ref() {
                    self.ui.modal(cx, ids!(tsp_verification_modal)).close(cx);
                    continue;
                }
            }

            // Handle a request to show the invite confirmation modal.
            if let Some(InviteAction::ShowInviteConfirmationModal(content_opt)) = action.downcast_ref() {
                if let Some(content) = content_opt.borrow_mut().take() {
                    invite_confirmation_modal_inner.show(cx, content);
                    self.ui.modal(cx, ids!(invite_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle a request to show the generic positive confirmation modal.
            if let Some(PositiveConfirmationModalAction::Show(content_opt)) = action.downcast_ref() {
                if let Some(content) = content_opt.borrow_mut().take() {
                    positive_confirmation_modal_inner.show(cx, content);
                    self.ui.modal(cx, ids!(positive_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle a request to show the delete confirmation modal.
            if let Some(ConfirmDeleteAction::Show(content_opt)) = action.downcast_ref() {
                if let Some(content) = content_opt.borrow_mut().take() {
                    self.ui.confirmation_modal(cx, ids!(delete_confirmation_modal_inner)).show(cx, content);
                    self.ui.modal(cx, ids!(delete_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle InviteModalAction to open/close the invite modal.
            match action.downcast_ref() {
                Some(InviteModalAction::Open(room_name_id)) => {
                    self.ui.invite_modal(cx, ids!(invite_modal_inner)).show(cx, room_name_id.clone());
                    self.ui.modal(cx, ids!(invite_modal)).open(cx); 
                    continue;
                }
                Some(InviteModalAction::Close) => {
                    self.ui.modal(cx, ids!(invite_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            // Handle EventSourceModalAction to open/close the event source modal.
            match action.downcast_ref() {
                Some(EventSourceModalAction::Open { room_id, event_id, original_json }) => {
                    self.ui.event_source_modal(cx, ids!(event_source_modal_inner))
                        .show(cx, room_id.clone(), event_id.clone(), original_json.clone());
                    self.ui.modal(cx, ids!(event_source_modal)).open(cx);
                    continue;
                }
                Some(EventSourceModalAction::Close) => {
                    self.ui.modal(cx, ids!(event_source_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            // Handle DirectMessageRoomActions
            match action.downcast_ref() {
                Some(DirectMessageRoomAction::FoundExisting { room_name_id, .. }) => {
                    self.navigate_to_room(cx, None, &BasicRoomDetails::RoomId(room_name_id.clone()));
                }
                Some(DirectMessageRoomAction::DidNotExist { user_profile }) => {
                    let user_profile = user_profile.clone();
                    let body_text = match &user_profile.username {
                        Some(un) if !un.is_empty() => format!(
                            "You don't have an existing direct message room with {} ({}).\n\n\
                            Would you like to create one now?",
                            un,
                            user_profile.user_id,
                        ),
                        _ => format!(
                            "You don't have an existing direct message room with {}.\n\n\
                            Would you like to create one now?",
                            user_profile.user_id,
                        ),
                    };
                    positive_confirmation_modal_inner.show(
                        cx,
                        ConfirmationModalContent {
                            title_text: "Create New Direct Message".into(),
                            body_text: body_text.into(),
                            accept_button_text: Some("Create DM".into()),
                            on_accept_clicked: Some(Box::new(move |_cx| {
                                submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                                    user_profile,
                                    allow_create: true,
                                });
                                enqueue_popup_notification(
                                    "Sending request to create DM room...\n\nThe room will be shown once it has been created by the homeserver.".to_string(),
                                    PopupKind::Info,
                                    Some(10.0),
                                );
                            })),
                            ..Default::default()
                        },
                    );
                    self.ui.modal(cx, ids!(positive_confirmation_modal)).open(cx);
                }
                Some(DirectMessageRoomAction::FailedToCreate { user_profile, error }) => {
                    enqueue_popup_notification(
                        format!("Failed to create a new DM room with {}.\n\nError: {error}", user_profile.displayable_name()),
                        PopupKind::Error,
                        None,
                    );
                }
                Some(DirectMessageRoomAction::NewlyCreated { room_name_id, .. }) => {
                    self.navigate_to_room(cx, None, &BasicRoomDetails::RoomId(room_name_id.clone()));
                }
                _ => {}
            }
        }
    }
}

/// Clears all thread-local UI caches (user profiles, invited rooms, and timeline states).
/// The `cx` parameter ensures that these thread-local caches are cleared on the main UI thread, 
fn clear_all_app_state(cx: &mut Cx) {
    clear_user_profile_cache(cx);
    clear_all_invited_rooms(cx);
    clear_timeline_states(cx);
    clear_avatar_cache(cx);
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> makepad_widgets::ScriptValue {
        // Order matters: base widgets first, then app widgets, then app UI.
        makepad_widgets::theme_mod(vm);
        // script_eval!(vm, {
        //     mod.theme = mod.themes.light
        // });
        makepad_widgets::widgets_mod(vm);
        makepad_code_editor::script_mod(vm);
        crate::shared::script_mod(vm);

        #[cfg(feature = "tsp")]
        crate::tsp::script_mod(vm);
        #[cfg(not(feature = "tsp"))]
        crate::tsp_dummy::script_mod(vm);

        crate::settings::script_mod(vm);
        // RoomInputBar depends on these Home widgets; preload them before room::script_mod.
        crate::home::location_preview::script_mod(vm);
        crate::home::tombstone_footer::script_mod(vm);
        crate::home::editing_pane::script_mod(vm);
        crate::room::script_mod(vm);
        crate::join_leave_room_modal::script_mod(vm);
        crate::verification_modal::script_mod(vm);
        crate::profile::script_mod(vm);
        crate::home::script_mod(vm);
        crate::login::script_mod(vm);
        crate::logout::script_mod(vm);

        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::Shutdown = event {
            let window_ref = self.ui.window(cx, ids!(main_window));
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
                let res = crate::sliding_sync::block_on_async_with_timeout(
                    Some(std::time::Duration::from_secs(3)),
                    async move {
                        match tsp_state.close_and_serialize().await {
                            Ok(saved_state) => match persistence::save_tsp_state_async(saved_state).await {
                                Ok(_) => { }
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

        if let Event::Pause = event {
            // App is being paused (e.g., on mobile when another app comes to foreground)
            // Stop the sync service to save battery and network resources
            log!("App paused - stopping sync service");
            if let Some(sync_service) = get_sync_service() {
                if let Ok(handle) = Handle::try_current() {
                    handle.spawn(async move {
                        sync_service.stop().await;
                        log!("Sync service stopped due to app pause");
                    });
                }
            }
        }

        if let Event::Resume = event {
            // App is resuming from a paused state
            // Restart the sync service to resume real-time updates
            log!("App resumed - restarting sync service");
            if let Some(sync_service) = get_sync_service() {
                if let Ok(handle) = Handle::try_current() {
                    handle.spawn(async move {
                        sync_service.start().await;
                        log!("Sync service restarted due to app resume");
                    });
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

        let new_message_context_menu = self.ui.new_message_context_menu(cx, ids!(new_message_context_menu));
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
                .modal(cx, ids!(login_screen_view.login_screen.login_status_modal))
                .close(cx);
        }
        self.ui.view(cx, ids!(login_screen_view)).set_visible(cx, show_login);
        self.ui.view(cx, ids!(home_screen_view)).set_visible(cx, !show_login);
    }

    /// Navigates to the given `destination_room`, optionally closing the `room_to_close`.
    fn navigate_to_room(
        &mut self,
        cx: &mut Cx,
        room_to_close: Option<&OwnedRoomId>,
        destination_room: &BasicRoomDetails,
    ) {
        // A closure that closes the given `room_to_close`, if it exists in an open tab.
        let close_room_closure_opt = room_to_close.map(|to_close| {
            let tab_id = LiveId::from_str(to_close.as_str());
            let widget_uid = self.ui.widget_uid();
            move |cx: &mut Cx| {
                cx.widget_action(
                    widget_uid, 
                    DockAction::TabCloseWasPressed(tab_id),
                );
                enqueue_rooms_list_update(RoomsListUpdate::HideRoom { room_id: to_close.clone() });
            }
        });

        let destination_room_id = destination_room.room_id();
        let room_state = cx.get_global::<RoomsListRef>().get_room_state(destination_room_id);
        let new_selected_room = match room_state {
            Some(RoomState::Joined) => SelectedRoom::JoinedRoom {
                room_name_id: destination_room.room_name_id().clone(),
            },
            Some(RoomState::Invited) => SelectedRoom::InvitedRoom {
                room_name_id: destination_room.room_name_id().clone(),
            },
            // If the destination room is not yet loaded, show a join modal.
            _ => {
                log!("Destination room {:?} not loaded, showing join modal...", destination_room.room_name_id());
                self.waiting_to_navigate_to_room = Some((
                    destination_room.clone(),
                    room_to_close.cloned(),
                ));
                cx.action(JoinLeaveRoomModalAction::Open {
                    kind: JoinLeaveModalKind::JoinRoom {
                        details: destination_room.clone(),
                        is_space: false,
                    },
                    show_tip: false,
                });
                return;
            }
        };


        log!("Navigating to destination room {:?}, closing room {:?}",
            destination_room.room_name_id(),
            room_to_close,
        );

        // Before we navigate to the room, if the AddRoom tab is currently shown,
        // then we programmatically navigate to the Home tab to show the actual room.
        if matches!(self.app_state.selected_tab, SelectedTab::AddRoom) {
            cx.action(NavigationBarAction::GoToHome);
        }
        cx.widget_action(
            self.ui.widget_uid(), 
            RoomsListAction::Selected(new_selected_room),
        );
        // Select and scroll to the destination room in the rooms list.
        enqueue_rooms_list_update(RoomsListUpdate::ScrollToRoom(destination_room_id.clone()));

        // Close a previously/currently-open room if specified.
        if let Some(closure) = close_room_closure_opt {
            closure(cx);
        }
    }

    /// Room StackNavigationView instances, one per stack depth.
    /// Each depth gets its own dedicated view widget to avoid
    /// complex state save/restore when views would otherwise be reused.
    const ROOM_VIEW_IDS: [LiveId; 16] = [
        live_id!(room_view_0),  live_id!(room_view_1),
        live_id!(room_view_2),  live_id!(room_view_3),
        live_id!(room_view_4),  live_id!(room_view_5),
        live_id!(room_view_6),  live_id!(room_view_7),
        live_id!(room_view_8),  live_id!(room_view_9),
        live_id!(room_view_10), live_id!(room_view_11),
        live_id!(room_view_12), live_id!(room_view_13),
        live_id!(room_view_14), live_id!(room_view_15),
    ];

    /// The RoomScreen widget IDs inside each room view,
    /// corresponding 1:1 with [`Self::ROOM_VIEW_IDS`].
    const ROOM_SCREEN_IDS: [LiveId; 16] = [
        live_id!(room_screen_0),  live_id!(room_screen_1),
        live_id!(room_screen_2),  live_id!(room_screen_3),
        live_id!(room_screen_4),  live_id!(room_screen_5),
        live_id!(room_screen_6),  live_id!(room_screen_7),
        live_id!(room_screen_8),  live_id!(room_screen_9),
        live_id!(room_screen_10), live_id!(room_screen_11),
        live_id!(room_screen_12), live_id!(room_screen_13),
        live_id!(room_screen_14), live_id!(room_screen_15),
    ];

    /// Returns the room view and room screen LiveIds for the given stack depth.
    /// Clamps to the last available view if depth exceeds the pool size.
    fn room_ids_for_depth(depth: usize) -> (LiveId, LiveId) {
        let index = depth.min(Self::ROOM_VIEW_IDS.len() - 1);
        (Self::ROOM_VIEW_IDS[index], Self::ROOM_SCREEN_IDS[index])
    }

    /// Pushes the appropriate StackNavigationView for the given `SelectedRoom`,
    /// configuring the view's content widget and header title.
    ///
    /// Each stack depth gets its own dedicated room view widget,
    /// supporting deep navigation (room → thread → room → thread → ...).
    ///
    /// In Desktop mode, the StackNavigation isn't drawn, so the push and
    /// screen configuration are effectively no-ops — MainDesktopUI handles
    /// room display via dock tabs instead.
    fn push_selected_room_view(&mut self, cx: &mut Cx, selected_room: SelectedRoom) {
        // Use the actual StackNavigation depth to pick the next room view slot.
        let new_depth = self.ui.stack_navigation(cx, ids!(view_stack)).depth();

        // Determine which view to push and configure its content.
        // The `set_displayed_room` / `set_displayed_invite` / `set_displayed_space` calls
        // configure the screen widget inside the mobile StackNavigationView.
        // In Desktop mode, these widgets exist but aren't drawn; the configuration
        // consumes timeline endpoints, but Desktop's MainDesktopUI processes the same
        // `RoomsListAction::Selected` in its own handler to set up dock tabs.
        let view_id = match &selected_room {
            SelectedRoom::JoinedRoom { room_name_id }
            | SelectedRoom::Thread { room_name_id, .. } => {
                let (view_id, room_screen_id) = Self::room_ids_for_depth(new_depth);

                let thread_root = if let SelectedRoom::Thread { thread_root_event_id, .. } = &selected_room {
                    Some(thread_root_event_id.clone())
                } else {
                    None
                };
                self.ui
                    .room_screen(cx, &[room_screen_id])
                    .set_displayed_room(cx, room_name_id, thread_root);

                view_id
            }
            SelectedRoom::InvitedRoom { room_name_id } => {
                self.ui
                    .invite_screen(cx, ids!(invite_screen))
                    .set_displayed_invite(cx, room_name_id);
                id!(invite_view)
            }
            SelectedRoom::Space { space_name_id } => {
                self.ui
                    .space_lobby_screen(cx, ids!(space_lobby_screen))
                    .set_displayed_space(cx, space_name_id);
                id!(space_lobby_view)
            }
        };

        // Set the header title for the view being pushed.
        let title_path = &[view_id, live_id!(header), live_id!(content), live_id!(title_container), live_id!(title)];
        self.ui.label(cx, title_path).set_text(cx, &selected_room.display_name());

        // Save the current selected_room onto the navigation stack before replacing it.
        if let Some(prev) = self.app_state.selected_room.take() {
            self.mobile_room_nav_stack.push(prev);
        }
        // Update app state (used by both Desktop and Mobile paths).
        self.app_state.selected_room = Some(selected_room);

        // Push the view onto the mobile navigation stack.
        self.ui.stack_navigation(cx, ids!(view_stack)).push(cx, view_id);
        self.ui.redraw(cx);
    }

}

/// App-wide state that is stored persistently across multiple app runs
/// and shared/updated across various parts of the app.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// The currently-selected room, which is highlighted (selected) in the RoomsList
    /// and considered "active" in the main rooms screen.
    pub selected_room: Option<SelectedRoom>,
    /// The currently-selected navigation tab: defines which top-level view is shown.
    ///
    /// This field is only updated by the `HomeScreen` widget, which has the
    /// necessary context to be able to determine how it should be modified.
    ///
    /// This is not saved to or restored from persistent storage,
    /// so the `Home` screen and tab are always selected upon app startup.
    #[serde(skip)]
    pub selected_tab: SelectedTab,
    /// The saved "snapshot" of the dock's UI layout/state for the main "all rooms" home view.
    pub saved_dock_state_home: SavedDockState,
    /// The saved "snapshot" of the dock's UI layout/state for each space,
    /// keyed by the space ID.
    pub saved_dock_state_per_space: HashMap<OwnedRoomId, SavedDockState>,
    /// Whether a user is currently logged in to Robrix or not.
    pub logged_in: bool,
    /// Local configuration and UI state for bot-assisted room binding.
    pub bot_settings: BotSettingsState,
}

/// Local bot integration settings persisted per Matrix account.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BotSettingsState {
    /// Whether bot-assisted room binding is enabled in the UI.
    pub enabled: bool,
    /// The configured botfather user, either as a full MXID or localpart.
    pub botfather_user_id: String,
    /// Rooms that Robrix currently considers bound to BotFather,
    /// paired with the exact BotFather MXID used for that room.
    pub room_bindings: Vec<RoomBotBindingState>,
}

/// A persisted room-level BotFather binding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomBotBindingState {
    pub room_id: OwnedRoomId,
    pub bot_user_id: OwnedUserId,
}

impl Default for BotSettingsState {
    fn default() -> Self {
        Self {
            enabled: false,
            botfather_user_id: Self::DEFAULT_BOTFATHER_LOCALPART.to_string(),
            room_bindings: Vec::new(),
        }
    }
}

impl BotSettingsState {
    pub const DEFAULT_BOTFATHER_LOCALPART: &'static str = "bot";

    fn room_binding_index(&self, room_id: &RoomId) -> Result<usize, usize> {
        self.room_bindings
            .binary_search_by(|binding| binding.room_id.as_str().cmp(room_id.as_str()))
    }

    /// Returns `true` if the given room is currently marked as bound locally.
    pub fn is_room_bound(&self, room_id: &RoomId) -> bool {
        self.room_binding_index(room_id).is_ok()
    }

    /// Returns the persisted BotFather MXID for the given room, if any.
    pub fn bound_bot_user_id(&self, room_id: &RoomId) -> Option<&UserId> {
        self.room_binding_index(room_id)
            .ok()
            .map(|index| self.room_bindings[index].bot_user_id.as_ref())
    }

    /// Updates the local bound/unbound state for the given room.
    pub fn set_room_bound(
        &mut self,
        room_id: OwnedRoomId,
        bot_user_id: Option<OwnedUserId>,
        bound: bool,
    ) {
        if bound {
            let Some(bot_user_id) = bot_user_id else { return };
            match self.room_binding_index(room_id.as_ref()) {
                Ok(existing_index) => {
                    self.room_bindings[existing_index].bot_user_id = bot_user_id;
                }
                Err(insert_index) => {
                    self.room_bindings.insert(insert_index, RoomBotBindingState {
                        room_id,
                        bot_user_id,
                    });
                }
            }
        } else {
            if let Ok(existing_index) = self.room_binding_index(room_id.as_ref()) {
                self.room_bindings.remove(existing_index);
            }
        }
    }

    /// Returns the configured botfather user ID, resolving a localpart against
    /// the current user's homeserver when needed.
    pub fn resolved_bot_user_id(&self, current_user_id: Option<&UserId>) -> Result<OwnedUserId, String> {
        let raw = self.botfather_user_id.trim();
        if raw.starts_with('@') || raw.contains(':') {
            let full_user_id = if raw.starts_with('@') {
                raw.to_string()
            } else {
                format!("@{raw}")
            };
            return UserId::parse(&full_user_id)
                .map(|user_id| user_id.to_owned())
                .map_err(|_| format!("Invalid bot user ID: {full_user_id}"));
        }

        let Some(current_user_id) = current_user_id else {
            return Err(
                "Current user ID is unavailable, so the bot homeserver cannot be resolved.".into(),
            );
        };

        let localpart = if raw.is_empty() {
            Self::DEFAULT_BOTFATHER_LOCALPART
        } else {
            raw
        };
        let full_user_id = format!("@{localpart}:{}", current_user_id.server_name());
        UserId::parse(&full_user_id)
            .map(|user_id| user_id.to_owned())
            .map_err(|_| format!("Invalid bot user ID: {full_user_id}"))
    }

    /// Returns the BotFather MXID that should be used for a room action.
    ///
    /// If the room already has a persisted binding, that exact MXID wins.
    /// Otherwise, the current global configuration is resolved.
    pub fn resolved_bot_user_id_for_room(
        &self,
        room_id: &RoomId,
        current_user_id: Option<&UserId>,
    ) -> Result<OwnedUserId, String> {
        if let Some(bot_user_id) = self.bound_bot_user_id(room_id) {
            return Ok(bot_user_id.to_owned());
        }

        self.resolved_bot_user_id(current_user_id)
    }
}

/// A snapshot of the main dock: all state needed to restore the dock tabs/layout.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SavedDockState {
    /// All items contained in the dock, keyed by their room or space ID.
    pub dock_items: HashMap<LiveId, DockItem>,
    /// The rooms that are currently open, keyed by their room or space ID.
    pub open_rooms: HashMap<LiveId, SelectedRoom>,
    /// The order in which the rooms were opened, in chronological order
    /// from first opened (at the beginning) to last opened (at the end).
    pub room_order: Vec<SelectedRoom>,
    /// The selected room tab in this dock when the dock state was saved.
    pub selected_room: Option<SelectedRoom>,
}


/// Represents a room currently or previously selected by the user.
///
/// ## PartialEq/Eq equality comparison behavior
/// Room/Space names are ignored for the purpose of equality comparison.
/// Two `SelectedRoom`s are considered equal if their `room_id`s are equal,
/// unless they are `Thread`s,` in which case their `thread_root_event_id`s
/// are also compared for equality.
/// A `Thread` is never considered equal to a non-`Thread`, even if their `room_id`s are equal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SelectedRoom {
    JoinedRoom {
        room_name_id: RoomNameId,
    },
    Thread {
        room_name_id: RoomNameId,
        /// The event ID of the root message of this thread,
        /// which is used to distinguish this thread from the main room timeline.
        thread_root_event_id: OwnedEventId,
    },
    InvitedRoom {
        room_name_id: RoomNameId,
    },
    Space {
        space_name_id: RoomNameId,
    },
}

impl SelectedRoom {
    pub fn room_id(&self) -> &OwnedRoomId {
        match self {
            SelectedRoom::JoinedRoom { room_name_id } => room_name_id.room_id(),
            SelectedRoom::InvitedRoom { room_name_id } => room_name_id.room_id(),
            SelectedRoom::Space { space_name_id } => space_name_id.room_id(),
            SelectedRoom::Thread { room_name_id, .. } => room_name_id.room_id(),
        }
    }

    pub fn room_name(&self) -> &RoomNameId {
        match self {
            SelectedRoom::JoinedRoom { room_name_id } => room_name_id,
            SelectedRoom::InvitedRoom { room_name_id } => room_name_id,
            SelectedRoom::Space { space_name_id } => space_name_id,
            SelectedRoom::Thread { room_name_id, .. } => room_name_id,
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
            SelectedRoom::InvitedRoom { room_name_id } if room_name_id.room_id() == room_id => {
                let name = room_name_id.clone();
                *self = SelectedRoom::JoinedRoom {
                    room_name_id: name,
                };
                true
            }
            _ => false,
        }
    }

    /// Returns the `LiveId` of the room tab corresponding to this `SelectedRoom`.
    pub fn tab_id(&self) -> LiveId {
        match self {
            SelectedRoom::Thread { room_name_id, thread_root_event_id } => {
                LiveId::from_str(
                    &format!("{}##{}", room_name_id.room_id(), thread_root_event_id)
                )
            }
            other => LiveId::from_str(other.room_id().as_str()),
        }
    }

    /// Returns the display name to be shown for this room in the UI.
    pub fn display_name(&self) -> String {
        match self {
            SelectedRoom::JoinedRoom { room_name_id } => room_name_id.to_string(),
            SelectedRoom::InvitedRoom { room_name_id } => room_name_id.to_string(),
            SelectedRoom::Space { space_name_id } => format!("[Space] {space_name_id}"),
            SelectedRoom::Thread { room_name_id, .. } => format!("[Thread] {room_name_id}"),
        }
    }
}

impl PartialEq for SelectedRoom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                SelectedRoom::Thread {
                    room_name_id: lhs_room_name_id,
                    thread_root_event_id: lhs_thread_root_event_id,
                },
                SelectedRoom::Thread {
                    room_name_id: rhs_room_name_id,
                    thread_root_event_id: rhs_thread_root_event_id,
                },
            ) => {
                lhs_room_name_id.room_id() == rhs_room_name_id.room_id()
                    && lhs_thread_root_event_id == rhs_thread_root_event_id
            }
            (SelectedRoom::Thread { .. }, _) | (_, SelectedRoom::Thread { .. }) => false,
            _ => self.room_id() == other.room_id(),
        }
    }
}
impl Eq for SelectedRoom {}

/// Actions sent to the top-level App in order to update / restore its [`AppState`].
///
/// These are *NOT* widget actions.
#[derive(Debug)]
pub enum AppStateAction {
    /// The given room was focused (selected).
    RoomFocused(SelectedRoom),
    /// Resets the focus to none, meaning that no room is selected.
    FocusNone,
    /// The given room has successfully been upgraded from being displayed
    /// as an InviteScreen to a RoomScreen.
    UpgradedInviteToJoinedRoom(OwnedRoomId),
    /// The given app state was loaded from persistent storage
    /// and is ready to be restored.
    RestoreAppStateFromPersistentState(AppState),
    /// A room-level BotFather bind or unbind action completed.
    BotRoomBindingUpdated {
        room_id: OwnedRoomId,
        bound: bool,
        bot_user_id: Option<OwnedUserId>,
        warning: Option<String>,
    },
    /// A room's member list indicates that the configured BotFather is already present.
    BotRoomBindingDetected {
        room_id: OwnedRoomId,
        bot_user_id: OwnedUserId,
    },
    /// The given room was successfully loaded from the homeserver
    /// and is now known to our client.
    ///
    /// The RoomScreen for this room can now fully display the room's timeline.
    RoomLoadedSuccessfully {
        room_name_id: RoomNameId,
        /// `true` if this room is an invitation, `false` otherwise.
        is_invite: bool,
    },
    /// A request to navigate to a different room, optionally closing a prior/current room.
    NavigateToRoom {
        room_to_close: Option<OwnedRoomId>,
        destination_room: BasicRoomDetails,
    },
    None,
}

/// An action to show the generic top-level positive confirmation modal.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum PositiveConfirmationModalAction {
    /// Show the confirmation modal with the given content.
    ///
    /// The content is wrapped in a `RefCell` to ensure that only one entity handles it
    /// and that that one entity can take ownership of the content object,
    /// which avoids having to clone it.
    Show(RefCell<Option<ConfirmationModalContent>>),
}

/// An action to show a deletion/removal confirmation modal.
///
/// This is NOT a widget action.
#[derive(Debug)]
pub enum ConfirmDeleteAction {
    /// Show the deletion confirmation modal with the given content.
    ///
    /// The content is wrapped in a `RefCell` to ensure that only one entity handles it
    /// and that that one entity can take ownership of the content object,
    /// which avoids having to clone it.
    Show(RefCell<Option<ConfirmationModalContent>>),
}
