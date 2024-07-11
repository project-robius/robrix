use makepad_widgets::Cx;

pub mod add_contact_screen;
pub mod contact_info;
pub mod contacts_group;
pub mod contacts_list;
pub mod contacts_screen;

pub fn live_design(cx: &mut Cx) {
    // Order matters here, as some widget definitions depend on others.
    contacts_screen::live_design(cx);
    contacts_group::live_design(cx);
    contacts_list::live_design(cx);
    add_contact_screen::live_design(cx);
}
