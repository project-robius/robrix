use makepad_widgets::*;

pub mod search_message;
pub mod right_panel;
pub fn live_design(cx: &mut Cx) {
    search_message::live_design(cx);
    right_panel::live_design(cx);
}