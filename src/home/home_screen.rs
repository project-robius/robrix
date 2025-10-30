use makepad_widgets::*;

use crate::settings::{settings_screen::SettingsScreenWidgetRefExt, SettingsAction};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::home::main_mobile_ui::MainMobileUI;
    use crate::home::rooms_sidebar::RoomsSideBar;
    use crate::home::navigation_tab_bar::NavigationTabBar;
    use crate::home::search_messages::*;
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

                // On the left, show the navigation tab bar vertically.
                <CachedWidget> {
                    navigation_tab_bar = <NavigationTabBar> {}
                }

                // To the right of that, we use the PageFlip widget to show either
                // the main desktop UI or the settings screen.
                home_screen_page_flip = <PageFlip> {
                    width: Fill, height: Fill

                    lazy_init: true,
                    active_page: main_page

                    main_page = <View> {
                        width: Fill, height: Fill
                        flow: Down

                        <View> {
                            width: Fill,
                            height: 39,
                            flow: Right
                            padding: {top: 2, bottom: 2}
                            margin: {right: 2}
                            spacing: 2
                            align: {y: 0.5}

                            <CachedWidget> {
                                room_filter_input_bar = <RoomFilterInputBar> {}
                            }

                            search_messages_button = <SearchMessagesButton> {
                                // make this button match/align with the RoomFilterInputBar
                                height: 32.5,
                                margin: {right: 2}
                            }
                        }

                        <MainDesktopUI> {}
                    }

                    settings_page = <View> {
                        width: Fill, height: Fill

                        <CachedWidget> {
                            settings_screen = <SettingsScreen> {}
                        }
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

                            // At the top of the root view, we use the PageFlip widget to show either
                            // the main list of rooms or the settings screen.
                            home_screen_page_flip = <PageFlip> {
                                width: Fill, height: Fill

                                lazy_init: true,
                                active_page: main_page

                                main_page = <View> {
                                    width: Fill, height: Fill
                                    flow: Down

                                    <RoomsSideBar> {}
                                }

                                settings_page = <View> {
                                    width: Fill, height: Fill

                                    <CachedWidget> {
                                        settings_screen = <SettingsScreen> {}
                                    }
                                }
                            }

                            // At the bottom of the root view, show the navigation tab bar horizontally.
                            <CachedWidget> {
                                navigation_tab_bar = <NavigationTabBar> {}
                            }
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


/// Which space is currently selected in the NavigationTabBar.
#[derive(Clone, Debug, Default)]
pub enum SelectedSpace {
    #[default]
    Home,
    Settings,
    // Once we support spaces and shortcut buttons (like directs only, etc),
    // we can add them here.
}


#[derive(Live, LiveHook, Widget)]
pub struct HomeScreen {
    #[deref] view: View,

    #[rust] selection: SelectedSpace,
    #[rust] previous_selection: SelectedSpace,
}

impl Widget for HomeScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            for action in actions {
                // Handle the settings screen being opened or closed.
                match action.downcast_ref() {
                    Some(SettingsAction::OpenSettings) => {
                        if !matches!(self.selection, SelectedSpace::Settings) {
                            self.previous_selection = self.selection.clone();
                            self.selection = SelectedSpace::Settings;
                            if let Some(settings_page) = self.update_active_page_from_selection(cx) {
                                settings_page
                                    .settings_screen(ids!(settings_screen))
                                    .populate(cx, None);
                                self.view.redraw(cx);
                            } else {
                                error!("BUG: failed to set active page to show settings screen.");
                            }
                        }
                    }
                    Some(SettingsAction::CloseSettings) => {
                        self.selection = self.previous_selection.clone();
                        self.update_active_page_from_selection(cx);
                        self.view.redraw(cx);
                    }
                    _ => {}
                }
            }
        }

        self.view.handle_event(cx, event, scope);  
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Note: We need to update the active page before drawing,
        // because if we switched between Desktop and Mobile views,
        // the PageFlip widget will have been reset to its default,
        // so we must re-set it to the correct page based on `self.selection`.
        self.update_active_page_from_selection(cx);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl HomeScreen {
    fn update_active_page_from_selection(&mut self, cx: &mut Cx) -> Option<WidgetRef> {
        self.view
            .page_flip(ids!(home_screen_page_flip))
            .set_active_page(
                cx,
                match self.selection {
                    SelectedSpace::Settings => id!(settings_page),
                    SelectedSpace::Home => id!(main_page),
                },
            )
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
        self.stack_navigation(ids!(view_stack))
            .handle_stack_view_actions(cx, actions);
    }
}
