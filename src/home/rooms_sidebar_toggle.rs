//! The `ToggleRoomsSideBarButton`, which collapses and expands the rooms sidebar in desktop view mode.

use std::collections::HashMap;
use makepad_widgets::{scroll_motion::critically_damped_spring, *};

use crate::{app::AppState, home::rooms_sidebar::{RoomsSideBarRef, RoomsSideBarWidgetRefExt}, room::room_action_bar::RoomActionTooltip};

/// The decay rate of the spring that moves the sidebar, which is the same spring
/// that finishes a swipe back to the previous room in mobile view mode.
const SPRING_DECAY_RATE: f64 = 18.257_418_583_505_537;
/// The spring is within a fraction of a pixel of resting after this long.
const ANIMATION_DURATION_SECS: f64 = 0.55;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ToggleRoomsSideBarButton = #(ToggleRoomsSideBarButton::register_widget(vm)) {
        width: Fit, height: Fit
        // The button's icon while the rooms list is showing, and while it's hidden.
        hide_icon: (ICON_SIDEBAR_HIDE)
        show_icon: (ICON_SIDEBAR_SHOW)

        button := mod.widgets.NavHistoryButton {
            draw_icon +: { svg: (ICON_SIDEBAR_HIDE) }
            icon_walk: Walk{width: 18, height: 18}
        }
    }
}

/// An action emitted to collapse the rooms sidebar in desktop view mode, or to expand it if it's collapsed.
#[derive(Debug)]
pub struct ToggleRoomsSideBarAction;

#[derive(Script, Widget)]
pub struct ToggleRoomsSideBarButton {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[live] hide_icon: Option<ScriptHandleRef>,
    #[live] show_icon: Option<ScriptHandleRef>,
    #[rust] tooltip: RoomActionTooltip,
    /// Whether the button shows `show_icon` rather than `hide_icon`.
    #[rust] is_show_icon_shown: bool,
}

impl ScriptHook for ToggleRoomsSideBarButton {
    fn on_after_reload(&mut self, _vm: &mut ScriptVm) {
        // A reload gives the button its default icon again.
        self.is_show_icon_shown = false;
    }
}

impl Widget for ToggleRoomsSideBarButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let is_collapsed = scope.data.get::<AppState>().is_some_and(|app_state| app_state.is_rooms_sidebar_collapsed);
        // The rooms list can also be hidden or shown without us, like by dragging its splitter, so we check on every event.
        if is_collapsed != self.is_show_icon_shown {
            self.is_show_icon_shown = is_collapsed;
            if let Some(mut button) = self.view.child(id!(button)).as_button().borrow_mut() {
                button.draw_icon.svg = if is_collapsed { self.show_icon.clone() } else { self.hide_icon.clone() };
            }
            self.view.redraw(cx);
        }

        // This is lazy, as the tooltip only looks at the button for the few events that can show it.
        let button = std::iter::once_with(|| {
            (self.view.child(id!(button)), if is_collapsed { "Show rooms list" } else { "Hide rooms list" })
        });
        self.tooltip.handle_event(cx, event, button, TooltipPosition::Bottom);

        self.view.handle_event(cx, event, scope);

        if let Event::Actions(actions) = event
            && self.view.child(id!(button)).as_button().clicked(actions)
        {
            cx.action(ToggleRoomsSideBarAction);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

/// Collapses the rooms sidebar in the desktop dock to zero width, and expands it again, animating its width.
///
/// Whether it's collapsed applies to every space's dock (see [`AppState::is_rooms_sidebar_collapsed`]),
/// so a dock is always saved with the sidebar's expanded width.
#[derive(Default)]
pub struct CollapsibleRoomsSideBar {
    /// The sidebar while it's collapsed or animating, which is `None` while it's fully expanded.
    collapsed: Option<CollapsedSideBar>,
}

struct CollapsedSideBar {
    splitter_id: LiveId,
    expanded_align: SplitterAlign,
    /// How much of the sidebar's expanded width is showing, from 0 (collapsed) to 1 (expanded).
    visible_fraction: f64,
    dock: DockRef,
    rooms_sidebar: RoomsSideBarRef,
    animation: Option<Animation>,
}

struct Animation {
    /// When the animation started, from [`Cx::monotonic_now()`].
    started_at: f64,
    from_fraction: f64,
    is_collapsing: bool,
    next_frame: NextFrame,
}

impl CollapsibleRoomsSideBar {
    /// Starts collapsing the rooms sidebar in the given dock,
    /// or expanding it back to its former width if it's collapsed or collapsing.
    pub fn toggle(&mut self, cx: &mut Cx, dock: &DockRef, app_state: &mut AppState) {
        if self.collapsed.is_none() {
            self.collapsed = CollapsedSideBar::find_in_dock(dock, 1.0);
        }
        let Some(collapsed) = &mut self.collapsed else { return };
        let is_collapsing = collapsed.animation.as_ref().map_or(collapsed.visible_fraction > 0.0, |animation| !animation.is_collapsing);
        app_state.is_rooms_sidebar_collapsed = is_collapsing;
        collapsed.animation = Some(Animation {
            started_at: Cx::monotonic_now(),
            from_fraction: collapsed.visible_fraction,
            is_collapsing,
            next_frame: cx.new_next_frame(),
        });
    }

    /// Moves the sidebar's edge along its animation on each frame.
    pub fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        let Some(collapsed) = &mut self.collapsed else { return };
        let Some(animation) = &mut collapsed.animation else { return };
        if animation.next_frame.is_event(event).is_none() {
            return;
        }
        let elapsed = Cx::monotonic_now() - animation.started_at;
        let is_done = elapsed >= ANIMATION_DURATION_SECS;
        let to_fraction = if animation.is_collapsing { 0.0 } else { 1.0 };
        collapsed.visible_fraction = if is_done {
            to_fraction
        } else {
            to_fraction + critically_damped_spring(animation.from_fraction - to_fraction, 0.0, SPRING_DECAY_RATE, elapsed)
        };
        let align = match collapsed.expanded_align {
            SplitterAlign::FromA(width) => SplitterAlign::FromA(width * collapsed.visible_fraction),
            SplitterAlign::Weighted(weight) => SplitterAlign::Weighted(weight * collapsed.visible_fraction),
            // We can't scale this without the dock's width, so the sidebar only shows once it's fully expanded.
            SplitterAlign::FromB(_) if collapsed.visible_fraction < 1.0 => SplitterAlign::FromA(0.0),
            SplitterAlign::FromB(_) => collapsed.expanded_align,
        };
        collapsed.rooms_sidebar.set_visible_fraction((!is_done).then_some(collapsed.visible_fraction));
        collapsed.dock.set_splitter_align(cx, collapsed.splitter_id, align, false);
        if !is_done {
            animation.next_frame = cx.new_next_frame();
        } else if animation.is_collapsing {
            collapsed.animation = None;
        } else {
            self.collapsed = None;
        }
    }

    /// Expands the rooms sidebar in the given dock back to its former width right away, if it's not fully expanded.
    pub fn expand(&mut self, cx: &mut Cx, dock: &DockRef) {
        if let Some(collapsed) = self.collapsed.take() {
            dock.set_splitter_align(cx, collapsed.splitter_id, collapsed.expanded_align, false);
        }
    }

    /// Collapses the rooms sidebar in the given dock that was just loaded, if the user collapsed it.
    pub fn restore_state(&mut self, cx: &mut Cx, dock: &DockRef, app_state: &AppState) {
        // The width we remembered belongs to the previous dock.
        self.collapsed = None;
        if app_state.is_rooms_sidebar_collapsed {
            self.collapsed = CollapsedSideBar::find_in_dock(dock, 0.0);
            if let Some(collapsed) = &self.collapsed {
                dock.set_splitter_align(cx, collapsed.splitter_id, SplitterAlign::FromA(0.0), false);
            }
        }
    }

    /// Puts the sidebar's width from before it was collapsed into the given dock items that are being saved.
    pub fn save_state(&self, dock_items: &mut HashMap<LiveId, DockItem>) {
        if let Some(collapsed) = &self.collapsed
            && let Some(DockItem::Splitter { align, .. }) = dock_items.get_mut(&collapsed.splitter_id)
        {
            *align = collapsed.expanded_align;
        }
    }

    /// Handles the user dragging the given splitter, which reopens a collapsed sidebar if it's the sidebar's splitter.
    pub fn handle_splitter_dragged(&mut self, splitter_id: LiveId, app_state: &mut AppState) {
        if self.collapsed.take_if(|collapsed| collapsed.splitter_id == splitter_id).is_some() {
            app_state.is_rooms_sidebar_collapsed = false;
        }
    }
}

impl CollapsedSideBar {
    /// Finds the rooms sidebar in the given dock, with the given fraction of its width showing.
    fn find_in_dock(dock: &DockRef, visible_fraction: f64) -> Option<Self> {
        let (splitter_id, align) = dock.clone_state()?.into_iter().find_map(|(splitter_id, item)| match item {
            DockItem::Splitter { a: id!(rooms_sidebar_tabs), align, .. } => Some((splitter_id, align)),
            _ => None,
        })?;
        // We can only animate the sidebar's own width or weight,
        // so if it's sized by the main side's width, we use the share of the dock it has now.
        let expanded_align = match (align, dock.splitter_position(splitter_id)) {
            (SplitterAlign::FromB(other_width), Some(position)) => SplitterAlign::Weighted(position / (position + other_width)),
            _ => align,
        };
        Some(Self {
            splitter_id,
            expanded_align,
            visible_fraction,
            dock: dock.clone(),
            rooms_sidebar: dock.item(id!(rooms_sidebar_tab)).as_rooms_side_bar(),
            animation: None,
        })
    }
}

impl Drop for CollapsedSideBar {
    fn drop(&mut self) {
        self.rooms_sidebar.set_visible_fraction(None);
    }
}
