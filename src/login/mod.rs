use makepad_widgets::*;

pub mod login_screen;
pub mod login_status_modal;
pub mod logout_confirm_modal;
pub mod logout_state_machine;
pub mod logout_errors;

pub fn live_design(cx: &mut Cx) {
    login_screen::live_design(cx);
    login_status_modal::live_design(cx);
    logout_confirm_modal::live_design(cx);
}
