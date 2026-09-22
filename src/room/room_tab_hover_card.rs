//! The room tab hover card is a little overlay view that shows a room's
//! avatar and full name when the user hovers over that tab in the dock.

use std::collections::HashMap;
use makepad_widgets::*;
use makepad_widgets::makepad_platform::event::ScrollPhase;
use crate::{app::SelectedRoom, home::rooms_list::RoomsListRef, room::FetchedRoomAvatar, shared::avatar::AvatarWidgetRefExt};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RoomTabHoverCard = #(RoomTabHoverCard::register_widget(vm)) {
        width: Fill, height: Fill
        flow: Overlay
        tooltip := Tooltip {
            content := RoundedShadowView {
                width: 272,
                // Note: this height value is set dynamically in `show_for_hovered_tab()`,
                // so this actually is NOT one of thoe "Fill" within a "Fit" makepad bugs.
                height: Fit
                flow: Down
                padding: 12
                align: Align{y: 0.5}
                draw_bg +: {
                    color: #fff
                    border_radius: 10
                    border_size: 1
                    border_color: #x00000018
                    shadow_color: #x00000033
                    shadow_radius: 4
                    shadow_offset: vec2(0, 3)
                }

                // rare case, but if a room name is really long, it might need to scroll to fit
                name_scroll := ScrollYView {
                    width: Fill, height: Fill
                    flow: Down
                    align: Align{y: 0.5}
                    padding: Inset{right: 10}

                    name_content := View {
                        width: Fill, height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        spacing: 12

                        avatar := Avatar {width: 44, height: 44}
                        tooltip_label := Label {
                            width: Fill, height: Fit
                            padding: 0
                            flow: Flow.Right{wrap: true}
                            max_lines: 0
                            draw_text +: {
                                color: ROOM_NAME_TEXT_COLOR
                                text_style: theme.font_regular {font_size: 11}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RoomTabHoverCard {
    #[deref] view: View,
    /// The tab whose hover card is open or waiting for the initial delay.
    #[rust] tab_id: Option<LiveId>,
    /// We impose a delay between hover-in and showing the hover card,
    /// just like canonical native tooltips.
    #[rust] show_timer: Timer,
    /// The latest mouse pointer position, used to check whether the card should stay open.
    #[rust] pointer: Option<Vec2d>,
    #[rust] is_open: bool,
    /// The latest drawn tab ID, name, and avatar; avoids re-populating unchanged cards.
    #[rust] drawn_content: Option<(LiveId, String, Option<FetchedRoomAvatar>)>,
    /// The latest rect/bounds of the tab that this hover card is positioned next to.
    #[rust] anchor_rect: Option<Rect>,
}

impl Widget for RoomTabHoverCard {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Scroll(scroll) = event {
            if self.handles_scroll_at(cx, scroll.abs) {
                self.view.tooltip(cx, ids!(tooltip)).view(cx, ids!(name_scroll))
                    .handle_event(cx, event, scope);
                return;
            }
            // some OSes send a scroll event when the trackpack is touched at all,
            // which shouldn't dismiss the hover card (only a real hover out should).
            if scroll.phase == ScrollPhase::Touched { return; }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RoomTabHoverCard {
    fn handles_scroll_at(&self, cx: &mut Cx, pos: Vec2d) -> bool {
        self.is_open && self.view.tooltip(cx, ids!(tooltip))
            .view(cx, ids!(content)).area().rect(cx).contains(pos)
    }

    /// Keep the card open while the mouse is moving across the small gap beneath its tab.
    fn contains_pointer(&self, cx: &mut Cx, dock: &DockRef, pos: Vec2d) -> bool {
        if !self.is_open { return false; }
        let card = self.view.tooltip(cx, ids!(tooltip))
            .view(cx, ids!(content)).area().rect(cx);
        if card.contains(pos) { return true; }
        let Some(tab_id) = self.tab_id else { return false; };
        let tab = dock
            .widget(cx, &[TabBar::tab_node_name(tab_id)]).area().clipped_rect(cx);
        let left = tab.pos.x.max(card.pos.x);
        let right = (tab.pos.x + tab.size.x).min(card.pos.x + card.size.x);
        let (top, bottom) = if card.pos.y >= tab.pos.y + tab.size.y {
            (tab.pos.y + tab.size.y, card.pos.y)
        } else if tab.pos.y >= card.pos.y + card.size.y {
            (card.pos.y + card.size.y, tab.pos.y)
        } else {
            return false;
        };
        right >= left && Rect{pos: dvec2(left, top), size: dvec2(right - left, bottom - top)}.contains(pos)
    }

    fn hide(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.show_timer);
        self.show_timer = Timer::empty();
        self.tab_id = None;
        self.pointer = None;
        if self.is_open {
            self.view.tooltip(cx, ids!(tooltip)).hide(cx);
            self.is_open = false;
        }
    }

    fn update_hover(&mut self, cx: &mut Cx, event: &Event, hover_available: bool, dock: &DockRef, rooms: &HashMap<LiveId, SelectedRoom>) {
        if self.tab_id.is_some_and(|id| !rooms.contains_key(&id)) {
            self.hide(cx);
        }
        match event {
            Event::MouseMove(mouse) => {
                let is_over_current_tab = self.tab_id.is_some_and(|tab_id| {
                    let rect = dock.widget(cx, &[TabBar::tab_node_name(tab_id)])
                        .area().clipped_rect(cx);
                    if rect.size.x > 0.0 && rect.size.y > 0.0 {
                        rect.contains(mouse.abs)
                    } else {
                        self.is_open
                            && self.drawn_content.as_ref().is_some_and(|(id, _, _)| *id == tab_id)
                            && self.anchor_rect.is_some_and(|rect| rect.contains(mouse.abs))
                    }
                });
                if cx.fingers.first_mouse_button.is_none()
                    && (is_over_current_tab || self.contains_pointer(cx, dock, mouse.abs))
                {
                    self.pointer = Some(mouse.abs);
                    // Mouse pointer motion within this tab/card must not restart or
                    // redraw the tooltip; it is already open (or waiting).
                    return;
                }
                if !hover_available || cx.fingers.first_mouse_button.is_some() {
                    self.hide(cx);
                    return;
                }
                let hovered = rooms.keys().copied().find(|id| {
                    // Only do hit-testing after drawing, when the native tab's final area rect
                    // is available, including the overlaid action buttons.
                    let rect = dock.widget(cx, &[TabBar::tab_node_name(*id)])
                        .area().clipped_rect(cx);
                    rect.size.x > 0.0 && rect.size.y > 0.0 && rect.contains(mouse.abs)
                });
                let changed = hovered != self.tab_id;
                self.tab_id = hovered;
                self.pointer = Some(mouse.abs);
                if hovered.is_none() {
                    self.hide(cx);
                } else if changed {
                    cx.stop_timer(self.show_timer);
                    self.show_timer = Timer::empty();
                    if self.is_open {
                        // Replace an open card immediately, without hiding it.
                        self.show_for_hovered_tab(cx, dock, rooms);
                    } else {
                        self.show_timer = cx.start_timeout(0.4);
                    }
                }
            }
            Event::MouseDown(_) | Event::MouseLeave(_) | Event::Scroll(_)
            | Event::TouchUpdate(_) | Event::KeyDown(_) | Event::BackPressed { .. }
            | Event::WindowLostFocus(_) | Event::WindowGeomChange(_) | Event::ClearHover => {
                self.hide(cx);
            }
            _ => {}
        }
        if self.show_timer.is_event(event).is_some() {
            self.show_timer = Timer::empty();
            self.show_for_hovered_tab(cx, dock, rooms);
        } else if self.is_open && matches!(event, Event::Signal | Event::Actions(_) | Event::NextFrame(_)) {
            self.show_for_hovered_tab(cx, dock, rooms);
        }
    }

    fn show_for_hovered_tab(&mut self, cx: &mut Cx, dock: &DockRef, rooms: &HashMap<LiveId, SelectedRoom>) {
        let Some(tab_id) = self.tab_id else { return };
        let Some(room) = rooms.get(&tab_id) else { return };
        let name = room.display_name();
        let avatar_data = if cx.has_global::<RoomsListRef>() {
            cx.get_global::<RoomsListRef>().get_room_avatar(room.room_id())
        } else {
            None
        };
        let content_changed = self.drawn_content.as_ref() != Some(&(tab_id, name.clone(), avatar_data.clone()));
        let tab_rect = dock.widget(cx, &[TabBar::tab_node_name(tab_id)]).area().clipped_rect(cx);

        // A redraw can temporarily invalidate a tab's area.
        // That doesn't mean the mouse hovered-out, so just wait for the next redraw.
        if tab_rect.size.x <= 0.0 || tab_rect.size.y <= 0.0 {
            return;
        }

        // Actions, sync signals, and animation frames must not rebuild a card
        // whose contents and anchor are unchanged.
        if self.is_open && !content_changed && self.anchor_rect == Some(tab_rect) {
            return;
        }
        if !self.pointer.is_some_and(|pos| {
            tab_rect.contains(pos) || self.contains_pointer(cx, dock, pos)
        }) {
            self.hide(cx);
            return;
        }

        let tooltip = self.view.tooltip(cx, ids!(tooltip));
        if content_changed {
            tooltip.view(cx, ids!(name_scroll)).set_scroll_pos(cx, Vec2d::default());
            let avatar = tooltip.avatar(cx, ids!(avatar));
            let avatar_name = match &avatar_data {
                Some(FetchedRoomAvatar::Text(text)) => text.clone(),
                _ => room.room_name().to_string(),
            };
            avatar.show_text(cx, None, None, &avatar_name);
            if let Some(FetchedRoomAvatar::Image(image)) = &avatar_data {
                let _ = avatar.show_image(cx, None, |cx, img| crate::utils::load_avatar_image(&img, cx, image));
            }
            self.drawn_content = Some((tab_id, name.clone(), avatar_data));
        }

        // set up all the layout parameters, width and height (w/ padding/margin spacing).
        let bounds = cx.get_window_id_of(&self.view.area()).map_or_else(
            || self.view.area().rect(cx),
            |window_id| Rect{pos: Vec2d::default(), size: cx.windows[window_id].get_inner_size()},
        );
        // Use the same gap value as the margin below the tab and for the left/right edge of the app window.
        // The shadow border also fits inside this margin.
        let gap = 4.0;
        let width = (bounds.size.x - 2.0 * gap).clamp(1.0, 272.0);
        let text_height = tooltip.label(cx, ids!(tooltip_label)).borrow().map_or(44.0, |label| {
            let text_width = (width - 24.0 - 44.0 - 12.0 - 10.0).max(1.0);
            let scale = (label.draw_text.font_scale as f64).max(0.0001);
            label.draw_text.layout(
                cx, 0.0, 0.0, Some((text_width / scale) as f32), true, label.align, &name,
            ).size_in_lpxs.height as f64 * scale
        });
        let available_height = (bounds.size.y - 2.0 * gap).max(1.0);
        let height = (24.0 + text_height.ceil().max(44.0)).min(available_height);
        let content = tooltip.view(cx, ids!(content));
        if let Some(mut content) = content.borrow_mut() {
            content.walk.width = Size::Fixed(width);
            content.walk.height = Size::Fixed(height);
        }
        tooltip.label(cx, ids!(tooltip_label)).set_text(cx, &name);
        tooltip.show_anchored(cx, TooltipAnchor {
            rect: tab_rect,
            side: TooltipPosition::Bottom,
            gap,
            natural_width: None,
        });
        self.is_open = true;
        self.anchor_rect = Some(tab_rect);
    }

}

impl RoomTabHoverCardRef {
    /// Handle scrolls over the card, and keep a trackpad touch from dismissing it.
    /// Returns true if the rest of the desktop view should skip this event.
    pub fn handle_scroll(&self, cx: &mut Cx, event: &Event, scope: &mut Scope, dock: &DockRef) -> bool {
        let Event::Scroll(scroll) = event else { return false };
        if self.handles_scroll_at(cx, scroll.abs) {
            self.handle_event(cx, event, scope);
            return true;
        }
        if scroll.phase == ScrollPhase::Touched {
            dock.handle_event(cx, event, scope);
            return true;
        }
        false
    }

    /// Prevents a scroll movement while hovered over the card from reaching the dock underneath it.
    pub fn handles_scroll_at(&self, cx: &mut Cx, pos: Vec2d) -> bool {
        self.borrow().is_some_and(|inner| inner.handles_scroll_at(cx, pos))
    }

    /// Call this after the dock's tab & button event handling has completed.
    pub fn update_hover(
        &self,
        cx: &mut Cx,
        event: &Event,
        hover_available: bool,
        dock: &DockRef,
        rooms: &HashMap<LiveId, SelectedRoom>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_hover(cx, event, hover_available, dock, rooms);
        }
    }
}
