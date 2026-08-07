//! Shared sizing and styling for Robrix's popup context menus,
//! matching that of the `RoomInputPopupMenu`.

use makepad_widgets::*;

pub const BUTTON_HEIGHT: f64 = 38.0;
const MENU_WIDTH: f64 = 235.0;
const MENU_PADDING: f64 = 6.0;
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
            color_hover: #EBEBEB
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

pub fn expected_menu_size(num_buttons: usize, num_dividers: usize) -> DVec2 {
    let height = num_buttons as f64 * BUTTON_HEIGHT
        + num_dividers as f64 * DIVIDER_HEIGHT
        + (num_buttons + num_dividers).saturating_sub(1) as f64 * MENU_SPACING
        + 2.0 * MENU_PADDING;
    dvec2(MENU_WIDTH, height)
}

/// Places the menu at `anchor_pos`, pulled back so it stays within `container_rect`.
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
