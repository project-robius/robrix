//! Shared widgets and DSL styling definitions for context menus.

use makepad_widgets::*;

use crate::home::{nav_history_menu::NavHistoryMenuWidgetRefExt, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::RoomContextMenuWidgetRefExt};

pub const BUTTON_HEIGHT: f64 = 38.0;
pub const MENU_WIDTH: f64 = 235.0;
pub const MENU_PADDING: f64 = 6.0;
const MENU_SPACING: f64 = 2.0;
const DIVIDER_MARGIN: f64 = 3.0;
const DIVIDER_HEIGHT: f64 = 2.0 + 2.0 * DIVIDER_MARGIN; // a `LineH` is 2pt tall

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ContextMenuButton = RobrixIconButton {
        height: #(BUTTON_HEIGHT)
        width: Fill
        margin: 0
        padding: Inset{left: 10, right: 12, top: 9, bottom: 9}
        spacing: 10
        align: Align{x: 0, y: 0.5}
        icon_walk: Walk{width: 18, height: 18}

        draw_bg +: {
            color: (COLOR_PRIMARY)
            color_hover: (COLOR_LIST_ITEM_BG_HOVER)
            color_down: #DCDCDC
            border_radius: 4.0
        }
        draw_icon.color: #000
        draw_text +: {
            color: #000, color_hover: #000, color_down: #000
            text_style: REGULAR_TEXT {font_size: 11}
        }
    }

    mod.widgets.ContextMenuDangerButton = mod.widgets.ContextMenuButton {
        draw_bg +: {
            color: (COLOR_BG_DANGER_RED)
            color_hover: #F0D4D4
            color_down: #E0B8B8
        }
        draw_icon.color: (COLOR_FG_DANGER_RED)
        draw_text +: {
            color: (COLOR_FG_DANGER_RED)
            color_hover: (COLOR_FG_DANGER_RED)
            color_down: (COLOR_FG_DANGER_RED)
        }
    }

    mod.widgets.ContextMenuDivider = LineH {
        width: Fill
        margin: Inset{top: #(DIVIDER_MARGIN), bottom: #(DIVIDER_MARGIN)}
    }

    // The dimmed backdrop that a context menu's content is shown in front of.
    mod.widgets.ContextMenuBackdrop = mod.widgets.SolidView {
        visible: false,
        flow: Overlay,
        width: Fill,
        height: Fill,
        cursor: MouseCursor.Default,
        // Align to top-left such that our coordinate adjustment
        // when showing this menu pane will work correctly.
        align: Align{x: 0, y: 0}

        // Show a slightly darkened translucent background to make the menu stand out.
        show_bg: true
        draw_bg +: {
            color: #0000004d
        }
    }

    // Deliberately has no children, since those don't inherit; each menu declares its own.
    mod.widgets.ContextMenuContent = RoundedView {
        flow: Down
        width: #(MENU_WIDTH)
        height: Fit
        padding: #(MENU_PADDING)
        spacing: #(MENU_SPACING)
        align: Align{x: 0, y: 0}

        show_bg: true
        draw_bg +: {
            color: (COLOR_PRIMARY)
            border_radius: 5.0
            border_size: 0.5
            border_color: #888
        }
    }
}

/// An action broadcast when a context menu was closed.
#[derive(Clone, Debug)]
pub struct ContextMenuClosed;

pub fn expected_menu_size(num_buttons: usize, num_dividers: usize) -> DVec2 {
    let height = num_buttons as f64 * BUTTON_HEIGHT
        + num_dividers as f64 * DIVIDER_HEIGHT
        + (num_buttons + num_dividers).saturating_sub(1) as f64 * MENU_SPACING
        + 2.0 * MENU_PADDING;
    dvec2(MENU_WIDTH, height)
}

/// Returns whether the given event is an Escape or Back press that a context menu's `cancel_scope` owns.
pub fn is_context_menu_cancelled(cx: &mut Cx, event: &Event, cancel_scope: Option<&CancelScope>) -> bool {
    cancel_scope.is_some_and(|scope| cx.owns_cancel(scope))
        && (event.back_pressed()
            || matches!(event, Event::KeyUp(key) if key.key_code == KeyCode::Escape)
            || matches!(event, Event::MouseUp(e) if e.button.is_back())
        )
}

/// Returns whether the given hit on a context menu's backdrop is a tap that starts and ends outside of its content,
/// so a drag that starts on its content (e.g., to scroll it) doesn't close it.
pub fn is_tap_outside(cx: &mut Cx, hit: &Hit, content_area: Area) -> bool {
    let Hit::FingerUp(fue) = hit else { return false };
    let content_rect = content_area.rect(cx);
    fue.is_over && !content_rect.contains(fue.abs_start) && !content_rect.contains(fue.abs)
}

/// Returns whether any of the app's context menus is shown.
pub fn is_context_menu_shown(cx: &mut Cx, ui: &WidgetRef) -> bool {
    ui.new_message_context_menu(cx, ids!(new_message_context_menu)).is_currently_shown(cx)
        || ui.room_context_menu(cx, ids!(room_context_menu)).is_currently_shown(cx)
        || ui.nav_history_menu(cx, ids!(nav_history_menu)).is_currently_shown()
}

/// Closes any of the app's context menus that are shown.
pub fn close_context_menus(cx: &mut Cx, ui: &WidgetRef) {
    ui.new_message_context_menu(cx, ids!(new_message_context_menu)).close(cx);
    ui.room_context_menu(cx, ids!(room_context_menu)).close(cx);
    ui.nav_history_menu(cx, ids!(nav_history_menu)).close(cx);
}

/// Returns a margin that positions the context menu at `anchor_pos`,
/// but ensuring that it stays within the given `container_rect`.
pub fn menu_position_margin(container_rect: Rect, anchor_pos: DVec2, menu_size: DVec2) -> Inset {
    Inset {
        left: (anchor_pos.x - container_rect.pos.x)
            .min(container_rect.size.x - menu_size.x)
            .max(0.0),
        top: (anchor_pos.y - container_rect.pos.y)
            .min(container_rect.size.y - menu_size.y)
            .max(0.0),
        right: 0.0,
        bottom: 0.0,
    }
}
