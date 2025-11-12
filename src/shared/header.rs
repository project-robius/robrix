use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;
use crate::shared::dropdown_menu::*;

live_design! {
    import makepad_draw::shader::std::*;
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    // import crate::shared::helpers::FillerX;
    import crate::shared::dropdown_menu::DropDown;

    SimpleHeaderContent = <View> {
        width: Fill, height: Fit
        flow: Right, align: {x: 0.5, y: 0.5}

        <Filler> {}

        title_container = <View> {
            width: Fill, height: Fit
            align: {x: 0.5, y: 0.5}

            title = <H3> {
                width: Fit, height: Fit
                text: "Joined Rooms"
            }
        }
    }

    SimpleHeader = <View> {
        width: Fill, height: Fit,
        align: {x: 0.5, y: 0.0},
        spacing: (SPACE_0),
        flow: Overlay

        content = <SimpleHeaderContent> {}
    }

    HeaderDropDownMenu = {{HeaderDropDownMenu}} {
        width: Fill, height: Fit,
        align: {x: 0.5, y: 0.0},
        margin: <MSPACE_0> {}, padding: <MSPACE_0> {},
        spacing: (SPACE_0),
        flow: Overlay

        show_bg: false,

        content = <SimpleHeaderContent> {
            flow: Right,
            width: Fill, height: Fit
            align: {x: 0.5, y: 0.5}

            button_container = <View> {
                width: Fill, height: Fit,
                align: {x: 1.0, y: 0.5},
                flow: Right,

                // TODO: this should be the searchbar, and we need consistent svgs
                left_button = <Button> {
                    width: Fit, height: Fit
                    padding: <MSPACE_1> {}
                    icon_walk: {width: 20, height: Fit}
                    draw_bg: {
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            return sdf.result
                        }
                    }
                    draw_icon: {
                        svg_file: (ICON_SEARCH)
                        color: (COLOR_D);
                        brightness: 0.8;
                    }
                }

                menu = <DropDown> {
                    height: Fit, width: Fit
                    draw_icon: {
                        svg_file: (ICON_CREATE),
                        color: (COLOR_D),
                        brightness: 0.8,
                    }
                    labels: ["Add Contact", "New Chat", "Scan", "Money"]
                    values: [AddContact, NewChat, Scan, Money]
                    icons: [
                        (ICON_SEARCH),
                        (ICON_CHAT),
                        (ICON_SCAN),
                        (ICON_HOME),
                    ]
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct HeaderDropDownMenu {
    #[deref]
    view: View,
}

impl Widget for HeaderDropDownMenu {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        let actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));

        if self.wechat_drop_down(id!(menu)).item_clicked(id!(AddContact), &actions) {
            cx.widget_action(uid, &scope.path, StackNavigationAction::NavigateTo(live_id!(add_contact_stack_view)));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep  {
        self.view.draw_walk(cx, scope, walk)
    }
}
