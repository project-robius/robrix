use makepad_widgets::*;

use crate::{
    app::{AppState, AppStateAction, SelectedRoom},
    home::{
        invite_screen::InviteScreenWidgetRefExt,
        navigation_tab_bar::{NavigationBarAction, SelectedTab},
        room_screen::RoomScreenWidgetRefExt,
        rooms_list::RoomsListAction,
        space_lobby::SpaceLobbyScreenWidgetRefExt,
    },
    settings::{
        app_preferences::{effective_is_desktop, AppPreferencesAction, ViewModeOverride},
        settings_screen::SettingsScreenWidgetRefExt,
    },
    shared::room_filter_input_bar::{MainFilterAction, RoomFilterInputBarWidgetExt},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // Defines the total height of the StackNavigationView's header.
    // This has to be set in multiple places because of how StackNavigation
    // uses an Overlay view internally.
    mod.widgets.STACK_VIEW_HEADER_HEIGHT = 45

    // A reusable base for StackNavigationView children in the mobile layout.
    // Each specific screen view (room, invite, space lobby) extends this
    // and places its own screen widget inside the body.
    mod.widgets.RobrixStackNavigationView = StackNavigationView {
        width: Fill, height: Fill
        draw_bg.color: (COLOR_PRIMARY)
        header +: {
            height: (mod.widgets.STACK_VIEW_HEADER_HEIGHT),
            padding: 0
            align: Align{y: 0.5}

            // Below is a shader to draw a shadow under the bottom half of the header
            clip_x: false,
            clip_y: false,
            show_bg: true,
            draw_bg +: {
                color: instance((COLOR_PRIMARY_DARKER))
                color_dither: uniform(1.0)
                gradient_border_horizontal: uniform(0.0)
                gradient_fill_horizontal: uniform(0.0)
                color_2: instance(vec4(-1))

                border_radius: uniform(4.0)
                border_size: uniform(0.0)
                border_color: instance(#0000)
                border_color_2: instance(vec4(-1))

                shadow_color: instance(#0005)
                shadow_radius: uniform(12.0)
                shadow_offset: uniform(vec2(0.0, 0.0))

                rect_size2: varying(vec2(0))
                rect_size3: varying(vec2(0))
                rect_pos2: varying(vec2(0))
                rect_shift: varying(vec2(0))
                sdf_rect_pos: varying(vec2(0))
                sdf_rect_size: varying(vec2(0))

                vertex: fn() {
                    let min_offset = min(self.shadow_offset vec2(0))
                    self.rect_size2 = self.rect_size + 2.0*vec2(self.shadow_radius)
                    self.rect_size3 = self.rect_size2 + abs(self.shadow_offset)
                    self.rect_pos2 = self.rect_pos - vec2(self.shadow_radius) + min_offset
                    self.sdf_rect_size = self.rect_size2 - vec2(self.shadow_radius * 2.0 + self.border_size * 2.0)
                    self.sdf_rect_pos = -min_offset + vec2(self.border_size + self.shadow_radius)
                    self.rect_shift = -min_offset

                    return self.clip_and_transform_vertex(self.rect_pos2 self.rect_size3)
                }

                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size3)

                    let mut fill_color = self.color
                    if self.color_2.x > -0.5 {
                        let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                        let dir = if self.gradient_fill_horizontal > 0.5 self.pos.x else self.pos.y
                        fill_color = mix(self.color self.color_2 dir + dither)
                    }

                    let mut stroke_color = self.border_color
                    if self.border_color_2.x > -0.5 {
                        let dither = Math.random_2d(self.pos.xy) * 0.04 * self.color_dither
                        let dir = if self.gradient_border_horizontal > 0.5 self.pos.x else self.pos.y
                        stroke_color = mix(self.border_color self.border_color_2 dir + dither)
                    }

                    sdf.box(
                        self.sdf_rect_pos.x
                        self.sdf_rect_pos.y
                        self.sdf_rect_size.x
                        self.sdf_rect_size.y
                        max(1.0 self.border_radius)
                    )
                    if sdf.shape > -1.0 {
                        let m = self.shadow_radius
                        let o = self.shadow_offset + self.rect_shift
                        let v = GaussShadow.rounded_box_shadow(vec2(m) + o self.rect_size2+o self.pos * (self.rect_size3+vec2(m)) self.shadow_radius*0.5 self.border_radius*2.0)
                        // Only draw shadow on the bottom half of the view
                        let pixel_y = self.pos.y * self.rect_size3.y
                        let mid_y = self.sdf_rect_pos.y + self.sdf_rect_size.y * 0.5
                        let bottom_mask = smoothstep(mid_y - m * 0.3 mid_y + m * 0.3 pixel_y)
                        sdf.clear(self.shadow_color * v * bottom_mask)
                    }

                    sdf.fill_keep(fill_color)

                    if self.border_size > 0.0 {
                        sdf.stroke(stroke_color self.border_size)
                    }
                    return sdf.result
                }
            }

            content +: {
                height: (mod.widgets.STACK_VIEW_HEADER_HEIGHT)
                align: Align{y: 0.5}
                padding: Inset{
                    left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                    right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                }
                button_container +: {
                    padding: 0,
                    margin: 0
                    left_button +: {
                        width: Fit, height: Fit,
                        padding: Inset{left: 20, right: 23, top: 10, bottom: 10}
                        margin: Inset{left: 8, right: 0, top: 0, bottom: 0}
                        draw_icon +: { color: (ROOM_NAME_TEXT_COLOR) }
                        icon_walk: Walk{width: 13, height: Fit}
                        spacing: 0
                        text: ""
                    }
                }
                title_container +: {
                    // padding: Inset{top: 8}
                    title +: {
                        draw_text +: {
                            color: (ROOM_NAME_TEXT_COLOR)
                        }
                    }
                }
            }
        }
        body +: {
            // The top margin leaves room for the stack nav header.
            // The other padding is for safe inset areas.
            margin: Inset{top: (mod.widgets.STACK_VIEW_HEADER_HEIGHT)}
            padding: Inset{
                left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                bottom: (mod.widgets.SAFE_INSET_PAD_BOTTOM),
            }
        }
    }

    // A wrapper view around the SpacesBar that lets us show/hide it via animation.
    mod.widgets.SpacesBarWrapper = set_type_default() do #(SpacesBarWrapper::register_widget(vm)) {
        ..mod.widgets.RoundedShadowView

        width: Fill,
        height: (NAVIGATION_TAB_BAR_SIZE)
        margin: Inset{
            left: (4.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (4.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
        }
        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY_DARKER)
            border_radius: 4.0
            border_size: 0.0
            shadow_color: #0005
            shadow_radius: 15.0
            shadow_offset: vec2(1.0, 0.0)
        }

        CachedWidget {
            root_spaces_bar := mod.widgets.SpacesBar {}
        }

        animator: Animator{
            spaces_bar_animator: {
                default: @hide
                show: AnimatorState{
                    redraw: true
                    from: { all: Forward { duration: (mod.widgets.SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: (NAVIGATION_TAB_BAR_SIZE),  draw_bg: { shadow_color: #x00000055 } }
                }
                hide: AnimatorState{
                    redraw: true
                    from: { all: Forward { duration: (mod.widgets.SPACES_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: 0,  draw_bg: { shadow_color: (COLOR_TRANSPARENT) } }
                }
            }
        }
    }

    // The home screen widget contains the main content:
    // rooms list, room screens, and the settings screen as an overlay.
    // It adapts to both desktop and mobile layouts.
    mod.widgets.HomeScreen = #(HomeScreen::register_widget(vm)) {
        main_adaptive_view := AdaptiveView {
            // NOTE: within each of these sub views, we used `CachedWidget` wrappers
            //       to ensure that there is only a single global instance of each
            //       of those widgets, which means they maintain their state
            //       across transitions between the Desktop and Mobile variant.
            Desktop := SolidView {
                width: Fill, height: Fill
                flow: Right
                align: Align{x: 0.0, y: 0.0}
                padding: 0,
                margin: 0,

                show_bg: true
                draw_bg +: {
                    color: (COLOR_SECONDARY)
                }

                // On the left, show the navigation tab bar vertically.
                CachedWidget {
                    navigation_tab_bar := mod.widgets.NavigationTabBar {}
                }

                // To the right of that, we use the PageFlip widget to show either
                // the main desktop UI or the settings screen.
                home_screen_page_flip := PageFlip {
                    width: Fill, height: Fill
                    // We only need bottom and right-side padding,
                    // as the others are handled by the parent widget
                    // or by the navigation bar.
                    padding: Inset{
                        bottom: (mod.widgets.SAFE_INSET_PAD_BOTTOM),
                        right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                    }

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
                            // The negative left/right margins compensate for the gray border,
                            // such that the inner white input part is aligned with other elements.
                            margin: Inset{left: -1.5, right: -1.5}
                            spacing: 2
                            align: Align{y: 0.5}

                            CachedWidget {
                                room_filter_input_bar := RoomFilterInputBar {}
                            }

                            // Hide this until it's implemented.
                            // search_messages_button := SearchMessagesButton {
                            //     // make this button match/align with the RoomFilterInputBar
                            //     height: 32.5,
                            //     margin: Inset{right: 2}
                            // }
                        }

                        mod.widgets.MainDesktopUI {}
                    }

                    settings_page := RoundedView {
                        width: Fill, height: Fill
                        // This weird margin is just to make it line up with the home_page content.
                        margin: Inset{top: 3, left: 1, right: 0, bottom: 0}
                        show_bg: true,
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                            border_radius: 4.0
                        }

                        CachedWidget {
                            settings_screen := mod.widgets.SettingsScreen {}
                        }
                    }

                    add_room_page := RoundedView {
                        width: Fill, height: Fill
                        // This weird margin is just to make it line up with the home_page content.
                        margin: Inset{top: 3, left: 1, right: 0, bottom: 0}
                        show_bg: true,
                        draw_bg +: {
                            color: (COLOR_PRIMARY)
                            border_radius: 4.0
                        }

                        CachedWidget {
                            add_room_screen := mod.widgets.AddRoomScreen {}
                        }
                    }
                }
            }

            Mobile := SolidView {
                width: Fill, height: Fill
                flow: Down

                show_bg: true
                draw_bg.color: (COLOR_PRIMARY)

                view_stack := StackNavigation {
                    root_view +: {
                        flow: Down
                        width: Fill, height: Fill

                        // At the top of the root view, we use the PageFlip widget to show either
                        // the main list of rooms or the settings screen.
                        home_screen_page_flip := PageFlip {
                            width: Fill, height: Fill
                            padding: Inset{
                                left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                                right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                            }

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

                                CachedWidget {
                                    settings_screen := mod.widgets.SettingsScreen {}
                                }
                            }

                            add_room_page := View {
                                width: Fill, height: Fill

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

                    stack_templates: {
                        RoomScreenStackNavigationView := mod.widgets.RobrixStackNavigationView {
                            body +: {
                                room_screen := mod.widgets.RoomScreen {}
                            }
                        }

                        InviteScreenStackNavigationView := mod.widgets.RobrixStackNavigationView {
                            body +: {
                                invite_screen := mod.widgets.InviteScreen {}
                            }
                        }

                        SpaceLobbyScreenStackNavigationView := mod.widgets.RobrixStackNavigationView {
                            body +: {
                                space_lobby_screen := mod.widgets.SpaceLobbyScreen {}
                            }
                        }
                    }
                }
            }
        }
    }
}


/// A simple wrapper around the SpacesBar that allows us to animate showing or hiding it.
#[derive(Script, Widget, Animator)]
pub struct SpacesBarWrapper {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
}

impl ScriptHook for SpacesBarWrapper {
    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        scope: &mut Scope,
        _value: ScriptValue,
    ) {
        // When the widget tree is re-applied (e.g. after a preference change),
        // the deref `view` resets its height to the DSL default,
        // which clashes with whatever animator state we were in (shown, hidden).
        // Thus, we re-apply the current animator state to prevent a hidden SpacesBar
        // from briefly becoming shown before being hidden again.
        // Note that we can't just call `animator_cut` cuz that uses the script VM
        // which is unavailable from this `on_after_apply`
        if !apply.is_script_reapply() {
            return;
        }
        if let Some(state_apply) = self
            .animator
            .current_state_apply(live_id!(spaces_bar_animator))
        {
            self.script_apply(vm, &Apply::Animate, scope, state_apply.into());
        }
    }
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

    /// A history of previously-selected screens for mobile stack navigation.
    /// When a view is popped off the stack, the previous `selected_room` is restored.
    #[rust] mobile_screen_history: Vec<SelectedRoom>,

    /// The most recently applied view-mode override, used to short-circuit
    /// redundant `AdaptiveView` selector reinstalls when an
    /// [`AppPreferencesAction::ViewModeChanged`] action repeats the current
    /// value (e.g., the unconditional broadcast on app-state restore).
    #[rust] applied_view_mode: ViewModeOverride,

    /// The last effective AdaptiveView mode we observed. `Some(true)` means desktop mode.
    #[rust] last_effective_is_desktop: Option<bool>,
}

impl Widget for HomeScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Actions(actions) = event {
            // On desktop, the RoomFilterInputBar is inside this HomeScreen.
            // Check if it changed and re-emit as a MainFilterAction so that
            // RoomsList and SpacesBar can respond without cross-talk from
            // other RoomFilterInputBar instances (e.g., SpaceLobbyScreen's).
            if let Some(keywords) = self.view.room_filter_input_bar(cx, ids!(room_filter_input_bar)).changed(actions) {
                cx.action(MainFilterAction::Changed(keywords));
            }

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
                                    .populate(cx, None, app_state);
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

                // React to App Settings changes that affect the HomeScreen layout.
                if let Some(AppPreferencesAction::ViewModeChanged(new_mode)) = action.downcast_ref() {
                    if *new_mode != self.applied_view_mode {
                        self.apply_view_mode(cx, *new_mode);
                        self.view.redraw(cx);
                    }
                    self.sync_effective_view_mode(cx);
                }

                if let WindowAction::WindowGeomChange(_) = action.as_widget_action().cast() {
                    self.sync_effective_view_mode(cx);
                }

                // Handle room selections. Desktop owns tab creation in MainDesktopUI,
                // while mobile owns StackNavigation screen pushes here.
                match action.as_widget_action().cast() {
                    RoomsListAction::Selected(selected_room) => {
                        if effective_is_desktop(cx) {
                            app_state.selected_room = Some(selected_room);
                        } else {
                            self.push_selected_screen_view(cx, app_state, selected_room);
                        }
                    }
                    RoomsListAction::InviteAccepted { room_name_id } => {
                        cx.action(AppStateAction::UpgradedInviteToJoinedRoom(
                            room_name_id.room_id().clone(),
                        ));
                    }
                    _ => {}
                }

                if let StackNavigationTransitionAction::ViewReleased(view_id) =
                    action.as_widget_action().cast()
                {
                    let stack_navigation = self.view.stack_navigation(cx, ids!(view_stack));
                    self.hide_released_stack_navigation_view(cx, &stack_navigation, view_id);
                }

                // When a stack navigation pop is requested (back button pressed),
                // reveal the previous screen from HomeScreen's mobile history.
                if let StackNavigationAction::Pop = action.as_widget_action().cast() {
                    self.pop_selected_screen_view(cx, app_state);
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
    /// Installs a variant selector on the main `AdaptiveView` that honors the
    /// current [`ViewModeOverride`] preference. `Automatic` falls back to the
    /// default width-based selector.
    fn apply_view_mode(&mut self, cx: &mut Cx, mode: ViewModeOverride) {
        self.view
            .adaptive_view(cx, ids!(main_adaptive_view))
            .set_variant_selector(mode.variant_selector());
        self.applied_view_mode = mode;
    }

    fn sync_effective_view_mode(&mut self, cx: &mut Cx) {
        let is_desktop = effective_is_desktop(cx);
        let Some(previous_is_desktop) = self.last_effective_is_desktop.replace(is_desktop) else {
            return;
        };
        if previous_is_desktop != is_desktop {
            self.clear_mobile_navigation_state(cx);
        }
    }

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

    fn configure_mobile_stack_navigation_view(
        &mut self,
        cx: &mut Cx,
        stack_navigation: &StackNavigationRef,
        selected_screen: &SelectedRoom,
    ) -> Option<LiveId> {
        let view_id = match selected_screen {
            SelectedRoom::JoinedRoom { room_name_id }
            | SelectedRoom::Thread { room_name_id, .. } => {
                let Some((view_id, stack_navigation_view)) =
                    stack_navigation.create_view_from_template(cx, id!(RoomScreenStackNavigationView))
                else {
                    error!("BUG: failed to create mobile RoomScreen StackNavigationView");
                    return None;
                };
                Self::hide_displayed_stack_screen(cx, &stack_navigation_view);
                let thread_root = if let SelectedRoom::Thread { thread_root_event_id, .. } = selected_screen {
                    Some(thread_root_event_id.clone())
                } else {
                    None
                };
                stack_navigation_view
                    .room_screen(cx, ids!(room_screen))
                    .set_displayed_room(cx, room_name_id, thread_root);
                view_id
            }
            SelectedRoom::InvitedRoom { room_name_id } => {
                let Some((view_id, stack_navigation_view)) =
                    stack_navigation.create_view_from_template(cx, id!(InviteScreenStackNavigationView))
                else {
                    error!("BUG: failed to create mobile InviteScreen StackNavigationView");
                    return None;
                };
                Self::hide_displayed_stack_screen(cx, &stack_navigation_view);
                stack_navigation_view
                    .invite_screen(cx, ids!(invite_screen))
                    .set_displayed_invite(cx, room_name_id);
                view_id
            }
            SelectedRoom::Space { space_name_id } => {
                let Some((view_id, stack_navigation_view)) =
                    stack_navigation.create_view_from_template(cx, id!(SpaceLobbyScreenStackNavigationView))
                else {
                    error!("BUG: failed to create mobile SpaceLobbyScreen StackNavigationView");
                    return None;
                };
                Self::hide_displayed_stack_screen(cx, &stack_navigation_view);
                stack_navigation_view
                    .space_lobby_screen(cx, ids!(space_lobby_screen))
                    .set_displayed_space(cx, space_name_id);
                view_id
            }
        };

        stack_navigation.set_title(cx, view_id, &selected_screen.display_name());
        Some(view_id)
    }

    fn hide_released_stack_navigation_view(
        &mut self,
        cx: &mut Cx,
        stack_navigation: &StackNavigationRef,
        view_id: LiveId,
    ) {
        // A ViewReleased action can arrive after this StackNavigationView has
        // already been reused for a new transition. In that case, the visible
        // view is displaying a newer screen and must not be hidden by the stale
        // release.
        if stack_navigation.stack_view_ids().contains(&view_id) {
            return;
        }
        let stack_navigation_view = stack_navigation.view_by_id(cx, view_id);
        Self::hide_displayed_stack_screen(cx, &stack_navigation_view);
    }

    fn clear_mobile_navigation_state(&mut self, cx: &mut Cx) {
        self.mobile_screen_history.clear();

        let stack_navigation = self.view.stack_navigation(cx, ids!(view_stack));
        for view_id in stack_navigation.dynamic_stack_view_ids() {
            let stack_navigation_view = stack_navigation.view_by_id(cx, view_id);
            Self::hide_displayed_stack_screen(cx, &stack_navigation_view);
        }
    }

    fn hide_displayed_stack_screen(cx: &mut Cx, stack_navigation_view: &WidgetRef) {
        stack_navigation_view
            .room_screen(cx, ids!(room_screen))
            .hide_displayed_room(cx);
        stack_navigation_view
            .invite_screen(cx, ids!(invite_screen))
            .hide_displayed_invite(cx);
        stack_navigation_view
            .space_lobby_screen(cx, ids!(space_lobby_screen))
            .hide_displayed_space(cx);
    }

    /// Pushes the given screen onto the mobile screen history and animates it in.
    fn push_selected_screen_view(
        &mut self,
        cx: &mut Cx,
        app_state: &mut AppState,
        sr: SelectedRoom,
    ) {
        // Ensure the view mode is known.
        if self.last_effective_is_desktop.is_none() {
            self.last_effective_is_desktop = Some(effective_is_desktop(cx));
        }

        let stack_navigation = self.view.stack_navigation(cx, ids!(view_stack));
        if stack_navigation.is_transitioning() {
            log!("Ignoring mobile room selection while StackNavigation is transitioning");
            return;
        }
        let has_current_mobile_screen = stack_navigation.current_view().is_some();

        if has_current_mobile_screen && app_state.selected_room.as_ref().is_some_and(|c| c == &sr) {
            return;
        }

        let Some(view_id) = self.configure_mobile_stack_navigation_view(cx, &stack_navigation, &sr) else {
            return;
        };

        // Save the current selected_room onto the navigation stack before replacing it.
        if has_current_mobile_screen {
            if let Some(prev) = app_state.selected_room.take() {
                self.mobile_screen_history.push(prev);
            }
        }
        app_state.selected_room = Some(sr);
        stack_navigation.push(cx, view_id);
        self.view.redraw(cx);
    }

    /// Pops the current mobile screen, revealing the previous screen or the room list root.
    fn pop_selected_screen_view(&mut self, cx: &mut Cx, app_state: &mut AppState) {
        let stack_navigation = self.view.stack_navigation(cx, ids!(view_stack));
        if stack_navigation.is_transitioning() {
            return;
        }

        let Some(current_screen) = app_state.selected_room.take() else {
            return;
        };
        let previous_screen = self.mobile_screen_history.pop();
        match previous_screen {
            Some(previous_screen) => {
                let Some(view_id) = self.configure_mobile_stack_navigation_view(cx, &stack_navigation, &previous_screen) else {
                    app_state.selected_room = Some(current_screen);
                    self.mobile_screen_history.push(previous_screen);
                    return;
                };
                app_state.selected_room = Some(previous_screen);
                stack_navigation.pop_to_view(cx, view_id);
            }
            None => {
                app_state.selected_room = None;
                stack_navigation.pop_to_root(cx);
            }
        }
        self.view.redraw(cx);
    }
}
