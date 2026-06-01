use makepad_widgets::{ScriptVm, event::{DigitId, FingerDownEvent, FingerLongPressEvent, FingerUpEvent}};

pub mod add_room;
pub mod bot_binding_modal;
pub mod create_bot_modal;
pub mod delete_bot_modal;
pub mod edited_indicator;
pub mod editing_pane;
pub mod encryption_notice;
pub mod event_source_modal;
pub mod home_screen;
pub mod invite_modal;
pub mod invite_screen;
pub mod light_themed_dock;
pub mod tombstone_footer;
pub mod loading_pane;
pub mod location_preview;
pub mod main_desktop_ui;
pub mod main_mobile_ui;
pub mod room_screen;
pub mod room_read_receipt;
pub mod rooms_list;
pub mod rooms_list_entry;
pub mod rooms_list_header;
pub mod rooms_sidebar;
pub mod search_messages;
pub mod space_lobby;
pub mod spaces_bar;
pub mod navigation_tab_bar;
pub mod welcome_screen;
pub mod event_reaction_list;
pub mod new_message_context_menu;
pub mod room_context_menu;
pub mod room_settings_modal;
pub mod link_preview;
pub mod room_image_viewer;
pub mod streaming_animation;
pub mod upload_progress;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenuOpenGesture {
    digit_id: DigitId,
    /// The time of the original `FingerDown` that opened the menu.
    /// Matches `FingerUpEvent.capture_time` for the same capture chain
    /// (see Makepad `finger.rs`: `capture_digit` stores `e.time` as `capture.time`,
    /// and `FingerUpEvent` reads it back as `capture_time`).
    capture_time: f64,
}

impl ContextMenuOpenGesture {
    pub fn from_finger_down(event: &FingerDownEvent) -> Self {
        Self {
            digit_id: event.digit_id,
            capture_time: event.time,
        }
    }

    pub fn from_long_press(event: &FingerLongPressEvent) -> Self {
        Self {
            digit_id: event.digit_id,
            capture_time: event.capture_time,
        }
    }

    fn matches_finger_up(&self, event: &FingerUpEvent) -> bool {
        self.digit_id == event.digit_id
            && self.capture_time == event.capture_time
    }
}

pub fn consume_context_menu_opening_finger_up(
    pending_open_gesture: &mut Option<ContextMenuOpenGesture>,
    event: &FingerUpEvent,
) -> bool {
    if pending_open_gesture
        .as_ref()
        .is_some_and(|gesture| gesture.matches_finger_up(event))
    {
        *pending_open_gesture = None;
        true
    } else {
        false
    }
}

pub fn script_mod(vm: &mut ScriptVm) {
    search_messages::script_mod(vm);
    loading_pane::script_mod(vm);
    location_preview::script_mod(vm);
    add_room::script_mod(vm);
    bot_binding_modal::script_mod(vm);
    create_bot_modal::script_mod(vm);
    delete_bot_modal::script_mod(vm);
    space_lobby::script_mod(vm);
    link_preview::script_mod(vm);
    event_reaction_list::script_mod(vm);
    room_read_receipt::script_mod(vm);
    rooms_list_entry::script_mod(vm);
    rooms_list_header::script_mod(vm);
    rooms_list::script_mod(vm);
    edited_indicator::script_mod(vm);
    editing_pane::script_mod(vm);
    encryption_notice::script_mod(vm);
    new_message_context_menu::script_mod(vm);
    event_source_modal::script_mod(vm);
    room_context_menu::script_mod(vm);
    room_settings_modal::script_mod(vm);
    invite_modal::script_mod(vm);
    invite_screen::script_mod(vm);
    tombstone_footer::script_mod(vm);
    room_screen::script_mod(vm);
    rooms_sidebar::script_mod(vm);
    welcome_screen::script_mod(vm);
    light_themed_dock::script_mod(vm);
    main_mobile_ui::script_mod(vm);
    main_desktop_ui::script_mod(vm);
    spaces_bar::script_mod(vm);
    navigation_tab_bar::script_mod(vm);
    // Note: upload_progress::script_mod is called earlier in app.rs
    // because RoomInputBar depends on it.
    // Keep HomeScreen last, it references many widgets registered above.
    home_screen::script_mod(vm);
}
