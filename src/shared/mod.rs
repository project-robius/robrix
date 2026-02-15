use makepad_widgets::Cx;

pub mod avatar;
pub mod callout_tooltip;
pub mod collapsible_header;
pub mod confirmation_modal;
pub mod helpers;
pub mod html_or_plaintext;
pub mod icon_button;
pub mod jump_to_bottom_button;
pub mod mentionable_text_input;
pub mod popup_list;
pub mod room_filter_input_bar;
pub mod styles;
pub mod text_or_image;
pub mod timestamp;
pub mod bouncing_dots;
pub mod unread_badge;
pub mod verification_badge;
pub mod restore_status_view;
pub mod image_viewer;

pub fn live_design(cx: &mut Cx) {
    // Order matters here, as some widget definitions depend on others.
    styles::live_design(cx);
    helpers::live_design(cx);
    icon_button::live_design(cx);
    unread_badge::live_design(cx);
    collapsible_header::live_design(cx);
    timestamp::live_design(cx);
    room_filter_input_bar::live_design(cx);
    avatar::live_design(cx);
    text_or_image::live_design(cx);
    html_or_plaintext::live_design(cx);
    bouncing_dots::live_design(cx);
    jump_to_bottom_button::live_design(cx);
    popup_list::live_design(cx);
    verification_badge::live_design(cx);
    callout_tooltip::live_design(cx);
    mentionable_text_input::live_design(cx);
    restore_status_view::live_design(cx);
    confirmation_modal::live_design(cx);
    image_viewer::live_design(cx);
}
