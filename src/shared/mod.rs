use makepad_widgets::Cx;

pub mod avatar;
pub mod color_tooltip;
pub mod helpers;
pub mod html_or_plaintext;
pub mod icon_button;
pub mod jump_to_bottom_button;
pub mod search_bar;
pub mod styles;
pub mod text_or_image;
pub mod typing_animation;
pub mod popup_list;
pub mod verification_badge;
pub mod callout_tooltip;

pub fn live_design(cx: &mut Cx) {
    // Order matters here, as some widget definitions depend on others.
    styles::live_design(cx);
    helpers::live_design(cx);
    icon_button::live_design(cx);
    search_bar::live_design(cx);
    avatar::live_design(cx);
    text_or_image::live_design(cx);
    html_or_plaintext::live_design(cx);
    typing_animation::live_design(cx);
    jump_to_bottom_button::live_design(cx);
    popup_list::live_design(cx);
    verification_badge::live_design(cx);
    color_tooltip::live_design(cx);
    callout_tooltip::live_design(cx);
}
