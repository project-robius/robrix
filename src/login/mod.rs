use makepad_widgets::*;

pub mod login_screen;
pub mod login_status_modal;

pub fn live_design(cx: &mut Cx) {
    login_screen::live_design(cx);
    login_status_modal::live_design(cx);
}
