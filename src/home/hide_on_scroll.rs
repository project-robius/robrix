//! `HideOnScrollView`: the mobile root view, whose header and bottom bars slide out of view
//! while the user scrolls down a page, and back in when they scroll up.
//!
//! The bars move along with the content as it scrolls, then finish sliding in or out (whichever is closer)
//! once it stops. Blank spaces in the content make room for the bars, so sliding them never moves the content.
//! If the user keeps the bottom bars shown, the nav bar shrinks into a floating pill instead (see `nav_bar_pill`).

use makepad_widgets::{animator::Ease, makepad_platform::event::finger::TouchState, *};

use crate::{
    app::AppState,
    home::{
        home_screen::{SPACES_BAR_SHADOW_RADIUS, SpacesBarWrapperRef, SpacesBarWrapperWidgetExt},
        nav_bar_pill::{PillMotion, PillState, ShowSpacesBarFromPillAction},
        navigation_tab_bar::{NavigationBarAction, NavigationTabBarRef, NavigationTabBarWidgetExt, SelectedTab},
        rooms_list::{SIDE_PADDING, RoomsListRef, RoomsListWidgetExt},
    },
    logout::logout_confirm_modal::LogoutAction,
    profile::user_profile::{UserProfileSlidingPaneRef, UserProfileSlidingPaneWidgetExt},
    settings::app_preferences::AppPreferencesGlobal,
};

/// How far the header's shadow reaches below it, which also has to slide out of view.
const HEADER_SHADOW_RADIUS: f64 = 12.0;
/// How long the shortest slide to fully shown or hidden takes. Longer slides take up to twice as long.
const SNAP_DURATION_SECS: f64 = 0.15;
/// How long a mouse wheel or trackpad has to stop scrolling before the bars finish sliding.
const WHEEL_STOP_DELAY_SECS: f64 = 0.15;
/// How far makepad keeps a widget that gets key focus from the edges of the view it scrolls into.
const FOCUS_SCROLL_MARGIN: f64 = 5.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    mod.widgets.HideOnScrollView = #(HideOnScrollView::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Overlay
        align: Align{y: 1.0}

        content := View {
            width: Fill, height: Fill
        }

        // This doesn't clip, so the spaces bar's shadow still shows above it,
        // and its cursor keeps hovers over its empty space from reaching the content beneath it.
        bottom_bars := View {
            width: Fill, height: Fit
            flow: Down
            clip_x: false, clip_y: false
            cursor: MouseCursor.Default
            grab_key_focus: false
        }

        header_area := View {
            width: Fill, height: Fill
            flow: Overlay

            // While the header is hidden, the content beneath the status bar fades out under it,
            // like iOS's scroll edge effect.
            status_bar_fade := View {
                width: Fill, height: 0
                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY)
                    // Nothing above this gets faded, so a page's own header can show through.
                    clip_top: uniform(0.0)
                    pixel: fn() {
                        // Mostly opaque across the top fifth of the status bar, then easing out to nothing at its bottom edge.
                        let alpha = 0.835 * (1.0 - smoothstep(0.2, 1.0, self.pos.y)) * step(self.clip_top, self.pos.y * self.rect_size.y)
                        return vec4(self.color.rgb * alpha, alpha)
                    }
                }
            }

            // The rooms list's title, filter bar, and filter bubbles, drawn over the top of the rooms list.
            header := RoundedShadowView {
                width: Fill, height: Fit
                margin: Inset{
                    left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                    right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
                }
                // The header's background extends up through the status bar area above its content.
                padding: Inset{top: (15.0 + mod.widgets.SAFE_INSET_PAD_TOP), left: #(SIDE_PADDING), right: #(SIDE_PADDING), bottom: 10}
                flow: Down,
                // This keeps hovers over the header's empty space from reaching the rooms beneath it.
                cursor: MouseCursor.Default
                grab_key_focus: false

                show_bg: true
                draw_bg +: {
                    color: (COLOR_PRIMARY_DARKER)
                    border_radius: 4.0
                    border_size: 0.0
                    shadow_color: (COLOR_HEADER_SHADOW)
                    shadow_radius: #(HEADER_SHADOW_RADIUS)
                    shadow_offset: vec2(0.0, 0.0)

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

                        // The box starts at the top of the shadow's area, so the clip cuts off its top edge and corners.
                        // Otherwise their anti-aliasing would let the list show through the header's top row.
                        sdf.box_y(
                            self.sdf_rect_pos.x
                            0.0
                            self.sdf_rect_size.x
                            self.sdf_rect_pos.y + self.sdf_rect_size.y
                            0.0
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

                CachedWidget {
                    rooms_list_header := RoomsListHeader {}
                }

                View {
                    width: Fill,
                    height: 45,
                    flow: Right
                    padding: Inset{top: 5, bottom: 2}
                    spacing: 5
                    align: Align{y: 0.5}

                    CachedWidget {
                        room_filter_input_bar := RoomFilterInputBar { is_main_filter: true }
                    }

                    // Hide this until it's implemented.
                    // search_messages_button := SearchMessagesButton { }
                }

                // Reaches out to the header's edges, so the bubbles scroll out of view right at the screen's edges.
                View {
                    width: Fill, height: Fit
                    margin: Inset{top: 6, left: #(-SIDE_PADDING), right: #(-SIDE_PADDING)}
                    CachedWidget {
                        room_filter_bubbles := RoomFilterBubbles {}
                    }
                }
            }
        }
    }
}


/// The mobile root view, which slides the shown page's header up and its `bottom_bars` down,
/// out of view, while the user scrolls down that page. The user's preferences pick which of them hide.
///
/// Its `content` holds the current page, and its `bottom_bars` sit at the bottom, over that page.
#[derive(Script, Widget)]
pub struct HideOnScrollView {
    #[deref] view: View,

    /// How far the bars have slid out of view, from 0 (fully shown) to 1 (fully hidden).
    #[rust] hidden_fraction: f64,
    /// The nav bar shrinking into its floating pill, which it does instead of hiding if the bottom bars don't hide.
    #[rust] pill: PillMotion,
    /// Whether the spaces bar waits out of view for the nav bar to fully expand from its pill, and then shows.
    #[rust] is_spaces_bar_waiting_to_show: bool,
    /// How tall the rooms list's header is, including its part behind the status bar.
    #[rust] header_height: f64,
    /// How tall the header of the Settings or AddRoom page we drew last is. It sits below the status bar.
    #[rust] page_header_height: f64,
    #[rust] bottom_bars_height: f64,
    /// The page we drew last time, if we've drawn it since the selected tab last changed.
    /// We only draw our `header` along with the rooms list, since the other pages have their own.
    #[rust] drawn_page: Option<Page>,
    /// The touches that started on this view and haven't ended yet.
    #[rust] touches_down: Vec<u64>,
    #[rust] is_mouse_down: bool,
    #[rust] snap: Snap,
    #[rust] child_refs: Option<ChildRefs>,
    /// The rooms list and its PortalList, which only exist once the rooms list page has been shown.
    #[rust] rooms_list: Option<(RoomsListRef, PortalListRef)>,
    /// The Settings and AddRoom pages' header and scrolling content, which only exist once that page has been shown.
    #[rust] settings_page: Option<PageRefs>,
    #[rust] add_room_page: Option<PageRefs>,
    #[rust] is_soft_keyboard_up: bool,
}

#[derive(Clone)]
struct ChildRefs {
    header: ViewRef,
    status_bar_fade: ViewRef,
    bottom_bars: ViewRef,
    spaces_bar_wrapper: SpacesBarWrapperRef,
    navigation_tab_bar: NavigationTabBarRef,
    filter_input: TextInputRef,
}

impl ScriptHook for HideOnScrollView {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_reload() {
            self.child_refs = None;
            self.rooms_list = None;
            self.settings_page = None;
            self.add_room_page = None;
        }
    }
}

impl Widget for HideOnScrollView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let ChildRefs { header, bottom_bars, spaces_bar_wrapper, navigation_tab_bar, filter_input, .. } = self.child_refs(cx).clone();
        let (old_fraction, old_pill_state) = (self.hidden_fraction, self.pill.state);
        // We go by the event's kind, not its height, since a floating Android keyboard reports no height.
        if let Event::VirtualKeyboard(keyboard_event) = event {
            self.is_soft_keyboard_up = matches!(
                keyboard_event,
                VirtualKeyboardEvent::WillShow { .. } | VirtualKeyboardEvent::DidShow { .. }
            );
        }
        if let Event::Actions(actions) = event {
            if actions.iter().any(|action|
                matches!(action.downcast_ref(), Some(NavigationBarAction::TabSelected(_)))
                    || matches!(action.downcast_ref(), Some(LogoutAction::ClearAppState { .. }))
            ) {
                // Every page starts out with its bars shown (logging out goes back to Home without a `TabSelected`).
                // The old page can keep coasting, so we stop following it until the new one is drawn.
                self.snap.stop(cx);
                self.hidden_fraction = 0.0;
                self.pill = PillMotion::default();
                self.drawn_page = None;
            } else if actions.iter().any(|action| action.downcast_ref::<ShowSpacesBarFromPillAction>().is_some()) {
                // A spaces bar that's still partly in view just slides back up along with the nav bar.
                self.is_spaces_bar_waiting_to_show = !spaces_bar_wrapper.is_shown(cx) || self.pill.state.shrunk_fraction >= 1.0;
                self.pill.expand_on_tap(cx);
            }
        }
        self.pill.handle_event(cx, event);
        // The spaces bar shows as soon as the nav bar first reaches its full size,
        // but if the nav bar shrinks back into its pill first, the spaces bar stays as it was.
        if self.is_spaces_bar_waiting_to_show {
            let PillState { is_shrunk, shrunk_fraction } = self.pill.state;
            if !is_shrunk && shrunk_fraction <= 0.0 {
                self.is_spaces_bar_waiting_to_show = false;
                spaces_bar_wrapper.show_from_hidden(cx);
            } else if is_shrunk && shrunk_fraction >= 1.0 {
                self.is_spaces_bar_waiting_to_show = false;
            }
        }

        let prefs = &cx.global::<AppPreferencesGlobal>().0;
        let (hides_header, hides_bottom_bars) = (prefs.hide_header_on_scroll, prefs.hide_bottom_bars_on_scroll);
        let scroller = match self.drawn_page {
            Some(Page::RoomsList) => self.rooms_list(cx).map(|(_, list)| Scroller::RoomsList(list)),
            Some(page) => self.page_refs(cx, page).map(|refs| Scroller::Page(refs.scroll_view)),
            None => None,
        };
        let position_before = scroller.as_ref().map(Scroller::position);

        // A press on the header or the bottom bars doesn't go to the content beneath them,
        // since the content would also start scrolling from that press.
        let is_pressed_on = |area: Area| match event {
            Event::MouseDown(e) => area.clipped_rect(cx).contains(e.abs),
            Event::TouchUpdate(e) => e.touches.iter().any(|touch|
                touch.state == TouchState::Start && area.clipped_rect(cx).contains(touch.abs)
            ),
            _ => false,
        };
        // While the nav bar is a pill, the content beside it gets the presses there.
        let bars_area = if self.pill.state.is_shrunk { navigation_tab_bar.pill_area() } else { bottom_bars.area() };
        // A page's open dropdown holds the sweep lock, and its menu can reach over the bars, so it still gets those presses.
        let is_pressed_on_bars = cx.sweep_lock_area().is_none() && self.drawn_page.is_some_and(|page|
            (page == Page::RoomsList && is_pressed_on(header.area())) || is_pressed_on(bars_area)
        );
        if is_pressed_on_bars {
            header.handle_event(cx, event, scope);
            bottom_bars.handle_event(cx, event, scope);
        } else {
            self.view.handle_event(cx, event, scope);
        }

        let was_pressed = !self.touches_down.is_empty() || self.is_mouse_down;
        // A press that ended while this view was covered never reached us,
        // so we only keep the presses that are still down.
        if cx.fingers.first_mouse_button.is_none() {
            self.is_mouse_down = false;
        }
        match event {
            Event::TouchUpdate(e) => {
                self.touches_down.retain(|uid| e.touches.iter().any(|touch|
                    touch.uid == *uid && !matches!(touch.state, TouchState::Stop | TouchState::Cancel)
                ));
                // Touching this view catches the bars wherever they are,
                // and touching the content starts a new scroll, which lets a tapped pill shrink again.
                for touch in &e.touches {
                    if touch.state == TouchState::Start && self.view.area().rect(cx).contains(touch.abs) {
                        self.touches_down.push(touch.uid);
                        self.snap.stop(cx);
                        if !is_pressed_on_bars {
                            self.pill.is_held_open = false;
                        }
                    }
                }
            }
            Event::MouseDown(e) if self.view.area().rect(cx).contains(e.abs) => {
                self.is_mouse_down = true;
                self.snap.stop(cx);
                if !is_pressed_on_bars {
                    self.pill.is_held_open = false;
                }
            }
            Event::MouseUp(_) => self.is_mouse_down = false,
            Event::Scroll(_) => self.pill.is_held_open = false,
            _ => {}
        }
        let is_pressed = !self.touches_down.is_empty() || self.is_mouse_down;

        if let (Some(scroller), Some(position_before)) = (scroller, position_before) {
            let rooms_list = match &scroller {
                Scroller::RoomsList(list) => Some(list),
                Scroller::Page(_) => None,
            };
            let mut position_after = scroller.position();
            let status_bar_height = cx.display_context.safe_area_insets.top;
            // Key focus scrolls its target into the page's view, which reaches under the bars and the status bar.
            let mut is_focus_scrolling = false;
            if let (Event::Trigger(trigger_event), Scroller::Page(scroll_view)) = (event, &scroller)
                // We follow the last such request, since AddRoom swaps makepad's Tab request for its own, which comes last.
                && let Some(trigger) = trigger_event.triggers.get(&scroll_view.area())
                    .and_then(|triggers| triggers.iter().rev().find(|trigger| trigger.id == live_id!(scroll_focus_nav)))
            {
                let view = self.view.area().rect(cx);
                let bars = bars_area.clipped_rect(cx);
                let target = trigger.from.rect(cx);
                // The target was drawn before this event scrolled the page. These are all from the top of this view.
                let target_top = target.pos.y - (position_before.1 - position_after.1) - view.pos.y;
                let target_bottom = target_top + target.size.y;
                let shown_header_bottom = status_bar_height + self.page_header_height;
                let header_fraction = if hides_header { self.hidden_fraction } else { 0.0 };
                let visible_top = status_bar_height.max(shown_header_bottom * (1.0 - header_fraction));
                let visible_bottom = if bars.size.y > 0.0 { bars.pos.y - view.pos.y } else { view.size.y };
                // Revealing a target scrolls the page by itself, which brings the bars back,
                // so we keep it between where they'll rest. One that's too tall to fit there shows its top.
                if position_after != position_before
                    || target_top < visible_top + FOCUS_SCROLL_MARGIN
                    || target_bottom > visible_bottom - FOCUS_SCROLL_MARGIN
                {
                    is_focus_scrolling = true;
                    let bottom_limit = view.size.y - self.bottom_bars_height - FOCUS_SCROLL_MARGIN;
                    let shift = (target_bottom - bottom_limit).max(0.0).min(target_top - shown_header_bottom - FOCUS_SCROLL_MARGIN);
                    if shift != 0.0 {
                        scroll_view.set_scroll_pos(cx, dvec2(0.0, shift - position_after.1));
                        position_after = scroller.position();
                    }
                }
            }
            // Only the user scrolling the content moves the bars. The rooms list jumping to another item,
            // stretching past its top, or being pushed past its end doesn't count.
            let scroll_delta = if position_after.0 != position_before.0 {
                0.0
            } else {
                let clamp_to_top = |(first_id, first_scroll): (usize, f64)| if first_id == 0 { first_scroll.min(0.0) } else { first_scroll };
                let delta = clamp_to_top(position_before) - clamp_to_top(position_after);
                if delta > 0.0 && rooms_list.is_some_and(|list| list.is_at_end()) { 0.0 } else { delta }
            };
            // E.g., the rooms list scrolling to a room the user navigated to, or a page to what got key focus.
            let is_scrolling_by_itself = is_focus_scrolling || rooms_list.is_some_and(|list| list.is_smooth_scrolling().is_some());
            // Only the rooms list's header has a filter bar, which keeps the bars shown while the user types into it
            // (on a phone without a hardware keyboard, that's only while the soft keyboard is up).
            let is_filtering = rooms_list.is_some() && filter_input.key_focus(cx)
                && (self.is_soft_keyboard_up
                    || cx.keyboard.has_physical_keyboard()
                    || !cfg!(any(target_os = "android", target_os = "ios")));
            // A page always shows its bottom bars at its very top, e.g., once its content got too short to scroll.
            let is_page_at_top = rooms_list.is_none() && position_after.1 > -0.5;
            // Where the header's bottom edge rests. The rooms list's header reaches up behind the status bar,
            // while the other pages' headers sit below it.
            let (header_bottom, header_shadow_radius) = if rooms_list.is_some() {
                (self.header_height, HEADER_SHADOW_RADIUS)
            } else {
                (status_bar_height + self.page_header_height, 0.0)
            };

            // The nav bar shrinks into its pill instead of hiding, but not while the user is typing into the filter bar.
            if !hides_bottom_bars {
                if is_scrolling_by_itself || is_page_at_top {
                    self.pill.set_shrunk(cx, false);
                } else if scroll_delta != 0.0 {
                    let distance_from_top = if position_after.0 == 0 { -position_after.1 } else { f64::INFINITY };
                    self.pill.on_scroll(cx, scroll_delta, distance_from_top, !is_filtering);
                }
            } else {
                // The bars hide instead, e.g., since the user turned that on while the bar was a pill.
                self.pill.set_shrunk(cx, false);
            }

            if hides_header || hides_bottom_bars {
                // The header moves along with the content, or the bottom bars do if the header doesn't hide.
                let hide_distance = if hides_header {
                    header_bottom + header_shadow_radius
                } else {
                    self.bottom_bars_height + SPACES_BAR_SHADOW_RADIUS
                };
                // How far the content scrolls from its top to rest just below the status bar.
                let rest_scroll = header_bottom - status_bar_height;

                if is_scrolling_by_itself {
                    if !matches!(&self.snap, Snap::Sliding(slide) if slide.to_fraction == 0.0) {
                        self.slide_to(cx, 0.0, None);
                    }
                }
                else if scroll_delta != 0.0 {
                    // The bars stay shown while the user is typing into the filter bar.
                    if scroll_delta < 0.0 || !is_filtering {
                        self.hidden_fraction = (self.hidden_fraction + scroll_delta / hide_distance).clamp(0.0, 1.0);
                    }
                    self.snap.stop(cx);
                    if let Event::Scroll(_) = event {
                        self.snap = Snap::AwaitingWheelStop(cx.start_timeout(WHEEL_STOP_DELAY_SECS));
                    } else if !is_pressed {
                        self.snap = Snap::AwaitingScrollStop(cx.new_next_frame());
                    }
                }
                // Once the user lets go, the bars finish sliding as soon as the content stops moving.
                if was_pressed && !is_pressed {
                    self.snap.stop(cx);
                    self.snap = Snap::AwaitingScrollStop(cx.new_next_frame());
                }

                let mut should_snap = false;
                let mut should_slide_back = false;
                match &mut self.snap {
                    Snap::AwaitingScrollStop(next_frame) if next_frame.is_event(event).is_some() => {
                        if position_after != position_before {
                            *next_frame = cx.new_next_frame();
                        } else {
                            should_snap = true;
                        }
                    }
                    Snap::AwaitingWheelStop(timer) if timer.is_event(event).is_some() => should_snap = !is_pressed,
                    Snap::Sliding(slide) if slide.next_frame.is_event(event).is_some() => {
                        // Content that's too short to scroll its header space away stops short of where we moved it,
                        // so the bars slide back in instead.
                        if slide.content_scroll.is_some_and(|(_, to)| to > 0.0)
                            && position_after.0 == 0
                            && -position_after.1 < slide.last_content_scroll - 0.5
                        {
                            should_slide_back = true;
                        } else {
                            let progress = Ease::OutQuad.map(((Cx::monotonic_now() - slide.started_at) / slide.duration_secs).min(1.0));
                            self.hidden_fraction = slide.from_fraction + (slide.to_fraction - slide.from_fraction) * progress;
                            if let Some((from, to)) = slide.content_scroll {
                                slide.last_content_scroll = from + (to - from) * progress;
                                match &scroller {
                                    Scroller::RoomsList(list) => {
                                        list.set_first_id_and_scroll_in_place(0, -slide.last_content_scroll);
                                        list.redraw(cx);
                                    }
                                    Scroller::Page(scroll_view) => {
                                        scroll_view.set_scroll_pos(cx, dvec2(0.0, slide.last_content_scroll));
                                        scroll_view.redraw(cx);
                                    }
                                }
                            }
                            if progress < 1.0 {
                                slide.next_frame = cx.new_next_frame();
                            } else {
                                self.snap = Snap::None;
                            }
                        }
                    }
                    _ => {}
                }
                if should_slide_back {
                    self.slide_to(cx, 0.0, None);
                }
                if should_snap {
                    self.snap.stop(cx);
                    if self.hidden_fraction > 0.0 && self.hidden_fraction < 1.0 {
                        let scrolled = -position_after.1;
                        let shows_header_space = position_after.0 == 0 && scrolled < header_bottom - 0.5;
                        let is_at_end = match &scroller {
                            Scroller::RoomsList(list) => list.is_at_end(),
                            Scroller::Page(scroll_view) => scroll_view.scroll_pos().y >= scroll_view.max_scroll_pos().y - 0.5,
                        };
                        let is_too_short = position_after.0 == 0 && scrolled < rest_scroll - 0.5 && is_at_end;
                        // The bars slide fully in or out, whichever is closer. They slide in if the filter bar
                        // has key focus, or if the content is too short to scroll its header space away.
                        let to_fraction = if self.hidden_fraction < 0.5 || is_filtering || is_too_short {
                            0.0
                        } else {
                            1.0
                        };
                        // While the content shows part of its header space, it scrolls along with the header,
                        // either back to its top or until it comes to rest below the status bar.
                        let content_scroll = (hides_header && shows_header_space && !is_too_short)
                            .then(|| (scrolled.max(0.0), if to_fraction == 0.0 { 0.0 } else { rest_scroll }));
                        self.slide_to(cx, to_fraction, content_scroll);
                    }
                }

                // Near the top, the header can only be as hidden as the content is close to coming to rest,
                // so it's fully shown at the very top, and it never jumps when the content leaves its resting place.
                let (first_id, first_scroll) = scroller.position();
                if hides_header && first_id == 0 && -first_scroll < rest_scroll - 0.5 {
                    self.hidden_fraction = self.hidden_fraction.min((-first_scroll).max(0.0) / rest_scroll);
                }
                if is_page_at_top
                    && self.hidden_fraction > 0.0
                    && !matches!(&self.snap, Snap::Sliding(slide) if slide.to_fraction == 0.0)
                {
                    self.slide_to(cx, 0.0, None);
                }
                // The bars never stay partly hidden, e.g., if the content got shorter under hidden bars.
                if matches!(self.snap, Snap::None) && !is_pressed && self.hidden_fraction > 0.0 && self.hidden_fraction < 1.0 {
                    self.snap = Snap::AwaitingScrollStop(cx.new_next_frame());
                }
            }
        }

        let has_pill_changed = self.pill.state != old_pill_state;
        if self.hidden_fraction != old_fraction || has_pill_changed {
            self.view.area().redraw(cx);
            // The nav bar keeps its own draw list, which doesn't see the bars moving.
            if has_pill_changed || (hides_bottom_bars && self.hidden_fraction != old_fraction) {
                bottom_bars.redraw(cx);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let ChildRefs { header, status_bar_fade, bottom_bars, spaces_bar_wrapper, navigation_tab_bar, .. } = self.child_refs(cx).clone();
        let page = scope.data.get::<AppState>().map(|app_state| match app_state.selected_tab {
            SelectedTab::Home | SelectedTab::Space { .. } => Page::RoomsList,
            SelectedTab::Settings => Page::Settings,
            SelectedTab::AddRoom => Page::AddRoom,
        });
        // Our header only belongs to the rooms list, since the Settings and AddRoom pages have their own.
        let is_rooms_list_shown = page == Some(Page::RoomsList);
        header.set_visible(cx, is_rooms_list_shown);
        let prefs = &cx.global::<AppPreferencesGlobal>().0;
        let header_fraction = if prefs.hide_header_on_scroll { self.hidden_fraction } else { 0.0 };
        let bottom_bars_fraction = if prefs.hide_bottom_bars_on_scroll { self.hidden_fraction } else { 0.0 };
        if let Some(mut header) = header.borrow_mut() {
            header.walk.margin.top = -header_fraction * (self.header_height + HEADER_SHADOW_RADIUS);
        }
        let status_bar_height = cx.display_context.safe_area_insets.top;
        // How far the other pages' own header has slid up, from where it rests below the status bar.
        let page_header_offset = header_fraction * (status_bar_height + self.page_header_height);
        // Our header covers the fade, but the other pages' headers are beneath it, so we only fade what's below them.
        let fade_clip_top = if is_rooms_list_shown { 0.0 } else { status_bar_height + self.page_header_height - page_header_offset };
        status_bar_fade.set_visible(cx, header_fraction > 0.0 && status_bar_height > 0.0 && fade_clip_top < status_bar_height);
        if let Some(mut fade) = status_bar_fade.borrow_mut() {
            fade.walk.height = Size::Fixed(status_bar_height);
            fade.draw_bg.set_uniform(cx, id!(clip_top), &[fade_clip_top as f32]);
        }
        if let Some(mut bottom_bars) = bottom_bars.borrow_mut() {
            bottom_bars.walk.margin.bottom = -bottom_bars_fraction * (self.bottom_bars_height + SPACES_BAR_SHADOW_RADIUS);
            // While the nav bar is a pill, hovers beside it reach the content beneath it.
            bottom_bars.cursor = (!self.pill.state.is_shrunk).then_some(MouseCursor::Default);
        }
        // The nav bar draws its pill from this, so a rebuilt mobile layout can't leave it out of date.
        navigation_tab_bar.set_pill_state(self.pill.state);
        // The spaces bar slides down behind the nav bar while the nav bar shrinks into its pill,
        // and it stays down there while it waits for the nav bar to fully expand again.
        let out_of_view_offset = self.bottom_bars_height + SPACES_BAR_SHADOW_RADIUS;
        spaces_bar_wrapper.set_slide_offset(if self.is_spaces_bar_waiting_to_show {
            out_of_view_offset
        } else {
            self.pill.state.shrunk_fraction.clamp(0.0, 1.0) * out_of_view_offset
        });
        if self.header_height > 0.0
            && let Some((rooms_list, _)) = self.rooms_list(cx)
        {
            rooms_list.set_bar_space_heights(cx, self.header_height, self.bottom_bars_height);
        }
        // The other pages only make room for our bars while we draw them, since the desktop layout draws those same pages.
        // They also reach up behind the status bar, so we move their header and profile pane down below it.
        let page_refs = page.and_then(|page| self.page_refs(cx, page));
        if let Some(refs) = &page_refs {
            // The header's space opens up above the content as far as the header slides up, so the content stays put.
            if let Some(mut page_header) = refs.header.borrow_mut() {
                page_header.walk.margin.top = status_bar_height - page_header_offset;
            }
            if let Some(mut space) = refs.header_space.borrow_mut() {
                space.walk.height = Size::Fixed(page_header_offset);
            }
            if let Some(mut space) = refs.bottom_bars_space.borrow_mut() {
                space.walk.height = Size::Fixed(self.bottom_bars_height);
            }
            if let Some(mut pane) = refs.profile_pane.as_ref().and_then(|pane| pane.borrow_mut()) {
                pane.walk.margin.top = status_bar_height;
            }
        }

        let step = self.view.draw_walk(cx, scope, walk);
        if step.is_done() {
            if let Some(refs) = &page_refs {
                if let Some(mut page_header) = refs.header.borrow_mut() {
                    page_header.walk.margin.top = 0.0;
                }
                for space in [&refs.header_space, &refs.bottom_bars_space] {
                    if let Some(mut space) = space.borrow_mut() {
                        space.walk.height = Size::Fixed(0.0);
                    }
                }
                if let Some(mut pane) = refs.profile_pane.as_ref().and_then(|pane| pane.borrow_mut()) {
                    pane.walk.margin.top = 0.0;
                }
                // A page that got too short to scroll just went back to its top without any event,
                // so we ask for one, which shows its bars again.
                if refs.scroll_view.scroll_pos().y < 0.5 && (self.hidden_fraction > 0.0 || self.pill.state.is_shrunk) {
                    cx.new_next_frame();
                }
            }
            self.drawn_page = page;
            let header_height = if is_rooms_list_shown { header.area().rect(cx).size.y } else { self.header_height };
            let page_header_height = page_refs.as_ref().map_or(self.page_header_height, |refs| refs.header.area().rect(cx).size.y);
            let bottom_bars_height = bottom_bars.area().rect(cx).size.y;
            if (header_height, page_header_height, bottom_bars_height) != (self.header_height, self.page_header_height, self.bottom_bars_height) {
                self.header_height = header_height;
                self.page_header_height = page_header_height;
                self.bottom_bars_height = bottom_bars_height;
                cx.redraw_area_in_draw(self.view.area());
            }
        }
        step
    }
}

impl HideOnScrollView {
    fn child_refs(&mut self, cx: &mut Cx) -> &ChildRefs {
        self.child_refs.get_or_insert_with(|| ChildRefs {
            header: self.view.view(cx, ids!(header)),
            status_bar_fade: self.view.view(cx, ids!(status_bar_fade)),
            bottom_bars: self.view.view(cx, ids!(bottom_bars)),
            spaces_bar_wrapper: self.view.spaces_bar_wrapper(cx, ids!(spaces_bar_wrapper)),
            navigation_tab_bar: self.view.navigation_tab_bar(cx, ids!(navigation_tab_bar)),
            filter_input: self.view.text_input(cx, ids!(room_filter_input_bar.input.text_input)),
        })
    }

    fn rooms_list(&mut self, cx: &mut Cx) -> Option<(RoomsListRef, PortalListRef)> {
        if self.rooms_list.is_none() {
            let rooms_list = self.view.rooms_list(cx, ids!(rooms_list));
            if !rooms_list.is_empty() {
                let list = rooms_list.portal_list(cx, ids!(list));
                self.rooms_list = Some((rooms_list, list));
            }
        }
        self.rooms_list.clone()
    }

    /// The Settings or AddRoom page's header and scrolling content, once that page has been shown.
    fn page_refs(&mut self, cx: &mut Cx, page: Page) -> Option<PageRefs> {
        let (page_refs, page_id, scroll_view_id) = match page {
            Page::RoomsList => return None,
            Page::Settings => (&mut self.settings_page, id!(settings_screen), id!(scroll_view)),
            Page::AddRoom => (&mut self.add_room_page, id!(add_room_screen), id!(content)),
        };
        if page_refs.is_none() {
            let scroll_view = self.view.view(cx, &[page_id, scroll_view_id]);
            if !scroll_view.is_empty() {
                *page_refs = Some(PageRefs {
                    header: self.view.view(cx, &[page_id, id!(page_header)]),
                    header_space: scroll_view.view(cx, ids!(header_space)),
                    bottom_bars_space: scroll_view.view(cx, ids!(bottom_bars_space)),
                    profile_pane: (page == Page::AddRoom)
                        .then(|| self.view.user_profile_sliding_pane(cx, &[page_id, id!(user_profile_sliding_pane)])),
                    scroll_view,
                });
            }
        }
        page_refs.clone()
    }

    /// Starts sliding the bars to the given hidden fraction, optionally along with the content's scroll position.
    fn slide_to(&mut self, cx: &mut Cx, to_fraction: f64, content_scroll: Option<(f64, f64)>) {
        self.snap.stop(cx);
        self.snap = Snap::Sliding(Slide {
            started_at: Cx::monotonic_now(),
            from_fraction: self.hidden_fraction,
            to_fraction,
            content_scroll,
            last_content_scroll: content_scroll.map_or(0.0, |(from, _)| from),
            duration_secs: SNAP_DURATION_SECS * (1.0 + (to_fraction - self.hidden_fraction).abs()),
            next_frame: cx.new_next_frame(),
        });
    }
}

/// The pages that the root view can show, one for each kind of selected tab.
#[derive(Clone, Copy, PartialEq)]
enum Page {
    RoomsList,
    Settings,
    AddRoom,
}

/// A Settings or AddRoom page's header above its scroll view, and the spaces at the scroll view's start and end
/// that make room for the header and the bottom bars.
#[derive(Clone)]
struct PageRefs {
    header: ViewRef,
    scroll_view: ViewRef,
    header_space: ViewRef,
    bottom_bars_space: ViewRef,
    /// The AddRoom page's profile pane, which covers the whole page.
    profile_pane: Option<UserProfileSlidingPaneRef>,
}

/// The scrolling content of the page we drew last, which the bars move along with.
enum Scroller {
    RoomsList(PortalListRef),
    /// The Settings or AddRoom page's scroll view.
    Page(ViewRef),
}

impl Scroller {
    /// The first item in view, and its top's offset from the top of the view, which is negative once scrolled past.
    /// A page scrolls like a list with just one item.
    fn position(&self) -> (usize, f64) {
        match self {
            Scroller::RoomsList(list) => (list.first_id(), list.scroll_position()),
            Scroller::Page(scroll_view) => (0, -scroll_view.scroll_pos().y),
        }
    }
}

/// How the bars finish sliding fully in or out once the user is done scrolling.
#[derive(Default)]
enum Snap {
    /// Waiting for the content to stop moving, which we check on each frame.
    AwaitingScrollStop(NextFrame),
    /// Waiting for the mouse wheel or trackpad to stop scrolling the content.
    AwaitingWheelStop(Timer),
    Sliding(Slide),
    #[default]
    None,
}

impl Snap {
    /// Stops waiting or sliding, which leaves the bars where they are.
    fn stop(&mut self, cx: &mut Cx) {
        if let Snap::AwaitingWheelStop(timer) = std::mem::take(self) {
            cx.stop_timer(timer);
        }
    }
}

struct Slide {
    /// When the slide started, from [`Cx::monotonic_now()`].
    started_at: f64,
    from_fraction: f64,
    to_fraction: f64,
    /// The content's scroll position (from its top) to move from and to, if it moves along with the header.
    content_scroll: Option<(f64, f64)>,
    last_content_scroll: f64,
    duration_secs: f64,
    next_frame: NextFrame,
}
