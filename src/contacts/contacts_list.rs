use crate::contacts::contact_info::*;
use crate::contacts::contacts_group::ContactsGroup;
use makepad_widgets::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    // import crate::shared::helpers::Divider;
    // import crate::shared::search_bar::SearchBar;

    import crate::contacts::contacts_group::ContactsGroup

    Options = <View> {
        width: Fill, height: Fit,
        flow: Down,
        padding: <MSPACE_0> {}, margin: <MSPACE_0> {},
        spacing: (SPACE_0),

        <OptionsItem> {
            content = {
                icon = {
                    source: (IMG_NEW_FRIENDS)
                }

                label = {
                    text: "New Friends"
                }
            }
        }

        <DividerH> {}

        <OptionsItem> {
            content = {
                icon = {
                    source: (IMG_GROUP_CHATS)
                }

                label = {
                    text: "Group Chats"
                }
            }
        }

        <DividerH> {}

        <OptionsItem> {
            content = {
                icon = {
                    source: (IMG_TAGS)
                }

                label = {
                    text: "Tags"
                }
            }
        }
    }

    ContactsList = {{ContactsList}} {
        width: Fill, height: Fill,
        flow: Down,
        margin: <MSPACE_0> {}, padding: <MSPACE_2> {},
        show_bg: true,
        draw_bg: { color: (COLOR_D_1) },

        list = <PortalList> {
            width: Fill, height: Fill,
            flow: Down,
            spacing: (SPACE_2),
            margin: 0., padding: 0.,

            search_bar = <SearchBar> { }
            options = <Options> {},
            contacts_group = <ContactsGroup> {},

            bottom = <View> {
                width: Fill, height: Fit,
                align: {x: 0.5, y: 0.},

                <Pbolditalic> {
                    margin: <MSPACE_2> {}
                    width: Fit,
                    text: "3 friends"
                }
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct ContactsList {
    #[deref]
    view: View,

    #[rust]
    data: Vec<ContactInfo>,
}

impl LiveHook for ContactsList {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.data = vec![
            ContactInfo {
                name: "File Transfer".to_string(),
                kind: ContactKind::FileTransfer,
            },
            ContactInfo {
                name: "John Doe".to_string(),
                kind: ContactKind::People,
            },
            ContactInfo {
                name: "Chris P. Bacon".to_string(),
                kind: ContactKind::People,
            },
            ContactInfo {
                name: "Marsha Mellow".to_string(),
                kind: ContactKind::People,
            },
            ContactInfo {
                name: "Olive Yew".to_string(),
                kind: ContactKind::People,
            },
            ContactInfo {
                name: "WeChat Team".to_string(),
                kind: ContactKind::WeChat,
            },
        ];
    }
}

impl Widget for ContactsList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope)
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let grouped_data = self.group_by_first_letter();
        let groups_count = grouped_data.len();

        while let Some(list_item) = self.view.draw_walk(cx, scope, walk).step(){
            if let Some(mut list) = list_item.as_portal_list().borrow_mut() {
                list.set_item_range(cx, 0, groups_count + 3);

                while let Some(item_id) = list.next_visible_item(cx) {
                    let template = match item_id {
                        0 => id!(search_bar),
                        1 => id!(options),
                        x if x == groups_count + 2 => id!(bottom),
                        _ => id!(contacts_group),
                    };
                    let item = list.item(cx, item_id, template[0]).unwrap();

                    if item_id >= 2 && item_id < groups_count + 2 {
                        let group = &grouped_data[(item_id - 2) as usize];
                        if let Some(mut group_widget) = item.borrow_mut::<ContactsGroup>() {
                            group_widget.set_header_label(&group[0].name[0..1]);
                            group_widget.set_contacts(group.to_vec());
                        }
                    }

                    item.draw_all(cx, &mut Scope::empty());
                }
            }
        }

        DrawStep::done()
    }
}

impl ContactsList {
    pub fn group_by_first_letter(&self) -> Vec<Vec<ContactInfo>> {
        let mut grouped_data: Vec<Vec<ContactInfo>> = vec![];

        // We assume data is sorted by name
        for contact in self.data.iter() {
            let first_char = contact.name.chars().next().unwrap_or('\0');

            match grouped_data.last_mut() {
                Some(last_group) if last_group[0].name.starts_with(first_char) => {
                    last_group.push(contact.clone());
                }
                _ => {
                    grouped_data.push(vec![contact.clone()]);
                }
            }
        }

        grouped_data
    }
}
