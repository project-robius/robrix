use makepad_widgets::ScriptVm;

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
pub mod command_text_input;
pub mod unread_badge;
pub mod verification_badge;
pub mod restore_status_view;
pub mod image_viewer;


pub fn script_mod(vm: &mut ScriptVm) {
    // Order matters here, as some widget definitions depend on others.
    styles::script_mod(vm);
    helpers::script_mod(vm);
    icon_button::script_mod(vm);
    unread_badge::script_mod(vm);
    collapsible_header::script_mod(vm);
    timestamp::script_mod(vm);
    room_filter_input_bar::script_mod(vm);
    avatar::script_mod(vm);
    text_or_image::script_mod(vm);
    html_or_plaintext::script_mod(vm);
    bouncing_dots::script_mod(vm);
    jump_to_bottom_button::script_mod(vm);
    popup_list::script_mod(vm);
    verification_badge::script_mod(vm);
    callout_tooltip::script_mod(vm);
    command_text_input::script_mod(vm);
    mentionable_text_input::script_mod(vm);
    restore_status_view::script_mod(vm);
    confirmation_modal::script_mod(vm);
    image_viewer::script_mod(vm);
}
