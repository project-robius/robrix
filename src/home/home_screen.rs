use makepad_widgets::*;

use crate::{
    app::{AppState, AppStateAction, SelectedRoom},
    home::{
        invite_screen::InviteScreenWidgetExt,
        navigation_tab_bar::{NavigationBarAction, SelectedTab},
        room_screen::RoomScreenWidgetExt,
        rooms_list::RoomsListAction,
        space_lobby::SpaceLobbyScreenWidgetExt,
    },
    settings::{
        app_preferences::{AppPreferencesAction, ViewModeOverride},
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
    // Each specific content view (room, invite, space lobby) extends this
    // and places its own screen widget inside the body.
    mod.widgets.RobrixContentView = StackNavigationView {
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
                // Inset the header controls (back button, title) by the safe area so they
                // don't render under a side cutout, while the header background (with its
                // shadow in draw_bg above) still extends edge-to-edge — matching the iOS
                // UINavigationBar pattern where the bar background spans the full width
                // and items anchor to the safe-area layout guide.
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
            // Top margin leaves room for the StackNavigationView's header.
            // Left/right/bottom padding respects the device safe area so room / invite /
            // space-lobby screens don't render under the Dynamic Island, notch, or home
            // indicator. The bottom inset matters here because pushed StackNavigationViews
            // are drawn fullscreen by StackNavigation and bypass the root_view's
            // NavigationTabBar (which would otherwise own the bottom edge), so each
            // pushed view must apply its own bottom inset to keep the message-input bar
            // and similar bottom-anchored controls clear of the home indicator.
            // Top safe-inset above the header is handled by StackNavigation itself
            // (stack_navigation.rs uses `safe_top.max(parent_rect.pos.y)` in fullscreen mode).
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
        // 4px decorative gap + safe-area inset so the SpacesBar doesn't render under a
        // side cutout in iPhone landscape. Using addition (rather than `max()`) because
        // DSL `max()` with `mod.widgets.X` heap paths does not evaluate reliably (see
        // image_viewer.rs::draw_walk for context). On desktop the safe inset is 0, so
        // this collapses to the original 4px; on mobile devices with a side cutout the
        // SpacesBar gets the cutout's inset plus 4px of decorative breathing room.
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
                    // Bottom padding absorbs the BOTTOM safe-area inset (home indicator).
                    // Pages with their own background color (settings_page, add_room_page)
                    // would otherwise extend under the home indicator and show COLOR_PRIMARY
                    // (white) there. With this padding, the pages stop above the inset and
                    // the parent Desktop SolidView's COLOR_SECONDARY fills the inset strip,
                    // matching the NavigationTabBar's color for a seamless bottom edge.
                    // Right padding respects a right-side cutout (e.g., iPhone landscape
                    // running in Desktop mode); left is owned by the NavigationTabBar.
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
                            margin: Inset{right: 2}
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

                    // The settings and add-room pages use RoundedView rather than a flat
                    // SolidView, with a small margin that reveals the outer Desktop
                    // SolidView's COLOR_SECONDARY background on all sides. This creates a
                    // "floating card" look that visually matches the RobrixDock on the
                    // home page (whose tab panels are similarly framed in COLOR_SECONDARY),
                    // softens the otherwise-sharp seam with the NavigationTabBar at the
                    // top-right, and rounds the visible inside-screen corners.
                    settings_page := RoundedView {
                        width: Fill, height: Fill
                        // Small asymmetric margin matches the home page's visual frame:
                        // top: 2 (gap under the window body's top padding / status-bar area),
                        // left: 1 (thin COLOR_SECONDARY strip separating card from NavigationTabBar),
                        // right: 0 / bottom: 0 (flush with body right-inset and the home-indicator
                        // strip owned by home_screen_page_flip's padding). 4px corner radius
                        // matches the dock's round_corner shader (dock.rs sdf.box r=4).
                        // Top/left tuned to visually match the home page's RobrixDock;
                        // right/bottom stay flush with home_screen_page_flip's padded inner
                        // area (which already applies SAFE_INSET_PAD_RIGHT and
                        // SAFE_INSET_PAD_BOTTOM at its level), so the outer Desktop
                        // COLOR_SECONDARY frame fills the safe-area strips uniformly.
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
                        // Small asymmetric margin matches the home page's visual frame:
                        // top: 2 (gap under the window body's top padding / status-bar area),
                        // left: 1 (thin COLOR_SECONDARY strip separating card from NavigationTabBar),
                        // right: 0 / bottom: 0 (flush with body right-inset and the home-indicator
                        // strip owned by home_screen_page_flip's padding). 4px corner radius
                        // matches the dock's round_corner shader (dock.rs sdf.box r=4).
                        // Top/left tuned to visually match the home page's RobrixDock;
                        // right/bottom stay flush with home_screen_page_flip's padded inner
                        // area (which already applies SAFE_INSET_PAD_RIGHT and
                        // SAFE_INSET_PAD_BOTTOM at its level), so the outer Desktop
                        // COLOR_SECONDARY frame fills the safe-area strips uniformly.
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
                            // Left/right safe-area insets applied at the page-flip level so all
                            // three pages (home, settings, add_room) respect device cutouts
                            // (Dynamic Island in landscape, camera notch, etc.). The
                            // NavigationTabBar handles its own left/right insets separately and
                            // sits below this, outside the page flip.
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

                    // Room views: multiple instances to support deep stacking
                    // (e.g., room -> thread -> room -> thread -> ...).
                    // Each stack depth gets its own dedicated view widget,
                    // avoiding complex state save/restore when views are reused.
                    room_view_0  := mod.widgets.RobrixContentView { body +: { room_screen_0  := mod.widgets.RoomScreen {} } }
                    room_view_1  := mod.widgets.RobrixContentView { body +: { room_screen_1  := mod.widgets.RoomScreen {} } }
                    room_view_2  := mod.widgets.RobrixContentView { body +: { room_screen_2  := mod.widgets.RoomScreen {} } }
                    room_view_3  := mod.widgets.RobrixContentView { body +: { room_screen_3  := mod.widgets.RoomScreen {} } }
                    room_view_4  := mod.widgets.RobrixContentView { body +: { room_screen_4  := mod.widgets.RoomScreen {} } }
                    room_view_5  := mod.widgets.RobrixContentView { body +: { room_screen_5  := mod.widgets.RoomScreen {} } }
                    room_view_6  := mod.widgets.RobrixContentView { body +: { room_screen_6  := mod.widgets.RoomScreen {} } }
                    room_view_7  := mod.widgets.RobrixContentView { body +: { room_screen_7  := mod.widgets.RoomScreen {} } }
                    room_view_8  := mod.widgets.RobrixContentView { body +: { room_screen_8  := mod.widgets.RoomScreen {} } }
                    room_view_9  := mod.widgets.RobrixContentView { body +: { room_screen_9  := mod.widgets.RoomScreen {} } }
                    room_view_10 := mod.widgets.RobrixContentView { body +: { room_screen_10 := mod.widgets.RoomScreen {} } }
                    room_view_11 := mod.widgets.RobrixContentView { body +: { room_screen_11 := mod.widgets.RoomScreen {} } }
                    room_view_12 := mod.widgets.RobrixContentView { body +: { room_screen_12 := mod.widgets.RoomScreen {} } }
                    room_view_13 := mod.widgets.RobrixContentView { body +: { room_screen_13 := mod.widgets.RoomScreen {} } }
                    room_view_14 := mod.widgets.RobrixContentView { body +: { room_screen_14 := mod.widgets.RoomScreen {} } }
                    room_view_15 := mod.widgets.RobrixContentView { body +: { room_screen_15 := mod.widgets.RoomScreen {} } }

                    invite_view := mod.widgets.RobrixContentView {
                        body +: {
                            invite_screen := mod.widgets.InviteScreen {}
                        }
                    }

                    space_lobby_view := mod.widgets.RobrixContentView {
                        body +: {
                            space_lobby_screen := mod.widgets.SpaceLobbyScreen {}
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

    /// A stack of previously-selected rooms for mobile stack navigation.
    /// When a view is popped off the stack, the previous `selected_room` is restored.
    #[rust] mobile_room_nav_stack: Vec<SelectedRoom>,

    /// The most recently applied view-mode override, used to short-circuit
    /// redundant `AdaptiveView` selector reinstalls when an
    /// [`AppPreferencesAction::ViewModeChanged`] action repeats the current
    /// value (e.g., the unconditional broadcast on app-state restore).
    #[rust] applied_view_mode: ViewModeOverride,
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
                }

                // Handle mobile stack navigation actions (push/pop room views).
                // In Desktop mode, MainDesktopUI also handles RoomsListAction::Selected
                // to manage dock tabs; the mobile push is harmless there (views aren't drawn).
                match action.as_widget_action().cast() {
                    RoomsListAction::Selected(selected_room) => {
                        self.push_selected_room_view(cx, app_state, selected_room);
                    }
                    RoomsListAction::InviteAccepted { room_name_id } => {
                        cx.action(AppStateAction::UpgradedInviteToJoinedRoom(
                            room_name_id.room_id().clone(),
                        ));
                    }
                    _ => {}
                }

                // When a stack navigation pop is initiated (back button pressed),
                // pop the mobile nav stack so it stays in sync with StackNavigation.
                if let StackNavigationAction::Pop = action.as_widget_action().cast() {
                    if app_state.selected_room.is_some() {
                        app_state.selected_room = self.mobile_room_nav_stack.pop();
                    }
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

    /// Room StackNavigationView instances, one per stack depth.
    /// Each depth gets its own dedicated view widget to avoid
    /// complex state save/restore when views would otherwise be reused.
    const ROOM_VIEW_IDS: [LiveId; 16] = [
        live_id!(room_view_0),  live_id!(room_view_1),
        live_id!(room_view_2),  live_id!(room_view_3),
        live_id!(room_view_4),  live_id!(room_view_5),
        live_id!(room_view_6),  live_id!(room_view_7),
        live_id!(room_view_8),  live_id!(room_view_9),
        live_id!(room_view_10), live_id!(room_view_11),
        live_id!(room_view_12), live_id!(room_view_13),
        live_id!(room_view_14), live_id!(room_view_15),
    ];

    /// The RoomScreen widget IDs inside each room view,
    /// corresponding 1:1 with [`Self::ROOM_VIEW_IDS`].
    const ROOM_SCREEN_IDS: [LiveId; 16] = [
        live_id!(room_screen_0),  live_id!(room_screen_1),
        live_id!(room_screen_2),  live_id!(room_screen_3),
        live_id!(room_screen_4),  live_id!(room_screen_5),
        live_id!(room_screen_6),  live_id!(room_screen_7),
        live_id!(room_screen_8),  live_id!(room_screen_9),
        live_id!(room_screen_10), live_id!(room_screen_11),
        live_id!(room_screen_12), live_id!(room_screen_13),
        live_id!(room_screen_14), live_id!(room_screen_15),
    ];

    /// Returns the room view and room screen LiveIds for the given stack depth.
    /// Clamps to the last available view if depth exceeds the pool size.
    fn room_ids_for_depth(depth: usize) -> (LiveId, LiveId) {
        let index = depth.min(Self::ROOM_VIEW_IDS.len() - 1);
        (Self::ROOM_VIEW_IDS[index], Self::ROOM_SCREEN_IDS[index])
    }

    /// Pushes the appropriate StackNavigationView for the given `SelectedRoom`,
    /// configuring the view's content widget and header title.
    ///
    /// Each stack depth gets its own dedicated room view widget,
    /// supporting deep navigation (room → thread → room → thread → ...).
    fn push_selected_room_view(
        &mut self,
        cx: &mut Cx,
        app_state: &mut AppState,
        selected_room: SelectedRoom,
    ) {
        let new_depth = self.view.stack_navigation(cx, ids!(view_stack)).depth();

        let view_id = match &selected_room {
            SelectedRoom::JoinedRoom { room_name_id }
            | SelectedRoom::Thread { room_name_id, .. } => {
                let (view_id, room_screen_id) = Self::room_ids_for_depth(new_depth);
                let thread_root = if let SelectedRoom::Thread { thread_root_event_id, .. } = &selected_room {
                    Some(thread_root_event_id.clone())
                } else {
                    None
                };
                self.view
                    .room_screen(cx, &[room_screen_id])
                    .set_displayed_room(cx, room_name_id, thread_root);
                view_id
            }
            SelectedRoom::InvitedRoom { room_name_id } => {
                self.view
                    .invite_screen(cx, ids!(invite_screen))
                    .set_displayed_invite(cx, room_name_id);
                id!(invite_view)
            }
            SelectedRoom::Space { space_name_id } => {
                self.view
                    .space_lobby_screen(cx, ids!(space_lobby_screen))
                    .set_displayed_space(cx, space_name_id);
                id!(space_lobby_view)
            }
        };

        // Set the header title once. `set_title` stores it on the
        // `StackNavigationView` itself, which re-asserts it on every apply walk
        // (rotation, preference change, AdaptiveView swap, etc.).
        let stack_navigation = self.view.stack_navigation(cx, ids!(view_stack));
        stack_navigation.set_title(cx, view_id, &selected_room.display_name());

        // Save the current selected_room onto the navigation stack before replacing it.
        if let Some(prev) = app_state.selected_room.take() {
            self.mobile_room_nav_stack.push(prev);
        }
        app_state.selected_room = Some(selected_room);

        // Push the view onto the mobile navigation stack.
        stack_navigation.push(cx, view_id);
        self.view.redraw(cx);
    }
}

