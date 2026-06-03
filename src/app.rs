//! The top-level application content.
//!
//! See `handle_startup()` for the first code that runs on app startup.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{fs::{File, OpenOptions}, io::Write, sync::Mutex};
use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    time::Duration,
};
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, RoomId, UserId, events::room::message::RoomMessageEventContent}};
use serde::{Deserialize, Serialize};
use url::Url;
use crate::{
    avatar_cache::{self, clear_avatar_cache}, room_preview_cache::clear_room_preview_cache, home::{
        add_room::{CreateRoomModalAction, CreateRoomModalWidgetRefExt, StartChatModalAction, StartChatModalWidgetRefExt},
        bot_binding_modal::{BotBindingModalAction, BotBindingModalWidgetRefExt},
        event_source_modal::{EventSourceModalAction, EventSourceModalWidgetRefExt}, invite_modal::{InviteModalAction, InviteModalWidgetRefExt, mark_invite_modal_closed}, invite_screen::{InviteScreenWidgetRefExt, LeaveRoomResultAction}, main_desktop_ui::MainDesktopUiAction, navigation_tab_bar::{NavigationBarAction, SelectedTab}, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::{RoomContextMenuAction, RoomContextMenuWidgetRefExt}, room_screen::{InviteAction, MessageAction, RoomScreenWidgetRefExt, TimelineUpdate, clear_timeline_states}, room_settings_modal::{RoomSettingsAction, RoomSettingsModalWidgetRefExt, StdinCommandAction}, rooms_list::{RoomsListAction, RoomsListRef, RoomsListUpdate, clear_all_invited_rooms, enqueue_rooms_list_update}, rooms_list_header::RoomsListHeaderAction, space_lobby::SpaceLobbyScreenWidgetRefExt, spaces_bar::SpacesBarRef
    }, i18n::{AppLanguage, tr_fmt, tr_key}, join_leave_room_modal::{
        JoinLeaveModalKind, JoinLeaveRoomModalAction, JoinLeaveRoomModalWidgetRefExt
    }, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction, LogoutConfirmModalWidgetRefExt}, persistence, profile::user_profile_cache::clear_user_profile_cache, register::RegisterAction, room::BasicRoomDetails, shared::{confirmation_modal::{ConfirmationModalContent, ConfirmationModalWidgetRefExt}, file_upload_modal::{FilePreviewerAction, FileUploadModalWidgetRefExt}, forward_modal::{ForwardMessageModalAction, ForwardMessageModalWidgetRefExt}, image_viewer::{ImageViewerAction, LoadState}, popup_list::{PopupKind, enqueue_popup_notification}, room_filter_input_bar::FilterAction}, sliding_sync::{DirectMessageRoomAction, MatrixRequest, RemoteDirectorySearchKind, RemoteDirectorySearchResult, RoomSettingsFetchedAction, RoomAvatarUploadedAction, TimelineKind, AccountSwitchAction, current_user_id, get_client, submit_async_request, get_timeline_update_sender}, updater::{UpdateCheckOutcome, check_for_updates, load_skipped_update_version, save_skipped_update_version, update_release_page_url}, utils::RoomNameId, verification::VerificationAction, verification_modal::{
        VerificationModalAction,
        VerificationModalWidgetRefExt,
    }, settings::app_preferences::{AppPreferences, AppPreferencesAction, UiZoom}
};
use crate::shared::room_filter_search_results::{RoomFilterResultAction, RoomFilterResultTarget};
use crate::shared::room_filter_search_results::RoomFilterSearchResultsListWidgetRefExt;
use crate::shared::video_message_player_modal::WindowFullscreenAction;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.draw.KeyCode

    load_all_resources() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.inner_size: vec2(1280, 800)
                window.title: "Robrix"
                pass.clear_color: (COLOR_SECONDARY)
                caption_bar +: {
                    draw_bg.color: #F3F3F3
                    caption_label +: {
                        label +: {
                            draw_text +: { color: #0 }
                            text: "Robrix"
                        }
                    }
                }

                window_menu := WindowMenu {
                    main := MenuItem.Main{items:[@app_menu]}
                    app_menu := MenuItem.Sub { name:"Robrix" items:[@quit] }
                    quit := MenuItem.Item { name:"Quit Robrix" key: KeyCode.KeyQ enabled: true }
                }

                body +: {
                    show_bg: true
                    draw_bg.color: (COLOR_SECONDARY)
                    padding: Inset{
                        top: (mod.widgets.SAFE_INSET_PAD_TOP),
                        bottom: (mod.widgets.SAFE_INSET_PAD_BOTTOM),
                        left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                        right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                    }

                    overlay_container := View {
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

                        register_screen_view := View {
                            visible: false
                            register_screen := RegisterScreen {}
                        }

                        image_viewer_modal := Modal {
                            content +: {
                                width: Fill, height: Fill,
                                image_viewer_modal_inner := ImageViewer {}
                            }
                        }

                        file_upload_modal := Modal {
                            content +: {
                                width: Fill, height: Fill,
                                align: Align{x: 0.5, y: 0.5},
                                file_upload_modal_inner := FileUploadModal {}
                            }
                        }

                        forward_message_modal := Modal {
                            content +: {
                                height: Fill,
                                width: Fill,
                                align: Align{x: 0.5, y: 0.5},
                                forward_message_modal_inner := ForwardMessageModal {}
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

                        // A modal to view and edit room settings.
                        room_settings_modal := Modal {
                            content +: {
                                height: Fill,
                                width: Fill,
                                align: Align{x: 0.5, y: 0.1},
                                room_settings_modal_inner := RoomSettingsModal {}
                            }
                        }
                        bot_binding_modal := Modal {
                            content +: {
                                height: Fill,
                                width: Fill,
                                align: Align{x: 0.5, y: 0.5},
                                bot_binding_modal_inner := BotBindingModal {}
                            }
                        }
                        room_filter_modal := Modal {
                            content +: {
                                room_filter_modal_inner := RoundedShadowView {
                                    width: 420,
                                    height: Fit
                                    flow: Down
                                    spacing: 8
                                    show_bg: true
                                    draw_bg +: {
                                        color: (COLOR_PRIMARY_DARKER)
                                        border_radius: 4.0
                                        border_size: 0.0
                                        shadow_color: #0005
                                        shadow_radius: 15.0
                                        shadow_offset: vec2(1.0, 0.0)
                                    }
                                    padding: Inset{top: 15, left: 15, right: 15, bottom: 15}

                                    room_filter_input_bar := RoomFilterInputBar {}

                                    search_results_title := Label {
                                        width: Fill,
                                        height: Fit,
                                        margin: Inset{left: 4, top: 2}
                                        text: ""
                                        draw_text +: {
                                            color: (COLOR_TEXT_INPUT_IDLE)
                                            text_style: REGULAR_TEXT {font_size: 10}
                                        }
                                    }

                                    search_results_scroll := ScrollYView {
                                        width: Fill,
                                        height: 260
                                        show_bg: false

                                        search_results := View {
                                            width: Fill,
                                            height: Fit,
                                            flow: Down
                                            spacing: 4

                                            search_results_empty := Label {
                                                width: Fill,
                                                height: Fit,
                                                flow: Flow.Right{wrap: true},
                                                text: ""
                                                draw_text +: {
                                                    color: (COLOR_TEXT)
                                                    text_style: REGULAR_TEXT {font_size: 10}
                                                }
                                            }

                                            remote_search_options := View {
                                                visible: false
                                                width: Fill,
                                                height: Fit,
                                                flow: Right
                                                spacing: 6
                                                margin: Inset{top: 6}

                                                remote_search_people_button := RobrixNeutralIconButton {
                                                    width: Fit,
                                                    text: ""
                                                }
                                                remote_search_rooms_button := RobrixNeutralIconButton {
                                                    width: Fit,
                                                    text: ""
                                                }
                                                remote_search_spaces_button := RobrixNeutralIconButton {
                                                    width: Fit,
                                                    text: ""
                                                }
                                            }

                                            search_results_list := mod.widgets.RoomFilterSearchResultsList {}
                                        }
                                    }
                                }
                            }
                        }

                        create_room_modal := Modal {
                            content +: {
                                create_room_modal_inner := CreateRoomModal {}
                            }
                        }

                        start_chat_modal := Modal {
                            content +: {
                                start_chat_modal_inner := StartChatModal {}
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
                            can_dismiss: false,
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

                        update_available_modal := Modal {
                            content +: {
                                update_available_modal_inner := RoundedView {
                                    width: 460
                                    height: Fit
                                    flow: Down
                                    padding: Inset{top: 24, right: 24, bottom: 20, left: 24}
                                    spacing: 10
                                    show_bg: true
                                    draw_bg +: {
                                        color: (COLOR_PRIMARY)
                                        border_radius: 6.0
                                    }

                                    update_available_title := Label {
                                        width: Fill
                                        height: Fit
                                        flow: Flow.Right{wrap: true}
                                        draw_text +: {
                                            text_style: TITLE_TEXT {font_size: 13}
                                            color: #000
                                        }
                                        text: "Update Available"
                                    }

                                    update_available_body := Label {
                                        width: Fill
                                        height: Fit
                                        flow: Flow.Right{wrap: true}
                                        draw_text +: {
                                            text_style: REGULAR_TEXT {font_size: 11.5}
                                            color: #000
                                        }
                                        text: ""
                                    }

                                    update_available_buttons := View {
                                        width: Fill
                                        height: Fit
                                        flow: Right
                                        align: Align{x: 1.0, y: 0.5}
                                        margin: Inset{top: 8}
                                        spacing: 10

                                        update_skip_button := RobrixNeutralIconButton {
                                            width: Fit
                                            padding: 13
                                            icon_walk: Walk{width: 0, height: 0, margin: 0}
                                            text: "Skip This Version"
                                        }

                                        update_cancel_button := RobrixNeutralIconButton {
                                            width: 100
                                            padding: 13
                                            icon_walk: Walk{width: 0, height: 0, margin: 0}
                                            text: "Cancel"
                                        }

                                        update_upgrade_button := RobrixPositiveIconButton {
                                            width: 100
                                            padding: 13
                                            icon_walk: Walk{width: 0, height: 0, margin: 0}
                                            text: "Upgrade"
                                        }
                                    }
                                }
                            }
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

#[derive(Clone, Debug)]
pub enum RoomFilterRemoteSearchAction {
    Results {
        query: String,
        kind: RemoteDirectorySearchKind,
        results: Vec<RemoteDirectorySearchResult>,
    },
    Failed {
        query: String,
        kind: RemoteDirectorySearchKind,
        error: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AuthUiState {
    #[default]
    CheckingSession,
    LoggedOut,
    LoggedIn,
}

#[derive(Script)]
pub struct App {
    #[live] ui: WidgetRef,
    /// The top-level app state, shared across various parts of the app.
    #[rust] app_state: AppState,
    #[rust] lifecycle: AppLifecycle,
    #[rust] auth_ui_state: AuthUiState,
    /// The details of a room we're waiting on to be loaded so that we can navigate to it.
    /// This can be either a room we're waiting to join, or one we're waiting to be invited to.
    /// Also includes an optional room ID to be closed once the awaited room has been loaded.
    #[rust] waiting_to_navigate_to_room: Option<(BasicRoomDetails, Option<OwnedRoomId>)>,
    /// A stack of previously-selected rooms for mobile navigation.
    /// When a view is popped off the stack, the previous `selected_room` is restored from here.
    #[rust] mobile_room_nav_stack: Vec<SelectedRoom>,
    #[rust(Timer::empty())] room_filter_debounce_timer: Timer,
    #[rust] pending_room_filter_keywords: String,
    #[rust] auto_update_check_started: bool,
    #[rust] skipped_update_version: Option<String>,
    #[rust] update_prompt_versions: Option<(String, String)>,
}

impl ScriptHook for App {
    /// After a hot-reload update, refresh the login/home screen visibility.
    fn on_after_reload(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            self.update_login_visibility(cx);
        });
    }

    /// After initial creation, set the global singleton for the PopupList widget
    /// and start a background thread to accept stdin commands for testing.
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            crate::shared::popup_list::set_global_popup_list(cx, &self.ui);
            cx.spawn_thread(|| {
                use std::io::BufRead;
                for line in std::io::stdin().lock().lines().flatten() {
                    let line = line.trim().to_string();
                    if !line.is_empty() {
                        log!("[stdin] {}", line);
                        Cx::post_action(StdinCommandAction(line));
                    }
                }
            });
        });
    }
}

// =============================================================================
// File Logging for Packaged Builds (non-mobile platforms)
// =============================================================================

/// Global log file handle for packaged builds.
/// Only used on desktop platforms when running as a packaged application.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
static LOG_FILE: std::sync::OnceLock<Option<Mutex<File>>> = std::sync::OnceLock::new();

/// Detects if the application is running as a packaged build (not via `cargo run`).
///
/// Detection methods per platform:
/// - macOS: Check if executable is inside a `.app/Contents/MacOS/` bundle
/// - Windows: Check if executable is in `Program Files` or similar installation directory
/// - Linux: Check if executable is in `/usr`, `/opt`, or is an AppImage
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn is_packaged_build() -> bool {
    let Ok(exe_path) = std::env::current_exe() else {
        return false;
    };
    let exe_path_str = exe_path.to_string_lossy();

    #[cfg(target_os = "macos")]
    {
        // Check if running from a .app bundle
        exe_path_str.contains(".app/Contents/MacOS/")
    }

    #[cfg(target_os = "windows")]
    {
        // Check if running from Program Files or a typical installation directory
        let exe_lower = exe_path_str.to_lowercase();
        exe_lower.contains("program files")
            || exe_lower.contains("programfiles")
            || exe_lower.contains("appdata\\local\\programs")
    }

    #[cfg(target_os = "linux")]
    {
        // Check if running from system directories or AppImage
        exe_path_str.starts_with("/usr/")
            || exe_path_str.starts_with("/opt/")
            || exe_path_str.contains(".AppImage")
            || std::env::var("APPIMAGE").is_ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Initializes file logging for packaged builds.
/// Creates a log file in the app data directory with timestamp.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn init_file_logging() -> Option<()> {
    if !is_packaged_build() {
        LOG_FILE.get_or_init(|| None);
        return None;
    }

    // Get platform-specific logs directory
    let logs_dir = logs_dir();
    std::fs::create_dir_all(&logs_dir).ok()?;

    // Create log file with timestamp
    let now = chrono::Local::now();
    let log_filename = format!("robrix_{}.log", now.format("%Y-%m-%d_%H-%M-%S"));
    let log_path = logs_dir.join(&log_filename);

    // Also create/update a symlink to the latest log file for convenience
    // Remove old symlink if it exists and create a new one (unix only)
    #[cfg(unix)]
    {
        let latest_log_path = logs_dir.join("robrix_latest.log");
        let _ = std::fs::remove_file(&latest_log_path);
        let _ = std::os::unix::fs::symlink(&log_filename, &latest_log_path);
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()?;

    LOG_FILE.get_or_init(|| Some(Mutex::new(file)));

    // Print to stderr so user knows where logs are going
    eprintln!("[Robrix] Logging to file: {}", log_path.display());

    Some(())
}

/// Writes a log message to the log file (if file logging is enabled).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[allow(dead_code)]
fn write_to_log_file(message: &str) {
    if let Some(Some(file_mutex)) = LOG_FILE.get() {
        if let Ok(mut file) = file_mutex.lock() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, message);
            let _ = file.flush();
        }
    }
}

/// Returns the path to the logs directory using platform-standard locations.
///
/// Platform-specific paths:
/// - macOS: `~/Library/Logs/Robrix/`
/// - Windows: `%APPDATA%/Robrix/logs/`
/// - Linux: `~/.local/share/robrix/logs/` (or `$XDG_DATA_HOME/robrix/logs/`)
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn logs_dir() -> std::path::PathBuf {
    use std::path::PathBuf;

    #[cfg(target_os = "macos")]
    {
        // macOS standard log location: ~/Library/Logs/Robrix/
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Logs")
                .join("Robrix");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: %APPDATA%/Robrix/logs/
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Robrix").join("logs");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Use XDG_DATA_HOME if set, otherwise ~/.local/share/
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg_data).join("robrix").join("logs");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("robrix")
                .join("logs");
        }
    }

    // Fallback to app data directory
    crate::app_data_dir().join("logs")
}

/// Cleans up old log files, keeping only the most recent N log files.
/// This should be called periodically to prevent disk space issues.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn cleanup_old_logs(max_logs_to_keep: usize) {
    let logs_dir = logs_dir();
    if !logs_dir.exists() {
        return;
    }

    // Collect all log files (excluding the symlink)
    let mut log_files: Vec<_> = match std::fs::read_dir(&logs_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                name_str.starts_with("robrix_")
                    && name_str.ends_with(".log")
                    && name_str != "robrix_latest.log"
            })
            .collect(),
        Err(_) => return,
    };

    // Sort by modification time (oldest first)
    log_files.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        a_time.cmp(&b_time)
    });

    // Remove old log files
    if log_files.len() > max_logs_to_keep {
        let files_to_remove = log_files.len() - max_logs_to_keep;
        for entry in log_files.into_iter().take(files_to_remove) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Maximum number of log files to keep
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const MAX_LOG_FILES_TO_KEEP: usize = 10;

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // only init logging/tracing once
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing_subscriber::filter::LevelFilter::ERROR)
            .try_init();
        // Initialize the project directory here from the main UI thread
        // such that background threads/tasks will be able to access it.
        // This must be done before initializing file logging.
        let _app_data_dir = crate::app_data_dir();

        // Initialize file logging for packaged builds (non-mobile platforms).
        // This must be done before setting up the log handler.
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            init_file_logging();
            // Clean up old log files to prevent disk space issues
            cleanup_old_logs(MAX_LOG_FILES_TO_KEEP);
        }
        // Initialize the project directory here from the main UI thread
        // such that background threads/tasks will be able to can access it.
        let _app_data_dir = crate::app_data_dir();
        log!("App::handle_startup(): app_data_dir: {:?}", _app_data_dir);

        if let Err(e) = persistence::load_window_state(self.ui.window(cx, ids!(main_window)), cx) {
            error!("Failed to load window state: {}", e);
        }

        self.update_login_visibility(cx);
        self.sync_app_language(cx);
        self.app_state.app_prefs.broadcast_all(cx);
        self.skipped_update_version = load_skipped_update_version();
        self.start_auto_update_check(cx);

        log!("App::Startup: starting matrix sdk loop");
        let _tokio_rt_handle = crate::sliding_sync::start_matrix_tokio().unwrap();

        #[cfg(feature = "tsp")] {
            log!("App::Startup: initializing TSP (Trust Spanning Protocol) module.");
            crate::tsp::tsp_init(_tokio_rt_handle).unwrap();
        }
    }

    fn handle_signal(&mut self, cx: &mut Cx) {
        avatar_cache::process_avatar_updates(cx);
        // Redraw search results list to pick up newly-loaded avatars
        self.ui.view(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list))
            .redraw(cx);
    }

    fn handle_timer(&mut self, cx: &mut Cx, event: &TimerEvent) {
        if self.room_filter_debounce_timer.is_timer(event).is_some() {
            self.room_filter_debounce_timer = Timer::empty();
            let keywords = std::mem::take(&mut self.pending_room_filter_keywords);
            self.update_room_filter_modal_results(cx, &keywords);
        }
    }

    fn handle_audio_devices(&mut self, cx: &mut Cx, devices: &AudioDevicesEvent) {
        cx.use_audio_outputs(&devices.default_output());
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.sync_app_language(cx);

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

        if self.ui.button(cx, ids!(update_available_modal_inner.update_upgrade_button)).clicked(actions) {
            let latest_version = self.update_prompt_versions
                .as_ref()
                .map(|(_, latest_version)| latest_version.clone());
            self.skipped_update_version = None;
            if let Err(error) = save_skipped_update_version(None) {
                error!("Failed to clear skipped update version. Error: {error}");
            }
            if let Some(latest_version) = latest_version {
                let release_page_url = update_release_page_url(&latest_version);
                if let Err(e) = robius_open::Uri::new(&release_page_url).open() {
                    error!("Failed to open update URL {:?}. Error: {:?}", release_page_url, e);
                    enqueue_popup_notification(
                        tr_fmt(self.app_state.app_language, "room_screen.popup.open_url_failed", &[("url", release_page_url.as_str())]),
                        PopupKind::Error,
                        Some(10.0),
                    );
                }
            }
            self.update_prompt_versions = None;
            self.ui.modal(cx, ids!(update_available_modal)).close(cx);
        }
        if self.ui.button(cx, ids!(update_available_modal_inner.update_cancel_button)).clicked(actions) {
            self.update_prompt_versions = None;
            self.ui.modal(cx, ids!(update_available_modal)).close(cx);
        }
        if self.ui.button(cx, ids!(update_available_modal_inner.update_skip_button)).clicked(actions) {
            if let Some((_, latest_version)) = self.update_prompt_versions.as_ref() {
                self.skipped_update_version = Some(latest_version.clone());
                if let Err(error) = save_skipped_update_version(Some(latest_version.as_str())) {
                    error!("Failed to persist skipped update version. Error: {error}");
                }
            }
            self.update_prompt_versions = None;
            self.ui.modal(cx, ids!(update_available_modal)).close(cx);
        }

        for action in actions.iter() {
            if let Some(
                AppPreferencesAction::ViewModeChanged(_)
                | AppPreferencesAction::SendOnEnterChanged(_)
                | AppPreferencesAction::UiZoomChanged(_)
            ) = action.downcast_ref() {
                if let Some(user_id) = current_user_id() {
                    if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                        error!("Failed to persist app state after updating app preferences. Error: {e}");
                    }
                }
                continue;
            }

            if let RoomFilterResultAction::Clicked(target) = action.as_widget_action().cast() {
                self.ui.modal(cx, ids!(room_filter_modal)).close(cx);
                match target {
                    RoomFilterResultTarget::LocalSpace { room_name_id: space_name_id, .. }
                    => {
                        cx.action(NavigationBarAction::GoToSpace { space_name_id: space_name_id.clone() });
                    }
                    RoomFilterResultTarget::LocalRoom { room_name_id, .. }
                    => {
                        self.navigate_to_room(cx, None, &BasicRoomDetails::RoomId(room_name_id.clone()));
                    }
                    RoomFilterResultTarget::RemoteSpace { space_name_id, .. } => {
                        self.open_join_from_search_result(
                            cx,
                            BasicRoomDetails::Name(space_name_id.clone()),
                            true,
                        );
                    }
                    RoomFilterResultTarget::RemoteRoom { room_name_id, .. } => {
                        self.open_join_from_search_result(
                            cx,
                            BasicRoomDetails::Name(room_name_id.clone()),
                            false,
                        );
                    }
                    RoomFilterResultTarget::RemoteUser(user_profile) => {
                        submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                            create_encrypted: self.app_state.bot_settings.should_create_encrypted_dm(
                                user_profile.user_id.as_ref(),
                                current_user_id().as_deref(),
                            ),
                            user_profile: user_profile.clone(),
                            allow_create: false,
                        });
                    }
                }
                return;
            }
        }

        if let Some(kind) = self.clicked_room_filter_remote_option(cx, actions) {
            let room_filter_input = self.ui.text_input(cx, ids!(room_filter_modal_inner.room_filter_input_bar.input));
            let query = room_filter_input.text().trim().to_owned();
            if !query.is_empty() {
                let kind_text = match &kind {
                    RemoteDirectorySearchKind::People => tr_key(self.app_state.app_language, "app.room_filter.remote.kind.people"),
                    RemoteDirectorySearchKind::Rooms => tr_key(self.app_state.app_language, "app.room_filter.remote.kind.rooms"),
                    RemoteDirectorySearchKind::Spaces => tr_key(self.app_state.app_language, "app.room_filter.remote.kind.spaces"),
                };
                let searching_text = tr_fmt(self.app_state.app_language, "app.room_filter.searching_remote", &[("kind", kind_text)]);
                self.set_room_filter_modal_empty_state(
                    cx,
                    &searching_text,
                    false,
                );
                submit_async_request(MatrixRequest::SearchDirectory {
                    query,
                    kind,
                    limit: 16,
                });
            }
            return;
        }

        if let Some(room_screen_id) = self.clicked_mobile_room_info_button(cx, actions) {
            let room_screen_widget_uid = self.ui.room_screen(cx, &[room_screen_id]).widget_uid();
            cx.widget_action(
                room_screen_widget_uid,
                MessageAction::ShowRoomInfoPane,
            );
        }

        for action in actions {
            if let Some(AppUpdateAction::AutoCheckFinished(result)) = action.downcast_ref() {
                if let UpdateCheckOutcome::UpdateAvailable { current_version, latest_version } = result {
                    self.show_update_prompt_if_needed(cx, current_version, latest_version, true);
                } else if let UpdateCheckOutcome::Error(error) = result {
                    warning!("Automatic update check failed: {error}");
                }
                continue;
            }
            if let Some(AppUpdateAction::ShowUpdatePrompt { current_version, latest_version, from_auto_check }) = action.downcast_ref() {
                self.show_update_prompt_if_needed(
                    cx,
                    current_version.as_str(),
                    latest_version.as_str(),
                    *from_auto_check,
                );
                continue;
            }

            match action.downcast_ref::<WindowFullscreenAction>() {
                Some(WindowFullscreenAction::Enable) => {
                    self.ui.window(cx, ids!(main_window)).fullscreen(cx);
                    continue;
                }
                Some(WindowFullscreenAction::Disable) => {
                    self.ui.window(cx, ids!(main_window)).disable_fullscreen(cx);
                    continue;
                }
                None => {}
            }
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
                    self.auth_ui_state = AuthUiState::LoggedOut;
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
                    // Keep the navigation tab bar's visual state in sync with app state.
                    cx.action(NavigationBarAction::TabSelected(SelectedTab::Home));
                    on_clear_appstate.notify_one();
                    continue;
                }
                _ => {}
            }

            if let Some(LoginAction::NavigateToRegister) = action.downcast_ref() {
                self.ui.view(cx, ids!(login_screen_view)).set_visible(cx, false);
                self.ui.view(cx, ids!(register_screen_view)).set_visible(cx, true);
                self.ui.redraw(cx);
                continue;
            }

            if let Some(RegisterAction::NavigateToLogin) = action.downcast_ref() {
                self.ui.view(cx, ids!(register_screen_view)).set_visible(cx, false);
                self.ui.view(cx, ids!(login_screen_view)).set_visible(cx, true);
                self.ui.redraw(cx);
                continue;
            }

            if let Some(LoginAction::ShowLoginScreen) = action.downcast_ref() {
                if !self.app_state.adding_account {
                    self.app_state.logged_in = false;
                    self.auth_ui_state = AuthUiState::LoggedOut;
                    self.update_login_visibility(cx);
                    self.ui.redraw(cx);
                }
                continue;
            }

            if let Some(LoginAction::LoginSuccess) = action.downcast_ref() {
                log!("Received LoginAction::LoginSuccess, hiding login view.");
                self.app_state.logged_in = true;
                self.app_state.adding_account = false;
                self.auth_ui_state = AuthUiState::LoggedIn;
                // If the user reached this success via the register flow, also hide
                // register_screen — update_login_visibility only manages login_screen_view.
                self.ui.view(cx, ids!(register_screen_view)).set_visible(cx, false);
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // Handle request to show login screen for adding another account
            if let Some(LoginAction::ShowAddAccountScreen) = action.downcast_ref() {
                log!("Received LoginAction::ShowAddAccountScreen, showing login view for adding account.");
                self.app_state.adding_account = true;
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // Handle successful addition of a new account
            if let Some(LoginAction::AddAccountSuccess) = action.downcast_ref() {
                log!("Received LoginAction::AddAccountSuccess, hiding login view.");
                self.app_state.adding_account = false;
                self.ui
                    .modal(cx, ids!(login_screen_view.login_screen.login_status_modal))
                    .close(cx);
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // Handle cancellation of adding a new account - go back to previous screen
            if let Some(LoginAction::CancelAddAccount) = action.downcast_ref() {
                log!("Received LoginAction::CancelAddAccount, hiding login view.");
                self.app_state.adding_account = false;
                self.ui
                    .modal(cx, ids!(login_screen_view.login_screen.login_status_modal))
                    .close(cx);
                self.update_login_visibility(cx);
                self.ui.redraw(cx);
                continue;
            }

            // Handle account switch actions
            match action.downcast_ref() {
                Some(AccountSwitchAction::Starting(user_id)) => {
                    log!("Account switch starting to: {}", user_id);
                    // Clear UI state during account switch
                    clear_all_app_state(cx);
                    self.app_state.selected_room = None;
                    // Clear saved dock state so tabs will be closed
                    self.app_state.saved_dock_state_home = Default::default();
                    // Reset navigation to Home tab
                    self.app_state.selected_tab = SelectedTab::Home;
                    cx.action(NavigationBarAction::TabSelected(SelectedTab::Home));
                    self.ui.redraw(cx);
                    continue;
                }
                Some(AccountSwitchAction::Switched(user_id)) => {
                    log!("Account switch completed to: {}", user_id);
                    enqueue_popup_notification(
                        format!("Switched to account {}", user_id),
                        PopupKind::Success,
                        Some(3.0),
                    );
                    self.ui.redraw(cx);
                    continue;
                }
                Some(AccountSwitchAction::Failed(error)) => {
                    log!("Account switch failed: {}", error);
                    enqueue_popup_notification(
                        format!("Failed to switch account: {}", error),
                        PopupKind::Error,
                        None,
                    );
                    continue;
                }
                _ => {}
            }

            // If a login failure occurs mid-session (e.g., an expired/revoked token detected
            // by `handle_session_changes`), navigate back to the login screen.
            // When not yet logged in, the login_screen widget handles displaying the failure modal.
            if let Some(LoginAction::LoginFailure(_)) = action.downcast_ref() {
                if !self.app_state.adding_account && self.auth_ui_state != AuthUiState::LoggedOut {
                    log!("Received LoginAction::LoginFailure while restoring or logged in; showing login screen.");
                    self.app_state.logged_in = false;
                    self.auth_ui_state = AuthUiState::LoggedOut;
                    self.update_login_visibility(cx);
                    self.ui.redraw(cx);
                }
                // Do NOT continue here — let the action propagate to the LoginScreen widget,
                // which will open the login_status_modal to show the failure message.
            }

            if let FilterAction::Changed(keywords) = action.as_widget_action().cast_ref() {
                cx.stop_timer(self.room_filter_debounce_timer);
                self.pending_room_filter_keywords = keywords.clone();
                self.room_filter_debounce_timer = cx.start_timeout(0.12);
                continue;
            }

            match action.downcast_ref() {
                Some(RoomFilterRemoteSearchAction::Results { query, kind: _, results }) => {
                    let room_filter_input = self.ui.text_input(cx, ids!(room_filter_modal_inner.room_filter_input_bar.input));
                    if room_filter_input.text().trim() != query.trim() {
                        continue;
                    }
                    let search_results_list = self.ui.room_filter_search_results_list(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list));
                    let mut new_results = Vec::new();
                    for result in results {
                        match result {
                            RemoteDirectorySearchResult::User(user_profile) => {
                                new_results.push(RoomFilterResultTarget::RemoteUser(user_profile.clone()));
                            }
                            RemoteDirectorySearchResult::Room { room_name_id, avatar_uri } => {
                                new_results.push(RoomFilterResultTarget::RemoteRoom {
                                    room_name_id: room_name_id.clone(),
                                    avatar_uri: avatar_uri.clone(),
                                });
                            }
                            RemoteDirectorySearchResult::Space { space_name_id, avatar_uri } => {
                                new_results.push(RoomFilterResultTarget::RemoteSpace {
                                    space_name_id: space_name_id.clone(),
                                    avatar_uri: avatar_uri.clone(),
                                });
                            }
                        }
                    }
                    if new_results.is_empty() {
                        self.set_room_filter_modal_empty_state(
                            cx,
                            &tr_fmt(self.app_state.app_language, "app.room_filter.no_server_results", &[
                                ("query", query),
                            ]),
                            true,
                        );
                    } else {
                        self.set_room_filter_modal_empty_state(cx, "", false);
                    }
                    search_results_list.set_results(cx, new_results);
                    continue;
                }
                Some(RoomFilterRemoteSearchAction::Failed { query, kind: _, error }) => {
                    let room_filter_input = self.ui.text_input(cx, ids!(room_filter_modal_inner.room_filter_input_bar.input));
                    if room_filter_input.text().trim() != query.trim() {
                        continue;
                    }
                    let search_results_list = self.ui.room_filter_search_results_list(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list));
                    search_results_list.clear(cx);
                    self.set_room_filter_modal_empty_state(
                        cx,
                        &tr_fmt(self.app_state.app_language, "app.room_filter.search_remote_failed", &[
                            ("error", error),
                        ]),
                        true,
                    );
                    continue;
                }
                _ => {}
            }

            if let Some(RoomsListHeaderAction::OpenRoomFilterModal) = action.downcast_ref() {
                self.ui.modal(cx, ids!(room_filter_modal)).open(cx);
                let room_filter_input = self.ui.text_input(cx, ids!(room_filter_modal_inner.room_filter_input_bar.input));
                room_filter_input.set_key_focus(cx);
                self.update_room_filter_modal_results(cx, &room_filter_input.text());
                continue;
            }

            // Handle an action requesting to open the new message context menu.
            if let MessageAction::OpenMessageContextMenu { details, abs_pos, opening_gesture } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                let new_message_context_menu = self.ui.new_message_context_menu(cx, ids!(new_message_context_menu));
                let expected_dimensions = new_message_context_menu.show(cx, details, self.app_state.app_language, opening_gesture);
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
            if let RoomsListAction::OpenRoomContextMenu { details, pos, opening_gesture } = action.as_widget_action().cast() {
                self.ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
                let room_context_menu = self.ui.room_context_menu(cx, ids!(room_context_menu));
                let expected_dimensions = room_context_menu.show(cx, details, self.app_state.app_language, opening_gesture);
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
            if let Some(LeaveRoomResultAction::Left { room_id }) = action.downcast_ref() {
                enqueue_rooms_list_update(RoomsListUpdate::HideRoom { room_id: room_id.clone() });
                self.app_state
                    .bot_settings
                    .set_room_bound(room_id.clone(), None, false);

                let removed_from_home = self.app_state.saved_dock_state_home.remove_room_id(room_id);
                let removed_from_spaces: usize = self.app_state.saved_dock_state_per_space
                    .values_mut()
                    .map(|saved| saved.remove_room_id(room_id))
                    .sum();
                let removed_tabs = removed_from_home + removed_from_spaces;
                let mut cleared_selected_room = false;

                if self.app_state.selected_room.as_ref().is_some_and(|selected| selected.room_id() == room_id) {
                    self.app_state.selected_room = None;
                    cleared_selected_room = true;
                }
                if removed_tabs > 0 || cleared_selected_room {
                    if let Some(user_id) = current_user_id() {
                        if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                            error!("Failed to persist app state after leaving room {room_id}. Error: {e}");
                        }
                    }
                }

                cx.action(MainDesktopUiAction::CloseRoomTabs { room_id: room_id.clone() });
                continue;
            }

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
                    self.app_state = *app_state.clone();
                    let removed_room_bindings = get_client()
                        .map(|client| {
                            self.app_state.bot_settings.remove_room_bindings_where(|room_id, _|
                                client.get_room(room_id).is_none()
                            )
                        })
                        .unwrap_or(0);
                    self.app_state.logged_in = logged_in_actual;
                    // Initialize the global translation config so RoomInputBar can access it.
                    crate::room::translation::set_global_config(&self.app_state.translation);
                    self.app_state.app_prefs.broadcast_all(cx);
                    if removed_room_bindings > 0 {
                        if let Some(user_id) = current_user_id() {
                            if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                                error!(
                                    "Failed to persist app state after pruning stale room bindings. Error: {e}"
                                );
                            }
                        }
                    }
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
                    let message = match (*bound, bot_user_id.as_ref(), warning.as_deref()) {
                        (true, Some(bot_user_id), Some(warning)) => {
                            format!("Bot {bot_user_id} is available for room {room_id}, but adding it reported a warning: {warning}")
                        }
                        (true, Some(bot_user_id), None) => {
                            format!("Added bot {bot_user_id} to room {room_id}.")
                        }
                        (false, Some(bot_user_id), Some(warning)) => {
                            format!("Removed bot {bot_user_id} from room {room_id}, with warning: {warning}")
                        }
                        (false, Some(bot_user_id), None) => {
                            format!("Removed bot {bot_user_id} from room {room_id}.")
                        }
                        (false, None, Some(warning)) => {
                            format!("Removed bot from room {room_id}, with warning: {warning}")
                        }
                        (false, None, None) => {
                            format!("Removed bot from room {room_id}.")
                        }
                        (true, None, Some(warning)) => {
                            format!("Bot is available for room {room_id}, with warning: {warning}")
                        }
                        (true, None, None) => {
                            format!("Added bot to room {room_id}.")
                        }
                    };
                    submit_async_request(MatrixRequest::SendMessage {
                        timeline_kind: TimelineKind::MainRoom { room_id: room_id.clone() },
                        message: RoomMessageEventContent::notice_plain(format!("[App Service] {message}")),
                        replied_to: None,
                        target_user_id: None,
                        explicit_room: false,
                        #[cfg(feature = "tsp")]
                        sign_with_tsp: false,
                    });
                    self.ui.redraw(cx);
                    continue;
                }
                Some(AppStateAction::KnownBotUserIdsDiscovered { bot_user_ids }) => {
                    if self
                        .app_state
                        .bot_settings
                        .record_known_bot_user_ids(bot_user_ids.iter().cloned())
                    {
                        if let Some(user_id) = current_user_id() {
                            if let Err(e) = persistence::save_app_state(self.app_state.clone(), user_id) {
                                error!("Failed to persist discovered bot user IDs. Error: {e}");
                            }
                        }
                    }
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
                        .join_leave_room_modal(cx, ids!(join_leave_modal_inner))
                        .set_kind(cx, kind.clone(), *show_tip, self.app_state.app_language);
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
            // Handle file upload modal actions
            match action.downcast_ref() {
                Some(FilePreviewerAction::Show(file_data)) => {
                    self.ui.file_upload_modal(cx, ids!(file_upload_modal_inner))
                        .set_file_data(cx, file_data.clone());
                    self.ui.modal(cx, ids!(file_upload_modal)).open(cx);
                    continue;
                }
                Some(FilePreviewerAction::Hide) | Some(FilePreviewerAction::Cancelled) => {
                    self.ui.modal(cx, ids!(file_upload_modal)).close(cx);
                    continue;
                }
                Some(FilePreviewerAction::UploadConfirmed(file_data)) => {
                    // Send the file upload event to the current room's timeline
                    if let Some(selected_room) = &self.app_state.selected_room {
                        if let Some(timeline_kind) = selected_room.timeline_kind() {
                            if let Some(sender) = get_timeline_update_sender(&timeline_kind) {
                                let _ = sender.send(TimelineUpdate::FileUploadConfirmed(file_data.clone()));
                                SignalToUI::set_ui_signal();
                            }
                        }
                    }
                    self.ui.modal(cx, ids!(file_upload_modal)).close(cx);
                    continue;
                }
                _ => {}
            }
            // Handle forward-message modal actions.
            match action.downcast_ref() {
                Some(ForwardMessageModalAction::Open(content)) => {
                    self.ui
                        .forward_message_modal(cx, ids!(forward_message_modal_inner))
                        .show(cx, (**content).clone(), self.app_state.app_language);
                    self.ui.modal(cx, ids!(forward_message_modal)).open(cx);
                    continue;
                }
                Some(ForwardMessageModalAction::Close) => {
                    self.ui.modal(cx, ids!(forward_message_modal)).close(cx);
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
                    self.ui.invite_modal(cx, ids!(invite_modal_inner)).show(cx, room_name_id.clone(), self.app_state.app_language);
                    self.ui.modal(cx, ids!(invite_modal)).open(cx); 
                    continue;
                }
                Some(InviteModalAction::Close) => {
                    mark_invite_modal_closed();
                    self.ui.modal(cx, ids!(invite_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            // Handle StdinCommandAction for manual testing.
            if let Some(StdinCommandAction(cmd)) = action.downcast_ref() {
                let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                match parts[0] {
                    "open-room-settings" => {
                        let keyword = parts.get(1).map(|s| s.trim()).unwrap_or("");
                        let rooms_list = cx.get_global::<RoomsListRef>().clone();
                        // Try: selected room → first joined room → keyword search
                        let result = if keyword.is_empty() {
                            self.app_state.selected_room
                                .as_ref()
                                .map(|sr| sr.room_id().to_owned())
                                .or_else(|| rooms_list.get_first_joined_room_id())
                        } else {
                            rooms_list.get_matching_room_items(keyword, 1)
                                .into_iter().next()
                                .map(|(rni, _)| rni.room_id().to_owned())
                        };
                        if let Some(room_id) = result {
                            log!("stdin: opening room settings for {}", room_id);
                            cx.action(RoomSettingsAction::Open { room_id });
                        } else {
                            log!("stdin: no room found for '{}'", keyword);
                        }
                    }
                    "close-room-settings" => {
                        self.ui.modal(cx, ids!(room_settings_modal)).close(cx);
                    }
                    _ => log!("stdin: unknown command '{}'", cmd),
                }
                continue;
            }

            // Handle RoomSettingsAction.
            match action.downcast_ref::<RoomSettingsAction>() {
                Some(RoomSettingsAction::Open { room_id }) => {
                    let room_id = room_id.clone();
                    let rooms_list = cx.get_global::<RoomsListRef>().clone();
                    let room_name = rooms_list.get_room_name(&room_id)
                        .map(|rni| rni.to_string())
                        .unwrap_or_else(|| room_id.as_str().to_string());
                    let canonical_alias = rooms_list.get_room_canonical_alias(&room_id);
                    let alias_str = canonical_alias.as_ref().map(|a| a.as_str());
                    log!("RoomSettingsAction::Open for {} (name: {})", room_id, room_name);
                    self.ui.room_settings_modal(cx, ids!(room_settings_modal_inner))
                        .show_settings(cx, room_id.clone(), &room_name, "", alias_str);
                    self.ui.modal(cx, ids!(room_settings_modal)).open(cx);
                    submit_async_request(MatrixRequest::FetchRoomSettings { room_id });
                    continue;
                }
                Some(RoomSettingsAction::Close) | Some(RoomSettingsAction::Cancel) => {
                    self.ui.modal(cx, ids!(room_settings_modal)).close(cx);
                    continue;
                }
                Some(RoomSettingsAction::Save { room_id, room_name, room_topic }) => {
                    submit_async_request(MatrixRequest::SetRoomName {
                        room_id: room_id.clone(),
                        name: room_name.clone(),
                    });
                    if !room_topic.is_empty() {
                        submit_async_request(MatrixRequest::SetRoomTopic {
                            room_id: room_id.clone(),
                            topic: room_topic.clone(),
                        });
                    }
                    enqueue_popup_notification("Saving room settings…", PopupKind::Info, Some(3.0));
                    self.ui.modal(cx, ids!(room_settings_modal)).close(cx);
                    continue;
                }
                Some(RoomSettingsAction::LeaveRoom { room_id }) => {
                    let room_id = room_id.clone();
                    let rooms_list = cx.get_global::<RoomsListRef>().clone();
                    let room_name_id = rooms_list.get_room_name(&room_id)
                        .unwrap_or_else(|| RoomNameId::from(
                            (matrix_sdk::RoomDisplayName::Empty, room_id.clone())
                        ));
                    cx.action(JoinLeaveRoomModalAction::Open {
                        kind: JoinLeaveModalKind::LeaveRoom(BasicRoomDetails::Name(room_name_id)),
                        show_tip: false,
                    });
                    self.ui.modal(cx, ids!(room_settings_modal)).close(cx);
                    continue;
                }
                Some(RoomSettingsAction::AddLocalAddress { .. }) => {
                    enqueue_popup_notification("Address management coming soon", PopupKind::Info, Some(3.0));
                    continue;
                }
                Some(RoomSettingsAction::SetDirectoryPublish { .. }) => {
                    enqueue_popup_notification("Directory publish coming soon", PopupKind::Info, Some(3.0));
                    continue;
                }
                Some(RoomSettingsAction::UploadRoomAvatar { room_id, avatar_path }) => {
                    submit_async_request(MatrixRequest::UploadRoomAvatar {
                        room_id: room_id.clone(),
                        avatar_path: avatar_path.clone(),
                    });
                    enqueue_popup_notification("Uploading room avatar…", PopupKind::Info, Some(3.0));
                    continue;
                }
                Some(RoomSettingsAction::SetMediaVisibility { .. }) | Some(RoomSettingsAction::None) => {
                    continue;
                }
                None => {}
            }

            // Handle RoomSettingsFetchedAction.
            if let Some(fetched) = action.downcast_ref::<RoomSettingsFetchedAction>() {
                self.ui.room_settings_modal(cx, ids!(room_settings_modal_inner))
                    .apply_fetched_settings(cx, fetched.topic.clone(), fetched.is_public);
                continue;
            }

            // Handle RoomAvatarUploadedAction — refresh the avatar widget.
            if let Some(uploaded) = action.downcast_ref::<RoomAvatarUploadedAction>() {
                self.ui.room_settings_modal(cx, ids!(room_settings_modal_inner))
                    .apply_avatar(cx, &uploaded.image_data);
                continue;
            }

            // Handle RoomContextMenuAction::OpenRoomSettings.
            if let Some(RoomContextMenuAction::OpenRoomSettings(room_id)) = action.downcast_ref::<RoomContextMenuAction>() {
                cx.action(RoomSettingsAction::Open { room_id: room_id.clone() });
                continue;
            }

            // Handle BotBindingModalAction to open/close the bot binding modal.
            match action.downcast_ref() {
                Some(BotBindingModalAction::Open(room_name_id)) => {
                    self.ui
                        .bot_binding_modal(cx, ids!(bot_binding_modal_inner))
                        .show(
                            cx,
                            room_name_id.clone(),
                            &self.app_state.bot_settings,
                            self.app_state.app_language,
                        );
                    self.ui.modal(cx, ids!(bot_binding_modal)).open(cx);
                    continue;
                }
                Some(BotBindingModalAction::Close) => {
                    self.ui.modal(cx, ids!(bot_binding_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(CreateRoomModalAction::Open { parent_space_id }) => {
                    self.ui.create_room_modal(cx, ids!(create_room_modal_inner)).show(cx, parent_space_id.clone());
                    self.ui.modal(cx, ids!(create_room_modal)).open(cx);
                    continue;
                }
                Some(CreateRoomModalAction::Close) => {
                    self.ui.modal(cx, ids!(create_room_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            match action.downcast_ref() {
                Some(StartChatModalAction::Open) => {
                    self.ui.start_chat_modal(cx, ids!(start_chat_modal_inner)).show(cx);
                    self.ui.modal(cx, ids!(start_chat_modal)).open(cx);
                    continue;
                }
                Some(StartChatModalAction::Close) => {
                    self.ui.modal(cx, ids!(start_chat_modal)).close(cx);
                    continue;
                }
                _ => {}
            }

            // Handle EventSourceModalAction to open/close the event source modal.
            match action.downcast_ref() {
                Some(EventSourceModalAction::Open { room_id, event_id, latest_json }) => {
                    self.ui.event_source_modal(cx, ids!(event_source_modal_inner))
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
                Some(DirectMessageRoomAction::FoundExisting { user_id, room_name_id }) => {
                    self.app_state.bot_settings.bind_dm_target_if_needed(
                        room_name_id.room_id().to_owned(),
                        user_id.as_ref(),
                        current_user_id().as_deref(),
                    );
                    self.navigate_to_room(cx, None, &BasicRoomDetails::RoomId(room_name_id.clone()));
                }
                Some(DirectMessageRoomAction::DidNotExist { user_profile }) => {
                    let user_profile = user_profile.clone();
                    let create_encrypted = self.app_state.bot_settings.should_create_encrypted_dm(
                        user_profile.user_id.as_ref(),
                        current_user_id().as_deref(),
                    );
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
                                    create_encrypted,
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
                Some(DirectMessageRoomAction::NewlyCreated { user_profile, room_name_id }) => {
                    self.app_state.bot_settings.bind_dm_target_if_needed(
                        room_name_id.room_id().to_owned(),
                        user_profile.user_id.as_ref(),
                        current_user_id().as_deref(),
                    );
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
        crate::home::upload_progress::script_mod(vm);
        crate::room::script_mod(vm);
        crate::join_leave_room_modal::script_mod(vm);
        crate::verification_modal::script_mod(vm);
        crate::profile::script_mod(vm);
        crate::home::script_mod(vm);
        crate::login::script_mod(vm);
        crate::register::script_mod(vm);
        crate::logout::script_mod(vm);

        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::LiveEdit = event {
            self.app_state.app_prefs.broadcast_all(cx);
        }
        if let Event::WindowGeomChange(_) = event {
            if !self.app_state.app_prefs.ui_zoom.is_default() {
                self.app_state.app_prefs.on_ui_zoom_changed(cx);
            }
        }

        self.handle_ui_zoom_shortcuts(cx, event);

        // Forward events to the MatchEvent trait implementation.
        self.match_event(cx, event);
        let scope = &mut Scope::with_data(&mut self.app_state);
        self.ui.handle_event(cx, event, scope);
        self.handle_lifecycle_event(cx, event);
    }
}

impl App {
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

    fn start_auto_update_check(&mut self, cx: &mut Cx) {
        if self.auto_update_check_started {
            return;
        }
        self.auto_update_check_started = true;
        cx.spawn_thread(move || {
            let result = check_for_updates();
            Cx::post_action(AppUpdateAction::AutoCheckFinished(result));
        });
    }

    fn show_update_prompt_if_needed(
        &mut self,
        cx: &mut Cx,
        current_version: &str,
        latest_version: &str,
        from_auto_check: bool,
    ) {
        if from_auto_check
            && self.skipped_update_version
                .as_deref()
                .is_some_and(|skipped_version| skipped_version == latest_version)
        {
            return;
        }

        self.update_prompt_versions = Some((current_version.to_owned(), latest_version.to_owned()));
        self.ui
            .label(cx, ids!(update_available_modal_inner.update_available_title))
            .set_text(cx, tr_key(self.app_state.app_language, "settings.update.modal.title"));
        self.ui
            .label(cx, ids!(update_available_modal_inner.update_available_body))
            .set_text(
                cx,
                &tr_fmt(self.app_state.app_language, "settings.update.modal.body", &[
                    ("latest", latest_version),
                    ("current", current_version),
                ]),
            );
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_skip_button))
            .set_text(cx, tr_key(self.app_state.app_language, "settings.update.modal.button.skip"));
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_cancel_button))
            .set_text(cx, tr_key(self.app_state.app_language, "settings.update.modal.button.cancel"));
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_upgrade_button))
            .set_text(cx, tr_key(self.app_state.app_language, "settings.update.modal.button.upgrade"));
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_skip_button))
            .reset_hover(cx);
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_cancel_button))
            .reset_hover(cx);
        self.ui
            .button(cx, ids!(update_available_modal_inner.update_upgrade_button))
            .reset_hover(cx);
        self.ui.modal(cx, ids!(update_available_modal)).open(cx);
    }

    fn sync_app_language(&self, cx: &mut Cx) {
        let app_language = self.app_state.app_language;
        self.ui.label(cx, ids!(room_filter_modal_inner.search_results_title))
            .set_text(cx, tr_key(app_language, "app.room_filter.search_results_title"));
        self.ui.label(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_empty))
            .set_text(cx, tr_key(app_language, "app.room_filter.empty_hint"));
        self.ui.button(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.remote_search_options.remote_search_people_button))
            .set_text(cx, tr_key(app_language, "app.room_filter.remote.people"));
        self.ui.button(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.remote_search_options.remote_search_rooms_button))
            .set_text(cx, tr_key(app_language, "app.room_filter.remote.rooms"));
        self.ui.button(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.remote_search_options.remote_search_spaces_button))
            .set_text(cx, tr_key(app_language, "app.room_filter.remote.spaces"));
    }

    fn open_join_from_search_result(
        &mut self,
        cx: &mut Cx,
        details: BasicRoomDetails,
        is_space: bool,
    ) {
        cx.action(JoinLeaveRoomModalAction::Open {
            kind: JoinLeaveModalKind::JoinRoom {
                details,
                is_space,
            },
            show_tip: false,
        });
    }

    fn update_login_visibility(&self, cx: &mut Cx) {
        let show_login = self.app_state.adding_account || self.auth_ui_state == AuthUiState::LoggedOut;
        let show_home = self.auth_ui_state != AuthUiState::LoggedOut;
        if !show_login {
            self.ui
                .modal(cx, ids!(login_screen_view.login_screen.login_status_modal))
                .close(cx);
        }
        self.ui.view(cx, ids!(login_screen_view)).set_visible(cx, show_login);
        self.ui.view(cx, ids!(home_screen_view)).set_visible(cx, show_home);
    }

    fn clicked_room_filter_remote_option(&self, cx: &mut Cx, actions: &Actions) -> Option<RemoteDirectorySearchKind> {
        let options_view = self.ui.view(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.remote_search_options));
        if options_view.button(cx, ids!(remote_search_people_button)).clicked(actions) {
            return Some(RemoteDirectorySearchKind::People);
        }
        if options_view.button(cx, ids!(remote_search_rooms_button)).clicked(actions) {
            return Some(RemoteDirectorySearchKind::Rooms);
        }
        if options_view.button(cx, ids!(remote_search_spaces_button)).clicked(actions) {
            return Some(RemoteDirectorySearchKind::Spaces);
        }
        None
    }

    fn clicked_mobile_room_info_button(&self, cx: &mut Cx, actions: &Actions) -> Option<LiveId> {
        for (view_id, room_screen_id) in Self::ROOM_VIEW_IDS.iter().zip(Self::ROOM_SCREEN_IDS.iter()) {
            let button_path = &[
                *view_id,
                live_id!(header),
                live_id!(content),
                live_id!(button_container),
                live_id!(right_button),
            ];
            if self.ui.button(cx, button_path).clicked(actions) {
                return Some(*room_screen_id);
            }
        }
        None
    }

    fn set_room_filter_modal_empty_state(
        &self,
        cx: &mut Cx,
        text: &str,
        show_remote_options: bool,
    ) {
        let empty_label = self.ui.label(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_empty));
        empty_label.set_visible(cx, !text.is_empty());
        if !text.is_empty() {
            empty_label.set_text(cx, text);
        }
        self.ui.view(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.remote_search_options))
            .set_visible(cx, show_remote_options);
    }

    fn update_room_filter_modal_results(&mut self, cx: &mut Cx, keywords: &str) {
        let keywords = keywords.trim();
        let mut results = Vec::new();

        if !keywords.is_empty() {
            let space_items = cx.get_global::<SpacesBarRef>()
                .get_matching_space_items(keywords, 4);
            let room_items = cx.get_global::<RoomsListRef>()
                .get_matching_room_items(keywords, 12);

            for (room_name_id, avatar) in space_items {
                results.push(RoomFilterResultTarget::LocalSpace { room_name_id, avatar });
            }
            for (room_name_id, avatar) in room_items {
                results.push(RoomFilterResultTarget::LocalRoom { room_name_id, avatar });
            }
        }

        if keywords.is_empty() {
            self.set_room_filter_modal_empty_state(
                cx,
                tr_key(self.app_state.app_language, "app.room_filter.empty_hint"),
                false,
            );
        } else if results.is_empty() {
            self.set_room_filter_modal_empty_state(
                cx,
                &tr_fmt(
                    self.app_state.app_language,
                    "app.room_filter.no_local_results",
                    &[("keywords", keywords)],
                ),
                true,
            );
        } else {
            self.set_room_filter_modal_empty_state(cx, "", false);
        }

        let search_results_list = self.ui.room_filter_search_results_list(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list));
        search_results_list.set_results(cx, results);
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
        if self.app_state.selected_room.as_ref().is_some_and(|current| current == &selected_room) {
            return;
        }

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
        let right_button_path = &[view_id, live_id!(header), live_id!(content), live_id!(button_container), live_id!(right_button)];
        let show_info_button = matches!(
            selected_room,
            SelectedRoom::JoinedRoom { .. }
            | SelectedRoom::Thread { .. }
        );
        let right_button = self.ui.button(cx, right_button_path);
        right_button.set_visible(cx, show_info_button);
        if show_info_button {
            right_button.set_text(cx, "");
            right_button.reset_hover(cx);
        }

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
#[serde(default)]
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
    /// The preferred app language.
    pub app_language: AppLanguage,
    /// App-wide UI/behavior preferences.
    #[serde(default)]
    pub app_prefs: AppPreferences,
    /// Whether the app is currently showing the login screen for adding another account.
    /// This is transient state and not persisted.
    #[serde(skip)]
    pub adding_account: bool,
    /// Local configuration and UI state for bot-assisted room binding.
    pub bot_settings: BotSettingsState,
    /// Translation API configuration.
    #[serde(default)]
    pub translation: crate::room::translation::TranslationConfig,
}

/// Local bot integration settings persisted per Matrix account.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BotSettingsState {
    /// Whether bot-assisted room binding is enabled in the UI.
    pub enabled: bool,
    /// The configured botfather user, either as a full MXID or localpart.
    pub botfather_user_id: String,
    /// The Octos service base URL used for health checks.
    pub octos_service_url: String,
    /// Bots discovered from BotFather `/listbots` replies.
    pub known_bot_user_ids: Vec<OwnedUserId>,
    /// Rooms that Robrix currently considers bot-bound,
    /// paired with the exact bot MXID used for that room.
    pub room_bindings: Vec<RoomBotBindingState>,
}

/// A persisted room-level bot binding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomBotBindingState {
    pub room_id: OwnedRoomId,
    pub bot_user_id: OwnedUserId,
    #[serde(default)]
    pub remark: String,
}

impl Default for BotSettingsState {
    fn default() -> Self {
        Self {
            enabled: false,
            botfather_user_id: Self::DEFAULT_BOTFATHER_LOCALPART.to_string(),
            octos_service_url: Self::DEFAULT_OCTOS_SERVICE_URL.to_string(),
            known_bot_user_ids: Vec::new(),
            room_bindings: Vec::new(),
        }
    }
}

impl BotSettingsState {
    pub const DEFAULT_BOTFATHER_LOCALPART: &'static str = "bot";
    pub const DEFAULT_OCTOS_SERVICE_URL: &'static str = "http://127.0.0.1:8010";

    pub fn resolved_octos_service_url(&self) -> &str {
        let raw = self.octos_service_url.trim();
        if raw.is_empty() {
            Self::DEFAULT_OCTOS_SERVICE_URL
        } else {
            raw
        }
    }

    pub fn validate_octos_service_url(service_url: &str) -> Result<(), String> {
        let service_url = service_url.trim();
        if service_url.is_empty() {
            return Err("Octos service URL cannot be empty.".into());
        }

        let parsed_url = Url::parse(service_url)
            .map_err(|e| format!("Invalid Octos service URL: {e}"))?;

        match parsed_url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(format!(
                    "Unsupported Octos service URL scheme `{scheme}`. Use http or https."
                ));
            }
        }

        if parsed_url.host_str().is_none() {
            return Err("Octos service URL must include a host.".into());
        }

        Ok(())
    }

    pub fn validate_botfather_user_id(
        botfather_user_id: &str,
        current_user_id: Option<&UserId>,
    ) -> Result<(), String> {
        let botfather_user_id = botfather_user_id.trim();
        if botfather_user_id.is_empty() {
            return Err("BotFather user ID cannot be empty.".into());
        }

        Self {
            botfather_user_id: botfather_user_id.to_string(),
            ..Self::default()
        }
        .resolved_bot_user_id(current_user_id)
        .map(|_| ())
    }

    fn room_binding_index(
        &self,
        room_id: &RoomId,
        bot_user_id: &UserId,
    ) -> Result<usize, usize> {
        self.room_bindings
            .binary_search_by(|binding|
                (
                    binding.room_id.as_str(),
                    binding.bot_user_id.as_str(),
                ).cmp(&(room_id.as_str(), bot_user_id.as_str()))
            )
    }

    fn room_binding_range(&self, room_id: &RoomId) -> std::ops::Range<usize> {
        let start = self
            .room_bindings
            .partition_point(|binding| binding.room_id.as_str() < room_id.as_str());
        let end = self
            .room_bindings
            .iter()
            .skip(start)
            .position(|binding| binding.room_id.as_str() != room_id.as_str())
            .map_or(self.room_bindings.len(), |offset| start + offset);
        start..end
    }

    /// Returns `true` if the given room is currently marked as bound locally.
    pub fn is_room_bound(&self, room_id: &RoomId) -> bool {
        !self.bound_bot_user_ids(room_id).is_empty()
    }

    /// Returns the persisted BotFather MXID for the given room, if any.
    pub fn bound_bot_user_id(&self, room_id: &RoomId) -> Option<&UserId> {
        let room_binding_range = self.room_binding_range(room_id);
        self.room_bindings
            .get(room_binding_range.start)
            .map(|binding| binding.bot_user_id.as_ref())
    }

    /// Returns all persisted bot MXIDs for the given room.
    pub fn bound_bot_user_ids(&self, room_id: &RoomId) -> Vec<OwnedUserId> {
        self.room_bindings[self.room_binding_range(room_id)]
            .iter()
            .map(|binding| binding.bot_user_id.clone())
            .collect()
    }

    /// Returns all bot bindings for the given room.
    pub fn room_bindings_for(&self, room_id: &RoomId) -> Vec<RoomBotBindingState> {
        self.room_bindings[self.room_binding_range(room_id)]
            .to_vec()
    }

    /// Returns all known bound bot MXIDs across every room, deduplicated.
    pub fn all_bound_bot_user_ids(&self) -> Vec<OwnedUserId> {
        let mut all_bots = self
            .room_bindings
            .iter()
            .map(|binding| binding.bot_user_id.clone())
            .collect::<Vec<_>>();
        all_bots.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        all_bots.dedup_by(|a, b| a.as_str() == b.as_str());
        all_bots
    }

    /// Returns bot MXIDs discovered from BotFather `/listbots` replies.
    pub fn known_bot_user_ids(&self) -> Vec<OwnedUserId> {
        self.known_bot_user_ids.clone()
    }

    /// Merges the given discovered bot IDs into the known bot list.
    ///
    /// Returns `true` if the list changed.
    pub fn record_known_bot_user_ids(
        &mut self,
        discovered_bot_user_ids: impl IntoIterator<Item = OwnedUserId>,
    ) -> bool {
        let mut changed = false;
        for bot_user_id in discovered_bot_user_ids {
            if !self
                .known_bot_user_ids
                .iter()
                .any(|existing| existing.as_str() == bot_user_id.as_str())
            {
                self.known_bot_user_ids.push(bot_user_id);
                changed = true;
            }
        }
        if changed {
            self.known_bot_user_ids
                .sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
            self.known_bot_user_ids
                .dedup_by(|lhs, rhs| lhs.as_str() == rhs.as_str());
        }
        changed
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
            match self.room_binding_index(room_id.as_ref(), bot_user_id.as_ref()) {
                Ok(_) => {}
                Err(insert_index) => {
                    self.room_bindings.insert(insert_index, RoomBotBindingState {
                        room_id,
                        bot_user_id,
                        remark: String::new(),
                    });
                }
            }
        } else {
            if let Some(bot_user_id) = bot_user_id {
                if let Ok(existing_index) = self.room_binding_index(room_id.as_ref(), bot_user_id.as_ref()) {
                    self.room_bindings.remove(existing_index);
                }
            } else {
                self.room_bindings.retain(|binding| binding.room_id != room_id);
            }
        }
    }

    /// Auto-binds a DM room when it targets the configured app-service bot or a known bot.
    ///
    /// Returns `true` if a bot binding should exist for this room/target pair.
    pub fn bind_dm_target_if_needed(
        &mut self,
        room_id: OwnedRoomId,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        let matches_configured_bot = self
            .resolved_bot_user_id(current_user_id)
            .ok()
            .is_some_and(|configured_bot_user_id|
                configured_bot_user_id.as_str() == target_user_id.as_str()
            );
        let matches_known_bot = self
            .known_bot_user_ids
            .iter()
            .any(|known_bot_user_id| known_bot_user_id.as_str() == target_user_id.as_str());

        if !(matches_configured_bot || matches_known_bot) {
            return false;
        }

        self.set_room_bound(room_id, Some(target_user_id.to_owned()), true);
        true
    }

    /// Updates the remark for a specific room bot binding.
    ///
    /// Returns `true` if a binding existed and was updated.
    pub fn set_room_bot_remark(
        &mut self,
        room_id: &RoomId,
        bot_user_id: &UserId,
        remark: String,
    ) -> bool {
        if let Ok(index) = self.room_binding_index(room_id, bot_user_id) {
            self.room_bindings[index].remark = remark;
            true
        } else {
            false
        }
    }

    pub fn remove_room_bindings_where(
        &mut self,
        mut predicate: impl FnMut(&RoomId, &UserId) -> bool,
    ) -> usize {
        let original_len = self.room_bindings.len();
        self.room_bindings
            .retain(|binding| !predicate(binding.room_id.as_ref(), binding.bot_user_id.as_ref()));
        original_len.saturating_sub(self.room_bindings.len())
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

    /// Returns `true` if new DM rooms for this target user should be encrypted.
    ///
    /// New DM rooms are always created unencrypted so appservice bots can
    /// receive and reply to messages without E2EE support.
    pub fn should_create_encrypted_dm(
        &self,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        let _ = (target_user_id, current_user_id);
        false
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

    /// Returns the `TimelineKind` for this selected room.
    ///
    /// Returns `None` for `InvitedRoom` and `Space` variants, as they don't have timelines.
    pub fn timeline_kind(&self) -> Option<TimelineKind> {
        match self {
            SelectedRoom::JoinedRoom { room_name_id } => {
                Some(TimelineKind::MainRoom {
                    room_id: room_name_id.room_id().clone(),
                })
            }
            SelectedRoom::Thread { room_name_id, thread_root_event_id } => {
                Some(TimelineKind::Thread {
                    room_id: room_name_id.room_id().clone(),
                    thread_root_event_id: thread_root_event_id.clone(),
                })
            }
            SelectedRoom::InvitedRoom { .. } | SelectedRoom::Space { .. } => None,
        }
    }
}

impl SavedDockState {
    /// Removes all tabs and selection state that belong to the given room ID.
    ///
    /// Returns the number of removed open tabs, including thread tabs tied to the room.
    pub fn remove_room_id(&mut self, room_id: &RoomId) -> usize {
        let tab_ids_to_remove: Vec<LiveId> = self.open_rooms.iter()
            .filter_map(|(tab_id, selected_room)| (selected_room.room_id() == room_id).then_some(*tab_id))
            .collect();

        let room_order_matches = self.room_order.iter()
            .any(|selected_room| selected_room.room_id() == room_id);
        let selected_room_matches = self.selected_room.as_ref()
            .is_some_and(|selected_room| selected_room.room_id() == room_id);

        if tab_ids_to_remove.is_empty() && !room_order_matches && !selected_room_matches {
            return 0;
        }

        for tab_id in &tab_ids_to_remove {
            self.open_rooms.remove(tab_id);
            self.dock_items.remove(tab_id);
        }

        self.room_order.retain(|selected_room| selected_room.room_id() != room_id);

        if selected_room_matches {
            self.selected_room = self.room_order.last().cloned();
        }

        tab_ids_to_remove.len()
    }

    /// Removes all rooms for which `should_remove` returns `true`.
    ///
    /// Returns the number of removed open tabs, including thread tabs tied to removed rooms.
    pub fn remove_room_ids_where<F>(&mut self, mut should_remove: F) -> usize
    where
        F: FnMut(&OwnedRoomId) -> bool,
    {
        let mut room_ids: Vec<OwnedRoomId> = self.open_rooms.values()
            .map(|selected_room| selected_room.room_id().clone())
            .collect();
        room_ids.extend(self.room_order.iter().map(|selected_room| selected_room.room_id().clone()));
        if let Some(selected_room) = self.selected_room.as_ref() {
            room_ids.push(selected_room.room_id().clone());
        }
        room_ids.sort();
        room_ids.dedup();

        room_ids.into_iter()
            .filter(|room_id| should_remove(room_id))
            .map(|room_id| self.remove_room_id(&room_id))
            .sum()
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

#[cfg(test)]
mod tests {
    use super::{AppState, BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};
    use crate::utils::RoomNameId;
    use matrix_sdk::{RoomDisplayName, ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, UserId}};

    fn joined_room(room_id_str: &str, name: &str) -> SelectedRoom {
        SelectedRoom::JoinedRoom {
            room_name_id: RoomNameId::new(
                RoomDisplayName::Named(name.into()),
                room_id_str.parse::<OwnedRoomId>().unwrap(),
            ),
        }
    }

    fn thread_room(room_id_str: &str, name: &str, event_id_str: &str) -> SelectedRoom {
        SelectedRoom::Thread {
            room_name_id: RoomNameId::new(
                RoomDisplayName::Named(name.into()),
                room_id_str.parse::<OwnedRoomId>().unwrap(),
            ),
            thread_root_event_id: event_id_str.parse::<OwnedEventId>().unwrap(),
        }
    }

    #[test]
    fn remove_room_id_removes_main_and_thread_tabs() {
        let joined = joined_room("!room:example.org", "octosbot");
        let thread = thread_room("!room:example.org", "octosbot", "$thread:example.org");
        let other = joined_room("!other:example.org", "other");
        let removed_room_id = joined.room_id().to_owned();
        let joined_tab = joined.tab_id();
        let thread_tab = thread.tab_id();
        let other_tab = other.tab_id();

        let mut saved = SavedDockState {
            dock_items: [
                (joined_tab, Default::default()),
                (thread_tab, Default::default()),
                (other_tab, Default::default()),
            ].into_iter().collect(),
            open_rooms: [
                (joined_tab, joined.clone()),
                (thread_tab, thread.clone()),
                (other_tab, other.clone()),
            ].into_iter().collect(),
            room_order: vec![joined, thread, other.clone()],
            selected_room: Some(thread_room("!room:example.org", "octosbot", "$thread:example.org")),
        };

        assert_eq!(saved.remove_room_id(&removed_room_id), 2);
        assert_eq!(saved.open_rooms.len(), 1);
        assert!(saved.open_rooms.contains_key(&other_tab));
        assert!(saved.dock_items.contains_key(&other_tab));
        assert!(!saved.dock_items.contains_key(&joined_tab));
        assert!(!saved.dock_items.contains_key(&thread_tab));
        assert_eq!(saved.room_order, vec![other.clone()]);
        assert_eq!(saved.selected_room, Some(other));
    }

    #[test]
    fn remove_room_id_is_noop_for_unknown_room() {
        let room = joined_room("!room:example.org", "octosbot");
        let tab_id = room.tab_id();
        let mut saved = SavedDockState {
            dock_items: [(tab_id, Default::default())].into_iter().collect(),
            open_rooms: [(tab_id, room.clone())].into_iter().collect(),
            room_order: vec![room.clone()],
            selected_room: Some(room.clone()),
        };

        assert_eq!(saved.remove_room_id(&"!missing:example.org".parse::<OwnedRoomId>().unwrap()), 0);
        assert_eq!(saved.open_rooms.len(), 1);
        assert_eq!(saved.room_order, vec![room.clone()]);
        assert_eq!(saved.selected_room, Some(room));
    }

    #[test]
    fn remove_room_id_clears_selected_room_even_without_open_tab() {
        let room = joined_room("!room:example.org", "octosbot");
        let other = joined_room("!other:example.org", "other");
        let mut saved = SavedDockState {
            dock_items: Default::default(),
            open_rooms: Default::default(),
            room_order: vec![other.clone()],
            selected_room: Some(room),
        };

        assert_eq!(saved.remove_room_id(&"!room:example.org".parse::<OwnedRoomId>().unwrap()), 0);
        assert_eq!(saved.room_order, vec![other.clone()]);
        assert_eq!(saved.selected_room, Some(other));
    }

    #[test]
    fn remove_room_ids_where_prunes_stale_rooms_from_all_state() {
        let stale_joined = joined_room("!stale:example.org", "octosbot");
        let stale_thread = thread_room("!stale:example.org", "octosbot", "$thread:example.org");
        let fresh = joined_room("!fresh:example.org", "fresh");
        let fresh_tab = fresh.tab_id();
        let stale_joined_tab = stale_joined.tab_id();
        let stale_thread_tab = stale_thread.tab_id();
        let mut saved = SavedDockState {
            dock_items: [
                (stale_joined_tab, Default::default()),
                (stale_thread_tab, Default::default()),
                (fresh_tab, Default::default()),
            ].into_iter().collect(),
            open_rooms: [
                (stale_joined_tab, stale_joined.clone()),
                (stale_thread_tab, stale_thread.clone()),
                (fresh_tab, fresh.clone()),
            ].into_iter().collect(),
            room_order: vec![stale_joined, stale_thread, fresh.clone()],
            selected_room: Some(fresh.clone()),
        };

        assert_eq!(
            saved.remove_room_ids_where(|room_id| room_id.as_str() == "!stale:example.org"),
            2
        );
        assert_eq!(saved.open_rooms, [(fresh_tab, fresh.clone())].into_iter().collect());
        assert_eq!(saved.room_order, vec![fresh.clone()]);
        assert_eq!(saved.selected_room, Some(fresh));
    }

    #[test]
    fn validate_botfather_user_id_accepts_localpart_and_full_mxid() {
        let current_user_id = UserId::parse("@alex:example.org").unwrap();

        assert!(BotSettingsState::validate_botfather_user_id(
            "octosbot",
            Some(current_user_id.as_ref()),
        ).is_ok());
        assert!(BotSettingsState::validate_botfather_user_id(
            "@octosbot:example.org",
            Some(current_user_id.as_ref()),
        ).is_ok());
        assert!(BotSettingsState::validate_botfather_user_id(
            "",
            Some(current_user_id.as_ref()),
        ).is_err());
    }

    #[test]
    fn remove_room_bindings_where_prunes_stale_bindings() {
        let mut settings = BotSettingsState {
            room_bindings: vec![
                RoomBotBindingState {
                    room_id: "!stale:example.org".parse::<OwnedRoomId>().unwrap(),
                    bot_user_id: "@octosbot:example.org".parse::<OwnedUserId>().unwrap(),
                    remark: String::new(),
                },
                RoomBotBindingState {
                    room_id: "!fresh:example.org".parse::<OwnedRoomId>().unwrap(),
                    bot_user_id: "@octosbot:example.org".parse::<OwnedUserId>().unwrap(),
                    remark: String::new(),
                },
            ],
            ..BotSettingsState::default()
        };

        let removed = settings.remove_room_bindings_where(|room_id, _| room_id.as_str() == "!stale:example.org");

        assert_eq!(removed, 1);
        assert_eq!(
            settings.room_bindings,
            vec![RoomBotBindingState {
                room_id: "!fresh:example.org".parse::<OwnedRoomId>().unwrap(),
                bot_user_id: "@octosbot:example.org".parse::<OwnedUserId>().unwrap(),
                remark: String::new(),
            }]
        );
    }

    // Regression guard for issue #94: on mobile, force-quit + relaunch previously lost the
    // App Service binding because handle_load_app_state gated RestoreAppStateFromPersistentState
    // behind a non-empty dock-state check. The production fix removes that guard. This test
    // protects the underlying serde contract so a future #[serde(skip)] on bot_settings (or a
    // breaking field rename) is caught at `cargo test` time instead of at Android runtime.
    #[test]
    fn test_app_state_roundtrip_preserves_bot_settings_with_empty_dock() {
        let mut state = AppState::default();
        state.bot_settings.enabled = true;
        state.bot_settings.botfather_user_id = "@octosbot:example.com".to_string();
        state.bot_settings.octos_service_url = "http://192.168.5.12:8010".to_string();
        assert!(
            state.saved_dock_state_home.open_rooms.is_empty(),
            "precondition: this test simulates the mobile / fresh-desktop case with empty dock",
        );
        assert!(
            state.saved_dock_state_home.dock_items.is_empty(),
            "precondition: this test simulates the mobile / fresh-desktop case with empty dock",
        );

        let serialized =
            serde_json::to_string(&state).expect("AppState must serialize via serde_json");
        let deserialized: AppState =
            serde_json::from_str(&serialized).expect("serialized AppState must deserialize back");

        assert!(
            deserialized.bot_settings.enabled,
            "bot_settings.enabled must survive the round-trip (issue #94 regression guard)",
        );
        assert_eq!(
            deserialized.bot_settings.botfather_user_id,
            "@octosbot:example.com",
            "botfather_user_id must survive the round-trip (issue #94 regression guard)",
        );
        assert_eq!(
            deserialized.bot_settings.octos_service_url,
            "http://192.168.5.12:8010",
            "octos_service_url must survive the round-trip (issue #94 regression guard)",
        );
    }

    #[test]
    fn test_app_state_roundtrip_preserves_selected_room_with_empty_dock() {
        let state = AppState {
            selected_room: Some(joined_room("!room:example.org", "octosbot")),
            ..Default::default()
        };
        assert!(
            state.saved_dock_state_home.open_rooms.is_empty(),
            "precondition: this test simulates the mobile case where selected_room persists without desktop dock tabs",
        );
        assert!(
            state.saved_dock_state_home.dock_items.is_empty(),
            "precondition: this test simulates the mobile case where selected_room persists without desktop dock tabs",
        );

        let serialized =
            serde_json::to_string(&state).expect("AppState must serialize via serde_json");
        let deserialized: AppState =
            serde_json::from_str(&serialized).expect("serialized AppState must deserialize back");

        assert_eq!(
            deserialized.selected_room,
            Some(joined_room("!room:example.org", "octosbot")),
            "selected_room must survive the round-trip even when dock state is empty",
        );
    }

    #[test]
    fn dm_target_matching_configured_bot_auto_binds_new_room() {
        let current_user_id = UserId::parse("@alice:example.org").unwrap();
        let bot_user_id = UserId::parse("@octosbot:example.org").unwrap();
        let room_id = "!dm:example.org".parse::<OwnedRoomId>().unwrap();
        let mut settings = BotSettingsState {
            enabled: true,
            botfather_user_id: "octosbot".into(),
            ..BotSettingsState::default()
        };

        let auto_bound = settings.bind_dm_target_if_needed(
            room_id.clone(),
            bot_user_id.as_ref(),
            Some(current_user_id.as_ref()),
        );

        assert!(auto_bound);
        assert_eq!(
            settings.bound_bot_user_ids(room_id.as_ref()),
            vec![bot_user_id.to_owned()]
        );
    }

    #[test]
    fn ordinary_dm_target_does_not_auto_bind_new_room() {
        let current_user_id = UserId::parse("@alice:example.org").unwrap();
        let ordinary_user_id = UserId::parse("@bob:example.org").unwrap();
        let room_id = "!dm:example.org".parse::<OwnedRoomId>().unwrap();
        let mut settings = BotSettingsState {
            enabled: true,
            botfather_user_id: "octosbot".into(),
            ..BotSettingsState::default()
        };

        let auto_bound = settings.bind_dm_target_if_needed(
            room_id.clone(),
            ordinary_user_id.as_ref(),
            Some(current_user_id.as_ref()),
        );

        assert!(!auto_bound);
        assert!(settings.bound_bot_user_ids(room_id.as_ref()).is_empty());
    }
}

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
    RestoreAppStateFromPersistentState(Box<AppState>),
    /// A room-level BotFather bind or unbind action completed.
    BotRoomBindingUpdated {
        room_id: OwnedRoomId,
        bound: bool,
        bot_user_id: Option<OwnedUserId>,
        warning: Option<String>,
    },
    /// Bot IDs discovered from BotFather replies (for example, `/listbots`).
    KnownBotUserIdsDiscovered {
        bot_user_ids: Vec<OwnedUserId>,
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

/// Actions related to application updates.
///
/// These are *NOT* widget actions.
#[derive(Debug)]
pub enum AppUpdateAction {
    /// Result of the background update check triggered automatically on startup.
    AutoCheckFinished(UpdateCheckOutcome),
    /// Request to show the update prompt modal.
    ShowUpdatePrompt {
        current_version: String,
        latest_version: String,
        from_auto_check: bool,
    },
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
