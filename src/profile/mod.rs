use makepad_widgets::Cx;

pub mod user_profile;
pub mod user_profile_cache;

pub fn live_design(cx: &mut Cx) {
    user_profile::live_design(cx);
}
