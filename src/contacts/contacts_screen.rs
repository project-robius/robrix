use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::header::HeaderDropDownMenu;
    import crate::shared::search_bar::SearchBar;
    import crate::contacts::add_contact_screen::AddContactScreen;
    import crate::contacts::contacts_list::ContactsList;

    IMG_NEW_FRIENDS = dep("crate://self/resources/img/new_friends.png")
    IMG_GROUP_CHATS = dep("crate://self/resources/img/group_chats.png")
    IMG_TAGS = dep("crate://self/resources/img/tags.png")

    ContactsHeader = <HeaderDropDownMenu> {
        content = {
            title_container = {
                title = {
                    text:"通讯录"
                }
            }
        }
    }

    <SearchBar> {}

    Divider = <View> {
        width: Fill, height: Fit
        flow: Down
        <RoundedView> {
            width: Fill,
            height: 1.,
            draw_bg: {color: (#ddd)}
        }
    }

    ContactsBody = <View> {
        show_bg: true
        width: Fill, height: Fill
        flow: Down, spacing: 0.0

        draw_bg: {
            color: #fff
        }

        <ContactsHeader> {}
        <ContactsList> {}
    }

    Contacts = <View> {
        width: Fill, height: Fill
        flow: Down, spacing: 0.0
        <ContactsBody> {}
    }

    ContactsScreen = <View> {
        width: Fill, height: Fill
        <Contacts> {}
    }
}