use makepad_widgets::*;

use crate::settings::{settings_screen::SettingsScreenWidgetExt, SettingsAction};

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
    use crate::settings::settings_screen::SettingsScreen;

    NavigationWrapper = {{NavigationWrapper}} {
        view_stack = <StackNavigation> {}
    }

    // The home screen widget contains the main content:
    // rooms list, room screens, and the settings screen as an overlay.
    // It adapts to both desktop and mobile layouts.
    pub HomeScreen = {{HomeScreen}} {
        <AdaptiveView> {
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

                // On the left, show the spaces sidebar.
                spaces = <SpacesDock> {}

                // To the right of that, show a view that contains the main desktop UI
                // with the settings screen able to be overlaid in front of it.
                <View> {
                    width: Fill, height: Fill
                    flow: Overlay,

                    <View> {
                        width: Fill, height: Fill
                        flow: Down

                        <CachedWidget> {
                            room_filter_input_bar = <RoomFilterInputBar> {}
                        }
                        <MainDesktopUI> {}
                    }

                    <CachedWidget> {
                        settings_screen = <SettingsScreen> {}
                    }
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

                            // At the top of the root view, show the main list of rooms.
                            // We use an overlay view to allow the Settings screen to display in front of it.
                            <View> {
                                width: Fill, height: Fill
                                flow: Overlay

                                sidebar = <RoomsSideBar> {}

                                <CachedWidget> {
                                    settings_screen = <SettingsScreen> {}
                                }
                            }

                            // At the bottom of the root view, show the spaces dock.
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
}


#[derive(Live, LiveHook, Widget)]
pub struct HomeScreen {
    #[deref] view: View,
}

impl Widget for HomeScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);  
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for HomeScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            // Handle the settings screen being opened or closed.
            match action.downcast_ref() {
                Some(SettingsAction::OpenSettings) => {
                    self.view.settings_screen(id!(settings_screen)).show(cx);
                    self.view.redraw(cx);
                }
                Some(SettingsAction::CloseSettings) => {
                    self.view.settings_screen(id!(settings_screen)).hide(cx);
                    self.view.redraw(cx);
                }
                _ => {}
            }
        }
    }
}

/// A wrapper around the StackNavigation widget
/// that simply forwards stack view actions to it.
#[derive(Live, LiveHook, Widget)]
pub struct NavigationWrapper {
    #[deref]
    view: View,
}

impl Widget for NavigationWrapper {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
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
