use crate::shared::clickable_view::*;
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

    OptionsItem = <View> {
        flow: Down,
        width: Fill, height: Fit
        padding: <MSPACE_2> {}
        show_bg: false,

        content = <View> {
            flow: Right,
            width: Fill, height: Fit,
            spacing: (SPACE_1),
            align: { x: 0.5, y: 0.5},

            icon = <Image> {
                width: 32.5, height: 32.5
            }

            label = <H4> {}
            <Filler> {}
            action_icon = <ActionIcon> {}
        }
    }

    Options = <View> {
        width: Fill, height: Fit,
        flow: Down
        margin: <MSPACE_V_2> {}, padding: {bottom: (SPACE_2)},
        spacing: 0.,

        show_bg: false,
    }

    Discover = {{Discover}} {
        width: Fill, height: Fit,
        flow: Down,
        spacing: 0.0,

        moments_link = <ClickableView> {
            width: Fill, height: Fit,
            flow: Down,
            margin: {top: 10., bottom: 10.}, padding: {bottom: 10.},
            spacing: 0.,
            show_bg: true,
            draw_bg: { color: (COLOR_D_0) } 
        }
        <Options> {
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
            <DividerH> { }
            <OptionsItem> {
                content = {
                    icon = {
                        source: (IMG_SCAN)
                    }

                    label = {
                        text: "Scan"
                    }
                }
            }
            <DividerH> { }
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
            <DividerH> { }
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
            <DividerH> { }
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
            <DividerH> { }
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

    DiscoverScreen = <View> {
        width: Fill, height: Fill,
        flow: Down,
        spacing: 0.0,

        show_bg: true,
        draw_bg: { color: (COLOR_D_0) }

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

#[derive(Live, LiveHook, Widget)]
pub struct Discover {
    #[deref]
    view: View
}

impl Widget for Discover {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let actions = cx.capture_actions(|cx| self.view.handle_event(cx, event, scope));
        let uid = self.widget_uid();

        if self
            .clickable_view(id!(moments_link))
            .clicked(&actions)
        {            
            cx.widget_action(uid, &scope.path, StackNavigationAction::NavigateTo(live_id!(moments_stack_view)));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}