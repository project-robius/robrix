use makepad_widgets::Cx;

pub mod my_profile_screen;
pub mod profile_screen;
pub mod user_profile;

pub fn live_design(cx: &mut Cx) {
    profile_screen::live_design(cx);
    my_profile_screen::live_design(cx);
    user_profile::live_design(cx);
}
