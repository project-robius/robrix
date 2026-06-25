//! The top-level application content.
//!
//! See `handle_startup()` for the first code that runs on app startup.

use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    time::Duration,
};
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, RoomId}};
use serde::{Deserialize, Serialize};
use crate::{
    avatar_cache::clear_avatar_cache, room_preview_cache::clear_room_preview_cache, home::{
        event_source_modal::{EventSourceModalAction, EventSourceModalWidgetRefExt}, invite_modal::{InviteModalAction, InviteModalWidgetRefExt}, main_desktop_ui::MainDesktopUiAction, navigation_tab_bar::{NavigationBarAction, SelectedTab}, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::RoomContextMenuWidgetRefExt, room_screen::{InviteAction, MessageAction, clear_timeline_states, invalidate_timeline_state}, rooms_list::{RoomsListAction, RoomsListRef, RoomsListUpdate, clear_all_invited_rooms, enqueue_rooms_list_update}
    }, join_leave_room_modal::{
        JoinLeaveModalKind, JoinLeaveRoomModalAction, JoinLeaveRoomModalWidgetRefExt
    }, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction, LogoutConfirmModalWidgetRefExt}, persistence, profile::user_profile_cache::clear_user_profile_cache, room::BasicRoomDetails, settings::app_preferences::{AppPreferences, UiZoom}, shared::{confirmation_modal::{ConfirmationModalContent, ConfirmationModalWidgetRefExt}, image_viewer::{ImageViewerAction, LoadState}, popup_list::{PopupKind, enqueue_popup_notification}}, sliding_sync::{DirectMessageRoomAction, MatrixRequest, TimelineKind, current_user_id, submit_async_request}, utils::RoomNameId, verification::VerificationAction, verification_modal::{
        VerificationModalAction,
        VerificationModalWidgetRefExt,
    }
};
use crate::shared::file_upload_modal::{FileUploadModalWidgetRefExt, FileUploadModalAction};

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
                            draw_text +: { color: #0 }
                            text: "Robrix"
                        }
                    }
                }

                body +: {
                    // Only the top part of safe area inset padding is applied here,
                    // since we don't have any bar or anything on the top.
                    // The other sides are handled by widgets that might draw in those areas,
                    // like the NavigationTabBar or room screen content.
                    padding: Inset{
                        top: (mod.widgets.SAFE_INSET_PAD_TOP),
                        bottom: 0,
                        left: 0,
                        right: 0,
                    }
                    keyboard_min_shift: 12.0

                    overlay_container := View {
                        width: Fill, height: Fill,
                        flow: Overlay,

                        home_screen_view := View {
                            visible: false
                            home_screen := HomeScreen {}
                        }
                        join_leave_modal := Modal {
                            content := JoinLeaveRoomModal {}
                        }
                        login_screen_view := View {
                            visible: true
                            login_screen := LoginScreen {}
                        }

                        image_viewer_modal := Modal {
                            content := ImageViewer {}
                        }
                        
                        // The popup that lets the user select users to mention, rooms to link,
                        // or a slash command to run (via kbd triggers like '@', '#', '/').
                        mention_popup := MentionablePopup { }

                        // Context menus should be shown in front of other UI elements,
                        // but behind verification modals.
                        new_message_context_menu := NewMessageContextMenu { }
                        room_context_menu := RoomContextMenu { }

                        // A modal to confirm sending out an invite to a room.
                        invite_confirmation_modal := Modal {
                            content := PositiveConfirmationModal {
                                buttons_view +: { accept_button +: {
                                    draw_icon +: { svg: (ICON_INVITE) }
                                    icon_walk: Walk{width: 28, height: Fit, margin: Inset{left: -10, right: 2} }
                                } }
                            }
                        }

                        // A modal to invite a user to a room.
                        invite_modal := Modal {
                            content := InviteModal {}
                        }

                        // Show the logout confirmation modal.
                        logout_confirm_modal := Modal {
                            content := LogoutConfirmModal {}
                        }

                        // Show the event source modal (View Source for messages).
                        event_source_modal := Modal {
                            content := EventSourceModal {}
                        }

                        // Show incoming verification requests in front of the aforementioned UI elements.
                        verification_modal := Modal {
                            can_dismiss: false,
                            content := VerificationModal {}
                        }
                        tsp_verification_modal := Modal {
                            content := TspVerificationModal {}
                        }

                        // A generic modal to confirm any positive action.
                        positive_confirmation_modal := Modal {
                            content := PositiveConfirmationModal {}
                        }

                        // A modal to confirm any deletion/removal action.
                        delete_confirmation_modal := Modal {
                            content := NegativeConfirmationModal {}
                        }

                        // A modal to preview and confirm file uploads.
                        file_upload_modal := Modal {
                            content := FileUploadModal {}
                        }

                        PopupList {}

                        // Tooltips must be shown in front of all other UI elements,
                        // since they can be shown as a hover atop any other widget.
                        // This tooltip widget handles TooltipActions directly by itself,
                        // so we don't need to call show/hide ourselves.
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
    #[rust] lifecycle: AppLifecycle,
    /// The details of a room we're waiting on to be loaded so that we can navigate to it.
    /// This can be either a room we're waiting to join, or one we're waiting to be invited to.
    /// Also includes an optional room ID to be closed once the awaited room has been loaded.
    #[rust] waiting_to_navigate_to_room: Option<(BasicRoomDetails, Option<OwnedRoomId>)>,
}

impl ScriptHook for App {
    /// After a hot-reload update, refresh the login/home screen visibility.
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.update_login_visibility(cx);
        });
    }

    /// After initial creation, set the global singletons for app-level shared widgets. (the PopupList and the mention autocomplete popup).
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            crate::shared::popup_list::set_global_popup_list(cx, &self.ui);
            crate::shared::mention_popup::set_global_mention_popup(cx, &self.ui);
        });
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // only init logging/tracing once.
        // `matrix_sdk::latest_events` emits a noisy per-room "Timer ... finished"
        // INFO line on every load; silence it by default. RUST_LOG still wins
        // when set, so you can re-enable it with `RUST_LOG=matrix_sdk::latest_events=info`.
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(
                "info,matrix_sdk::latest_events=warn",
            ));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();

        // On iOS (and potentially other devices), there is a very low limit
        // (albeit a soft limit) on max file descriptors, as little as 256.
        // We increase that preemptively to avoid running out later due to sqlite ops.
        #[cfg(unix)]
        unsafe {
            let mut rlim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) == 0 && rlim.rlim_cur < 4096 {
                let new_soft = (4096 as libc::rlim_t).min(rlim.rlim_max);
                let bumped = libc::rlimit { rlim_cur: new_soft, rlim_max: rlim.rlim_max };
                libc::setrlimit(libc::RLIMIT_NOFILE, &bumped);
            }
        }

        // Initialize the project directory here from the main UI thread
        // such that background threads/tasks will be able to can access it.
        let _app_data_dir = crate::app_data_dir();
        log!("App::handle_startup(): app_data_dir: {:?}", _app_data_dir);

        if let Err(e) = persistence::load_window_state(self.ui.window(cx, ids!(main_window)), cx) {
            error!("Failed to load window state: {}", e);
        }

        #[cfg(target_os = "macos")]
        self.install_macos_menu(cx);

        self.update_login_visibility(cx);

        log!("App::Startup: starting matrix sdk loop");
        let _tokio_rt_handle = crate::sliding_sync::start_matrix_tokio().unwrap();

        #[cfg(feature = "tsp")] {
            log!("App::Startup: initializing TSP (Trust Spanning Protocol) module.");
            crate::tsp::tsp_init(_tokio_rt_handle).unwrap();
        }
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let invite_confirmation_modal_content = self.ui.confirmation_modal(cx, ids!(invite_confirmation_modal.content));
        if let Some(_accepted) = invite_confirmation_modal_content.closed(actions) {
            self.ui.modal(cx, ids!(invite_confirmation_modal)).close(cx);
        }

        let delete_confirmation_modal_content = self.ui.confirmation_modal(cx, ids!(delete_confirmation_modal.content));
        if let Some(_accepted) = delete_confirmation_modal_content.closed(actions) {
            self.ui.modal(cx, ids!(delete_confirmation_modal)).close(cx);
        }

        let positive_confirmation_modal_content = self.ui.confirmation_modal(cx, ids!(positive_confirmation_modal.content));
        if let Some(_accepted) = positive_confirmation_modal_content.closed(actions) {
            self.ui.modal(cx, ids!(positive_confirmation_modal)).close(cx);
        }

        for action in actions {
            match action.downcast_ref() {
                Some(LogoutConfirmModalAction::Open) => {
                    self.ui.logout_confirm_modal(cx, ids!(logout_confirm_modal.content)).reset_state(cx);
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
                // Do NOT continue here — let the action propagate to the LoginScreen widget,
                // which will open the login_status_modal to show the failure message.
            }

            // Handle actions for the file upload modal.
            if let Some(action) = action.downcast_ref::<FileUploadModalAction>() {
                let outer_modal = self.ui.modal(cx, ids!(file_upload_modal));
                self.ui.file_upload_modal(cx, ids!(file_upload_modal.content))
                    .handle_file_previewer_action(cx, outer_modal, action);
                continue;
            }

            // Handle an action requesting to open the new message context menu.
            if let MessageAction::OpenMessageContextMenu { details, abs_pos } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                let new_message_context_menu = self.ui.new_message_context_menu(cx, ids!(new_message_context_menu));
                let expected_dimensions = new_message_context_menu.show(cx, details);
                // Use the overlay container's rect (not the window's) to correctly position
                // the context menu relative to the body area, which excludes the caption bar.
                let rect = self.ui.view(cx, ids!(overlay_container)).area().rect(cx);
                let pos_x = min(abs_pos.x - rect.pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(abs_pos.y - rect.pos.y, rect.size.y - expected_dimensions.y);
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
                // Use the overlay container's rect (not the window's) to correctly position
                // the context menu relative to the body area, which excludes the caption bar.
                let rect = self.ui.view(cx, ids!(overlay_container)).area().rect(cx);
                let pos_x = min(pos.x - rect.pos.x, rect.size.x - expected_dimensions.x);
                let pos_y = min(pos.y - rect.pos.y, rect.size.y - expected_dimensions.y);
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
                    // Broadcast the restored preferences first so listeners
                    // (e.g. the Dock's captured `room_screen` template) are
                    // refreshed before `LoadDockFromAppState` instantiates
                    // any tabs from them.
                    self.app_state.app_prefs.broadcast_all(cx);
                    cx.action(MainDesktopUiAction::LoadDockFromAppState);
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

            // Handle actions needed to open/close the join/leave room modal.
            match action.downcast_ref() {
                Some(JoinLeaveRoomModalAction::Open { kind, show_tip }) => {
                    self.ui
                        .join_leave_room_modal(cx, ids!(join_leave_modal.content))
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
                self.ui.verification_modal(cx, ids!(verification_modal.content))
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
                    self.ui.tsp_verification_modal(cx, ids!(tsp_verification_modal.content))
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
                    invite_confirmation_modal_content.show(cx, content);
                    self.ui.modal(cx, ids!(invite_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle a request to show the generic positive confirmation modal.
            if let Some(PositiveConfirmationModalAction::Show(content_opt)) = action.downcast_ref() {
                if let Some(content) = content_opt.borrow_mut().take() {
                    positive_confirmation_modal_content.show(cx, content);
                    self.ui.modal(cx, ids!(positive_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle a request to show the delete confirmation modal.
            if let Some(ConfirmDeleteAction::Show(content_opt)) = action.downcast_ref() {
                if let Some(content) = content_opt.borrow_mut().take() {
                    self.ui.confirmation_modal(cx, ids!(delete_confirmation_modal.content)).show(cx, content);
                    self.ui.modal(cx, ids!(delete_confirmation_modal)).open(cx);
                }
                continue;
            }

            // Handle InviteModalAction to open/close the invite modal.
            match action.downcast_ref() {
                Some(InviteModalAction::Open(room_name_id)) => {
                    self.ui.invite_modal(cx, ids!(invite_modal.content)).show(cx, room_name_id.clone());
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
                Some(EventSourceModalAction::Open { room_id, event_id, latest_json }) => {
                    self.ui.event_source_modal(cx, ids!(event_source_modal.content))
                        .show(cx, room_id.clone(), event_id.clone(), latest_json.clone());
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
                    positive_confirmation_modal_content.show(
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
    clear_room_preview_cache(cx);
}

#[derive(Debug)]
struct AppLifecycle {
    is_foreground: bool,
    is_active: bool,
    last_app_state_save: Option<AppStateSaveFingerprint>,
    shutdown_started: bool,
}

impl Default for AppLifecycle {
    fn default() -> Self {
        Self {
            is_foreground: true,
            is_active: true,
            last_app_state_save: None,
            shutdown_started: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AppStateSaveFingerprint {
    user_id: OwnedUserId,
    hash: u64,
    len: usize,
}

impl AppStateSaveFingerprint {
    fn new(user_id: OwnedUserId, bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self {
            user_id,
            hash: hasher.finish(),
            len: bytes.len(),
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> makepad_widgets::ScriptValue {
        // Order matters: base widgets first, then app widgets, then app UI.
        makepad_widgets::theme_mod(vm);
        script_eval!(vm, {
            mod.theme = mod.themes.light
        });
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
        crate::home::upload_progress::script_mod(vm);
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
        if let Event::LiveEdit = event {
            self.app_state.app_prefs.broadcast_all(cx);
        }

        self.handle_ui_zoom_shortcuts(cx, event);
        if let Event::MacosMenuCommand(command) = event {
            self.handle_ui_zoom_menu_command(cx, *command);
        }

        // Forward events to the MatchEvent trait implementation.
        self.match_event(cx, event);
        let scope = &mut Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, scope);
        self.handle_lifecycle_event(cx, event);

    }
}

impl App {
    fn apply_ui_zoom(&mut self, cx: &mut Cx, new_zoom: UiZoom) {
        if new_zoom != self.app_state.app_prefs.ui_zoom {
            self.app_state.app_prefs.ui_zoom = new_zoom;
            self.app_state.app_prefs.on_ui_zoom_changed(cx);
        }
    }

    fn handle_ui_zoom_shortcuts(&mut self, cx: &mut Cx, event: &Event) {
        let Event::KeyDown(e) = event else { return };
        if !e.modifiers.is_primary() {
            return;
        }
        let current = self.app_state.app_prefs.ui_zoom;
        let new_zoom = match e.key_code {
            KeyCode::Equals | KeyCode::NumpadEquals | KeyCode::NumpadAdd => {
                current.zoom_in_by(UiZoom::STEP)
            }
            KeyCode::Minus | KeyCode::NumpadSubtract => current.zoom_out_by(UiZoom::STEP),
            KeyCode::Key0 | KeyCode::Numpad0 => UiZoom::reset(),
            _ => return,
        };
        self.apply_ui_zoom(cx, new_zoom);
    }

    /// Set up the menu bar entries, currently macOS-only.
    #[cfg(target_os = "macos")]
    fn install_macos_menu(&self, cx: &mut Cx) {
        cx.update_macos_menu(MacosMenu::Main {
            items: vec![
                MacosMenu::Sub {
                    name: "Robrix".into(),
                    items: vec![MacosMenu::Item {
                        name: "Quit Robrix".into(),
                        command: live_id!(quit),
                        key: KeyCode::KeyQ,
                        shift: false,
                        enabled: true,
                    }],
                },
                MacosMenu::Sub {
                    name: "View".into(),
                    items: vec![
                        MacosMenu::Item {
                            name: "Zoom In".into(),
                            command: live_id!(zoom_in),
                            key: KeyCode::Equals,
                            shift: true,
                            enabled: true,
                        },
                        MacosMenu::Item {
                            name: "Zoom Out".into(),
                            command: live_id!(zoom_out),
                            key: KeyCode::Minus,
                            shift: false,
                            enabled: true,
                        },
                        MacosMenu::Line,
                        MacosMenu::Item {
                            name: "Reset Zoom".into(),
                            command: live_id!(reset_zoom),
                            key: KeyCode::Key0,
                            shift: false,
                            enabled: true,
                        },
                    ],
                },
            ],
        });
    }

    /// Connects `MacosMenuCommand`s to the existing zoom shortcut handlers.
    fn handle_ui_zoom_menu_command(&mut self, cx: &mut Cx, command: LiveId) {
        let current = self.app_state.app_prefs.ui_zoom;
        let new_zoom = if command == live_id!(zoom_in) {
            current.zoom_in_by(UiZoom::STEP)
        } else if command == live_id!(zoom_out) {
            current.zoom_out_by(UiZoom::STEP)
        } else if command == live_id!(reset_zoom) {
            UiZoom::reset()
        } else {
            return;
        };
        self.apply_ui_zoom(cx, new_zoom);
    }

    fn handle_lifecycle_event(&mut self, cx: &mut Cx, event: &Event) {
        match event {
            Event::QuitRequested(e) => {
                log!("Received quit request: {:?}. Persisting state before allowing quit.", e.reason);
                self.persist_runtime_state(cx, "quit request");
            }
            Event::Pause => {
                if self.lifecycle.is_active {
                    log!("App paused; persisting runtime state.");
                    self.lifecycle.is_active = false;
                }
                self.persist_runtime_state(cx, "pause");
            }
            Event::Resume => {
                if !self.lifecycle.is_active {
                    log!("App resumed.");
                    self.lifecycle.is_active = true;
                }
                crate::sliding_sync::set_sync_service_desired_running(true, "app resume");
            }
            Event::Background => {
                if self.lifecycle.is_foreground {
                    log!("App entered background; persisting state and stopping Matrix sync.");
                    self.lifecycle.is_foreground = false;
                }
                self.persist_runtime_state(cx, "background");
                crate::sliding_sync::set_sync_service_desired_running(false, "app background");
            }
            Event::WindowCloseRequested(e)
                if self.ui.window(cx, ids!(main_window)).window_id() == Some(e.window_id) => {
                    log!("Main window close requested; persisting runtime state.");
                    self.persist_runtime_state(cx, "main window close request");
                }
            Event::Foreground => {
                if !self.lifecycle.is_foreground {
                    log!("App entered foreground; starting Matrix sync.");
                    self.lifecycle.is_foreground = true;
                }
                crate::sliding_sync::set_sync_service_desired_running(true, "app foreground");
            }
            Event::Shutdown => self.handle_shutdown(cx),
            _ => {}
        }
    }

    fn persist_runtime_state(&mut self, cx: &mut Cx, reason: &'static str) {
        let window_ref = self.ui.window(cx, ids!(main_window));
        if let Err(e) = persistence::save_window_state(window_ref, cx) {
            error!("Failed to save window state during {reason}. Error: {e}");
        }

        let Some(user_id) = current_user_id() else {
            log!("Skipping app state persistence during {reason}: no logged-in Matrix user.");
            return;
        };

        let app_state_json = match persistence::serialize_app_state(&self.app_state) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to serialize app state during {reason}. Error: {e}");
                return;
            }
        };
        let fingerprint = AppStateSaveFingerprint::new(user_id.clone(), &app_state_json);
        if self.lifecycle.last_app_state_save.as_ref() == Some(&fingerprint) {
            log!("Skipping app state persistence during {reason}: state is unchanged.");
            return;
        }

        if let Err(e) = persistence::save_app_state_bytes(&app_state_json, &user_id) {
            error!("Failed to save app state during {reason}. Error: {e}");
        } else {
            self.lifecycle.last_app_state_save = Some(fingerprint);
        }
    }

    fn handle_shutdown(&mut self, cx: &mut Cx) {
        if self.lifecycle.shutdown_started {
            log!("Ignoring duplicate shutdown lifecycle event.");
            return;
        }
        self.lifecycle.shutdown_started = true;

        self.persist_runtime_state(cx, "shutdown");

        if let Err(_e) = crate::sliding_sync::stop_sync_service_for_shutdown(Duration::from_secs(3)) {
            error!("Failed to stop Matrix sync service before shutdown. Error: Timed out.");
        }

        #[cfg(feature = "tsp")] {
            let tsp_state = std::mem::take(&mut *crate::tsp::tsp_state_ref().lock().unwrap());
            let res = crate::sliding_sync::block_on_async_with_timeout(
                Some(Duration::from_secs(3)),
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


}

/// App-wide state that is stored persistently across multiple app runs
/// and shared/updated across various parts of the app.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct AppState {
    /// The currently-selected room, which is highlighted (selected) in the RoomsList
    /// and considered "active" in the main rooms screen.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
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
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub saved_dock_state_home: SavedDockState,
    /// The saved "snapshot" of the dock's UI layout/state for each space,
    /// keyed by the space ID.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub saved_dock_state_per_space: HashMap<OwnedRoomId, SavedDockState>,
    /// Whether a user is currently logged in to Robrix or not.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub logged_in: bool,
    /// App-wide user preferences/settings.
    #[serde(default, deserialize_with = "crate::utils::deserialize_or_default")]
    pub app_prefs: AppPreferences,
}

/// A snapshot of the main dock: all state needed to restore the dock tabs/layout.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SavedDockState {
    /// All items contained in the dock, keyed by their room or space ID.
    pub dock_items: HashMap<LiveId, DockItem>,
    /// The rooms that are currently open, keyed by their room or space ID.
    pub open_rooms: HashMap<LiveId, SelectedRoom>,
    /// The order in which room/thread tabs were last viewed,
    /// from oldest at the front to most recent at the end.
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

    /// Closes & cleans up the UI-side cached state and stops the backend async task
    /// for this thread timeline (if it is one).
    ///
    /// Does nothing for non-thread room kinds (e.g., main room timelines).
    ///
    /// This should be called only when the RoomScreen showing this room thread
    /// has been hidden (e.g., its tab was closed, or the user navigated back on mobile view mode),
    /// but not when it's still reachable via the mobile nav stack or a saved desktop dock.
    pub fn close_thread_timeline(&self, cx: &mut Cx) {
        let SelectedRoom::Thread { room_name_id, thread_root_event_id } = self else { return };
        let room_id = room_name_id.room_id().clone();
        // Drop the stale UI cache so reopening rebuilds a fresh timeline instead of reusing
        // a cache whose backend is about to be freed.
        invalidate_timeline_state(cx, &TimelineKind::Thread {
            room_id: room_id.clone(),
            thread_root_event_id: thread_root_event_id.clone(),
        });
        submit_async_request(MatrixRequest::CloseThreadTimeline {
            room_id,
            thread_root_event_id: thread_root_event_id.clone(),
        });
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

    /// Returns the dock-tab `kind` template id used to render this room.
    pub fn dock_kind(&self) -> LiveId {
        match self {
            SelectedRoom::JoinedRoom { .. } | SelectedRoom::Thread { .. } => id!(room_screen),
            SelectedRoom::InvitedRoom { .. } => id!(invite_screen),
            SelectedRoom::Space { .. } => id!(space_lobby_screen),
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
