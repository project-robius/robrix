use crate::contacts::contact_info::*;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::styles::*;

    REGULAR_TEXT = {
        font_size: (12),
        font: {path: dep("crate://makepad-widgets/resources/GoNotoKurrent-Regular.ttf")}
    }

    ContactItem = <View> {
        width: Fill, height: Fit,
        flow: Down

        content = <View> {
            width: Fill, height: Fit,
            flow: Right
            align: {x: 0.0, y: 0.5},
            spacing: (SPACE_2),
            avatar = <Image> {
                source: (IMG_DEFAULT_AVATAR),
                width: 32.5, height: 32.5,
            }

            label = <H4> {}
        }
    }

    ContactsGroup = {{ContactsGroup}} {
        width: Fill, height: Fit,
        flow: Down
        spacing: (SPACE_0),
        margin: {top: (SPACE_2)}

        header: <View> {
            width: Fill, height: 100,
            padding: <MSPACE_1> {}

            show_bg: true,
            draw_bg: { color: (COLOR_D_1) }
            label = <H4> { }
        }

        people_contact_template: <ContactItem> {}

        file_transfer_template: <ContactItem> {
            content = {
                avatar = {
                    source: (IMG_FILE_TRANSFER_AVATAR)
                }
            }
        }

        wechat_template: <ContactItem> {
            content = {
                avatar = {
                    source: (IMG_WECHAT_AVATAR)
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Copy, PartialEq, FromLiveId)]
pub struct ContactItemId(pub LiveId);

#[derive(Live, LiveHook, Widget)]
pub struct ContactsGroup {
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[live] #[redraw]
    header: View,

    #[live]
    people_contact_template: Option<LivePtr>,
    #[live]
    file_transfer_template: Option<LivePtr>,
    #[live]
    wechat_template: Option<LivePtr>,

    #[rust]
    data: Vec<ContactInfo>,
    #[rust]
    contacts: ComponentMap<ContactItemId,WidgetRef>,
}

impl Widget for ContactsGroup {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let _ = self.header.draw_walk(cx, scope, walk);

        for contact in self.data.iter() {
            let contact_widget_id = LiveId::from_str(&contact.name).into();
            let current_contact = self.contacts.get_or_insert(cx, contact_widget_id, |cx| {
                let template = match contact.kind {
                    ContactKind::People => self.people_contact_template,
                    ContactKind::FileTransfer => self.file_transfer_template,
                    ContactKind::WeChat => self.wechat_template,
                };
                WidgetRef::new_from_ptr(cx, template)
            });

            current_contact
                .label(id!(content.label))
                .set_text(&contact.name);
            let _ = current_contact.draw_walk(cx, scope, walk);
        }
        cx.end_turtle();

        DrawStep::done()
    }
}

impl ContactsGroup {
    pub fn set_header_label(&mut self, text: &str) {
        let label = self.header.label(id!(label));
        label.set_text(text);
    }

    pub fn set_contacts(&mut self, data: Vec<ContactInfo>) {
        self.data = data;
    }
}
