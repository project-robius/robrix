//! Support for Robrix's custom dock tabs: extra buttons, layout, and input handling.

use std::collections::{HashMap, HashSet};
use makepad_widgets::*;
use makepad_widgets::makepad_platform::event::{ScrollPhase, finger::TAP_COUNT_DISTANCE};
use crate::app::SelectedRoom;
use super::{room_action_bar::RoomActionBarWidgetRefExt, room_tab_hover_card::RoomTabHoverCardWidgetExt};

script_mod! {
    use mod.prelude.widgets.*

    mod.widgets.RoomTabsBase = #(RoomTabs::script_component(vm))
}

/// Handles dock tab buttons and tab mgmt for the main desktop UI.
#[derive(Script, ScriptHook)]
pub struct RoomTabs {
    #[source] source: ScriptObjectRef,
    /// The template for each room tab's expand/collapse buttons.
    #[live] room_tab_actions: ScriptObjectRef,
    /// Used to measure the room name so we know how wide to make its tab.
    #[live] tab_title_measure: DrawText,
    /// The set of each tab's buttons, so we don't recreate them on every draw.
    #[rust] controls: HashMap<LiveId, WidgetRef>,
    /// Tabs that have visible buttons, which need to receive events.
    #[rust] visible_tabs: HashSet<LiveId>,
    /// State that tracks what a user is doing with a click/press on a tab button
    /// (basically are they clicking a button or just doing a drag that starts over a button).
    #[rust] interaction: TabButtonInteraction,
    #[rust] layout_redraw: NextFrame,
}

impl RoomTabs {
    pub fn tab_template(room: &SelectedRoom) -> LiveId {
        match room {
            SelectedRoom::JoinedRoom { .. } | SelectedRoom::Thread { .. } => id!(RoomTab),
            _ => id!(CloseableTab),
        }
    }

    /// Forward events to the dock, with the tab buttons and hover card layered over it.
    pub fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope, view: &mut View, rooms: &HashMap<LiveId, SelectedRoom>) {
        let dock = view.dock(cx, ids!(dock));
        let hover_card = view.room_tab_hover_card(cx, ids!(room_tab_hover));
        let hover_available = event.pointer_claimed_area().is_empty();
        // Just touching the trackpad shouldn't cancel a button press or hide its hover card.
        if !matches!(event, Event::Scroll(scroll) if scroll.phase == ScrollPhase::Touched) {
            self.interaction.update_pending_button_press(cx, event);
        }
        if hover_card.handle_scroll(cx, event, scope, &dock) {
            return;
        }
        if self.layout_redraw.is_event(event).is_some() {
            view.redraw(cx);
        }
        let mut tab_buttons = Vec::new();
        for tab_id in &self.visible_tabs {
            if !rooms.contains_key(tab_id) { continue; }
            if let Some(controls) = self.controls.get(tab_id) {
                for button_id in [id!(expand_room_actions_button), id!(collapse_room_actions_button)] {
                    tab_buttons.push((*tab_id, controls.button(cx, &[button_id])));
                }
            }
        }
        let actions = cx.capture_actions(|cx| {
            for tab_id in &self.visible_tabs {
                if !rooms.contains_key(tab_id) { continue; }
                if let Some(controls) = self.controls.get(tab_id) {
                    controls.handle_event(cx, event, scope);
                }
            }
            view.handle_event(cx, event, scope);
        });
        self.interaction.process_tab_button_actions(cx, event, &dock, &tab_buttons, actions);
        hover_card.update_hover(cx, event, hover_available, &dock, rooms);
    }

    /// Draws the dock first and then places the tab buttons over the dock's drawn tabs. 
    pub fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk, view: &mut View, rooms: &HashMap<LiveId, SelectedRoom>) -> DrawStep {
        let dock = view.dock(cx, ids!(dock));
        let tab_sizes_changed = self.update_room_tab_sizes(cx, &dock, rooms);
        let step = view.draw_walk(cx, scope, walk);
        if step.is_done() {
            self.draw_tab_expand_collapse_buttons(cx, scope, &dock, rooms);
            // If any tab changed size, update its hover card with that tab's new area rect.
            if self.update_room_tab_sizes(cx, &dock, rooms) || tab_sizes_changed {
                self.layout_redraw = cx.new_next_frame();
            }
        }
        step
    }

    /// Returns the ID of the tab whose expand/collapse button was clicked, if any.
    pub fn expansion_clicked(&self, cx: &mut Cx, actions: &Actions, rooms: &HashMap<LiveId, SelectedRoom>) -> Option<LiveId> {
        let mut clicked = None;
        for tab_id in &self.visible_tabs {
            if !rooms.contains_key(tab_id) { continue; }
            let Some(controls) = self.controls.get(tab_id) else { continue };
            if controls.button(cx, ids!(expand_room_actions_button)).clicked(actions)
                || controls.button(cx, ids!(collapse_room_actions_button)).clicked(actions)
            {
                clicked = Some(*tab_id);
            }
        }
        clicked
    }

    /// Recalculates and updates each tab's width and padding.
    ///
    /// Each tab is resized to fit its room name and buttons, up to a max of 260px wide,
    /// and requests a redraw for any tabs that actually changed.
    ///
    /// Returns `true` if any tab's width or padding changed, or `false` otherwise.
    fn update_room_tab_sizes(&mut self, cx: &mut Cx2d, dock: &DockRef, rooms: &HashMap<LiveId, SelectedRoom>) -> bool {
        let mut changed = false;
        for (tab_id, room) in rooms {
            let tab = dock.widget(cx, &[TabBar::tab_node_name(*tab_id)]);
            if let Some(mut tab) = tab.borrow_mut::<Tab>() {
                let right = if matches!(room, SelectedRoom::JoinedRoom { .. } | SelectedRoom::Thread { .. }) {
                    // Include a 4px gap before the expansion control.
                    37.0
                } else {
                    9.0
                };
                let title_width = self.tab_title_measure.layout(
                    cx, 0.0, 0.0, None, false, Align::default(), &room.display_name(),
                ).size_in_lpxs.width as f64 * self.tab_title_measure.font_scale as f64;
                let width = (38.0 + title_width.ceil() + right).min(260.0);
                if !matches!(tab.walk.width, Size::Fixed(value) if value == width)
                    || tab.layout.padding.left != 38.0
                    || tab.layout.padding.right != right
                {
                    tab.walk.width = Size::Fixed(width);
                    tab.layout.padding.left = 38.0;
                    tab.layout.padding.right = right;
                    tab.redraw(cx);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Draws the expand/collapse button on each visible joined room or thread tab.
    ///
    /// Call this after the dock is drawn (when each tab's position is known),
    /// so it can properly place each button in the right spot.
    /// Also restyles the overlaid buttons to match whether the tab is currently hovered over.
    fn draw_tab_expand_collapse_buttons(
        &mut self,
        cx: &mut Cx2d,
        scope: &mut Scope,
        dock: &DockRef,
        rooms: &HashMap<LiveId, SelectedRoom>,
    ) {
        self.visible_tabs.clear();
        self.controls.retain(|tab_id, _| rooms.contains_key(tab_id));
        let Some(geometry) = dock.borrow().map(|dock| dock.compact_dump(cx)) else { return };
        let tab_bar_rects: HashMap<_, _> = geometry.tab_headers.iter().filter_map(|tab| {
            geometry.tabs.iter().find(|bar| bar.tabs_id == tab.tabs_id)
                .map(|bar| (tab.tab_id, bar.rect))
        }).collect();
        for (tab_id, room) in rooms {
            if !matches!(room, SelectedRoom::JoinedRoom { .. } | SelectedRoom::Thread { .. }) {
                continue;
            }
            let tab_widget = dock.widget(cx, &[TabBar::tab_node_name(*tab_id)]);
            let Some(tab) = tab_widget.borrow::<Tab>() else { continue };
            let Some(bar_rect) = tab_bar_rects.get(tab_id) else { continue };
            let rect = tab.area().rect(cx);
            let clip = rect.clip((bar_rect.pos, bar_rect.pos + bar_rect.size));
            if clip.size.x <= 0.0 || clip.size.y <= 0.0 {
                continue;
            }
            let color = if tab.is_active() { vec4(1.0, 1.0, 1.0, 1.0) } else { vec4(0.0, 0.0, 0.0, 1.0) };
            let hover_color = if tab.is_active() { vec4(1.0, 1.0, 1.0, 0.2) } else { vec4(0.0, 0.0, 0.0, 31.0 / 255.0) };
            let down_color = if tab.is_active() { vec4(1.0, 1.0, 1.0, 0.3) } else { vec4(0.0, 0.0, 0.0, 0.2) };
            let controls = self.controls.entry(*tab_id).or_insert_with(|| {
                cx.with_vm(|vm| WidgetRef::script_from_value(vm, self.room_tab_actions.as_object().into()))
            });
            let expanded = dock.item(*tab_id).room_action_bar(cx, ids!(room_actions)).is_expanded();
            controls.widget(cx, ids!(expand_room_actions_button)).set_visible(cx, !expanded);
            controls.widget(cx, ids!(collapse_room_actions_button)).set_visible(cx, expanded);
            for button_id in [id!(expand_room_actions_button), id!(collapse_room_actions_button)] {
                let mut button = controls.widget(cx, &[button_id]);
                script_apply_eval!(cx, button, {
                    draw_icon +: {color: #(color)}
                    draw_bg +: {color_hover: #(hover_color), color_down: #(down_color)}
                });
            }
            let controls_rect = Rect {
                pos: dvec2(rect.pos.x + rect.size.x - 33.0, rect.pos.y),
                size: dvec2(30.0, rect.size.y),
            };
            cx.push_clip_rect(clip);
            controls.draw_walk_all(cx, scope, Walk::abs_rect(controls_rect));
            cx.pop_clip_rect();
            self.visible_tabs.insert(*tab_id);
        }
    }

}

/// State that allows dock tabs to "own" FingerDown drag hits that start on their buttons,
/// while still only activating those buttons upon FingerUp.
#[derive(Default)]
struct TabButtonInteraction {
    pressed: Option<PressedTabButton>,
    hovered_button_tab: Option<WidgetRef>,
}

struct PressedTabButton {
    tab: WidgetRef,
    button: Option<ButtonRef>,
    close_action: Option<Action>,
    original_area: Area,
    original_rect: Rect,
}

impl PressedTabButton {
    fn reset(&self, cx: &mut Cx) {
        if let Some(button) = &self.button {
            button.reset_hover(cx);
        }
    }
}

impl TabButtonInteraction {
    fn sync_button_hover(&mut self, cx: &mut Cx, event: &Event, dock: &DockRef, buttons: &[(LiveId, ButtonRef)]) {
        let hovered = match event {
            Event::MouseMove(_) => buttons.iter().find_map(|(tab_id, b)| {
                (b.visible() && !b.area().is_empty() && event.pointer_claimed_area() == b.area())
                    .then(|| dock.widget(cx, &[TabBar::tab_node_name(*tab_id)]))
            }),
            Event::MouseLeave(_) | Event::WindowLostFocus(_) | Event::WindowGeomChange(_)
            | Event::ClearHover => None,
            _ => return,
        };
        if let Some(previous) = self.hovered_button_tab.take() {
            let still_inside = matches!(event, Event::MouseMove(mouse)
                if previous.area().clipped_rect(cx).contains(mouse.abs));
            if !still_inside && let Some(mut tab) = previous.borrow_mut::<Tab>() {
                tab.animator_play(cx, ids!(hover.off));
            }
        }
        if let Some(widget) = &hovered && let Some(mut tab) = widget.borrow_mut::<Tab>() {
            // Buttons overlaid atop the tab will handle the hover before the tab gets to handle it,
            // so we have to watch for that and ensure that the tab doesn't hover out when that button hovers in.
            tab.animator_play(cx, ids!(hover.on));
        }
        self.hovered_button_tab = hovered;
    }

    /// Updates the button press we're waiting to release.
    ///
    /// Call this before the dock and buttons handle the given event, so that
    /// we have the opportunity to cancel the button click if the mouse/finger moved too far,
    /// or if a long press occurred or the button press was otherwise interrupted.
    ///
    /// If a proper button click occurred (and was released) over the button,
    /// this will give the pointer capture back to the button so it receives FingerUp
    /// and emits a click event as normal.
    ///
    /// For a tab close request, this also emits the saved close action.
    pub fn update_pending_button_press(&mut self, cx: &mut Cx, event: &Event) {
        let Some(mut pressed) = self.pressed.take() else { return };
        let tab_area = pressed.tab.area();
        if !cx.fingers.is_area_captured(tab_area)
            || matches!(event, Event::Scroll(_) | Event::KeyDown(_) | Event::MouseLeave(_)
                | Event::WindowLostFocus(_) | Event::WindowClosed(_) | Event::WindowGeomChange(_)
                | Event::Drag(_) | Event::Drop(_) | Event::DragEnd)
        {
            pressed.reset(cx);
            return;
        }
        match event.hits(cx, tab_area) {
            Hit::FingerMove(e) if e.move_distance() >= TAP_COUNT_DISTANCE => {
                // Cancel upon finger/mouse mvmt, even if it doesn't actually result in a tab drag-n-drop.
                pressed.reset(cx);
            }
            Hit::FingerLongPress(_) => pressed.reset(cx),
            Hit::FingerUp(e) => {
                let button_area = pressed.button.as_ref().map_or(pressed.original_area, |b| b.area());
                let button_rect = pressed.button.as_ref().map_or(pressed.original_rect, |b| b.area().clipped_rect(cx));
                // This is similar to `was_tap()` but doesn't check TAP_COUNT_TIME, so we don't use it.
                if !e.has_long_press_occurred
                    && (e.abs - e.abs_start).length() < TAP_COUNT_DISTANCE
                    && button_rect.contains(e.abs)
                    && cx.switch_finger_capture(tab_area, button_area, Area::Empty)
                {
                    // The native tab-close button doesn't work like regular buttons unfortunately,
                    // so we capture the "tab close" action it emits and re-add it to the actions here.
                    if let Some(action) = pressed.close_action.take() {
                        cx.extend_actions(vec![action]);
                    }
                } else {
                    pressed.reset(cx);
                }
            }
            _ => self.pressed = Some(pressed),
        }
    }

    /// Handles new button presses and saves tab close requests until mouse/finger up (release).
    ///
    /// Call this after the dock and buttons have handled the given event,
    /// and also pass in the actions they emitted.
    /// This also keeps the tab hovered while any of its buttons are hovered.
    pub fn process_tab_button_actions(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        dock: &DockRef,
        buttons: &[(LiveId, ButtonRef)],
        actions: ActionsBuf,
    ) {
        self.sync_button_hover(cx, event, dock, buttons);
        for (tab_id, button) in buttons {
            // Buttons will gobble up a middle mouse button click without emitting any actions,
            // so we have to specifically check for those here.
            // Should prob just fix that in Makepad's button widget...
            let was_middle_clicked = || {
                matches!(event, Event::MouseDown(e) if e.button.is_middle())
                    && button.visible()
                    && !button.area().is_empty()
                    && event.pointer_claimed_area() == button.area()
            };
            if button.pressed(&actions) || was_middle_clicked() {
                let tab = dock.widget(cx, &[TabBar::tab_node_name(*tab_id)]);
                let original_area = button.area();
                if cx.switch_finger_capture(original_area, tab.area(), Area::Empty) {
                    if let Some(old) = self.pressed.take() {
                        old.reset(cx);
                    }
                    self.pressed = Some(PressedTabButton {
                        tab,
                        button: Some(button.clone()),
                        close_action: if was_middle_clicked() {
                            Some(Box::new(WidgetAction {
                                widget_uid: dock.widget_uid(),
                                data: None,
                                action: Box::new(DockAction::TabCloseWasPressed(*tab_id)),
                                group: None,
                            }))
                        } else { None },
                        original_area,
                        original_rect: original_area.clipped_rect(cx),
                    });
                }
            }
        }
        for action in actions {
            let widget_action = action.as_widget_action();
            if widget_action.is_some_and(|a| a.widget_uid == dock.widget_uid())
                && let DockAction::TabCloseWasPressed(tab_id) = widget_action.cast()
                && let original_area = event.pointer_claimed_area()
                && !original_area.is_empty()
                && matches!(event.hits(cx, original_area), Hit::FingerDown(_))
            {
                let tab = dock.widget(cx, &[TabBar::tab_node_name(tab_id)]);
                if cx.switch_finger_capture(original_area, tab.area(), Area::Empty) {
                    if let Some(old) = self.pressed.take() { old.reset(cx); }
                    self.pressed = Some(PressedTabButton {
                        tab,
                        button: None,
                        close_action: Some(action),
                        original_area,
                        original_rect: original_area.clipped_rect(cx),
                    });
                    continue;
                }
            }
            cx.extend_actions(vec![action]);
        }
    }
}
