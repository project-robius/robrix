use makepad_widgets::Cx;

pub mod home_screen;
pub mod light_themed_dock;  
pub mod loading_modal;
pub mod main_desktop_ui;
pub mod main_mobile_ui;
pub mod room_preview;
pub mod room_screen;
pub mod rooms_list;
pub mod rooms_sidebar;
pub mod spaces_dock;
pub mod welcome_screen;
pub mod message_context_menu;
pub mod message_source_modal;

pub fn live_design(cx: &mut Cx) {
    home_screen::live_design(cx);
    loading_modal::live_design(cx);
    rooms_list::live_design(cx);
    room_preview::live_design(cx);
    room_screen::live_design(cx);
    rooms_sidebar::live_design(cx);
    main_mobile_ui::live_design(cx);
    main_desktop_ui::live_design(cx);
    spaces_dock::live_design(cx);
    welcome_screen::live_design(cx);
    light_themed_dock::live_design(cx);
    message_context_menu::live_design(cx);
    // message_source_modal::live_design(cx);
}
