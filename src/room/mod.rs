use makepad_widgets::Cx;

pub mod room_details;

pub fn live_design(cx: &mut Cx) {
    room_details::live_design(cx);
}
