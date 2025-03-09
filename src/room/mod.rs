use makepad_widgets::Cx;

pub mod room_input_bar;
pub mod room_member_manager;


pub fn live_design(cx: &mut Cx) {
    room_input_bar::live_design(cx)
}
