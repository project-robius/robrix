use crate::shared::clickable_view::*;
use crate::shared::stack_view_action::StackViewAction;
use makepad_widgets::widget::WidgetCache;
use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::clickable_view::ClickableView;
    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::header::HeaderDropDownMenu;
    import makepad_widgets::image::*;

    IMG_MOMENTS = dep("crate://self/resources/img/moments.png")
    IMG_SCAN = dep("crate://self/resources/img/scan.png")
    IMG_SHAKE = dep("crate://self/resources/img/shake.png")
    IMG_SEARCH = dep("crate://self/resources/img/search.png")
    IMG_PEOPLE_NEARBY = dep("crate://self/resources/img/people_nearby.png")
    IMG_MINI_PROGRAMS = dep("crate://self/resources/img/mini_programs.png")

    ActionIcon = <Label> {
        width: Fit, height: Fit
        text: ">"
        draw_text:{
            color: #b4
            text_style: <REGULAR_TEXT>{font_size: 16},
        }
    }

    OptionsItem = <View> {
        width: Fill, height: Fit
        padding: {left: 10., top: 10., right: 20. bottom: 2.}, spacing: 8., flow: Down

        content = <View> {
            width: Fill, height: Fit
            padding: 0, align: {x: 0.0, y: 0.5}, spacing: 10., flow: Right

            icon = <Image> {
                width: 24., height: 24.
            }

            label = <Label> {
                width: Fit, height: Fit
                draw_text:{
                    color: #000,
                    text_style: <REGULAR_TEXT>{},
                },
            }

            <FillerX> {}

            action_icon = <ActionIcon> {}
        }
    }

    Options = <View> {
        width: Fill, height: Fit, margin: {top: 10., bottom: 10.}
        padding: {bottom: 10.}, spacing: 0., flow: Down

        show_bg: true
        draw_bg: {
            color: #fff
        }
    }

    ClickableOptions = <ClickableView> {
        width: Fill, height: Fit, margin: {top: 10., bottom: 10.}
        padding: {bottom: 10.}, spacing: 0., flow: Down
    }

    Discover = {{Discover}} {
        view: <View> {
            width: Fill, height: Fit
            flow: Down, spacing: 0.0

            moments_link = <ClickableOptions> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_MOMENTS)
                        }

                        label = {
                            text: "Moments"
                        }
                    }
                }
            }

            <Options> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_SCAN)
                        }

                        label = {
                            text: "Scan"
                        }
                    }

                    divider = <Divider> {
                        margin: {left: 42.0}
                    }
                }

                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_SHAKE)
                        }

                        label = {
                            text: "Shake"
                        }

                    }
                }
            }

            <Options> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_SEARCH)
                        }

                        label = {
                            text: "Search"
                        }
                    }
                }
            }

            <Options> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_PEOPLE_NEARBY)
                        }

                        label = {
                            text: "People Nearby"
                        }
                    }
                }
            }

            <Options> {
                <OptionsItem> {
                    content = {
                        icon = {
                            source: (IMG_MINI_PROGRAMS)
                        }

                        label = {
                            text: "Mini Programs"
                        }
                    }
                }
            }
        }
    }

    DiscoverScreen = <View> {
        width: Fill, height: Fill
        flow: Down, spacing: 0.0

        show_bg: true
        draw_bg: {
            color: #ddd
        }

        <HeaderDropDownMenu> {
            content = {
                title_container = {
                    title = {
                        text: "发现"
                    }
                }
            }
        }

        <Discover> {}
    }
}

#[derive(Live)]
pub struct Discover {
    #[live]
    view:View,
}

impl LiveHook for Discover {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, Discover);
    }
}

impl Widget for Discover {
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

    fn find_widgets(&mut self, path: &[LiveId], cached: WidgetCache, results: &mut WidgetSet) {
        self.view.find_widgets(path, cached, results);
    }

    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        self.view.draw_walk_widget(cx, walk)
    }
}

impl Discover {
    fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dispatch_action: &mut dyn FnMut(&mut Cx, StackViewAction),
    ) {
        let actions = self.view.handle_widget_event(cx, event);

        if self
            .clickable_view(id!(moments_link))
            .clicked(&actions)
        {
            dispatch_action(cx, StackViewAction::ShowMoments);
        }
    }
}
