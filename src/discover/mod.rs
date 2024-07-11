use makepad_widgets::Cx;

pub mod discover_screen;
pub mod moments_screen;
pub mod moment_list;

pub fn live_design(cx: &mut Cx) {
    // Order matters here: moments_screen depends on moment_list.
    discover_screen::live_design(cx);
    moment_list::live_design(cx);
    moments_screen::live_design(cx);
}
