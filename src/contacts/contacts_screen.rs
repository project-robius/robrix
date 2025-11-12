use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::header::HeaderDropDownMenu;
    // import crate::shared::search_bar::SearchBar;
    import crate::contacts::add_contact_screen::AddContactScreen;
    import crate::contacts::contacts_list::ContactsList;

    ContactsHeader = <HeaderDropDownMenu> {
        show_bg: true,
        draw_bg: { color: (COLOR_D_1)}
        content = {
            title_container = {
                title = {
                    text:"通讯录"
                }
            }
        }
    }

    <SearchBar> {}

    ContactsBody = <View> {
        width: Fill, height: Fill,
        flow: Down,
        spacing: (SPACE_0),
        show_bg: true,
        draw_bg: { color: (COLOR_U) }

        <ContactsHeader> {}
        <ContactsList> {}
    }

    Contacts = <View> {
        width: Fill, height: Fill,
        flow: Down,
        spacing: (SPACE_0),
        <ContactsBody> {}
    }

    ContactsScreen = <View> {
        width: Fill, height: Fill,
        <Contacts> {}
    }
}