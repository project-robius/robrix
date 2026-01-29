use makepad_widgets::*;

use crate::{app::AppState, home::navigation_tab_bar::{NavigationBarAction, SelectedTab}, settings::settings_screen::SettingsScreenWidgetRefExt};

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::home::main_mobile_ui::MainMobileUI;
    use crate::home::rooms_sidebar::RoomsSideBar;
    use crate::home::navigation_tab_bar::NavigationTabBar;
    use crate::home::search_messages::*;
    use crate::home::spaces_bar::*;
    use crate::home::add_room::*;
    use crate::shared::styles::*;
    use crate::shared::room_filter_input_bar::RoomFilterInputBar;
    use crate::home::main_desktop_ui::MainDesktopUI;
    use crate::settings::settings_screen::SettingsScreen;

    // Defines the total height of the StackNavigationView's header.
    // This has to be set in multiple places because of how StackNavigation
    // uses an Overlay view internally.
    STACK_VIEW_HEADER_HEIGHT = 75

    StackNavigationWrapper = {{StackNavigationWrapper}} {
        view_stack = <StackNavigation> {}
    }

    // A wrapper view around the SpacesBar that lets us show/hide it via animation.
    SpacesBarWrapper = {{SpacesBarWrapper}}<RoundedShadowView> {
        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE)
        margin: {left: 4, right: 4}
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY_DARKER),
            border_radius: 4.0,
            border_size: 0.0
            shadow_color: #0005
            shadow_radius: 15.0
            shadow_offset: vec2(1.0, 0.0),
        }

        <CachedWidget> {
            root_spaces_bar = <SpacesBar> {}
        }

        animator: {
            spaces_bar_animator = {
                default: hide,
                show = {
                    from: { all: Forward { duration: (SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: (NAVIGATION_TAB_BAR_SIZE),  draw_bg: { shadow_color: #x00000055 } }
                }
                hide = {
                    from: { all: Forward { duration: (SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: 0,  draw_bg: { shadow_color: #x00000000 } }
                }
            }
        }
    }

    // The home screen widget contains the main content:
    // rooms list, room screens, and the settings screen as an overlay.
    // It adapts to both desktop and mobile layouts.
    pub HomeScreen = {{HomeScreen}} {
        <AdaptiveView> {
            // NOTE: within each of these sub views, we used `CachedWidget` wrappers
            //       to ensure that there is only a single global instance of each
            //       of those widgets, which means they maintain their state
            //       across transitions between the Desktop and Mobile variant.
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
                    active_page: home_page

                    home_page = <View> {
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
                        show_bg: true,
                        draw_bg: {
                            color: (COLOR_PRIMARY)
                        }

                        <CachedWidget> {
                            settings_screen = <SettingsScreen> {}
                        }
                    }

                    add_room_page = <View> {
                        width: Fill, height: Fill
                        show_bg: true,
                        draw_bg: {
                            color: (COLOR_PRIMARY)
                        }

                        <CachedWidget> {
                            add_room_screen = <AddRoomScreen> {}
                        }
                    }
                }
            }

            Mobile = {
                width: Fill, height: Fill
                flow: Down

                show_bg: true
                draw_bg: {
                    color: (COLOR_PRIMARY)
                }

                <StackNavigationWrapper> {
                    view_stack = <StackNavigation> {
                        root_view = {
                            flow: Down
                            width: Fill, height: Fill

                            // At the top of the root view, we use the PageFlip widget to show either
                            // the main list of rooms or the settings screen.
                            home_screen_page_flip = <PageFlip> {
                                width: Fill, height: Fill

                                lazy_init: true,
                                active_page: home_page

                                home_page = <View> {
                                    width: Fill, height: Fill
                                    // Note: while the other page views have top padding, we do NOT add that here
                                    // because it is added in the `RoomsSideBar`'s `RoundedShadowView` itself.
                                    flow: Down

                                    <RoomsSideBar> {}
                                }

                                settings_page = <View> {
                                    width: Fill, height: Fill
                                    padding: {top: 20}

                                    <CachedWidget> {
                                        settings_screen = <SettingsScreen> {}
                                    }
                                }

                                add_room_page = <View> {
                                    width: Fill, height: Fill
                                    padding: {top: 20}

                                    <CachedWidget> {
                                        add_room_screen = <AddRoomScreen> {}
                                    }
                                }
                            }

                            // Show the SpacesBar right above the navigation tab bar.
                            // We wrap it in the SpacesBarWrapper in order to animate it in or out,
                            // and wrap *that* in a CachedWidget in order to maintain its shown/hidden state
                            // across AdaptiveView transitions between Mobile view mode and Desktop view mode.
                            // 
                            // ... Then we wrap *that* in a ... <https://www.youtube.com/watch?v=evUWersr7pc>
                            <CachedWidget> {
                                spaces_bar_wrapper = <SpacesBarWrapper> {}
                            }

                            // At the bottom of the root view, show the navigation tab bar horizontally.
                            <CachedWidget> {
                                navigation_tab_bar = <NavigationTabBar> {}
                            }
                        }

                        main_content_view = <StackNavigationView> {
                            width: Fill, height: Fill
                            header = {
                                // The following shader stuff was copied from `RoundedShadowView`.
                                // We can't directly use RoundedShadowView here for `header` because it is already
                                // a special view, the `StackViewHeader`.
                                clip_x:false,
                                clip_y:false,
                                                    
                                show_bg: true,
                                draw_bg: {
                                    uniform color_dither: 1.0
                                    uniform gradient_border_horizontal: 0.0; 
                                    uniform gradient_fill_horizontal: 0.0; 

                                    color: (COLOR_PRIMARY_DARKER)
                                    uniform color_2: vec4(-1.0, -1.0, -1.0, -1.0)

                                    uniform border_radius: 4.0
                                    uniform border_size: 0.0
                                    uniform border_color: #0000
                                    uniform border_color_2: vec4(-1.0, -1.0, -1.0, -1.0)

                                    uniform shadow_color: #0005
                                    uniform shadow_radius: 9.0,
                                    uniform shadow_offset: vec2(1.0, 0.0)
                                                                    
                                    varying rect_size2: vec2,
                                    varying rect_size3: vec2,
                                    varying rect_pos2: vec2,     
                                    varying rect_shift: vec2,    
                                    varying sdf_rect_pos: vec2,
                                    varying sdf_rect_size: vec2,
                                                                    
                                    fn vertex(self) -> vec4 {
                                        let min_offset = min(self.shadow_offset,vec2(0));
                                        self.rect_size2 = self.rect_size + 2.0*vec2(self.shadow_radius);
                                        self.rect_size3 = self.rect_size2 + abs(self.shadow_offset);
                                        self.rect_pos2 = self.rect_pos - vec2(self.shadow_radius) + min_offset;
                                        self.sdf_rect_size = self.rect_size2 - vec2(self.shadow_radius * 2.0 + self.border_size * 2.0)
                                        self.sdf_rect_pos = -min_offset + vec2(self.border_size + self.shadow_radius);
                                        self.rect_shift = -min_offset;
                                                                                    
                                        return self.clip_and_transform_vertex(self.rect_pos2, self.rect_size3)
                                    }
                                                                                
                                    fn pixel(self) -> vec4 {                                                
                                        let sdf = Sdf2d::viewport(self.pos * self.rect_size3)
                                        let dither = Math::random_2d(self.pos.xy) * 0.04 * self.color_dither;

                                        let color_2 = self.color;
                                        if (self.color_2.x > -0.5) {
                                            color_2 = self.color_2;
                                        }

                                        let border_color_2 = self.border_color;
                                        if (self.border_color_2.x > -0.5) {
                                            border_color_2 = self.border_color_2;
                                        }

                                        let gradient_border_dir = self.pos.y + dither;
                                        if (self.gradient_border_horizontal > 0.5) {
                                            gradient_border_dir = self.pos.x + dither;
                                        }

                                        let gradient_fill_dir = self.pos.y + dither;
                                        if (self.gradient_fill_horizontal > 0.5) {
                                            gradient_fill_dir = self.pos.x + dither;
                                        }

                                        sdf.box(
                                            self.sdf_rect_pos.x,
                                            self.sdf_rect_pos.y,
                                            self.sdf_rect_size.x,
                                            self.sdf_rect_size.y, 
                                            max(1.0, self.border_radius)
                                        )
                                        if sdf.shape > -1.0{ // try to skip the expensive gauss shadow
                                            let m = self.shadow_radius;
                                            let o = self.shadow_offset + self.rect_shift;
                                            let v = GaussShadow::rounded_box_shadow(vec2(m) + o, self.rect_size2+o, self.pos * (self.rect_size3+vec2(m)), self.shadow_radius*0.5, self.border_radius*2.0);
                                            sdf.clear(self.shadow_color*v)
                                        }
                                                                                            
                                        sdf.fill_keep(mix(self.color, color_2, gradient_fill_dir))

                                        if self.border_size > 0.0 {
                                            sdf.stroke(
                                                mix(self.border_color, border_color_2, gradient_border_dir),
                                                self.border_size)
                                        }
                                        return sdf.result
                                    }
                                }

                                padding: {top: 30, bottom: 0}
                                height: (STACK_VIEW_HEADER_HEIGHT),
                                content = {
                                    height: (STACK_VIEW_HEADER_HEIGHT)
                                    button_container = {
                                        padding: 0,
                                        margin: 0
                                        left_button = {
                                            draw_bg: {
                                                fn pixel(self) -> vec4 {
                                                    return #FFFFFF00;
                                                }
                                            }
                                            width: Fit, height: Fit,
                                            padding: {left: 20, right: 23, top: 10, bottom: 10}
                                            margin: {left: 8, right: 0, top: 0, bottom: 0}
                                            icon_walk: {width: 13, height: Fit}
                                            spacing: 0
                                            text: ""
                                        }
                                    }
                                    title_container = {
                                        padding: {top: 8}
                                        title = {
                                            draw_text: {
                                                color: (ROOM_NAME_TEXT_COLOR)
                                            }
                                        }
                                    }
                                }
                            }
                            body = {
                                margin: {top: (STACK_VIEW_HEADER_HEIGHT)}
                                main_content = <MainMobileUI> {}
                            }
                        }
                    }
                }
            }
        }
    }
}


/// A simple wrapper around the SpacesBar that allows us to animate showing or hiding it.
#[derive(Live, LiveHook, Widget)]
pub struct SpacesBarWrapper {
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


#[derive(Live, LiveHook, Widget)]
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
                                    .settings_screen(ids!(settings_screen))
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
                        self.view.spaces_bar_wrapper(ids!(spaces_bar_wrapper))
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
            .page_flip(ids!(home_screen_page_flip))
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
#[derive(Live, LiveHook, Widget)]
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
        self.stack_navigation(ids!(view_stack))
            .handle_stack_view_actions(cx, actions);
    }
}
