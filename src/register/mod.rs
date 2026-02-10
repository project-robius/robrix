use makepad_widgets::*;

pub mod register_screen;
pub mod register_status_modal;
mod validation;

pub fn live_design(cx: &mut Cx) {
    register_screen::live_design(cx);
    register_status_modal::live_design(cx);
}
