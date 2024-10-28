use makepad_widgets::Cx;
/// The room details main pane, including the room info and members panes.
pub mod room_details;
/// The room info pane.
pub mod room_info_pane;
/// The room members pane.
pub mod room_members_pane;

pub fn live_design(cx: &mut Cx) {
    room_details::live_design(cx);
    room_info_pane::live_design(cx);
    room_members_pane::live_design(cx);
}
