//! A dock around a room's timeline that shows that room's panes, e.g., its member list.
//!
//! Each pane is docked to one of the four edges around the timeline,
//! can be resized along its edge's one meaningful axis via a grab handle on its inner border,
//! moved to the next edge, popped out into its own tab or view, or closed.
//! A dock's panes belong to its current timeline, and are saved and restored along with it.

use std::sync::Arc;

use makepad_widgets::*;
use matrix_sdk::room::RoomMember;

use crate::{
    app::AppStateAction,
    shared::styles::COLOR_ROBRIX_PURPLE,
    sliding_sync::{MatrixRequest, TimelineKind, submit_async_request},
    utils::{self, RoomNameId},
    LivePtr, widget_ref_from_live_ptr,
};
use super::{
    room_action_bar::RoomActionTooltip,
    room_members_list::{RoomMembersListRef, RoomMembersListWidgetRefExt, SavedRoomMembersList},
    room_pane::{self, PaneLayout, PaneSide, RoomPaneKind, RoomPanesPending},
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A small icon-only button in a pane's header.
    mod.widgets.RoomPaneHeaderButton = RobrixIconButton {
        width: #(HEADER_BUTTON_SIZE), height: #(HEADER_BUTTON_SIZE)
        padding: 0, spacing: 0, margin: 0
        align: Align{x: 0.5, y: 0.5}
        icon_walk: Walk{width: 13, height: 13, margin: 0}
        draw_icon.color: #666
        draw_bg +: {
            border_size: 0
            color: #0000
            color_hover: #00000015
            color_down: #00000025
        }
    }

    // A pane's icon and title, and the name of its room beneath the title.
    // The icon and title are as tall as a header button, so they line up with the top row of buttons.
    mod.widgets.RoomPaneTitle = View {
        width: Fill, height: Fit
        flow: Right
        spacing: #(HEADER_SPACING)
        pane_icon := Icon {
            height: #(HEADER_BUTTON_SIZE)
            align: Align{y: 0.5}
            icon_walk: Walk{width: 16, height: 16, margin: Inset{left: 2, right: 2}}
            draw_icon +: { color: (COLOR_TEXT) }
        }
        titles := View {
            width: Fill, height: Fit
            flow: Down
            // Its top padding is set by `set_pane_title()`, so that the title's first line
            // stays centered on the top row of buttons even when it wraps.
            title_line := View {
                width: Fill, height: Fit{min: FitBound.Abs(#(HEADER_BUTTON_SIZE))}
                pane_title := Label {
                    width: Fill, height: Fit
                    flow: Flow.Right{wrap: true}
                    max_lines: 2, text_overflow: Ellipsis
                    padding: 0, margin: 0
                    draw_text +: {
                        text_style: theme.font_bold {font_size: 11},
                        color: (COLOR_TEXT)
                    }
                }
            }
            // The name of the pane's room, which is only needed once the pane is popped out of it.
            pane_room := Label {
                width: Fill, height: Fit
                flow: Flow.Right{wrap: true}
                padding: 0, margin: 0
                draw_text +: {
                    text_style: REGULAR_TEXT {font_size: 8.5},
                    color: (MESSAGE_TEXT_COLOR)
                }
            }
        }
    }

    // The frame around a pane's content: a header bar with the pane's title and buttons.
    // Its edge draws the divider on its inner side, and its content spaces itself vertically.
    mod.widgets.RoomPaneFrame = SolidView {
        width: Fill, height: Fill
        flow: Down
        padding: Inset{top: 0, right: #(FRAME_PADDING), bottom: 0, left: #(FRAME_PADDING)}
        show_bg: true
        draw_bg +: { color: (COLOR_PRIMARY) }

        header := View {
            width: Fill, height: Fit
            flow: Right
            spacing: #(HEADER_SPACING)
            margin: Inset{top: 3, bottom: 4}

            // This stays at the top when the buttons are stacked beside it.
            // A docked pane is shown within its room, so it doesn't show the room's name.
            title_row := mod.widgets.RoomPaneTitle {
                titles +: { pane_room +: { visible: false } }
            }

            // The close button stays in the top-right corner, and the other buttons
            // are placed around it by `layout_headers()`: leftwards, then below it.
            header_buttons := View {
                width: Fit, height: Fit
                flow: Overlay

                pane_edge_button := mod.widgets.RoomPaneHeaderButton {
                    draw_icon.svg: (ICON_CARET_DOWN)
                }
                pane_pop_out_button := mod.widgets.RoomPaneHeaderButton {
                    margin: Inset{left: #(HEADER_BUTTON_STEP)}
                    draw_icon.svg: (ICON_EXTERNAL_LINK)
                }
                pane_close_button := mod.widgets.RoomPaneHeaderButton {
                    margin: Inset{left: #(2.0 * HEADER_BUTTON_STEP)}
                    icon_walk: Walk{width: 12, height: 12, margin: 0}
                    draw_icon.svg: (ICON_CLOSE)
                }
            }
        }

        content := View {
            width: Fill, height: Fill
            flow: Down
            padding: Inset{top: 2}
        }
    }

    // One edge of the dock. Its panes are owned by the RoomPaneDock and drawn here manually.
    mod.widgets.RoomPaneEdge = set_type_default() do #(RoomPaneEdge::register_widget(vm)) {
        ..mod.widgets.RoundedView
        width: Fit, height: Fit
        draw_bg +: { color: #0000 }
        draw_handle +: {
            color: #4D4D4D
            // The grab handle: a soft capsule with dots that is quiet until you get near it.
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let w = self.rect_size.x
                let h = self.rect_size.y
                let dot = vec4(1.0, 1.0, 1.0, 0.92)
                if h > w {
                    sdf.box(0.0, 0.0, w, h, w * 0.5)
                    sdf.fill(self.color)
                    let gap = w * 0.52
                    let cy = h * 0.5
                    sdf.circle(w * 0.5, cy - gap * 2.0, 1.15)
                    sdf.circle(w * 0.5, cy - gap, 1.15)
                    sdf.circle(w * 0.5, cy, 1.15)
                    sdf.circle(w * 0.5, cy + gap, 1.15)
                    sdf.circle(w * 0.5, cy + gap * 2.0, 1.15)
                    return sdf.fill(dot)
                }
                sdf.box(0.0, 0.0, w, h, h * 0.5)
                sdf.fill(self.color)
                let gap = h * 0.52
                let cx = w * 0.5
                sdf.circle(cx - gap * 2.0, h * 0.5, 1.15)
                sdf.circle(cx - gap, h * 0.5, 1.15)
                sdf.circle(cx, h * 0.5, 1.15)
                sdf.circle(cx + gap, h * 0.5, 1.15)
                sdf.circle(cx + gap * 2.0, h * 0.5, 1.15)
                return sdf.fill(dot)
            }
        }
        draw_grab +: { color: #00000001 }
        draw_divider +: {
            color: #4D4D4D
            // Rounded end caps, like a dock splitter's.
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, min(self.rect_size.x, self.rect_size.y) * 0.5)
                return sdf.fill(self.color)
            }
        }

        // Panes slide in and out from this edge's side of the dock.
        slide: 1.0
        // Like a dock splitter, the divider and grab handle turn purple when hovered or dragged.
        hover: 0.0
        animator: Animator {
            hover: {
                default: @off
                off: AnimatorState{
                    redraw: true
                    from: {all: Forward {duration: 0.1}}
                    apply: { hover: 0.0 }
                }
                on: AnimatorState{
                    redraw: true
                    from: {all: Snap}
                    apply: { hover: 1.0 }
                }
            }
            panel: {
                default: @hide
                show: AnimatorState{
                    redraw: true
                    from: {all: Forward {duration: 0.35}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { slide: 0.0 }
                }
                hide: AnimatorState{
                    redraw: true
                    from: {all: Forward {duration: 0.35}}
                    ease: Ease.ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { slide: 1.0 }
                }
            }
        }
    }

    // Docked panes are real flow children around the center, so they reflow the timeline.
    mod.widgets.RoomPaneDock = set_type_default() do #(RoomPaneDock::register_widget(vm)) {
        ..mod.widgets.RoundedView
        width: Fill, height: Fill
        flow: Overlay

        body := View {
            width: Fill, height: Fill
            flow: Down
            edge_top := mod.widgets.RoomPaneEdge {}
            mid := View {
                width: Fill, height: Fill
                flow: Right
                edge_left := mod.widgets.RoomPaneEdge {}
                center := View { width: Fill, height: Fill, flow: Down }
                edge_right := mod.widgets.RoomPaneEdge {}
            }
            edge_bottom := mod.widgets.RoomPaneEdge {}
        }

        // The templates for each kind of pane.
        members_pane: mod.widgets.RoomPaneFrame {
            header +: { title_row +: { pane_icon +: { draw_icon +: { svg: (ICON_MEMBERS) } } } }
            content +: { room_members := mod.widgets.RoomMembersList {} }
        }
    }
}

/// The spacing between the header's icon, titles, and buttons.
/// The horizontal padding of a pane's frame.
const FRAME_PADDING: f64 = 10.0;
const HEADER_SPACING: f64 = 4.0;
const HEADER_BUTTON_SIZE: f64 = 25.0;
const HEADER_BUTTON_SPACING: f64 = 2.0;
/// The distance between the starts of adjacent header buttons.
const HEADER_BUTTON_STEP: f64 = HEADER_BUTTON_SIZE + HEADER_BUTTON_SPACING;
/// The dock always leaves at least this much space for the timeline in the center.
const MIN_CENTER_SIZE: f64 = 150.0;

/// A timeline's room members: `None` until they're fetched, or the error if that failed.
type TimelineMembers = Option<Result<Arc<Vec<RoomMember>>, String>>;

/// Shows the given room's info in the content of the given pane.
fn populate_content(
    cx: &mut Cx,
    kind: RoomPaneKind,
    frame: &WidgetRef,
    room_name_id: &RoomNameId,
    room_members: &TimelineMembers,
) {
    match kind {
        RoomPaneKind::Members => {
            let list = frame.child_by_path(ids!(content.room_members)).as_room_members_list();
            show_members(cx, &list, room_name_id, room_members);
        }
    }
}

fn show_members(cx: &mut Cx, list: &RoomMembersListRef, room_name_id: &RoomNameId, room_members: &TimelineMembers) {
    match room_members {
        Some(Ok(members)) => list.set_members(cx, room_name_id, Some(members.clone())),
        Some(Err(error)) => {
            list.set_members(cx, room_name_id, None);
            list.set_error(cx, error.clone());
        }
        None => list.set_members(cx, room_name_id, None),
    }
}

/// The saved state of a pane's content.
#[derive(Clone)]
enum SavedPaneContent {
    Members(SavedRoomMembersList),
}

fn save_content(kind: RoomPaneKind, frame: &WidgetRef) -> SavedPaneContent {
    match kind {
        RoomPaneKind::Members => SavedPaneContent::Members(
            frame.child_by_path(ids!(content.room_members)).as_room_members_list().save_state()
        ),
    }
}

fn restore_content(
    cx: &mut Cx,
    frame: &WidgetRef,
    room_name_id: &RoomNameId,
    content: SavedPaneContent,
    room_members: &TimelineMembers,
) {
    match content {
        SavedPaneContent::Members(saved) => {
            let list = frame.child_by_path(ids!(content.room_members)).as_room_members_list();
            list.restore_state(cx, room_name_id, saved);
            show_members(cx, &list, room_name_id, room_members);
        }
    }
}

/// The state of a docked pane that is saved and restored along with its timeline.
#[derive(Clone)]
pub struct SavedRoomPane {
    kind: RoomPaneKind,
    layout: PaneLayout,
    content: SavedPaneContent,
}

/// Sets the title of the given pane (or popped-out pane), with its first line centered
/// on the top row of the header buttons.
pub fn set_pane_title(cx: &mut Cx, pane: &WidgetRef, title: &str) {
    let label = pane.label(cx, ids!(pane_title));
    label.set_text(cx, title);
    let line_height = label.borrow()
        .map_or(0.0, |label| utils::text_line_height(cx, &label.draw_text));
    let top = ((HEADER_BUTTON_SIZE - line_height) * 0.5).max(0.0);
    let mut title_line = pane.widget(cx, ids!(title_line));
    script_apply_eval!(cx, title_line, {
        padding: mod.prelude.widgets.Inset{top: #(top)}
    });
}

/// Returns the width of the given pane's title laid out on one line.
fn measure_title_width(cx: &mut Cx, frame: &WidgetRef) -> f64 {
    let title = frame.label(cx, ids!(pane_title));
    let text = title.text();
    title.borrow().map_or(0.0, |label| utils::unwrapped_text_width(cx, &label.draw_text, &text))
}

/// Returns the tooltip for a pane's edge button, which moves it to the given side.
fn move_tooltip(side: PaneSide) -> &'static str {
    match side {
        PaneSide::Top => "Move to the top",
        PaneSide::Bottom => "Move to the bottom",
        PaneSide::Left => "Move to the left",
        PaneSide::Right => "Move to the right",
    }
}

struct DockedPane {
    kind: RoomPaneKind,
    frame: WidgetRef,
    layout: PaneLayout,
    /// The edge this pane is on, or `None` while it waits to slide out of its previous edge.
    placed: Option<PaneSide>,
    /// How many columns the header's buttons were last placed in, or 0 if not yet placed,
    /// so they're only moved (which redraws them) when this changes.
    button_cols: usize,
    /// The width of the title on one line, which the buttons must leave room for.
    title_width: f64,
}

#[derive(Script, Widget)]
pub struct RoomPaneDock {
    #[deref] view: View,
    #[live] members_pane: Option<LivePtr>,
    #[rust] room_name_id: Option<RoomNameId>,
    /// The timeline that this dock's panes belong to.
    #[rust] timeline_kind: Option<TimelineKind>,
    /// Our timeline's room members, shared with its RoomScreen and given to any members pane.
    #[rust] room_members: TimelineMembers,
    #[rust] panes: Vec<DockedPane>,
    #[rust] sides_assigned: bool,
    /// A pane was removed while this dock was hidden, so the timeline beneath it
    /// wasn't laid out again; the next time this room is focused, we redraw everything.
    #[rust] needs_full_redraw: bool,
    /// Whether we're in the middle of drawing (across multiple draw steps).
    #[rust] is_drawing: bool,
    #[rust] tooltip: RoomActionTooltip,
}

impl ScriptHook for RoomPaneDock {
    fn on_after_apply(&mut self, vm: &mut ScriptVm, _apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        vm.cx_mut().widget_tree_mark_dirty(self.widget_uid());
    }
}

impl Widget for RoomPaneDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.sides_assigned {
            self.sides_assigned = true;
            for side in ALL_SIDES {
                self.edge(cx, side).set_side(side);
            }
        }
        let buttons: Vec<(WidgetRef, &'static str)> = self.panes.iter().flat_map(|pane| [
            (pane.frame.widget(cx, ids!(pane_edge_button)), move_tooltip(pane.layout.side.next())),
            (pane.frame.widget(cx, ids!(pane_pop_out_button)), "Pop out"),
            (pane.frame.widget(cx, ids!(pane_close_button)), "Close"),
        ]).collect();
        self.tooltip.handle_event(cx, event, buttons, TooltipPosition::Bottom);

        // Makepad resolves overlapping hits by dispatch order, not draw order:
        // the grab handles overlap both the panes and the timeline, and the panes are drawn manually.
        let mut grab_handle_pressed = false;
        for side in ALL_SIDES {
            grab_handle_pressed |= self.edge(cx, side).handle_grab_handle_event(cx, event);
        }
        // A press on a grab handle is handled only by it, not by anything beneath it.
        if !grab_handle_pressed {
            for pane in &self.panes {
                pane.frame.handle_event(cx, event, scope);
            }
            self.view.handle_event(cx, event, scope);
        }

        // An edge can't redraw itself (it doesn't draw its own view), and its size affects our layout.
        if ALL_SIDES.iter().any(|side| self.edge(cx, *side).take_redraw_request()) {
            self.view.redraw(cx);
        }

        let Event::Actions(actions) = event else { return };
        let Some(room_name_id) = self.room_name_id.clone() else { return };
        let room_id = room_name_id.room_id();

        for action in actions {
            if let Some(RoomPanesPending { timeline_kind }) = action.downcast_ref()
                && self.timeline_kind.as_ref() == Some(timeline_kind)
            {
                self.dock_pending(cx);
                continue;
            }
            if self.needs_full_redraw
                && let Some(AppStateAction::RoomFocused(room)) = action.downcast_ref()
                && room.room_id() == room_id
            {
                self.needs_full_redraw = false;
                cx.redraw_all();
                continue;
            }
            if let Some(AppStateAction::RoomNameUpdated(new_name)) = action.downcast_ref()
                && new_name.room_id() == room_id
            {
                self.set_room_name(cx, new_name);
                continue;
            }
            if let Some(widget_action) = action.as_widget_action() {
                match widget_action.cast() {
                    // The user let go of an edge's grab handle, so all panes on that edge keep the new size.
                    RoomPaneEdgeAction::Resized { side, size }
                        if widget_action.widget_uid == self.edge(cx, side).widget_uid() =>
                    {
                        for pane in self.panes.iter_mut().filter(|pane| pane.layout.side == side) {
                            pane.layout.edge_size = size;
                            room_pane::set_last_layout(pane.layout);
                        }
                        self.place_panes(cx, true);
                    }
                    _ => {}
                }
            }
        }

        // Press-based: a manual redraw between down and up can lose the finger capture,
        // so these manually-drawn frames never reliably see a click. A press always arrives.
        // A button's tooltip may emit an action before its press, so check all of its actions.
        let pressed: Vec<(RoomPaneKind, PaneButton)> = self.panes.iter().filter_map(|pane| {
            let hit = |id: &[LiveId]| actions
                .filter_widget_actions(pane.frame.button(cx, id).widget_uid())
                .any(|action| matches!(action.cast(), ButtonAction::Pressed(_)));
            let button = if hit(ids!(pane_close_button)) {
                PaneButton::Close
            } else if hit(ids!(pane_edge_button)) {
                PaneButton::MoveToNextEdge
            } else if hit(ids!(pane_pop_out_button)) {
                PaneButton::PopOut
            } else {
                return None;
            };
            Some((pane.kind, button))
        }).collect();
        for (kind, button) in pressed {
            self.tooltip.hide(cx);
            match button {
                PaneButton::Close => self.remove_pane(cx, kind, true),
                PaneButton::MoveToNextEdge => {
                    if let Some(pane) = self.panes.iter_mut().find(|pane| pane.kind == kind) {
                        pane.layout.side = pane.layout.side.next();
                        room_pane::set_last_layout(pane.layout);
                    }
                    self.place_panes(cx, true);
                }
                PaneButton::PopOut => {
                    self.remove_pane(cx, kind, true);
                    if let Some(timeline_kind) = self.timeline_kind.clone() {
                        room_pane::pop_out(cx, self.widget_uid(), &room_name_id, kind, timeline_kind);
                    }
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Only upon the first draw step, before our own turtle has begun.
        if !self.is_drawing {
            self.is_drawing = true;
            let rect = cx.peek_walk_turtle(walk);
            self.clamp_edges(cx, rect.size);
            self.layout_headers(cx, rect.size);
        }
        // This yields the timeline's PortalList to our parent RoomScreen.
        let step = self.view.draw_walk(cx, scope, walk);
        if step.is_done() {
            self.is_drawing = false;
            // The grab handles straddle the panes' borders, so they're drawn over the timeline.
            for side in ALL_SIDES {
                self.edge(cx, side).draw_divider(cx);
            }
        }
        step
    }
}

enum PaneButton {
    Close,
    MoveToNextEdge,
    PopOut,
}

const ALL_SIDES: [PaneSide; 4] = [PaneSide::Top, PaneSide::Bottom, PaneSide::Left, PaneSide::Right];

impl RoomPaneDock {
    fn edge(&self, cx: &mut Cx, side: PaneSide) -> RoomPaneEdgeRef {
        let id = match side {
            PaneSide::Top => ids!(edge_top),
            PaneSide::Bottom => ids!(edge_bottom),
            PaneSide::Left => ids!(edge_left),
            PaneSide::Right => ids!(edge_right),
        };
        self.view.room_pane_edge(cx, id)
    }

    /// Updates the room name used by each pane's content, e.g., when showing a member's profile.
    fn set_room_name(&mut self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if self.room_name_id.as_ref().is_none_or(|r| r.room_id() != room_name_id.room_id()) {
            return;
        }
        for pane in &self.panes {
            populate_content(cx, pane.kind, &pane.frame, room_name_id, &self.room_members);
        }
        self.room_name_id = Some(room_name_id.clone());
    }

    /// Shows the given members of our timeline's room in any members pane.
    fn set_room_members(&mut self, cx: &mut Cx, room_members: TimelineMembers) {
        self.room_members = room_members;
        let Some(room_name_id) = self.room_name_id.as_ref() else { return };
        for pane in self.panes.iter().filter(|pane| pane.kind == RoomPaneKind::Members) {
            populate_content(cx, pane.kind, &pane.frame, room_name_id, &self.room_members);
        }
    }

    /// Removes all panes right away, e.g., before this dock shows another timeline.
    fn clear(&mut self, cx: &mut Cx) {
        let kinds: Vec<_> = self.panes.iter().map(|pane| pane.kind).collect();
        for kind in kinds {
            self.remove_pane(cx, kind, false);
        }
        // Also drop any closed pane that's still sliding out.
        for side in ALL_SIDES {
            self.edge(cx, side).clear(cx);
        }
        self.room_name_id = None;
        self.timeline_kind = None;
        self.room_members = None;
    }

    /// Shows the given timeline's saved panes right away, and then slides in any pending panes.
    fn show_timeline(
        &mut self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        timeline_kind: TimelineKind,
        room_members: Option<Arc<Vec<RoomMember>>>,
        saved_panes: Vec<SavedRoomPane>,
    ) {
        self.clear(cx);
        self.room_name_id = Some(room_name_id.clone());
        self.timeline_kind = Some(timeline_kind);
        self.room_members = room_members.map(Ok);
        for saved in saved_panes {
            self.create_pane(cx, saved.kind, saved.layout, Some(saved.content));
        }
        self.place_panes(cx, false);
        self.dock_pending(cx);
    }

    /// Docks any panes waiting to be docked in our timeline.
    fn dock_pending(&mut self, cx: &mut Cx) {
        let Some(timeline_kind) = self.timeline_kind.as_ref() else { return };
        for kind in room_pane::take_pending(timeline_kind) {
            if !self.panes.iter().any(|pane| pane.kind == kind) {
                self.create_pane(cx, kind, room_pane::last_layout(), None);
            }
        }
        self.place_panes(cx, true);
    }

    /// Opens the given kind of pane at the last-chosen layout, or closes it if it's open.
    fn toggle(&mut self, cx: &mut Cx, kind: RoomPaneKind) {
        if self.panes.iter().any(|pane| pane.kind == kind) {
            self.remove_pane(cx, kind, true);
        } else if self.room_name_id.is_some() {
            self.create_pane(cx, kind, room_pane::last_layout(), None);
            self.place_panes(cx, true);
        }
    }

    /// Returns the state of our panes, to be restored when our timeline is shown again.
    fn save_state(&self) -> Vec<SavedRoomPane> {
        self.panes.iter()
            .map(|pane| SavedRoomPane {
                kind: pane.kind,
                layout: pane.layout,
                content: save_content(pane.kind, &pane.frame),
            })
            .collect()
    }

    /// Places each pane on the edge of its layout's side,
    /// optionally sliding panes into edges that had no other panes.
    fn place_panes(&mut self, cx: &mut Cx, animate: bool) {
        for index in 0..self.panes.len() {
            let pane = &self.panes[index];
            let (kind, frame, layout, placed) = (pane.kind, pane.frame.clone(), pane.layout, pane.placed);
            match placed {
                Some(side) if side == layout.side => {
                    self.edge(cx, side).set_size(layout.edge_size);
                    continue;
                }
                // A moving pane vanishes from its old edge, then slides into its new one.
                Some(old_side) => {
                    self.edge(cx, old_side).remove_pane(cx, kind, false);
                    self.panes[index].placed = None;
                }
                None => {}
            }
            self.apply_edge_icon(cx, index);
            // A pane that was closed and then reopened may still be sliding out of an edge:
            // it reverses course if that's its new edge, and otherwise vanishes from the old one.
            for side in ALL_SIDES.into_iter().filter(|side| *side != layout.side) {
                self.edge(cx, side).remove_pane(cx, kind, false);
            }
            let edge = self.edge(cx, layout.side);
            edge.set_size(layout.edge_size);
            edge.add_pane(cx, kind, frame, animate);
            self.panes[index].placed = Some(layout.side);
        }
        self.view.redraw(cx);
    }

    /// Returns the DSL template of the given kind of pane.
    fn frame_template(&self, kind: RoomPaneKind) -> Option<LivePtr> {
        match kind {
            RoomPaneKind::Members => self.members_pane,
        }
    }

    /// Creates a new pane, which is placed on an edge by `place_panes()`.
    fn create_pane(&mut self, cx: &mut Cx, kind: RoomPaneKind, layout: PaneLayout, content: Option<SavedPaneContent>) {
        let Some(room_name_id) = self.room_name_id.clone() else { return };
        let frame = widget_ref_from_live_ptr(cx, self.frame_template(kind));
        if frame.is_empty() {
            error!("BUG: missing the room pane template for {kind:?}");
            return;
        }
        // Link the manually-drawn frame into the widget tree so its children can be found.
        cx.widget_tree_insert_child_deep(
            self.widget_uid(),
            LiveId::from_str(&format!("room_pane_{}", kind.as_str())),
            frame.clone(),
        );
        set_pane_title(cx, &frame, kind.title());
        match content {
            Some(content) => restore_content(cx, &frame, &room_name_id, content, &self.room_members),
            None => populate_content(cx, kind, &frame, &room_name_id, &self.room_members),
        }
        // Our timeline only keeps its members up to date while a members pane is open,
        // so refresh them when one is opened (and fetch any that are missing).
        if kind == RoomPaneKind::Members
            && let Some(timeline_kind) = self.timeline_kind.clone()
        {
            submit_async_request(MatrixRequest::GetRoomMembers {
                timeline_kind,
                memberships: matrix_sdk::RoomMemberships::ACTIVE,
                local_only: false,
            });
        }
        let title_width = measure_title_width(cx, &frame);
        self.panes.push(DockedPane { kind, frame, layout, placed: None, button_cols: 0, title_width });
    }

    fn remove_pane(&mut self, cx: &mut Cx, kind: RoomPaneKind, animate: bool) {
        let Some(index) = self.panes.iter().position(|pane| pane.kind == kind) else { return };
        let pane = self.panes.remove(index);
        if let Some(side) = pane.placed {
            // The edge keeps drawing the pane until it has slid out.
            self.edge(cx, side).remove_pane(cx, kind, animate);
        }
        self.needs_full_redraw = true;
        self.view.redraw(cx);
    }

    /// Points the edge button at wherever the next click would move the pane.
    fn apply_edge_icon(&self, cx: &mut Cx, index: usize) {
        let Some(pane) = self.panes.get(index) else { return };
        let mut button = pane.frame.button(cx, ids!(pane_edge_button));
        match pane.layout.side.next() {
            PaneSide::Top => script_apply_eval!(cx, button, { draw_icon +: { svg: (mod.widgets.ICON_CARET_UP) } }),
            PaneSide::Bottom => script_apply_eval!(cx, button, { draw_icon +: { svg: (mod.widgets.ICON_CARET_DOWN) } }),
            PaneSide::Left => script_apply_eval!(cx, button, { draw_icon +: { svg: (mod.widgets.ICON_CARET_LEFT) } }),
            PaneSide::Right => script_apply_eval!(cx, button, { draw_icon +: { svg: (mod.widgets.ICON_CARET_RIGHT) } }),
        }
    }

    /// Limits each pair of opposite edges such that the center keeps at least `MIN_CENTER_SIZE`.
    fn clamp_edges(&mut self, cx: &mut Cx, dock_size: Vec2d) {
        for (a, b, available) in [
            (PaneSide::Left, PaneSide::Right, dock_size.x),
            (PaneSide::Top, PaneSide::Bottom, dock_size.y),
        ] {
            let (edge_a, edge_b) = (self.edge(cx, a), self.edge(cx, b));
            let (wanted_a, wanted_b) = (edge_a.wanted_extent(), edge_b.wanted_extent());
            let max_total = (available - MIN_CENTER_SIZE).max(0.0);
            let scale = if wanted_a + wanted_b > max_total && available > 0.0 {
                Some(max_total / (wanted_a + wanted_b))
            } else {
                None
            };
            edge_a.set_max_extent(scale.map(|s| wanted_a * s));
            edge_b.set_max_extent(scale.map(|s| wanted_b * s));
        }
    }

    /// Lays out each pane's header buttons in a single column, taking more columns
    /// (leftwards) only while the pane's title still fits unwrapped beside them.
    /// Places each pane's header buttons for the width its edge gives it, before it's drawn.
    fn layout_headers(&mut self, cx: &mut Cx, dock_size: Vec2d) {
        // From the top-right corner: the close button never moves, and the others
        // go to its left while the title still fits beside them, otherwise below it.
        const BUTTONS: [&[LiveId]; 3] = [
            ids!(pane_close_button),
            ids!(pane_pop_out_button),
            ids!(pane_edge_button),
        ];
        for index in 0..self.panes.len() {
            let side = self.panes[index].layout.side;
            let pane_width = if side.is_vertical() {
                self.edge(cx, side).pane_size()
            } else {
                let count = self.panes.iter().filter(|pane| pane.layout.side == side).count();
                dock_size.x / count.max(1) as f64
            };
            let pane = &mut self.panes[index];
            // The icon hasn't been measured before its first draw.
            let icon_width = pane.frame.widget(cx, ids!(pane_icon)).area().rect(cx).size.x;
            let icon_width = if icon_width > 0.0 { icon_width } else { 20.0 };
            let title_area = |cols: usize| pane_width - FRAME_PADDING * 2.0 - icon_width - HEADER_SPACING * 2.0
                - (HEADER_BUTTON_STEP * cols as f64 - HEADER_BUTTON_SPACING);
            let cols = if side.is_vertical() {
                (1..=BUTTONS.len()).rev()
                    .find(|cols| title_area(*cols) >= pane.title_width)
                    .unwrap_or(1)
            } else {
                // Top and bottom panes are short, so their buttons stay in one row.
                BUTTONS.len()
            };
            if cols == pane.button_cols {
                continue;
            }
            pane.button_cols = cols;
            for (i, id) in BUTTONS.iter().enumerate() {
                let x = (cols - 1 - i % cols) as f64 * HEADER_BUTTON_STEP;
                let y = (i / cols) as f64 * HEADER_BUTTON_STEP;
                let mut button = pane.frame.widget(cx, id);
                script_apply_eval!(cx, button, {
                    margin: mod.prelude.widgets.Inset{left: #(x), top: #(y), right: 0, bottom: 0}
                });
            }
        }
    }
}

impl RoomPaneDockRef {
    /// See [`RoomPaneDock::show_timeline()`].
    pub fn show_timeline(
        &self,
        cx: &mut Cx,
        room_name_id: &RoomNameId,
        timeline_kind: TimelineKind,
        room_members: Option<Arc<Vec<RoomMember>>>,
        saved_panes: Vec<SavedRoomPane>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_timeline(cx, room_name_id, timeline_kind, room_members, saved_panes);
        }
    }

    /// Shows the given members of our timeline's room in any members pane.
    pub fn set_room_members(&self, cx: &mut Cx, room_members: Option<Arc<Vec<RoomMember>>>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(cx, room_members.map(Ok));
        }
    }

    /// Shows the given error in any members pane, as our timeline's members couldn't be fetched.
    pub fn set_room_members_error(&self, cx: &mut Cx, error: String) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(cx, Some(Err(error)));
        }
    }

    /// Returns whether the given kind of pane is docked here.
    pub fn has_pane(&self, kind: RoomPaneKind) -> bool {
        self.borrow().is_some_and(|inner| inner.panes.iter().any(|pane| pane.kind == kind))
    }

    /// See [`RoomPaneDock::save_state()`].
    pub fn save_state(&self) -> Vec<SavedRoomPane> {
        self.borrow().map(|inner| inner.save_state()).unwrap_or_default()
    }

    /// See [`RoomPaneDock::clear()`].
    pub fn clear(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear(cx);
        }
    }

    /// See [`RoomPaneDock::set_room_name()`].
    pub fn set_room_name(&self, cx: &mut Cx, room_name_id: &RoomNameId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_name(cx, room_name_id);
        }
    }

    /// See [`RoomPaneDock::toggle()`].
    pub fn toggle(&self, cx: &mut Cx, kind: RoomPaneKind) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.toggle(cx, kind);
        }
    }
}


/// The visible grab handle: a small pill centered on the pane's inner border.
const GRAB_LEN: f64 = 38.0;
const GRAB_THICK: f64 = 8.0;
/// A small floor, so that the grab handle stays usable.
const EDGE_MIN_SIZE: f64 = 60.0;
/// The thickness of the divider on a pane's inner side.
const DIVIDER_THICKNESS: f64 = 2.0;
/// The dark gray of a dock splitter (#4D4D4D), which the divider and grab handle share.
const DIVIDER_COLOR: Vec4 = Vec4 { x: 0.302, y: 0.302, z: 0.302, w: 1.0 };

/// Widget actions emitted by a [`RoomPaneEdge`].
#[derive(Clone, Debug, Default)]
pub enum RoomPaneEdgeAction {
    /// The user finished dragging the edge's grab handle.
    Resized { side: PaneSide, size: f64 },
    #[default]
    None,
}

/// One edge of a [`RoomPaneDock`], which draws its panes side by side
/// plus a grab handle on its inner border for resizing it.
#[derive(Script, Widget, Animator)]
pub struct RoomPaneEdge {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    /// How far this edge's panes have slid out of view: 0 when fully shown, 1 when hidden.
    #[live] slide: f32,
    /// How hovered (or dragged) the divider is, from 0 to 1.
    #[live] hover: f32,
    #[live] draw_handle: DrawColor,
    /// The (invisible) hit zone for the full strip; the pill is just the visual.
    #[live] draw_grab: DrawColor,
    /// The line between this edge's panes and the timeline.
    #[live] draw_divider: DrawColor,
    #[rust] side: PaneSide,
    #[rust] panes: Vec<(RoomPaneKind, WidgetRef)>,
    #[rust(300.0)] size: f64,
    /// The most that this edge may extend, if limited.
    #[rust] max_extent: Option<f64>,
    /// The drag start position and the size at that time.
    #[rust] drag: Option<(f64, f64)>,
    #[rust] handle_area: Area,
    /// Our rect as of our last draw, if we drew any panes, which our dock then draws our divider along.
    #[rust] area: Area,
    #[rust] has_panes_drawn: bool,
    /// Whether this edge's last pane is sliding out, after which it's removed.
    #[rust] is_sliding_out: bool,
    /// Whether our size changed such that our dock must redraw.
    #[rust] needs_dock_redraw: bool,
    /// Whether the animator must be cut to our current state, e.g., after a reapply.
    #[rust] needs_animator_resync: bool,
}

impl ScriptHook for RoomPaneEdge {
    fn on_after_apply(&mut self, _vm: &mut ScriptVm, apply: &Apply, _scope: &mut Scope, _value: ScriptValue) {
        if apply.is_reload() {
            self.slide = if !self.panes.is_empty() && !self.is_sliding_out { 0.0 } else { 1.0 };
            self.needs_animator_resync = true;
            self.apply_walk();
        }
    }
}

impl RoomPaneEdge {
    /// The dark gray of a dock splitter, turning purple as the divider is hovered or dragged.
    fn divider_color(&self) -> Vec4 {
        let t = self.hover.clamp(0.0, 1.0);
        Vec4 {
            x: DIVIDER_COLOR.x + (COLOR_ROBRIX_PURPLE.x - DIVIDER_COLOR.x) * t,
            y: DIVIDER_COLOR.y + (COLOR_ROBRIX_PURPLE.y - DIVIDER_COLOR.y) * t,
            z: DIVIDER_COLOR.z + (COLOR_ROBRIX_PURPLE.z - DIVIDER_COLOR.z) * t,
            w: 1.0,
        }
    }

    /// Handles resizing via the grab handle, which our dock calls before any other widget handles the event.
    ///
    /// Returns whether the grab handle was pressed, in which case no other widget should handle the event.
    fn handle_grab_handle_event(&mut self, cx: &mut Cx, event: &Event) -> bool {
        if self.panes.is_empty() || self.is_sliding_out {
            return false;
        }

        // The strip reaches a little into our panes, but must not take presses on their header buttons.
        let buttons: Vec<Rect> = self.panes.iter()
            .map(|(_, frame)| frame.view(cx, ids!(header_buttons)).area().rect(cx))
            .collect();
        // Fingers are blunter than a cursor, so the strip gets a wider margin on touch.
        let hit = event.hits_with_options_and_test(
            cx,
            self.handle_area,
            HitOptions::new()
                .with_margin(self.grab_inset(3.0))
                .with_touch_margin(self.grab_inset(8.0)),
            |abs, rect, margin| Inset::rect_contains_with_inset(abs, rect, margin)
                && !buttons.iter().any(|button| button.contains(abs)),
        );
        match hit {
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(self.resize_cursor());
                self.animator_play(cx, ids!(hover.on));
                // Snapping to the hover state doesn't animate, so it doesn't request a redraw.
                self.needs_dock_redraw = true;
            }
            Hit::FingerHoverOut(_) => {
                if self.drag.is_none() {
                    self.animator_play(cx, ids!(hover.off));
                }
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                cx.set_cursor(self.resize_cursor());
                let start = if self.side.is_vertical() { fe.abs.x } else { fe.abs.y };
                self.drag = Some((start, self.effective_size()));
                self.animator_play(cx, ids!(hover.on));
                self.needs_dock_redraw = true;
                return true;
            }
            Hit::FingerMove(fe) => {
                if let Some((start, start_size)) = self.drag {
                    cx.set_cursor(self.resize_cursor());
                    let now = if self.side.is_vertical() { fe.abs.x } else { fe.abs.y };
                    let delta = match self.side {
                        // Dragging the inner handle away from its edge grows it.
                        PaneSide::Left | PaneSide::Top => now - start,
                        PaneSide::Right | PaneSide::Bottom => start - now,
                    };
                    let max_size = self.max_extent.unwrap_or(f64::INFINITY);
                    self.size = (start_size + delta).min(max_size).max(EDGE_MIN_SIZE);
                    self.apply_walk();
                    // Only the parent's layout reads our walk; redrawing just ourselves would keep the old rect.
                    cx.redraw_all();
                }
            }
            Hit::FingerUp(fe) => {
                if self.drag.take().is_some() {
                    cx.widget_action(self.widget_uid(), RoomPaneEdgeAction::Resized { side: self.side, size: self.size });
                }
                if !(fe.is_over && fe.device.has_hovers()) {
                    self.animator_play(cx, ids!(hover.off));
                }
            }
            _ => {}
        }
        false
    }

    /// Draws the divider along our panes' inner side, and the grab handle and its hit strip straddling it,
    /// all from our final rect, so they always line up with each other and with our panes.
    fn draw_divider(&mut self, cx: &mut Cx2d) {
        if !self.has_panes_drawn {
            return;
        }
        // Our layout may have moved us after we drew our panes (e.g., once the timeline
        // before us got its final size), so use where we actually ended up.
        let rect = self.area.rect(cx);
        let t = DIVIDER_THICKNESS;
        let divider = match self.side {
            PaneSide::Left => Rect { pos: dvec2(rect.pos.x + rect.size.x - t, rect.pos.y), size: dvec2(t, rect.size.y) },
            PaneSide::Right => Rect { pos: rect.pos, size: dvec2(t, rect.size.y) },
            PaneSide::Top => Rect { pos: dvec2(rect.pos.x, rect.pos.y + rect.size.y - t), size: dvec2(rect.size.x, t) },
            PaneSide::Bottom => Rect { pos: rect.pos, size: dvec2(rect.size.x, t) },
        };
        // The hit strip runs along the whole divider, only as thick as the grab handle, so over the timeline
        // it doesn't reach its scroll bar; any slop is added on our panes' side (see `grab_inset()`).
        let center = divider.pos + divider.size * 0.5;
        let (grab_rect, hit_rect) = if self.side.is_vertical() {
            let len = GRAB_LEN.min(divider.size.y * 0.5);
            (
                Rect { pos: dvec2(center.x - GRAB_THICK * 0.5, center.y - len * 0.5), size: dvec2(GRAB_THICK, len) },
                Rect { pos: dvec2(center.x - GRAB_THICK * 0.5, divider.pos.y), size: dvec2(GRAB_THICK, divider.size.y) },
            )
        } else {
            let len = GRAB_LEN.min(divider.size.x * 0.5);
            (
                Rect { pos: dvec2(center.x - len * 0.5, center.y - GRAB_THICK * 0.5), size: dvec2(len, GRAB_THICK) },
                Rect { pos: dvec2(divider.pos.x, center.y - GRAB_THICK * 0.5), size: dvec2(divider.size.x, GRAB_THICK) },
            )
        };
        let color = self.divider_color();
        self.draw_divider.color = color;
        self.draw_handle.color = color;
        self.draw_divider.draw_abs(cx, divider);
        self.draw_grab.draw_abs(cx, hit_rect);
        self.draw_handle.draw_abs(cx, grab_rect);
        self.handle_area = self.draw_grab.area();
    }

    fn resize_cursor(&self) -> MouseCursor {
        if self.side.is_vertical() { MouseCursor::ColResize } else { MouseCursor::RowResize }
    }

    /// Slop beside the strip, only on our panes' side of it, as the timeline's scroll bar is on the other side.
    fn grab_inset(&self, slop: f64) -> Inset {
        let mut inset = Inset::default();
        match self.side {
            PaneSide::Left => inset.left = slop,
            PaneSide::Right => inset.right = slop,
            PaneSide::Top => inset.top = slop,
            PaneSide::Bottom => inset.bottom = slop,
        }
        inset
    }

    /// The size of this edge's panes as drawn, which may be less than the size the user chose.
    fn effective_size(&self) -> f64 {
        self.max_extent.map_or(self.size, |max| self.size.min(max.max(EDGE_MIN_SIZE)))
    }

    /// Writes this edge's size into its own walk, which the parent's flow reads upon its next layout.
    fn apply_walk(&mut self) {
        let shown = 1.0 - (self.slide as f64).clamp(0.0, 1.0);
        let extent = if self.panes.is_empty() { 0.0 } else { self.effective_size() * shown };
        let (width, height) = if self.side.is_vertical() {
            (Size::Fixed(extent), Size::fill())
        } else {
            (Size::fill(), Size::Fixed(extent))
        };
        self.view.walk.width = width;
        self.view.walk.height = height;
    }
}

impl Widget for RoomPaneEdge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        if std::mem::take(&mut self.needs_animator_resync) {
            if !self.panes.is_empty() && !self.is_sliding_out {
                self.animator_cut(cx, ids!(panel.show));
            } else {
                self.animator_cut(cx, ids!(panel.hide));
            }
        }
        // Our dock redraws itself while we're animating.
        if self.animator_handle_event(cx, event).must_redraw() {
            self.apply_walk();
            self.needs_dock_redraw = true;
        }
        if self.is_sliding_out && !self.animator.is_track_animating(id!(panel)) {
            self.is_sliding_out = false;
            self.panes.clear();
            // Keep the animator's state in sync, e.g., if a rebake cut the slide short.
            self.animator_cut(cx, ids!(panel.hide));
            self.apply_walk();
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.has_panes_drawn = false;
        if self.panes.is_empty() {
            return DrawStep::done();
        }
        // The dock hands us our exact rect via our walk.
        cx.begin_turtle(walk, Layout::flow_overlay());
        let rect = cx.turtle().rect();
        let size = self.effective_size();

        // While sliding, the panes stay attached to our inner border (facing the timeline).
        let pane_rect = match self.side {
            PaneSide::Left => Rect { pos: dvec2(rect.pos.x + rect.size.x - size, rect.pos.y), size: dvec2(size, rect.size.y) },
            PaneSide::Right => Rect { pos: rect.pos, size: dvec2(size, rect.size.y) },
            PaneSide::Top => Rect { pos: dvec2(rect.pos.x, rect.pos.y + rect.size.y - size), size: dvec2(rect.size.x, size) },
            PaneSide::Bottom => Rect { pos: rect.pos, size: dvec2(rect.size.x, size) },
        };
        // While sliding, the panes' outer part is clipped at the edge.
        cx.push_clip_rect(rect);

        // Panes split the edge evenly along its long axis.
        let n = self.panes.len() as f64;
        for (i, (_, pane)) in self.panes.iter().enumerate() {
            let i = i as f64;
            let sub = if self.side.is_vertical() {
                let h = pane_rect.size.y / n;
                Rect { pos: dvec2(pane_rect.pos.x, pane_rect.pos.y + i * h), size: dvec2(pane_rect.size.x, h) }
            } else {
                let w = pane_rect.size.x / n;
                Rect { pos: dvec2(pane_rect.pos.x + i * w, pane_rect.pos.y), size: dvec2(w, pane_rect.size.y) }
            };
            let pane_walk = Walk {
                abs_pos: Some(sub.pos),
                width: Size::Fixed(sub.size.x),
                height: Size::Fixed(sub.size.y),
                ..Walk::default()
            };
            pane.draw_walk_all(cx, &mut Scope::empty(), pane_walk);
        }

        cx.pop_clip_rect();
        cx.end_turtle_with_area(&mut self.area);
        // The divider's grab handle sticks out over the timeline, so our dock draws it after the timeline.
        self.has_panes_drawn = true;
        DrawStep::done()
    }
}

impl RoomPaneEdgeRef {
    /// See [`RoomPaneEdge::handle_grab_handle_event()`].
    fn handle_grab_handle_event(&self, cx: &mut Cx, event: &Event) -> bool {
        self.borrow_mut().is_some_and(|mut inner| inner.handle_grab_handle_event(cx, event))
    }

    /// The size of this edge's panes across its thin axis.
    fn pane_size(&self) -> f64 {
        self.borrow().map_or(0.0, |inner| inner.effective_size())
    }

    /// See [`RoomPaneEdge::draw_divider()`].
    fn draw_divider(&self, cx: &mut Cx2d) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.draw_divider(cx);
        }
    }

    /// How much of the dock's cross axis this edge wants, ignoring any limit.
    fn wanted_extent(&self) -> f64 {
        self.borrow().map_or(0.0, |inner| if inner.panes.is_empty() { 0.0 } else { inner.size })
    }

    fn set_max_extent(&self, max_extent: Option<f64>) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.max_extent != max_extent {
            inner.max_extent = max_extent;
            inner.apply_walk();
        }
    }

    /// Returns whether this edge's size changed such that its dock must redraw, and resets it.
    fn take_redraw_request(&self) -> bool {
        self.borrow_mut().is_some_and(|mut inner| std::mem::take(&mut inner.needs_dock_redraw))
    }

    /// Adds the given pane, sliding it in if this edge had no other panes.
    fn add_pane(&self, cx: &mut Cx, kind: RoomPaneKind, pane: WidgetRef, animate: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let was_empty = inner.panes.is_empty();
        // A sliding-out pane is replaced, and the slide reverses from where it is.
        let was_sliding_out = std::mem::take(&mut inner.is_sliding_out);
        if was_sliding_out {
            inner.panes.clear();
        }
        inner.panes.retain(|(k, _)| *k != kind);
        inner.panes.push((kind, pane));
        if was_empty || was_sliding_out {
            if animate {
                if was_empty {
                    inner.animator_cut(cx, ids!(panel.hide));
                }
                inner.animator_play(cx, ids!(panel.show));
            } else {
                inner.animator_cut(cx, ids!(panel.show));
            }
        }
        inner.apply_walk();
    }

    /// Removes all panes right away, including one that's sliding out.
    fn clear(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.panes.clear();
        inner.is_sliding_out = false;
        inner.drag = None;
        inner.animator_cut(cx, ids!(panel.hide));
        inner.apply_walk();
    }

    /// Removes the given pane, sliding it out first if it's this edge's last pane.
    fn remove_pane(&self, cx: &mut Cx, kind: RoomPaneKind, animate: bool) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if !inner.panes.iter().any(|(k, _)| *k == kind) {
            return;
        }
        if animate && inner.panes.len() == 1 {
            if !inner.is_sliding_out {
                inner.is_sliding_out = true;
                inner.drag = None;
                inner.animator_play(cx, ids!(panel.hide));
            }
        } else {
            inner.panes.retain(|(k, _)| *k != kind);
            if inner.panes.is_empty() {
                inner.is_sliding_out = false;
                inner.animator_cut(cx, ids!(panel.hide));
            }
        }
        inner.apply_walk();
    }

    fn set_side(&self, side: PaneSide) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.side = side;
        inner.apply_walk();
    }

    fn set_size(&self, size: f64) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.size = size.max(EDGE_MIN_SIZE);
        inner.apply_walk();
    }
}
