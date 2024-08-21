use makepad_widgets::Cx;

pub mod home_screen;
pub mod main_content;
pub mod room_screen;
pub mod rooms_list;
pub mod spaces_dock;

pub fn live_design(cx: &mut Cx) {
    home_screen::live_design(cx);
    rooms_list::live_design(cx);
    room_screen::live_design(cx);
    main_content::live_design(cx);
    spaces_dock::live_design(cx);
}
