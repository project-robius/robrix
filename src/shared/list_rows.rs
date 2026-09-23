//! Rows shared by lists of users, rooms, etc., e.g., the mention popup and a room's member list.

use makepad_widgets::*;

/// The height of each of the rows below.
pub const LIST_ROW_HEIGHT: f64 = 52.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The background color of a hovered row.
    mod.widgets.COLOR_LIST_ROW_HOVER = #xEAEFF5

    // A clickable row with an avatar, a title, and a smaller subtitle beneath it.
    // Its owner sets `draw_bg.color` to highlight it, e.g., upon hover.
    mod.widgets.AvatarListRow = RoundedView {
        width: Fill, height: #(LIST_ROW_HEIGHT), flow: Right, spacing: 9, align: Align{y: 0.5}
        padding: Inset{left: 10, right: 10}
        show_bg: true, cursor: MouseCursor.Hand
        // Tapping a row shouldn't take key focus away from a text input.
        grab_key_focus: false
        draw_bg +: { color: #00000000, border_radius: 5.0 }
        avatar := Avatar { width: 30, height: 30 }
        info := View {
            width: Fill, height: Fit, flow: Down, spacing: 1
            title := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: (COLOR_TEXT), text_style: theme.font_bold {font_size: 11, line_spacing: 1.0} }
            }
            subtitle := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: #555, text_style: theme.font_regular {font_size: 9.5, line_spacing: 1.0} }
            }
        }
    }

    // A row shown while a list's items are still loading.
    mod.widgets.ListLoadingRow = View {
        width: Fill, height: #(LIST_ROW_HEIGHT), flow: Right, spacing: 10, align: Align{x: 0.5, y: 0.5}
        loading_spinner := LoadingSpinner {
            width: 20, height: 20
            draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), border_size: 2.5 }
        }
        loading_label := Label {
            height: Fit
            draw_text +: { color: #555, text_style: theme.font_regular {font_size: 10.5} }
        }
    }

    // A row shown instead of a list's items, e.g., when there are none.
    mod.widgets.ListEmptyRow = View {
        width: Fill, height: #(LIST_ROW_HEIGHT), align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 12, right: 12}
        empty_label := Label {
            width: Fill, height: Fit, max_lines: 2, text_overflow: Ellipsis
            align: Align{x: 0.5}
            draw_text +: { color: #555, text_style: theme.font_regular {font_size: 10.5} }
        }
    }
}

/// Handles taps and hovers on the rows of the given list.
///
/// Updates `hovered` to the index of the hovered row, and returns the index of the tapped row, if any,
/// and whether `hovered` changed.
pub fn handle_row_actions(list: &PortalListRef, actions: &Actions, hovered: &mut Option<usize>) -> (Option<usize>, bool) {
    let mut tapped = None;
    let mut hover_changed = false;
    for (index, item) in list.items_with_actions(actions) {
        let row = item.as_view();
        // Don't treat a touch that drags (a scroll motion) as a regular tap/click.
        if !list.was_scrolling()
            && let Some(fe) = row.finger_up(actions)
            && fe.is_over && fe.is_primary_hit() && fe.was_tap()
        {
            tapped = Some(index);
        }
        if row.finger_hover_in(actions).is_some() {
            *hovered = Some(index);
            hover_changed = true;
        }
        if row.finger_hover_out(actions).is_some() && *hovered == Some(index) {
            *hovered = None;
            hover_changed = true;
        }
    }
    (tapped, hover_changed)
}

/// Returns a `ListLoadingRow` or `ListEmptyRow` (from the list's `loading_row` or `empty_row` template)
/// at the given index of the given list, showing the given text.
pub fn status_row(cx: &mut Cx, list: &mut PortalList, index: usize, is_loading: bool, text: &str) -> WidgetRef {
    let (template, label) = if is_loading {
        (id!(loading_row), ids!(loading_label))
    } else {
        (id!(empty_row), ids!(empty_label))
    };
    let row = list.item(cx, index, template);
    row.child_by_path(label).set_text(cx, text);
    row
}
