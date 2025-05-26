use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::home::main_mobile_ui::MainMobileUI;
    use crate::home::rooms_sidebar::RoomsSideBar;
    use crate::home::spaces_dock::SpacesDock;
    use crate::shared::styles::*;
    use crate::shared::room_filter_input_bar::RoomFilterInputBar;
    use crate::home::main_desktop_ui::MainDesktopUI;

    NavigationWrapper = {{NavigationWrapper}} {
        view_stack = <StackNavigation> {}
    }

    pub HomeScreen = <AdaptiveView> {
        Desktop = {
            show_bg: true
            draw_bg: {
                color: (COLOR_SECONDARY),
            }
            width: Fill, height: Fill
            flow: Right
            align: {x: 0.0, y: 0.0}
            padding: 0,
            margin: 0,

            spaces = <SpacesDock> {}

            <View> {
                flow: Down
                width: Fill, height: Fill

                <CachedWidget> {
                    <RoomFilterInputBar> {}
                }
                <MainDesktopUI> {}
            }
        }

        Mobile = {
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
            }
            width: Fill, height: Fill
            flow: Down

            <NavigationWrapper> {
                view_stack = <StackNavigation> {

                    root_view = {
                        padding: {top: 40.}
                        flow: Down
                        width: Fill, height: Fill

                        sidebar = <RoomsSideBar> {}
                        spaces = <SpacesDock> {}
                    }

                    main_content_view = <StackNavigationView> {
                        width: Fill, height: Fill
                        header = {
                            content = {
                                button_container = {
                                    padding: {left: 14}
                                }
                                title_container = {
                                    title = {
                                        draw_text: {
                                            color: (ROOM_NAME_TEXT_COLOR)
                                        }
                                    }
                                }
                            }
                        }
                        body = {
                            main_content = <MainMobileUI> {}
                        }
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct NavigationWrapper {
    #[deref]
    view: View,
}

impl Widget for NavigationWrapper {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.match_event(cx, event);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for NavigationWrapper {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.stack_navigation(id!(view_stack))
            .handle_stack_view_actions(cx, actions);
    }
}
