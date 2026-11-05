//! Shared widgets and DSL styling definitions for context menus.

use makepad_widgets::*;

use crate::{
    home::{nav_history_menu::NavHistoryMenuWidgetRefExt, new_message_context_menu::NewMessageContextMenuWidgetRefExt, room_context_menu::RoomContextMenuWidgetRefExt},
    shared::emoji_picker_popup::EmojiPickerPopupWidgetRefExt,
};

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
            color: (COLOR_BG_FLOATING)
            color_hover: (COLOR_FLOATING_ITEM_BG_HOVER)
            color_down: (COLOR_CONTEXT_MENU_ITEM_BG_DOWN)
            border_radius: 4.0
        }
        draw_icon.color: (COLOR_TEXT_BODY)
        draw_text +: {
            color: (COLOR_TEXT_BODY), color_hover: (COLOR_TEXT_BODY), color_down: (COLOR_TEXT_BODY)
            text_style: REGULAR_TEXT {font_size: 11}
        }
    }

    mod.widgets.ContextMenuDangerButton = mod.widgets.ContextMenuButton {
        draw_bg +: {
            color: (COLOR_BG_DANGER_RED)
            color_hover: (COLOR_BG_DANGER_RED_HOVER)
            color_down: (COLOR_BG_DANGER_RED_DOWN)
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
            color: (COLOR_CONTEXT_MENU_OVERLAY_BG)
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
            color: (COLOR_BG_FLOATING)
            border_radius: 5.0
            border_size: 0.5
            border_color: (COLOR_CONTEXT_MENU_BORDER)
        }
    }

    // The panel of a popup that floats beside what opened it, like the mention popup or the emoji picker.
    // Like `ContextMenuContent`, each popup declares its own children.
    mod.widgets.FloatingPopupContent = RoundedShadowView {
        width: Fill, height: Fill
        flow: Down
        show_bg: true
        cursor: MouseCursor.Default
        grab_key_focus: false
        draw_bg +: {
            color: (COLOR_BG_FLOATING)
            border_radius: 5.0
            border_size: 1.0
            border_color: (COLOR_MENTION_POPUP_BORDER)
            shadow_color: (COLOR_SHADOW_STRONG)
            shadow_radius: 12.0
            shadow_offset: vec2(0.0, 3.0)
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

/// Returns whether any of the app's context menus (or its emoji picker popup) is shown.
pub fn is_context_menu_shown(cx: &mut Cx, ui: &WidgetRef) -> bool {
    ui.new_message_context_menu(cx, ids!(new_message_context_menu)).is_currently_shown(cx)
        || ui.room_context_menu(cx, ids!(room_context_menu)).is_currently_shown(cx)
        || ui.nav_history_menu(cx, ids!(nav_history_menu)).is_currently_shown()
        || ui.emoji_picker_popup(cx, ids!(emoji_picker_popup)).is_currently_shown()
}

/// Closes any of the app's context menus (or its emoji picker popup) that are shown.
pub fn close_context_menus(cx: &mut Cx, ui: &WidgetRef) {
    ui.new_message_context_menu(cx, ids!(new_message_context_menu)).close(cx);
    ui.room_context_menu(cx, ids!(room_context_menu)).close(cx);
    ui.nav_history_menu(cx, ids!(nav_history_menu)).close(cx);
    ui.emoji_picker_popup(cx, ids!(emoji_picker_popup)).close(cx);
}

/// Returns a margin that positions the context menu at `anchor_pos`,
/// but ensuring that it stays within the given `container_rect` and below the status bar.
pub fn menu_position_margin(cx: &Cx, container_rect: Rect, anchor_pos: DVec2, menu_size: DVec2) -> Inset {
    Inset {
        left: (anchor_pos.x - container_rect.pos.x)
            .min(container_rect.size.x - menu_size.x)
            .max(0.0),
        top: (anchor_pos.y - container_rect.pos.y)
            .min(container_rect.size.y - menu_size.y)
            .max(cx.display_context.safe_area_insets.top),
        right: 0.0,
        bottom: 0.0,
    }
}

/// Returns how much of the window the soft keyboard covers, if the given event changed that.
pub fn get_keyboard_height(event: &Event) -> Option<f64> {
    let Event::VirtualKeyboard(keyboard_event) = event else { return None };
    Some(match keyboard_event {
        VirtualKeyboardEvent::WillShow { height, .. } | VirtualKeyboardEvent::DidShow { height, .. } => *height,
        VirtualKeyboardEvent::WillHide { .. } | VirtualKeyboardEvent::DidHide { .. } => 0.0,
    })
}

/// Returns the part of the window a popup can draw in, `edge_gap` away from the safe area and the soft keyboard.
///
/// The soft keyboard can pan `overlay_origin` up, past the top of the window.
pub fn get_popup_bounds(cx: &mut Cx2d, overlay_origin: DVec2, keyboard_height: f64, edge_gap: f64) -> Rect {
    let window_size = cx.current_pass_size();
    let insets = cx.display_context.safe_area_insets;
    let left = overlay_origin.x.max(0.0) + insets.left + edge_gap;
    let right = window_size.x - insets.right - edge_gap;
    let top = overlay_origin.y.max(0.0) + insets.top + edge_gap;
    let bottom = window_size.y - keyboard_height.max(insets.bottom) - edge_gap;
    Rect { pos: dvec2(left, top), size: dvec2((right - left).max(0.0), (bottom - top).max(0.0)) }
}
