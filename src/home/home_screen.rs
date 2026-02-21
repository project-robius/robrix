use makepad_widgets::*;

use crate::{app::AppState, home::navigation_tab_bar::{NavigationBarAction, SelectedTab}, settings::settings_screen::SettingsScreenWidgetRefExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // Defines the total height of the StackNavigationView's header.
    // This has to be set in multiple places because of how StackNavigation
    // uses an Overlay view internally.
    mod.widgets.STACK_VIEW_HEADER_HEIGHT = 75

    mod.widgets.StackNavigationWrapper = #(StackNavigationWrapper::register_widget(vm)) {
        view_stack := StackNavigation {}
    }

    // A wrapper view around the SpacesBar that lets us show/hide it via animation.
    mod.widgets.SpacesBarWrapper = #(SpacesBarWrapper::register_widget(vm)) {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE)
        margin: Inset{left: 4, right: 4}
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY_DARKER),
            border_radius: 4.0,
            border_size: 0.0
            shadow_color: #0005
            shadow_radius: 15.0
            shadow_offset: vec2(1.0, 0.0),
        }

        CachedWidget {
            root_spaces_bar := mod.widgets.SpacesBar {}
        }

        animator: Animator{
            spaces_bar_animator: {
                default: @hide
                show: AnimatorState{
                    from: { all: Forward { duration: (mod.widgets.SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: (NAVIGATION_TAB_BAR_SIZE),  draw_bg: { shadow_color: #x00000055 } }
                }
                hide: AnimatorState{
                    from: { all: Forward { duration: (mod.widgets.SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: 0,  draw_bg: { shadow_color: #x00000000 } }
                }
            }
        }
    }

    // The home screen widget contains the main content:
    // rooms list, room screens, and the settings screen as an overlay.
    // It adapts to both desktop and mobile layouts.
    mod.widgets.HomeScreen = #(HomeScreen::register_widget(vm)) {
        AdaptiveView {
            // NOTE: within each of these sub views, we used `CachedWidget` wrappers
            //       to ensure that there is only a single global instance of each
            //       of those widgets, which means they maintain their state
            //       across transitions between the Desktop and Mobile variant.
            Desktop := View {
                show_bg: true
                draw_bg +: {
                    color: (COLOR_SECONDARY),
                }
                width: Fill, height: Fill
                flow: Right
                align: Align{x: 0.0, y: 0.0}
                padding: 0,
                margin: 0,

                // On the left, show the navigation tab bar vertically.
                CachedWidget {
                    navigation_tab_bar := mod.widgets.NavigationTabBar {}
                }

                // To the right of that, we use the PageFlip widget to show either
                // the main desktop UI or the settings screen.
                home_screen_page_flip := PageFlip {
                    width: Fill, height: Fill

                    lazy_init: true,
                    active_page: @home_page

                    home_page := View {
                        width: Fill, height: Fill
                        flow: Down

                        View {
                            width: Fill,
                            height: 39,
                            flow: Right
                            padding: Inset{top: 2, bottom: 2}
                            margin: Inset{right: 2}
                            spacing: 2
                            align: Align{y: 0.5}

                            CachedWidget {
                                room_filter_input_bar := RoomFilterInputBar {}
                            }

                            search_messages_button := SearchMessagesButton {
                                // make this button match/align with the RoomFilterInputBar
                                height: 32.5,
                                margin: Inset{right: 2}
                            }
                        }

                        mod.widgets.MainDesktopUI {}
                    }

                    settings_page := View {
                        width: Fill, height: Fill
                        show_bg: true,
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                        }

                        CachedWidget {
                            settings_screen := mod.widgets.SettingsScreen {}
                        }
                    }

                    add_room_page := View {
                        width: Fill, height: Fill
                        show_bg: true,
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                        }

                        CachedWidget {
                            add_room_screen := mod.widgets.AddRoomScreen {}
                        }
                    }
                }
            }

            Mobile := View {
                width: Fill, height: Fill
                flow: Down

                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                }

                mod.widgets.StackNavigationWrapper {
                    view_stack := StackNavigation {
                        root_view: {
                            flow: Down
                            width: Fill, height: Fill

                            // At the top of the root view, we use the PageFlip widget to show either
                            // the main list of rooms or the settings screen.
                            home_screen_page_flip := PageFlip {
                                width: Fill, height: Fill

                                lazy_init: true,
                                active_page: @home_page

                                home_page := View {
                                    width: Fill, height: Fill
                                    // Note: while the other page views have top padding, we do NOT add that here
                                    // because it is added in the `RoomsSideBar`'s `RoundedShadowView` itself.
                                    flow: Down

                                    mod.widgets.RoomsSideBar {}
                                }

                                settings_page := View {
                                    width: Fill, height: Fill
                                    padding: Inset{top: 20}

                                    CachedWidget {
                                        settings_screen := mod.widgets.SettingsScreen {}
                                    }
                                }

                                add_room_page := View {
                                    width: Fill, height: Fill
                                    padding: Inset{top: 20}

                                    CachedWidget {
                                        add_room_screen := mod.widgets.AddRoomScreen {}
                                    }
                                }
                            }

                            // Show the SpacesBar right above the navigation tab bar.
                            // We wrap it in the SpacesBarWrapper in order to animate it in or out,
                            // and wrap *that* in a CachedWidget in order to maintain its shown/hidden state
                            // across AdaptiveView transitions between Mobile view mode and Desktop view mode.
                            // 
                            // ... Then we wrap *that* in a ... <https://www.youtube.com/watch?v=evUWersr7pc>
                            CachedWidget {
                                spaces_bar_wrapper := mod.widgets.SpacesBarWrapper {}
                            }

                            // At the bottom of the root view, show the navigation tab bar horizontally.
                            CachedWidget {
                                navigation_tab_bar := mod.widgets.NavigationTabBar {}
                            }
                        }

                        main_content_view := StackNavigationView {
                            width: Fill, height: Fill
                            header := StackViewHeader {
                                padding: Inset{top: 30, bottom: 0}
                                height: (mod.widgets.STACK_VIEW_HEADER_HEIGHT),
                            }
                            body := View {
                                margin: Inset{top: (mod.widgets.STACK_VIEW_HEADER_HEIGHT)}
                                main_content := mod.widgets.MainMobileUI {}
                            }
                        }
                    }
                }
            }
        }
    }
}


/// A simple wrapper around the SpacesBar that allows us to animate showing or hiding it.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct SpacesBarWrapper {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[animator] animator: Animator,
}

impl Widget for SpacesBarWrapper {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // TODO: i want to uncomment this, but adding it back in will break
        //       the animation of showing the SpacesBarWrapper.
        //       I'm not sure why the SpacesBar is getting redrawn constantly though.
        // if walk.height.to_fixed().is_some_and(|h| h < 0.01) {
        //     return DrawStep::done();
        // }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SpacesBarWrapperRef {
    /// Shows or hides the spaces bar by animating it in or out.
    fn show_or_hide(&self, cx: &mut Cx, show: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if show {
            inner.animator_play(cx, ids!(spaces_bar_animator.show));
        } else {
            inner.animator_play(cx, ids!(spaces_bar_animator.hide));
        }
        inner.redraw(cx);
    }
}


#[derive(Script, ScriptHook, Widget)]
pub struct HomeScreen {
    #[deref] view: View,

    /// The previously-selected navigation tab, used to determine which tab
    /// and top-level view we return to after closing the settings screen.
    ///
    /// Note that the current selected tap is stored in `AppState` so that
    /// other widgets can easily access it.
    #[rust] previous_selection: SelectedTab,
    #[rust] is_spaces_bar_shown: bool,
}

impl Widget for HomeScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            let app_state = scope.data.get_mut::<AppState>().unwrap();
            for action in actions {
                match action.downcast_ref() {
                    Some(NavigationBarAction::GoToHome) => {
                        if !matches!(app_state.selected_tab, SelectedTab::Home) {
                            self.previous_selection = app_state.selected_tab.clone();
                            app_state.selected_tab = SelectedTab::Home;
                            cx.action(NavigationBarAction::TabSelected(app_state.selected_tab.clone()));
                            self.update_active_page_from_selection(cx, app_state);
                            self.view.redraw(cx);
                        }
                    }
                    Some(NavigationBarAction::GoToAddRoom) => {
                        if !matches!(app_state.selected_tab, SelectedTab::AddRoom) {
                            self.previous_selection = app_state.selected_tab.clone();
                            app_state.selected_tab = SelectedTab::AddRoom;
                            cx.action(NavigationBarAction::TabSelected(app_state.selected_tab.clone()));
                            self.update_active_page_from_selection(cx, app_state);
                            self.view.redraw(cx);
                        }
                    }
                    Some(NavigationBarAction::GoToSpace { space_name_id }) => {
                        let new_space_selection = SelectedTab::Space { space_name_id: space_name_id.clone() };
                        if app_state.selected_tab != new_space_selection {
                            self.previous_selection = app_state.selected_tab.clone();
                            app_state.selected_tab = new_space_selection;
                            cx.action(NavigationBarAction::TabSelected(app_state.selected_tab.clone()));
                            self.update_active_page_from_selection(cx, app_state);
                            self.view.redraw(cx);
                        }
                    }
                    // Only open the settings screen if it is not currently open.
                    Some(NavigationBarAction::OpenSettings) => {
                        if !matches!(app_state.selected_tab, SelectedTab::Settings) {
                            self.previous_selection = app_state.selected_tab.clone();
                            app_state.selected_tab = SelectedTab::Settings;
                            cx.action(NavigationBarAction::TabSelected(app_state.selected_tab.clone()));
                            if let Some(settings_page) = self.update_active_page_from_selection(cx, app_state) {
                                settings_page
                                    .settings_screen(cx, ids!(settings_screen))
                                    .populate(cx, None);
                                self.view.redraw(cx);
                            } else {
                                error!("BUG: failed to set active page to show settings screen.");
                            }
                        }
                    }
                    Some(NavigationBarAction::CloseSettings) => {
                        if matches!(app_state.selected_tab, SelectedTab::Settings) {
                            app_state.selected_tab = self.previous_selection.clone();
                            cx.action(NavigationBarAction::TabSelected(app_state.selected_tab.clone()));
                            self.update_active_page_from_selection(cx, app_state);
                            self.view.redraw(cx);
                        }
                    }
                    Some(NavigationBarAction::ToggleSpacesBar) => {
                        self.is_spaces_bar_shown = !self.is_spaces_bar_shown;
                        self.view.spaces_bar_wrapper(cx, ids!(spaces_bar_wrapper))
                            .show_or_hide(cx, self.is_spaces_bar_shown);
                    }
                    // We're the ones who emitted this action, so we don't need to handle it again.
                    Some(NavigationBarAction::TabSelected(_))
                    | None => { }
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let app_state = scope.data.get_mut::<AppState>().unwrap();
        // Note: We need to update the active page before drawing,
        // because if we switched between Desktop and Mobile views,
        // the PageFlip widget will have been reset to its default,
        // so we must re-set it to the correct page based on `app_state.selected_tab`.
        self.update_active_page_from_selection(cx, app_state);

        self.view.draw_walk(cx, scope, walk)
    }
}

impl HomeScreen {
    fn update_active_page_from_selection(
        &mut self,
        cx: &mut Cx,
        app_state: &mut AppState,
    ) -> Option<WidgetRef> {
        self.view
            .page_flip(cx, ids!(home_screen_page_flip))
            .set_active_page(
                cx,
                match app_state.selected_tab {
                    SelectedTab::Space { .. }
                    | SelectedTab::Home => id!(home_page),
                    SelectedTab::Settings => id!(settings_page),
                    SelectedTab::AddRoom => id!(add_room_page),
                },
            )
    }
}

/// A wrapper around the StackNavigation widget
/// that simply forwards stack view actions to it.
#[derive(Script, ScriptHook, Widget)]
pub struct StackNavigationWrapper {
    #[deref] view: View,
}

impl Widget for StackNavigationWrapper {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.match_event(cx, event);
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MatchEvent for StackNavigationWrapper {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        self.stack_navigation(cx, ids!(view_stack))
            .handle_stack_view_actions(cx, actions);
    }
}
