use makepad_widgets::Cx;

pub mod adaptive_layout_view;
pub mod avatar;
pub mod clickable_view;
pub mod dropdown_menu;
pub mod helpers;
pub mod html_or_plaintext;
pub mod modal;
pub mod popup_menu;
pub mod portal;
pub mod search_bar;
pub mod styles;
pub mod text_or_image;

pub fn live_design(cx: &mut Cx) {
    // Order matters here, as some widget definitions depend on others.
    styles::live_design(cx);
    helpers::live_design(cx);
    search_bar::live_design(cx);
    popup_menu::live_design(cx);
    dropdown_menu::live_design(cx);
    clickable_view::live_design(cx);
    avatar::live_design(cx);
    text_or_image::live_design(cx);
    html_or_plaintext::live_design(cx);
    portal::live_design(cx);
    modal::live_design(cx);
    adaptive_layout_view::live_design(cx);
}
