use makepad_widgets::Cx;

pub mod settings_screen;
pub mod account_settings;

pub fn live_design(cx: &mut Cx) {
    account_settings::live_design(cx);
    settings_screen::live_design(cx);
}
