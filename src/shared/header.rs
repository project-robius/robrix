use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;
use crate::shared::stack_view_action::StackViewAction;
use crate::shared::dropdown_menu::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::FillerX;
    import crate::shared::dropdown_menu::DropDown;

    SimpleHeaderContent = <View> {
        width: Fill, height: Fit
        flow: Right, align: {x: 0.5, y: 0.5}

        <FillerX> {}

        title_container = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0.5}

            title = <Label> {
                width: Fit, height: Fit
                draw_text: {
                    color: #000,
                    text_style: <TITLE_TEXT>{},
                },
                text: "Joined Rooms"
            }
        }
    }

    SimpleHeader = <View> {
        width: Fill , height: Fit, margin: 0
        padding: {bottom: 7., top: 50.}, align: {x: 0.5, y: 0.0}, spacing: 0.0, flow: Overlay
        show_bg: true
        draw_bg: {
            color: #EDEDED
        }

        content = <SimpleHeaderContent> {}
    }

    HeaderWithLeftActionButton = <SimpleHeader> {
        content = {
            flow: Overlay

            button_container = <View> {
                left_button = <Button> {
                    width: Fit, height: 68
                    icon_walk: {width: 20, height: 68}
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            return sdf.result
                        }
                    }
                    draw_icon: {
                        color: #000;
                        brightness: 0.8;
                    }
                }
                divider = <View> {
                    width: Fill, height: Fit
                    right_button = <Button> {
                        width: Fit, height: 68
                        icon_walk: {width: 20, height: 68}
                        draw_bg: {
                            fn pixel(self) -> vec4 {
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                return sdf.result
                            }
                        }
                        draw_icon: {
                            color: #000;
                            brightness: 0.8;
                        }
                    }
                }
            }
        }
    }

    HeaderWithRightActionButton = <SimpleHeader> {
        content = {
            flow: Overlay

            button_container = <View> {
                spacer = <View> {
                    width: Fill, height: Fit
                    right_button = <Button> {
                        width: Fit, height: 68
                        icon_walk: {width: 20, height: 68}
                        draw_bg: {
                            fn pixel(self) -> vec4 {
                                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                                return sdf.result
                            }
                        }
                        draw_icon: {
                            color: #000;
                            brightness: 0.8;
                        }
                    }
                }
            }
        }
    }

    HeaderDropDownMenu = {{HeaderDropDownMenu}} {
        width: Fill, height: Fit, margin: 0
        padding: {bottom: 7., top: 50.}, align: {x: 0.5, y: 0.0}, spacing: 0.0, flow: Overlay
        show_bg: true
        draw_bg: {
            color: #EDEDED
        }

        content = <SimpleHeaderContent> {
            width: Fill, height: Fit
            flow: Right, align: {x: 0.5, y: 0.5}

            button_container = <View> {
                width: Fill, height: Fit
                align: {x: 1.0, y: 0.5}, flow: Right, spacing: 5., padding: {right: 5.}

                // TODO: this should be the searchbar, and we need consistent svgs
                left_button = <Button> {
                    width: Fit, height: Fit
                    padding: 0.
                    icon_walk: {width: 20, height: Fit}
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            return sdf.result
                        }
                    }
                    draw_icon: {
                        svg_file: dep("crate://self/resources/icons/search.svg")
                        color: #000;
                        brightness: 0.8;
                    }
                }

                menu = <DropDown> {
                    height: Fit, width: Fit
                    draw_icon: {
                        svg_file: dep("crate://self/resources/icons/menu.svg")
                        color: #000;
                        brightness: 0.8;
                    }
                    labels: ["Add Contact", "New Chat", "Scan", "Money"]
                    values: [AddContact, NewChat, Scan, Money]
                    icons: [
                        dep("crate://self/resources/icons/add_contact.svg"),
                        dep("crate://self/resources/icons/chat.svg"),
                        dep("crate://self/resources/icons/scan.svg"),
                        dep("crate://self/resources/icons/money.svg")
                    ]
                }
            }
        }
    }
}

#[derive(Live)]
pub struct HeaderDropDownMenu {
    #[deref]
    view: View,
}

impl LiveHook for HeaderDropDownMenu {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, HeaderDropDownMenu);
    }
}

impl Widget for HeaderDropDownMenu {
    fn handle_widget_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, WidgetActionItem),
    ) {
        let uid = self.widget_uid();
        self.handle_event_with(cx, event, &mut |cx, action: StackViewAction| {
            dispatch_action(cx, WidgetActionItem::new(action.into(), uid));
        });
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.view.redraw(cx);
    }

    fn walk(&mut self, cx: &mut Cx) -> Walk {
        self.view.walk(cx)
    }

    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.view.draw_walk_widget(cx, walk)
    }
}

impl HeaderDropDownMenu {
    pub fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, StackViewAction),
    ) {
        let actions = self.view.handle_widget_event(cx, event);

        if self.wechat_drop_down(id!(menu)).item_clicked(id!(AddContact), &actions) {
            dispatch_action(cx, StackViewAction::ShowAddContact);
        }
    }
}
