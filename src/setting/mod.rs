use makepad_widgets::Cx;


pub mod setting_page;
pub mod side_bar;
pub mod keyboard_page;
pub mod notification_page;
pub mod account_page;
pub mod router;


pub fn live_design(cx: &mut Cx) {
    setting_page::live_design(cx);
    side_bar::live_design(cx);
    keyboard_page::live_design(cx);
    notification_page::live_design(cx);
    account_page::live_design(cx);
    router::live_design(cx);
}