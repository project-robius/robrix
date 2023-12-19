use makepad_widgets::*;
use std::iter;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;

    IMG_DEFAULT_AVATAR = dep("crate://self/resources/img/default_avatar.png")

    IMG_BANNER = dep("crate://self/resources/img/hero.jpg")
    IMG_POST1 = dep("crate://self/resources/img/post1.jpg")
    IMG_POST2 = dep("crate://self/resources/img/post2.jpg")

    Hero = <View> {
        width: Fill, height: Fit
        flow: Overlay, align: {y: 1, x: 1}
        banner = <Image> {
            width: Fill, height: 200.0
            source: (IMG_BANNER),
        }
        content = <View> {
            width: Fit, height: Fit
            align: {y: 0.5}
            username = <Label> {
                width: Fit, height: Fit
                draw_text:{
                    color: #fff,
                    text_style: <REGULAR_TEXT>{}
                }
                text:"減活乗治外進"
            }
            avatar = <Image> {
                source: (IMG_DEFAULT_AVATAR),
                width: 50., height: 50.
            }
        }
    }

    TextPost = <View> {
        flow: Right, spacing: 10., padding: 10.
        width: Fill, height: Fit

        avatar = <Image> {
            source: (IMG_DEFAULT_AVATAR),
            width: 36., height: 36.
        }

        content = <View> {
            width: Fill, height: Fit
            flow: Down, spacing: 7.

            username = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{}
                }
                text:"Josh"
            }

            text = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{}
                }
                text:"Lorem ipsum dolor sit amet, consectetur"
            }
        }
    }

    ImagePost = <View> {
        flow: Right, spacing: 10., padding: 10.
        width: Fill, height: Fit

        avatar = <Image> {
            source: (IMG_DEFAULT_AVATAR),
            width: 36., height: 36.
        }

        content = <View> {
            flow: Down, spacing: 7.
            width: Fill, height: Fit

            username = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{}
                }
                text:"Josh"
            }

            text = <Label> {
                width: Fill, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{font_size: 11.}
                }
                text:"Lorem ipsum dolor sit amet, consectetur"
            }

            images = <View> {
                width: Fill, height: 110.
                flow: Right, spacing: 7.

                image_1 = <Image> {
                    source: (IMG_POST1),
                    width: 90., height: 110.
                }

                image_2 = <Image> {
                    source: (IMG_POST2),
                    width: 180., height: 110.
                }
            }
        }
    }

    MomentList = {{MomentList}} {
        width: Fill, height: Fill
        flow: Down
        list: <PortalList> {
            width: Fill, height: Fill
            flow: Down, spacing: 0.0

            image_post = <ImagePost> {}
            text_post = <TextPost> {}
            hero = <Hero> {}
        }
    }
}

#[derive(Debug, Clone, WidgetAction)]
pub enum MomentListAction {
    None,
}

#[derive(Live)]
pub struct MomentList {
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    list: PortalList,
    #[rust]
    moment_entries: Vec<MomentEntry>,
}

impl LiveHook for MomentList {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, MomentList);
    }

    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        let entries: Vec<MomentEntry> = vec![
            MomentEntry {
                username: "John Doe".to_string(),
                text: "無嶋可済野誰実玉全示餌強".to_string(),
            },
            MomentEntry {
                username: "Andrew Lin".to_string(),
                text: "俳権竹減活乗治外進梨詰鉄掲動覇予載".to_string(),
            },
            MomentEntry {
                username: "Chris Huxley".to_string(),
                text: "犯福併読併棋一御質慰".to_string(),
            },
            MomentEntry {
                username: "Adam Adler".to_string(),
                text: "体議速人幅触無持編聞組込".to_string(),
            },
            MomentEntry {
                username: "Eric Ford".to_string(),
                text: "体議速人幅触無持編聞組込 減活乗治外進".to_string(),
            }
        ];

        let repeated = iter::repeat(entries.clone()).take(10).flatten().collect();
        self.moment_entries = repeated;
    }
}

impl Widget for MomentList {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let _actions = self.list.handle_widget_event(cx, event);

        for action in _actions {
            dispatch_action(cx, action);
        }
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.list.redraw(cx)
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.draw_walk(cx, walk);
        WidgetDraw::done()
    }
}

impl MomentList {
    pub fn draw_walk(&mut self, cx: &mut Cx2d, walk: Walk) {
        let moment_entries_count = self.moment_entries.len() as u64;

        cx.begin_turtle(walk, self.layout);
        self.list
            .set_item_range(cx, 0, moment_entries_count + 1);

        while self.list.draw_widget(cx).hook_widget().is_some() {
            while let Some(item_id) = self.list.next_visible_item(cx) {
                let template = match item_id {
                    0 => id!(hero),
                    x if x % 2 == 0 => id!(text_post),
                    _ => id!(image_post),
                };

                let item = self.list.item(cx, item_id, template[0]).unwrap();

                if item_id >= 1 && item_id < moment_entries_count + 1 {
                    let post = &self.moment_entries[item_id as usize - 1]; // offset by 1 to account for the hero

                    item.label(id!(content.username))
                        .set_text(&post.username);
                    item.label(id!(content.text)).set_text(&post.text);
                }

                item.draw_widget_all(cx);
            }
        }

        cx.end_turtle();
    }
}

#[derive(Clone)]
struct MomentEntry {
    username: String,
    text: String,
}
