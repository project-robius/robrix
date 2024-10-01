use makepad_widgets::*;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::home::main_content::MainContent;
    import crate::home::rooms_sidebar::RoomsSideBar;
    import crate::home::spaces_dock::SpacesDock;
    import crate::shared::styles::*;
    import crate::shared::adaptive_view::AdaptiveView;

    NavigationWrapper = {{NavigationWrapper}} {
        view_stack = <StackNavigation> {}
    }

    HomeScreen = <AdaptiveView> {
        Desktop = {
            show_bg: true
            draw_bg: {
                color: (COLOR_PRIMARY)
            }
            width: Fill, height: Fill
            padding: 0, margin: 0, align: {x: 0.0, y: 0.0}
            flow: Right
            
            spaces = <SpacesDock> {}
            rooms_sidebar = <RoomsSideBar> {}
            main_content = <MainContent> {}
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
                        body = {
                            main_content = <MainContent> {}
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
    view: View
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
    fn handle_actions(&mut self, cx: &mut Cx, actions:&Actions) {
        self.stack_navigation(id!(view_stack)).handle_stack_view_actions(cx, actions);
    }
}
