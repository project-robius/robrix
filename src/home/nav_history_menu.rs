//! A popup menu for viewing and going back/forward through nav history, like a web browser.
//!
//! Currently this is shown when long-pressing (including clicking and holding the mouse):
//! * the go-back button in the title/caption bar,
//! * the go-forward button in the title/caption bar,
//! * the go-back button in the stack nav header.

use makepad_widgets::{makepad_platform::event::finger::{TAP_COUNT_DISTANCE, TAP_COUNT_TIME, TouchState}, *};
use crate::{
    app::{AppState, SelectedRoom},
    home::nav_history::{GoBackAction, GoForwardAction, NavHistory, can_navigate_history, can_show, current_place},
    room::FetchedRoomAvatar,
    shared::{avatar::AvatarWidgetRefExt, context_menu::{BUTTON_HEIGHT, ContextMenuClosed, MENU_PADDING, MENU_WIDTH, is_context_menu_cancelled, is_tap_outside, menu_position_margin}},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NavHistoryMenu = set_type_default() do #(NavHistoryMenu::register_widget(vm)) {
        ..mod.widgets.ContextMenuBackdrop

        main_content := mod.widgets.ContextMenuContent {
            // No right padding here, that's within the portallist itself
            // such that the scrollbar is nicely positioned in the right gutter.
            padding: Inset{top: #(MENU_PADDING), right: 0, bottom: #(MENU_PADDING), left: #(MENU_PADDING)}
            list := PortalList {
                width: Fill, height: Fill
                flow: Down
                padding: Inset{right: #(MENU_PADDING)}
                scroll_bar: ScrollBar { bar_size: 6.5, draw_bg +: { size: 4.0 } }

                // A screen's avatar goes where its button's icon would be.
                screen_row := View {
                    width: Fill, height: #(BUTTON_HEIGHT)
                    flow: Overlay
                    align: Align{y: 0.5}
                    button := mod.widgets.ContextMenuButton {
                        // Only a tap picks a place, not a press (which scrolls the list).
                        enable_long_press: true
                        padding: Inset{left: 40, right: 12, top: 9, bottom: 9}
                        spacing: 0
                        icon_walk: Walk{width: 0, height: 0}
                        label_walk: Walk{width: Fill, height: Fit}
                        draw_text +: { max_lines: 1, text_overflow: TextOverflow.Ellipsis }
                    }
                    avatar := Avatar {
                        width: 22, height: 22
                        margin: Inset{left: 8}
                        text_view +: { text +: { draw_text +: { text_style: TITLE_TEXT { font_size: 9 } } } }
                    }
                }
                home_row := View {
                    width: Fill, height: #(BUTTON_HEIGHT)
                    button := mod.widgets.ContextMenuButton {
                        enable_long_press: true
                        spacing: 12
                        draw_icon +: { svg: (ICON_HOME) }
                        text: "Home"
                    }
                }
            }
        }
    }
}

/// Which way the [`NavHistoryMenu`] is going through the nav history.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NavHistoryDirection {
    #[default]
    Back,
    Forward,
}

impl NavHistoryDirection {
    /// Returns the places that going this way from the `current` place can reach, from nearest to furthest.
    fn places<'a>(
        self,
        nav_history: &'a NavHistory,
        current: Option<&'a SelectedRoom>,
        can_show: impl Fn(&SelectedRoom) -> bool + 'a,
    ) -> Box<dyn Iterator<Item = Option<SelectedRoom>> + 'a> {
        match self {
            NavHistoryDirection::Back => Box::new(nav_history.previous_places(current, can_show)),
            NavHistoryDirection::Forward => Box::new(nav_history.next_places(current, can_show)),
        }
    }
}

/// An action emitted to show the nav history popup menu.
#[derive(Debug)]
pub struct ShowNavHistoryMenuAction {
    pub direction: NavHistoryDirection,
    /// The location of the button that caused the nav history menu to be shown,
    /// such that the menu can be positioned right below it.
    pub anchor: Rect,
    /// The mouse or touch that's still pressing that button.
    pub held_pointer: HeldPointer,
}

/// A mouse or touch that's still pressing the button that opened a [`NavHistoryMenu`],
/// which can drag over the menu's rows and be released on one to go to its place, like in a web browser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeldPointer {
    Mouse,
    Touch(u64),
}

/// The state of a place's row content & avatar since a [`NavHistoryMenu`] was shown.
#[derive(PartialEq)]
enum RowState {
    Unpopulated,
    /// Currently showing this avatar of its screen, if it's known.
    Populated(Option<FetchedRoomAvatar>),
    /// Its screen's avatar changed since it was first shown (an avatar image loaded).
    AvatarChanged,
}

#[derive(Script, ScriptHook, Widget)]
pub struct NavHistoryMenu {
    #[deref] view: View,
    #[source] source: ScriptObjectRef,
    #[rust] direction: NavHistoryDirection,
    /// The place that this menu goes back or forward from.
    #[rust] current: Option<SelectedRoom>,
    /// The places in this menu, from nearest to furthest, along with their row states.
    #[rust] places: Vec<(Option<SelectedRoom>, RowState)>,
    /// The mouse or touch still pressing the button that opened this menu, if any.
    #[rust] held_pointer: Option<HeldPointer>,
    /// The index of the row that the `held_pointer` has highlighted by dragging over it.
    #[rust] dragged_over_index: Option<usize>,
    /// Held while this menu is shown, so it receives Escape/Back cancel events.
    #[rust] cancel_scope: Option<CancelScope>,
}

impl Widget for NavHistoryMenu {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.places.is_empty() {
            self.visible = false;
        }
        while let Some(widget) = self.view.draw_walk(cx, scope, walk).step() {
            let portal_list = widget.as_portal_list();
            let Some(mut list) = portal_list.borrow_mut() else { continue };
            list.set_item_range(cx, 0, self.places.len());
            while let Some(index) = list.next_visible_item(cx) {
                let Some((place, row_state)) = self.places.get_mut(index) else { continue };
                let template = if place.is_some() { id!(screen_row) } else { id!(home_row) };
                let (row, existed) = list.item_with_existed(cx, index, template);
                let is_unpopulated = !existed || *row_state == RowState::Unpopulated;
                if is_unpopulated {
                    let button = row.button(cx, ids!(button));
                    if let Some(screen) = place {
                        button.set_text(cx, &screen.display_name());
                    }
                    button.reset_hover(cx);
                }
                if is_unpopulated || *row_state == RowState::AvatarChanged {
                    let mut avatar = None;
                    if let Some(screen) = place {
                        avatar = screen.get_or_fetch_avatar(cx);
                        row.avatar(cx, ids!(avatar)).show_room_avatar(cx, avatar.as_ref(), screen.room_name());
                    }
                    *row_state = RowState::Populated(avatar);
                }
                row.draw_all(cx, scope);
            }
        }
        if self.visible {
            let main_content_area = self.view(cx, ids!(main_content)).area();
            cx.block_scrolling_except_within(main_content_area);
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            // A list that was still scrolling when this menu closed has to stop scrolling,
            // otherwise it'd ignore the next tap on it.
            if let Event::NextFrame(_) = event {
                self.view.handle_event(cx, event, scope);
            }
            return;
        }
        // The rows can't scroll away from a press that's still held, as releasing it picks the row under it.
        if !(self.held_pointer.is_some() && matches!(event, Event::Scroll(_))) {
            self.view.handle_event(cx, event, scope);
        }

        let hit = event.hits_with_capture_overload(cx, self.view.area(), true);
        let close_menu = is_context_menu_cancelled(cx, event, self.cancel_scope.as_ref())
            || is_tap_outside(cx, &hit, self.view(cx, ids!(main_content)).area())
            // A reapply resets this menu's size, and a resized window may no longer fit the menu where it was
            || matches!(event, Event::ScriptReapply)
            || matches!(event, Event::Actions(actions) if actions.iter().any(|action|
                matches!(action.as_widget_action().cast(), WindowAction::WindowGeomChange(_))
            ));
        if close_menu {
            self.close(cx);
            return;
        }
        // The mouse or touch still pressing this menu's button highlights the row it's dragged over,
        // and releasing it on a row goes to that place.
        if let Some(held_pointer) = self.held_pointer {
            let pointer_update = match (held_pointer, event) {
                (HeldPointer::Mouse, Event::MouseMove(e)) => {
                    // A mouse that isn't held anymore has ended its press, even if we missed that release.
                    let state = if cx.fingers.first_mouse_button.is_some() { TouchState::Move } else { TouchState::Cancel };
                    Some((e.abs, state))
                }
                (HeldPointer::Mouse, Event::MouseUp(e)) if e.button.is_primary() => Some((e.abs, TouchState::Stop)),
                (HeldPointer::Touch(uid), Event::TouchUpdate(e)) => e.touches.iter()
                    .find(|touch| touch.uid == uid)
                    .map(|touch| (touch.abs, touch.state)),
                _ => None,
            };
            if let Some((abs, state)) = pointer_update {
                let list = self.view.portal_list(cx, ids!(list));
                let row_index = (!matches!(state, TouchState::Cancel) && list.area().rect(cx).contains(abs))
                    .then(|| (0..self.places.len()).find(|&index| self.places[index].1 != RowState::Unpopulated
                        && list.get_item(index).is_some_and(|(_, row)| row.area().rect(cx).contains(abs))
                    ))
                    .flatten();
                if row_index != self.dragged_over_index {
                    for (index, hover_state) in [(self.dragged_over_index, ids!(hover.off)), (row_index, ids!(hover.on))] {
                        if let Some((_, row)) = index.and_then(|index| list.get_item(index))
                            && let Some(mut button) = row.button(cx, ids!(button)).borrow_mut()
                        {
                            button.animator_play(cx, hover_state);
                        }
                    }
                    self.dragged_over_index = row_index;
                }
                if matches!(state, TouchState::Stop | TouchState::Cancel) {
                    self.held_pointer = None;
                    if matches!(state, TouchState::Stop) && let Some(index) = row_index {
                        self.go_to_place(cx, index);
                        return;
                    }
                }
            }
        }
        // Background updates can change a row's avatar, e.g., when a space's preview or an image loads.
        if let Event::Signal | Event::Actions(_) = event {
            let mut has_changed_avatars = false;
            for (place, row_state) in &mut self.places {
                if let (Some(screen), RowState::Populated(avatar)) = (place.as_ref(), &*row_state)
                    && screen.get_or_fetch_avatar(cx) != *avatar
                {
                    *row_state = RowState::AvatarChanged;
                    has_changed_avatars = true;
                }
            }
            if has_changed_avatars {
                self.redraw(cx);
            }
        }

        let Event::Actions(actions) = event else { return };
        // Going back or forward obviously changed the content of the nav history menu.
        let can_show = can_show(cx);
        let are_places_stale = scope.data.get::<AppState>().is_some_and(|app_state| {
            let mut places = self.direction.places(&app_state.nav_history, self.current.as_ref(), can_show);
            !self.places.iter().all(|(place, _)| places.next().as_ref() == Some(place)) || places.next().is_some()
        });
        if are_places_stale {
            self.close(cx);
            return;
        }
        let clicked_index = self.view.portal_list(cx, ids!(list)).items_with_actions(actions).into_iter()
            .find_map(|(index, row)| row.button(cx, ids!(button)).clicked(actions).then_some(index));
        if let Some(index) = clicked_index {
            self.go_to_place(cx, index);
        }
    }
}

impl NavHistoryMenu {
    fn show(
        &mut self,
        cx: &mut Cx,
        action: &ShowNavHistoryMenuAction,
        current: Option<SelectedRoom>,
        places: Vec<Option<SelectedRoom>>,
        container_rect: Rect,
    ) {
        let anchor = action.anchor;
        // A title bar button is above the container, so the menu can't start any higher than the container.
        let anchor_pos = dvec2(anchor.pos.x, (anchor.pos.y + anchor.size.y).max(container_rect.pos.y));
        let one_row_height = BUTTON_HEIGHT + 2.0 * MENU_PADDING;
        // The list must scroll if it's too long to fit beneath the anchor.
        let full_height = places.len() as f64 * BUTTON_HEIGHT + 2.0 * MENU_PADDING;
        let height = full_height.min((container_rect.pos.y + container_rect.size.y - anchor_pos.y).max(one_row_height));
        let margin = menu_position_margin(container_rect, anchor_pos, dvec2(MENU_WIDTH, height));
        if let Some(mut main_content) = self.view(cx, ids!(main_content)).borrow_mut() {
            main_content.walk.margin = margin;
            main_content.walk.height = Size::Fixed(height);
        }
        // If everything fits in the popup menu list, it doesn't need to scroll nor bounce.
        let can_scroll = height < full_height;
        let list = self.view.portal_list(cx, ids!(list));
        if let Some(mut list) = list.borrow_mut() {
            list.bounce_at_start = can_scroll;
            list.bounce_at_end = can_scroll;
        }
        list.set_first_id_and_scroll(0, 0.0);
        self.direction = action.direction;
        self.current = current;
        self.places = places.into_iter().map(|place| (place, RowState::Unpopulated)).collect();
        // The press may have already ended before this menu could be shown.
        self.held_pointer = Some(action.held_pointer).filter(|held_pointer| match held_pointer {
            HeldPointer::Mouse => cx.fingers.first_mouse_button.is_some(),
            HeldPointer::Touch(uid) => cx.fingers.touch_capture_area(*uid).is_some(),
        });
        self.dragged_over_index = None;
        self.visible = true;
        self.cancel_scope = Some(self.begin_cancel_scope(cx));
        cx.set_key_focus(self.view.area());
        self.redraw(cx);
    }

    /// Goes back or forward to the place at the given `index` in this menu, and closes it.
    fn go_to_place(&mut self, cx: &mut Cx, index: usize) {
        let steps = index + 1;
        match self.direction {
            NavHistoryDirection::Back => cx.action(GoBackAction { steps }),
            NavHistoryDirection::Forward => cx.action(GoForwardAction { steps }),
        }
        self.close(cx);
    }

    fn close(&mut self, cx: &mut Cx) {
        self.visible = false;
        self.places.clear();
        self.held_pointer = None;
        self.dragged_over_index = None;
        self.cancel_scope = None;
        cx.revert_key_focus();
        cx.unblock_scrolling();
        cx.action(ContextMenuClosed);
        cx.clear_all_hovers();
        self.redraw(cx);
    }
}

impl NavHistoryMenuRef {
    pub fn is_currently_shown(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.visible)
    }

    /// Closes this menu if it's shown.
    pub fn close(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.visible {
            inner.close(cx);
        }
    }
}

pub fn show_nav_history_menu(cx: &mut Cx, ui: &WidgetRef, app_state: &AppState, action: &ShowNavHistoryMenuAction) {
    // A modal may have opened while the button was held.
    if !can_navigate_history(cx, ui, app_state) { return; }
    let current = current_place(cx, ui, app_state);
    let places: Vec<_> = action.direction.places(&app_state.nav_history, current, can_show(cx)).collect();
    if places.is_empty() { return; }
    ui.callout_tooltip(cx, ids!(app_tooltip)).hide(cx);
    let container_rect = ui.view(cx, ids!(overlay_container)).area().rect(cx);
    if let Some(mut menu) = ui.nav_history_menu(cx, ids!(nav_history_menu)).borrow_mut() {
        menu.show(cx, action, current.cloned(), places, container_rect);
    }
}

/// Asks to show the nav history menu beneath the given button, which the `held_pointer` is still pressing.
pub fn request_nav_history_menu(cx: &mut Cx, direction: NavHistoryDirection, button: &ButtonRef, held_pointer: HeldPointer) {
    // The menu follows the rest of this press, so the button must not get its release, e.g., as a click.
    cx.switch_finger_capture(button.area(), Area::Empty, Area::Empty);
    button.reset_hover(cx);
    cx.action(ShowNavHistoryMenuAction { direction, anchor: button.area().rect(cx), held_pointer });
}

/// Handles a long click/press on a button, both native long presses and mouse buttons held down.
#[derive(Default)]
pub struct LongPressWatcher {
    timer: Timer,
    /// The button being pressed and the mouse or touch pressing it, until it's released or its long press is reported.
    pressed_button: Option<(WidgetUid, HeldPointer)>,
    /// Where the mouse was last pressed down.
    mouse_down_pos: DVec2,
    /// The touch that started most recently.
    touch_start_uid: u64,
}

impl LongPressWatcher {
    /// Returns the UID of the button that was just long pressed, out of the buttons that `is_button` accepts,
    /// along with the mouse or touch that's still pressing it.
    pub fn handle_event(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        mut is_button: impl FnMut(&mut Cx, WidgetUid) -> bool,
    ) -> Option<(WidgetUid, HeldPointer)> {
        if self.timer.is_event(event).is_some() {
            // A disabled button doesn't report its release, so we check that the mouse is still held.
            return self.pressed_button.take().filter(|_| cx.fingers.first_mouse_button.is_some());
        }
        match event {
            Event::MouseDown(e) => self.mouse_down_pos = e.abs,
            Event::TouchUpdate(e) => {
                if let Some(touch) = e.touches.iter().rev().find(|touch| matches!(touch.state, TouchState::Start)) {
                    self.touch_start_uid = touch.uid;
                }
            }
            // Here we copy web browser behavior: dragging the mouse down on a held button immediately shows the popup menu.
            Event::MouseMove(e) if self.pressed_button.is_some()
                && cx.fingers.first_mouse_button.is_some()
                && e.abs.y - self.mouse_down_pos.y > TAP_COUNT_DISTANCE =>
            {
                cx.stop_timer(self.timer);
                return self.pressed_button.take();
            }
            _ => {}
        }
        let Event::Actions(actions) = event else { return None };
        let mut long_pressed_button = None;
        for action in actions {
            let Some(action) = action.as_widget_action() else { continue };
            let uid = action.widget_uid;
            match action.cast::<ButtonAction>() {
                ButtonAction::Pressed(_) if is_button(cx, uid) => {
                    cx.stop_timer(self.timer);
                    if cx.fingers.first_mouse_button.is_some() {
                        self.pressed_button = Some((uid, HeldPointer::Mouse));
                        self.timer = cx.start_timeout(TAP_COUNT_TIME);
                    } else {
                        self.pressed_button = Some((uid, HeldPointer::Touch(self.touch_start_uid)));
                    }
                }
                ButtonAction::LongPressed if self.pressed_button.is_some_and(|(pressed, _)| pressed == uid) => {
                    cx.stop_timer(self.timer);
                    long_pressed_button = self.pressed_button.take();
                }
                ButtonAction::Clicked(_) | ButtonAction::Released(_) if self.pressed_button.is_some_and(|(pressed, _)| pressed == uid) => {
                    cx.stop_timer(self.timer);
                    self.pressed_button = None;
                }
                _ => {}
            }
        }
        long_pressed_button
    }
}
