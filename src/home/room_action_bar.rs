//! `RoomActionBar`: the full two-row header for a room screen.
//!
//! The widget owns every element that appears in the header:
//! * an optional back button (shown on mobile, hidden on desktop);
//! * the room name label (ellipsis-truncated only as a last resort);
//! * the four fixed action buttons (search, threads, members, info); and
//! * an expand / collapse chevron trigger that appears when there isn't
//!   enough horizontal room to show all four action buttons inline.
//!
//! ## Visual states
//! There are exactly three states, driven by the bar's allocated width and
//! the current room name:
//!
//! 1. **All inline.** Back + title + four action buttons all fit on a
//!    single row. No chevron is shown; the second row is collapsed.
//! 2. **Expand-only.** There isn't room for every action button alongside
//!    the full room name, but there IS room for the name plus a single
//!    chevron. The four action buttons move to row 2, which appears
//!    beneath row 1 when the chevron is tapped. The room name is NOT
//!    abbreviated.
//! 3. **Ellipsized.** There isn't room even for the name plus the chevron.
//!    The name is ellipsized; the chevron still sits flush with the right
//!    edge and its tap still reveals row 2.
//!
//! ## Why the icons are declared statically
//! An earlier revision set button icons at runtime through
//! `script_apply_eval!` on `draw_icon.svg`. That approach was unreliable
//! when rapidly re-applied across instances, so every icon on this bar is
//! now pre-declared in the DSL and layout only toggles widget visibility.
//! The expand / collapse affordance is likewise split into two separate
//! buttons (`expand_button` with chevron-down, `collapse_button` with
//! chevron-up) so swapping them is also a visibility toggle.

use makepad_widgets::*;

use crate::shared::popup_list::{PopupKind, enqueue_popup_notification};

/// Pixel size of each icon button (square).
const BUTTON_SIZE: f64 = 36.0;
/// Horizontal spacing between adjacent icons.
const BUTTON_SPACING: f64 = 4.0;
/// Pitch = button + one spacing gap.
const BUTTON_PITCH: f64 = BUTTON_SIZE + BUTTON_SPACING;
/// Right-edge padding reserved outside the layout math.
const BAR_EDGE_PADDING: f64 = 8.0;
/// Approximate width of the back button region when shown.
const BACK_BUTTON_RESERVE: f64 = 58.0;
/// Rough per-character width estimate for the title font. Over-estimate
/// slightly so we pick the more-conservative layout state when the true
/// width is ambiguous (fewer inline buttons is the "safer" decision).
const TITLE_CHAR_WIDTH_PX: f64 = 8.2;

/// Height of a single row of icons. The bar's collapsed height.
pub const ROOM_ACTION_BAR_ROW_HEIGHT: f64 = 45.0;

/// Height of the bar when expanded (exactly two rows).
pub const ROOM_ACTION_BAR_EXPANDED_HEIGHT: f64 = 2.0 * ROOM_ACTION_BAR_ROW_HEIGHT;

/// Minimum width the bar should ever occupy: one button (the chevron
/// trigger) plus right-edge padding. Used as the DSL-level `Fill{min:..}`
/// so the trigger can never be pushed off-screen.
pub const ROOM_ACTION_BAR_MIN_WIDTH: f64 = BUTTON_SIZE + BAR_EDGE_PADDING;

/// The four configured actions. Kept as plain constants so external click
/// handlers can match on them without a dedicated enum.
pub const ACTION_ID_SEARCH:  LiveId = live_id!(search);
pub const ACTION_ID_THREADS: LiveId = live_id!(threads);
pub const ACTION_ID_MEMBERS: LiveId = live_id!(members);
pub const ACTION_ID_INFO:    LiveId = live_id!(info);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT = 45
    mod.widgets.ROOM_ACTION_BAR_EXPANDED_HEIGHT = 90
    mod.widgets.ROOM_ACTION_BAR_BUTTON_SIZE = 36
    mod.widgets.ROOM_ACTION_BAR_ANIMATION_DURATION_SECS = 0.2

    // Each icon button is `RobrixIconButton` tuned to be icon-only with a
    // transparent background and a discreet hover tint. The SVG is set on
    // the slot directly in the DSL below — never swapped at runtime.
    mod.widgets.RoomActionBarInlineButton = RobrixIconButton {
        width: (mod.widgets.ROOM_ACTION_BAR_BUTTON_SIZE)
        height: (mod.widgets.ROOM_ACTION_BAR_BUTTON_SIZE)
        visible: false
        padding: Inset{left: 8, right: 8, top: 8, bottom: 8}
        margin: 0
        spacing: 0
        align: Align{x: 0.5, y: 0.5}
        icon_walk: Walk{width: 20, height: 20}
        draw_bg +: {
            color: (COLOR_TRANSPARENT)
            color_hover: #00000014
            color_down: #00000024
            border_size: 0.0
            border_radius: 4.0
        }
        draw_icon +: { color: (COLOR_TEXT) }
        text: ""
    }

    mod.widgets.RoomActionBar = set_type_default() do #(RoomActionBar::register_widget(vm)) {
        ..mod.widgets.View

        width: Fill{min: (mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT)}
        height: (mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT)
        flow: Down
        clip_x: true
        clip_y: true

        row_1 := View {
            width: Fill
            height: (mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT)
            flow: Right
            align: Align{y: 0.5}
            padding: Inset{left: 0, right: 8, top: 0, bottom: 0}
            spacing: 0

            // Back button — visible by default (mobile case). Desktop
            // calls `set_back_button_visible(false)` to hide the wrapping
            // container. Named `left_button` so the path matches the
            // Makepad base, but the click-to-Pop wiring is handled by
            // `RoomActionBar::handle_actions` below (emitting
            // `StackNavigationAction::Pop` directly) — not by the base's
            // `self.button(cx, ids!(left_button))` scan, which doesn't
            // reliably descend into this custom widget's subtree.
            back_button_container := View {
                width: Fit, height: Fit
                align: Align{y: 0.5}
                left_button := ButtonFlatterIcon {
                    width: Fit, height: Fit
                    padding: Inset{left: 20, right: 23, top: 10, bottom: 10}
                    margin: Inset{left: 8, right: 0, top: 0, bottom: 0}
                    spacing: 0
                    text: ""
                    icon_walk: Walk{width: 13, height: 13}
                    draw_icon +: {
                        color: (ROOM_NAME_TEXT_COLOR)
                        svg: crate_resource("self://resources/icons/back.svg")
                    }
                }
            }

            // Title fills whatever space remains between the back button
            // (if shown) and the right-side button cluster. It ellipsizes
            // only when the available width forces it to — state #3.
            title_container := View {
                width: Fill, height: Fit
                align: Align{x: 0.0, y: 0.5}
                padding: Inset{left: 12, right: 8}
                title := Label {
                    width: Fill, height: Fit
                    max_lines: 1
                    text_overflow: Ellipsis
                    margin: 0
                    draw_text +: {
                        color: (ROOM_NAME_TEXT_COLOR)
                        text_style: mod.widgets.TITLE_TEXT {}
                    }
                    text: ""
                }
            }

            // The four action-button slots, one per action, each with its
            // static SVG. Visibility is toggled by `layout_for_width`
            // based on whether we're in the "all inline" or "expand-only"
            // state. Never created or destroyed at runtime.
            row_1_search := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_SEARCH) }
            }
            row_1_threads := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_DOUBLE_CHAT) }
            }
            row_1_members := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_ADD_USER) }
            }
            row_1_info := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_INFO) }
            }

            // Two separate chevron buttons — the "swap" is a visibility
            // toggle, not an SVG re-apply. Exactly one is visible at a
            // time while the bar is in the expand-only state; both hidden
            // in the all-inline state.
            expand_button := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_CHEVRON_DOWN) }
            }
            collapse_button := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_CHEVRON_UP) }
            }
        }

        // Row 2 spans the full bar width. Its buttons right-align against
        // the right edge so they line up with the chevron on row 1. These
        // are shown together (all four) only while the bar is expanded.
        row_2 := View {
            width: Fill
            height: (mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT)
            flow: Right
            align: Align{x: 1.0, y: 0.5}
            padding: Inset{left: 0, right: 8, top: 0, bottom: 0}
            spacing: 0

            row_2_search := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_SEARCH) }
            }
            row_2_threads := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_DOUBLE_CHAT) }
            }
            row_2_members := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_ADD_USER) }
            }
            row_2_info := mod.widgets.RoomActionBarInlineButton {
                draw_icon +: { svg: (mod.widgets.ICON_INFO) }
            }
        }

        animator: Animator {
            expansion: {
                default: @collapsed
                collapsed: AnimatorState {
                    redraw: true
                    from: { all: Forward { duration: (mod.widgets.ROOM_ACTION_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: (mod.widgets.ROOM_ACTION_BAR_ROW_HEIGHT) }
                }
                expanded: AnimatorState {
                    redraw: true
                    from: { all: Forward { duration: (mod.widgets.ROOM_ACTION_BAR_ANIMATION_DURATION_SECS) } }
                    apply: { height: (mod.widgets.ROOM_ACTION_BAR_EXPANDED_HEIGHT) }
                }
            }
        }
    }
}

/// The four row-1 action-button slot ids, paired with their action id.
const ROW_1_ACTIONS: [(LiveId, LiveId); 4] = [
    (live_id!(row_1_search),  ACTION_ID_SEARCH),
    (live_id!(row_1_threads), ACTION_ID_THREADS),
    (live_id!(row_1_members), ACTION_ID_MEMBERS),
    (live_id!(row_1_info),    ACTION_ID_INFO),
];

/// Row-2 analogs, in matching order.
const ROW_2_ACTIONS: [(LiveId, LiveId); 4] = [
    (live_id!(row_2_search),  ACTION_ID_SEARCH),
    (live_id!(row_2_threads), ACTION_ID_THREADS),
    (live_id!(row_2_members), ACTION_ID_MEMBERS),
    (live_id!(row_2_info),    ACTION_ID_INFO),
];

/// Actions emitted by a [`RoomActionBar`].
#[derive(Clone, Debug, Default)]
pub enum RoomActionBarAction {
    /// One of the configured action buttons was tapped (either row).
    ButtonClicked { id: LiveId },
    /// The expand/collapse state flipped. Coordinators (e.g. mobile
    /// `HomeScreen`) use this to keep dependent layout (e.g. the stack
    /// nav body's top margin) in sync with the bar's new height.
    ExpansionToggled { is_expanded: bool },
    #[default]
    None,
}

/// Which of the three layout states the bar is currently in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutState {
    /// Back + full title + all four action buttons fit on row 1.
    /// Row 2 / chevron are hidden.
    AllInline,
    /// Back + full title + one chevron fit on row 1, but all four buttons
    /// don't. Row 2 reveals them when expanded. Title is NOT abbreviated.
    ExpandOnly,
    /// Even back + chevron + the full title don't fit on row 1; the title
    /// is ellipsized. Row 2 still reveals the buttons when expanded.
    Ellipsized,
}

#[derive(Script, ScriptHook, Widget, Animator)]
pub struct RoomActionBar {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,

    /// Current layout state (drives slot visibility).
    #[rust(LayoutState::AllInline)] state: LayoutState,
    /// Whether the bar is animated into its expanded (two-row) state.
    /// Meaningless in `LayoutState::AllInline`.
    #[rust] is_expanded: bool,
    /// Whether the back-button region is rendered in row 1 (mobile).
    #[rust(true)] back_button_visible: bool,
    /// Whether the action buttons (search / threads / members / info) and
    /// their expand/collapse chevron should be rendered at all. Set to
    /// `false` for non-room views (invites, space lobbies) where these
    /// actions don't apply; the bar then shows just back + title.
    #[rust(true)] show_action_buttons: bool,
    /// The current room-name text, cached for the layout heuristic.
    #[rust] room_name: String,
    /// Width we last computed a layout for. -1.0 forces a recompute.
    #[rust(-1.0)] last_laid_out_width: f64,
}

impl Widget for RoomActionBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.peek_walk_turtle(walk);
        let allocated_width = rect.size.x.max(0.0);

        if (allocated_width - self.last_laid_out_width).abs() > 0.5 {
            self.last_laid_out_width = allocated_width;
            self.recompute_layout(cx, allocated_width);
        }

        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }
}

impl WidgetMatchEvent for RoomActionBar {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let self_uid = self.widget_uid();

        // Back button → pop the enclosing `StackNavigationView`.
        //
        // Makepad's `StackNavigationView::handle_stack_view_closure_request`
        // tries to detect the click itself via
        // `self.button(cx, ids!(left_button)).clicked(&actions)`, but that
        // path-scan does not reliably descend into custom widgets like this
        // one. So we emit the `Pop` action directly — `StackNavigation`
        // (the parent) handles any `Pop` it observes in its action stream.
        if self.back_button_visible
            && self.view.button(cx, ids!(left_button)).clicked(actions)
        {
            cx.widget_action(self_uid, StackNavigationAction::Pop);
            return;
        }

        // Row-1 action buttons. Only live in `AllInline`.
        if self.state == LayoutState::AllInline {
            for (slot_id, action_id) in ROW_1_ACTIONS {
                if self.view.button(cx, &[slot_id]).clicked(actions) {
                    cx.widget_action(
                        self_uid,
                        RoomActionBarAction::ButtonClicked { id: action_id },
                    );
                    return;
                }
            }
        }

        // Expand / collapse chevron. Only live in the other states.
        if self.state != LayoutState::AllInline {
            if !self.is_expanded
                && self.view.button(cx, ids!(expand_button)).clicked(actions)
            {
                self.set_expanded(cx, true);
                return;
            }
            if self.is_expanded
                && self.view.button(cx, ids!(collapse_button)).clicked(actions)
            {
                self.set_expanded(cx, false);
                return;
            }
        }

        // Row-2 action buttons. Only live while expanded.
        if self.state != LayoutState::AllInline && self.is_expanded {
            for (slot_id, action_id) in ROW_2_ACTIONS {
                if self.view.button(cx, &[slot_id]).clicked(actions) {
                    cx.widget_action(
                        self_uid,
                        RoomActionBarAction::ButtonClicked { id: action_id },
                    );
                    // Fall through: keep the bar expanded; the user might
                    // want to tap another action in the same reveal. They
                    // can collapse via the chevron when done.
                    return;
                }
            }
        }
    }
}

impl RoomActionBar {
    /// Sets the room-name label text. Cheap; triggers a relayout on the
    /// next draw.
    pub fn set_room_name(&mut self, cx: &mut Cx, name: &str) {
        if self.room_name != name {
            self.room_name = name.to_string();
            self.view
                .label(cx, ids!(title_container.title))
                .set_text(cx, name);
            self.last_laid_out_width = -1.0;
            self.redraw(cx);
        }
    }

    /// Toggles whether the back-button region on the left of row 1 is
    /// rendered. Desktop sets this to `false`; mobile to `true`.
    pub fn set_back_button_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.back_button_visible != visible {
            self.back_button_visible = visible;
            self.view
                .view(cx, ids!(back_button_container))
                .set_visible(cx, visible);
            self.last_laid_out_width = -1.0;
            self.redraw(cx);
        }
    }

    /// Toggles whether the four action buttons (and the expand/collapse
    /// chevron) are rendered.
    ///
    /// Set to `false` for non-room stack views (invites, space lobbies)
    /// where "search messages", "threads", "members", and "room info"
    /// are not applicable — the bar then shows only back + title, and
    /// the chevron is suppressed (so no row-2 reveal is possible).
    pub fn set_action_buttons_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.show_action_buttons == visible {
            return;
        }
        self.show_action_buttons = visible;
        // If we're turning buttons off while expanded, snap back to the
        // one-row state — there's nothing meaningful to reveal any more.
        if !visible && self.is_expanded {
            self.set_expanded(cx, false);
        }
        self.last_laid_out_width = -1.0;
        self.redraw(cx);
    }

    /// Flips the expand state to `expanded`, plays the height animation,
    /// updates chevron / row-2 visibility, and emits
    /// [`RoomActionBarAction::ExpansionToggled`].
    fn set_expanded(&mut self, cx: &mut Cx, expanded: bool) {
        if self.is_expanded == expanded { return; }
        self.is_expanded = expanded;
        self.animator_play(
            cx,
            if expanded { ids!(expansion.expanded) } else { ids!(expansion.collapsed) },
        );
        self.apply_chevron_visibility(cx);
        self.apply_row_2_visibility(cx);
        cx.widget_action(
            self.widget_uid(),
            RoomActionBarAction::ExpansionToggled { is_expanded: expanded },
        );
    }

    /// Decides the current [`LayoutState`] from `available_width` and the
    /// current room-name length, then applies the slot visibility to
    /// match.
    fn recompute_layout(&mut self, cx: &mut Cx, available_width: f64) {
        let back_reserve = if self.back_button_visible { BACK_BUTTON_RESERVE } else { 0.0 };
        // Title intrinsic width estimate (plus its internal padding: 12 + 8).
        let title_padding = 12.0 + 8.0;
        let title_natural = estimate_title_width(&self.room_name) + title_padding;

        let all_buttons_width = 4.0 * BUTTON_PITCH - BUTTON_SPACING;
        let single_button_width = BUTTON_SIZE;

        let needed_for_all_inline = back_reserve + title_natural + all_buttons_width + BAR_EDGE_PADDING;
        let needed_for_expand_only = back_reserve + title_natural + single_button_width + BAR_EDGE_PADDING;

        let new_state = if available_width >= needed_for_all_inline {
            LayoutState::AllInline
        } else if available_width >= needed_for_expand_only {
            LayoutState::ExpandOnly
        } else {
            LayoutState::Ellipsized
        };

        let state_changed = new_state != self.state;
        self.state = new_state;

        // Transitioning out of an expanded-capable state (to AllInline)
        // collapses the bar; the expand-capable states preserve their
        // current is_expanded.
        if self.state == LayoutState::AllInline && self.is_expanded {
            self.set_expanded(cx, false);
        }

        // Apply visibilities even when state didn't change (e.g. room
        // name changed without crossing a state boundary — buttons stay,
        // but the chevron up/down choice may still need refreshing).
        self.apply_row_1_visibility(cx);
        self.apply_chevron_visibility(cx);
        self.apply_row_2_visibility(cx);
        let _ = state_changed;
    }

    fn apply_row_1_visibility(&mut self, cx: &mut Cx) {
        let show_inline = self.show_action_buttons && self.state == LayoutState::AllInline;
        for (slot_id, _) in ROW_1_ACTIONS {
            self.view.button(cx, &[slot_id]).set_visible(cx, show_inline);
        }
    }

    fn apply_chevron_visibility(&mut self, cx: &mut Cx) {
        let needs_chevron = self.show_action_buttons && self.state != LayoutState::AllInline;
        let expand_visible   = needs_chevron && !self.is_expanded;
        let collapse_visible = needs_chevron &&  self.is_expanded;
        self.view.button(cx, ids!(expand_button)).set_visible(cx, expand_visible);
        self.view.button(cx, ids!(collapse_button)).set_visible(cx, collapse_visible);
    }

    fn apply_row_2_visibility(&mut self, cx: &mut Cx) {
        let show = self.show_action_buttons
            && self.state != LayoutState::AllInline
            && self.is_expanded;
        for (slot_id, _) in ROW_2_ACTIONS {
            self.view.button(cx, &[slot_id]).set_visible(cx, show);
        }
    }
}

impl RoomActionBarRef {
    /// See [`RoomActionBar::set_room_name`].
    pub fn set_room_name(&self, cx: &mut Cx, name: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_name(cx, name);
        }
    }

    /// See [`RoomActionBar::set_back_button_visible`].
    pub fn set_back_button_visible(&self, cx: &mut Cx, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_back_button_visible(cx, visible);
        }
    }

    /// See [`RoomActionBar::set_action_buttons_visible`].
    pub fn set_action_buttons_visible(&self, cx: &mut Cx, visible: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_action_buttons_visible(cx, visible);
        }
    }
}

/// Surfaces a placeholder popup for the default room action IDs.
///
/// Intended as a shared stub while the underlying features (search, threads
/// list, members list, room info) are still unimplemented.
pub fn handle_default_action_stub(id: LiveId) {
    let message = match id {
        ACTION_ID_SEARCH  => "Room search is not yet implemented.",
        ACTION_ID_THREADS => "Threads list is not yet implemented.",
        ACTION_ID_MEMBERS => "Members list is not yet implemented.",
        ACTION_ID_INFO    => "Room info is not yet implemented.",
        _ => return,
    };
    enqueue_popup_notification(message, PopupKind::Info, Some(3.0));
}

/// Rough pixel width of `text` rendered in the title font. Used by
/// [`RoomActionBar::recompute_layout`] to decide how much room the action
/// buttons have. A small over-estimate is preferable to an under-estimate
/// (fewer inline buttons is the safer fallback).
fn estimate_title_width(text: &str) -> f64 {
    (text.chars().count() as f64) * TITLE_CHAR_WIDTH_PX
}
