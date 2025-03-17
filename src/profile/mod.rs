use makepad_widgets::Cx;

pub mod my_profile_screen;
pub mod user_profile;
pub mod user_profile_cache;

pub fn live_design(cx: &mut Cx) {
    my_profile_screen::live_design(cx);
    user_profile::live_design(cx);
}
