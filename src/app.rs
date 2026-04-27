//! The top-level application content.
//!
//! See `handle_startup()` for the first code that runs on app startup.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{fs::{File, OpenOptions}, io::Write, sync::Mutex};
use std::{cell::RefCell, collections::HashMap};
use makepad_widgets::*;
use matrix_sdk::{RoomState, ruma::{OwnedEventId, OwnedMxcUri, OwnedRoomId, OwnedUserId, RoomId, UserId, events::room::message::RoomMessageEventContent}};
use serde::{Deserialize, Serialize};
use url::Url;
use crate::{
    avatar_cache::{self, AvatarCacheEntry, clear_avatar_cache}, home::{
        add_room::{CreateRoomModalAction, CreateRoomModalWidgetRefExt, StartChatModalAction, StartChatModalWidgetRefExt},
        bot_binding_modal::{BotBindingModalAction, BotBindingModalWidgetRefExt},
        event_source_modal::{EventSourceModalAction, EventSourceModalWidgetRefExt}, invite_modal::{InviteModalAction, InviteModalWidgetRefExt, mark_invite_modal_closed}, invite_screen::{InviteScreenWidgetRefExt, LeaveRoomResultAction}, main_desktop_ui::MainDesktopUiAction, navigation_tab_bar::{NavigationBarAction, SelectedTab}, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::RoomContextMenuWidgetRefExt, room_screen::{InviteAction, MessageAction, RoomScreenWidgetRefExt, TimelineUpdate, clear_timeline_states}, rooms_list::{RoomsListAction, RoomsListRef, RoomsListUpdate, clear_all_invited_rooms, enqueue_rooms_list_update}, rooms_list_header::RoomsListHeaderAction, space_lobby::SpaceLobbyScreenWidgetRefExt, spaces_bar::SpacesBarRef
    }, i18n::{AppLanguage, tr_fmt, tr_key}, join_leave_room_modal::{
        JoinLeaveModalKind, JoinLeaveRoomModalAction, JoinLeaveRoomModalWidgetRefExt
    }, login::login_screen::LoginAction, logout::logout_confirm_modal::{LogoutAction, LogoutConfirmModalAction, LogoutConfirmModalWidgetRefExt}, persistence, profile::{user_profile::UserProfile, user_profile_cache::clear_user_profile_cache}, room::{BasicRoomDetails, FetchedRoomAvatar}, shared::{avatar::{AvatarState, AvatarWidgetRefExt}, confirmation_modal::{ConfirmationModalContent, ConfirmationModalWidgetRefExt}, file_upload_modal::{FilePreviewerAction, FileUploadModalWidgetRefExt}, image_viewer::{ImageViewerAction, LoadState}, popup_list::{PopupKind, enqueue_popup_notification}, room_filter_input_bar::FilterAction}, sliding_sync::{DirectMessageRoomAction, MatrixRequest, RemoteDirectorySearchKind, RemoteDirectorySearchResult, TimelineKind, AccountSwitchAction, current_user_id, get_client, submit_async_request, get_timeline_update_sender}, updater::{UpdateCheckOutcome, check_for_updates, load_skipped_update_version, save_skipped_update_version, update_release_page_url}, utils::RoomNameId, verification::VerificationAction, verification_modal::{
        VerificationModalAction,
        VerificationModalWidgetRefExt,
    }
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let RoomFilterResultItem = View {
        visible: false
        width: Fill
        height: 48
        flow: Overlay

        row := View {
            width: Fill
            height: Fill
            flow: Right
            align: Align{y: 0.5}
            spacing: 8
            padding: Inset{left: 8, right: 8, top: 5, bottom: 5}

            avatar := Avatar { width: 30, height: 30 }

            text_col := View {
                width: Fill
                height: Fit
                flow: Down
                spacing: 0

                name_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        color: (COLOR_TEXT)
                        text_style: REGULAR_TEXT {font_size: 10}
                    }
                }

                id_label := Label {
                    width: Fill
                    height: Fit
                    draw_text +: {
                        color: (COLOR_TEXT_INPUT_IDLE)
                        text_style: REGULAR_TEXT {font_size: 8.5}
                    }
                }
            }
        }

        click_button := RobrixNeutralIconButton {
            width: Fill
            height: Fill
            text: ""
            icon_walk: Walk{width: 0, height: 0}
            draw_bg +: {
                color: #0000
                color_hover: #FFFFFF22
                color_down: #FFFFFF11
            }
        }
    }

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

                                            search_results_list := View {
                                                width: Fill,
                                                height: Fit,
                                                flow: Down
                                                spacing: 3

                                                result_item_0 := RoomFilterResultItem {}
                                                result_item_1 := RoomFilterResultItem {}
                                                result_item_2 := RoomFilterResultItem {}
                                                result_item_3 := RoomFilterResultItem {}
                                                result_item_4 := RoomFilterResultItem {}
                                                result_item_5 := RoomFilterResultItem {}
                                                result_item_6 := RoomFilterResultItem {}
                                                result_item_7 := RoomFilterResultItem {}
                                            }
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
                        app_tooltip := CalloutTooltip {}
                    }
                } // end of body
            }
        }
    }
}

app_main!(App);

#[derive(Clone)]
enum RoomFilterResultTarget {
    LocalSpace { room_name_id: RoomNameId, avatar: FetchedRoomAvatar },
    LocalRoom { room_name_id: RoomNameId, avatar: FetchedRoomAvatar },
    RemoteSpace { space_name_id: RoomNameId, avatar_uri: Option<OwnedMxcUri> },
    RemoteRoom { room_name_id: RoomNameId, avatar_uri: Option<OwnedMxcUri> },
    RemoteUser(UserProfile),
}

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
    #[rust] auth_ui_state: AuthUiState,
    /// The details of a room we're waiting on to be loaded so that we can navigate to it.
    /// This can be either a room we're waiting to join, or one we're waiting to be invited to.
    /// Also includes an optional room ID to be closed once the awaited room has been loaded.
    #[rust] waiting_to_navigate_to_room: Option<(BasicRoomDetails, Option<OwnedRoomId>)>,
    /// A stack of previously-selected rooms for mobile navigation.
    /// When a view is popped off the stack, the previous `selected_room` is restored from here.
    #[rust] mobile_room_nav_stack: Vec<SelectedRoom>,
    #[rust] room_filter_modal_results: Vec<RoomFilterResultTarget>,
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

    /// After initial creation, set the global singleton for the PopupList widget.
    fn on_after_new(&mut self, vm: &mut ScriptVm) {
        vm.with_cx_mut(|cx| {
            crate::shared::popup_list::set_global_popup_list(cx, &self.ui);
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

        self.update_login_visibility(cx);
        self.sync_app_language(cx);
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
        self.refresh_room_filter_modal_result_buttons(cx);
    }

    fn handle_timer(&mut self, cx: &mut Cx, event: &TimerEvent) {
        if self.room_filter_debounce_timer.is_timer(event).is_some() {
            self.room_filter_debounce_timer = Timer::empty();
            let keywords = std::mem::take(&mut self.pending_room_filter_keywords);
            self.update_room_filter_modal_results(cx, &keywords);
        }
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

        if let Some(clicked_index) = self.clicked_room_filter_result_index(cx, actions) {
            if let Some(target) = self.room_filter_modal_results.get(clicked_index).cloned() {
                self.ui.modal(cx, ids!(room_filter_modal)).close(cx);
                match target {
                    RoomFilterResultTarget::LocalSpace { room_name_id: space_name_id, .. }
                    => {
                        cx.action(NavigationBarAction::GoToSpace { space_name_id });
                    }
                    RoomFilterResultTarget::LocalRoom { room_name_id, .. }
                    => {
                        self.navigate_to_room(cx, None, &BasicRoomDetails::RoomId(room_name_id));
                    }
                    RoomFilterResultTarget::RemoteSpace { space_name_id, .. } => {
                        self.open_join_from_search_result(
                            cx,
                            BasicRoomDetails::Name(space_name_id),
                            true,
                        );
                    }
                    RoomFilterResultTarget::RemoteRoom { room_name_id, .. } => {
                        self.open_join_from_search_result(
                            cx,
                            BasicRoomDetails::Name(room_name_id),
                            false,
                        );
                    }
                    RoomFilterResultTarget::RemoteUser(user_profile) => {
                        submit_async_request(MatrixRequest::OpenOrCreateDirectMessage {
                            create_encrypted: self.app_state.bot_settings.should_create_encrypted_dm(
                                user_profile.user_id.as_ref(),
                                current_user_id().as_deref(),
                            ),
                            user_profile,
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
                    self.room_filter_modal_results.clear();
                    for result in results {
                        match result {
                            RemoteDirectorySearchResult::User(user_profile) => {
                                self.room_filter_modal_results.push(RoomFilterResultTarget::RemoteUser(user_profile.clone()));
                            }
                            RemoteDirectorySearchResult::Room { room_name_id, avatar_uri } => {
                                self.room_filter_modal_results.push(RoomFilterResultTarget::RemoteRoom {
                                    room_name_id: room_name_id.clone(),
                                    avatar_uri: avatar_uri.clone(),
                                });
                            }
                            RemoteDirectorySearchResult::Space { space_name_id, avatar_uri } => {
                                self.room_filter_modal_results.push(RoomFilterResultTarget::RemoteSpace {
                                    space_name_id: space_name_id.clone(),
                                    avatar_uri: avatar_uri.clone(),
                                });
                            }
                        }
                        if self.room_filter_modal_results.len() >= Self::ROOM_FILTER_RESULT_ITEM_IDS.len() {
                            break;
                        }
                    }
                    if self.room_filter_modal_results.is_empty() {
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
                    self.refresh_room_filter_modal_result_buttons(cx);
                    continue;
                }
                Some(RoomFilterRemoteSearchAction::Failed { query, kind: _, error }) => {
                    let room_filter_input = self.ui.text_input(cx, ids!(room_filter_modal_inner.room_filter_input_bar.input));
                    if room_filter_input.text().trim() != query.trim() {
                        continue;
                    }
                    self.room_filter_modal_results.clear();
                    self.refresh_room_filter_modal_result_buttons(cx);
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
    const ROOM_FILTER_RESULT_ITEM_IDS: [LiveId; 8] = [
        live_id!(result_item_0), live_id!(result_item_1),
        live_id!(result_item_2), live_id!(result_item_3),
        live_id!(result_item_4), live_id!(result_item_5),
        live_id!(result_item_6), live_id!(result_item_7),
    ];

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

    fn clicked_room_filter_result_index(&self, cx: &mut Cx, actions: &Actions) -> Option<usize> {
        let list_view = self.ui.view(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list));
        for (index, item_id) in Self::ROOM_FILTER_RESULT_ITEM_IDS.iter().enumerate() {
            if list_view.button(cx, &[*item_id, live_id!(click_button)]).clicked(actions) {
                return Some(index);
            }
        }
        None
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

    fn set_room_filter_result_avatar(
        &self,
        cx: &mut Cx,
        avatar_ref: &crate::shared::avatar::AvatarRef,
        fallback_text: &str,
        local_avatar: Option<&FetchedRoomAvatar>,
        remote_avatar_uri: Option<&OwnedMxcUri>,
        remote_avatar_state: Option<&AvatarState>,
    ) {
        if let Some(local_avatar) = local_avatar {
            match local_avatar {
                FetchedRoomAvatar::Text(text) => {
                    avatar_ref.show_text(cx, None, None, text);
                }
                FetchedRoomAvatar::Image(image_data) => {
                    let res = avatar_ref.show_image(
                        cx,
                        None,
                        |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, image_data),
                    );
                    if res.is_err() {
                        avatar_ref.show_text(cx, None, None, fallback_text);
                    }
                }
            }
            return;
        }

        if let Some(avatar_state) = remote_avatar_state {
            if let Some(image_data) = avatar_state.data() {
                let res = avatar_ref.show_image(
                    cx,
                    None,
                    |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, image_data),
                );
                if res.is_ok() {
                    return;
                }
            }
            if let Some(uri) = avatar_state.uri() {
                if let AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri) {
                    let res = avatar_ref.show_image(
                        cx,
                        None,
                        |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, &image_data),
                    );
                    if res.is_ok() {
                        return;
                    }
                }
            }
        }

        if let Some(uri) = remote_avatar_uri {
            if let AvatarCacheEntry::Loaded(image_data) = avatar_cache::get_or_fetch_avatar(cx, uri) {
                let res = avatar_ref.show_image(
                    cx,
                    None,
                    |cx, img_ref| crate::utils::load_png_or_jpg(&img_ref, cx, &image_data),
                );
                if res.is_ok() {
                    return;
                }
            }
        }

        avatar_ref.show_text(cx, None, None, fallback_text);
    }

    fn refresh_room_filter_modal_result_buttons(&self, cx: &mut Cx) {
        let list_view = self.ui.view(cx, ids!(room_filter_modal_inner.search_results_scroll.search_results.search_results_list));
        for (index, item_id) in Self::ROOM_FILTER_RESULT_ITEM_IDS.iter().enumerate() {
            let item = list_view.view(cx, &[*item_id]);
            if let Some(target) = self.room_filter_modal_results.get(index) {
                let (name, raw_id) = match target {
                    RoomFilterResultTarget::LocalSpace { room_name_id, .. }
                    | RoomFilterResultTarget::LocalRoom { room_name_id, .. } => {
                        (room_name_id.to_string(), room_name_id.room_id().to_string())
                    }
                    RoomFilterResultTarget::RemoteSpace { space_name_id, .. }
                    | RoomFilterResultTarget::RemoteRoom { room_name_id: space_name_id, .. } => {
                        (space_name_id.to_string(), space_name_id.room_id().to_string())
                    }
                    RoomFilterResultTarget::RemoteUser(user_profile) => {
                        (user_profile.displayable_name().to_owned(), user_profile.user_id.to_string())
                    }
                };

                item.label(cx, ids!(row.text_col.name_label)).set_text(cx, &name);
                item.label(cx, ids!(row.text_col.id_label)).set_text(cx, &raw_id);

                let avatar_ref = item.avatar(cx, ids!(row.avatar));
                match target {
                    RoomFilterResultTarget::LocalSpace { avatar, .. }
                    | RoomFilterResultTarget::LocalRoom { avatar, .. } => {
                        self.set_room_filter_result_avatar(cx, &avatar_ref, &name, Some(avatar), None, None);
                    }
                    RoomFilterResultTarget::RemoteSpace { avatar_uri, .. }
                    | RoomFilterResultTarget::RemoteRoom { avatar_uri, .. } => {
                        self.set_room_filter_result_avatar(cx, &avatar_ref, &name, None, avatar_uri.as_ref(), None);
                    }
                    RoomFilterResultTarget::RemoteUser(user_profile) => {
                        self.set_room_filter_result_avatar(
                            cx,
                            &avatar_ref,
                            &name,
                            None,
                            None,
                            Some(&user_profile.avatar_state),
                        );
                    }
                }

                item.set_visible(cx, true);
            } else {
                item.set_visible(cx, false);
            }
        }
    }

    fn update_room_filter_modal_results(&mut self, cx: &mut Cx, keywords: &str) {
        let keywords = keywords.trim();
        self.room_filter_modal_results.clear();

        if !keywords.is_empty() {
            let space_items = cx.get_global::<SpacesBarRef>()
                .get_matching_space_items(keywords, 4);
            let room_items = cx.get_global::<RoomsListRef>()
                .get_matching_room_items(keywords, 8);

            for (room_name_id, avatar) in space_items {
                self.room_filter_modal_results.push(RoomFilterResultTarget::LocalSpace { room_name_id, avatar });
                if self.room_filter_modal_results.len() >= Self::ROOM_FILTER_RESULT_ITEM_IDS.len() {
                    break;
                }
            }
            if self.room_filter_modal_results.len() < Self::ROOM_FILTER_RESULT_ITEM_IDS.len() {
                for (room_name_id, avatar) in room_items {
                    self.room_filter_modal_results.push(RoomFilterResultTarget::LocalRoom { room_name_id, avatar });
                    if self.room_filter_modal_results.len() >= Self::ROOM_FILTER_RESULT_ITEM_IDS.len() {
                        break;
                    }
                }
            }
        }

        if keywords.is_empty() {
            self.set_room_filter_modal_empty_state(
                cx,
                tr_key(self.app_state.app_language, "app.room_filter.empty_hint"),
                false,
            );
        } else if self.room_filter_modal_results.is_empty() {
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

        self.refresh_room_filter_modal_result_buttons(cx);
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
    use super::{BotSettingsState, RoomBotBindingState, SavedDockState, SelectedRoom};
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
