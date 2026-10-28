//! Rows shared by lists of users, rooms, etc., e.g., the mention popup and a room's member list.

use makepad_widgets::*;

/// The height of each of the rows below, though a status row grows if its text wraps.
pub const LIST_ROW_HEIGHT: f64 = 52.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A clickable row with an avatar, a title, and a smaller subtitle beneath it.
    // Its owner sets `draw_bg.color` to highlight it, e.g., upon hover.
    mod.widgets.AvatarListRow = RoundedView {
        width: Fill, height: #(LIST_ROW_HEIGHT), flow: Right, spacing: 9, align: Align{y: 0.5}
        padding: Inset{left: 10, right: 10}
        show_bg: true, cursor: MouseCursor.Hand
        // Tapping a row shouldn't take key focus away from a text input.
        grab_key_focus: false
        draw_bg +: { color: (COLOR_TRANSPARENT), border_radius: 5.0 }
        avatar := Avatar { width: 30, height: 30 }
        info := View {
            width: Fill, height: Fit, flow: Down, spacing: 1
            title := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: (COLOR_TEXT), text_style: theme.font_bold {font_size: 11, line_spacing: 1.0} }
            }
            subtitle := Label {
                width: Fill, height: Fit, max_lines: 1, text_overflow: Ellipsis, padding: 0
                draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 9.5, line_spacing: 1.0} }
            }
        }
    }

    // A row shown while a list's items are still loading.
    mod.widgets.ListLoadingRow = View {
        width: Fill, height: Fit, flow: Right, spacing: 10, align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 12, right: 12, top: 13, bottom: 13}
        loading_spinner := LoadingSpinner {
            width: 20, height: 20
            draw_bg +: { color: (COLOR_ACTIVE_PRIMARY), stroke_width: 2.5 }
        }
        loading_label := Label {
            width: Fit{max: FitBound.Rel{base: Base.Line, factor: 1.0}}, height: Fit, text_overflow: Ellipsis
            draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 10.5} }
        }
    }

    // A row shown instead of a list's items, e.g., when there are none.
    mod.widgets.ListEmptyRow = View {
        width: Fill, height: Fit
        padding: Inset{left: 12, right: 12, top: 13, bottom: 13}
        empty_label := Label {
            width: Fill, height: Fit
            align: Align{x: 0.5}
            draw_text +: { color: (COLOR_LIST_ROW_TEXT_MUTED), text_style: theme.font_regular {font_size: 10.5} }
        }
    }
}

/// Handles taps and hovers on the given rows of a list, e.g., from its `items_with_actions()`.
///
/// Updates `hovered` to the key of the hovered (or pressed) row, and returns the key of the tapped row, if any,
/// and whether `hovered` changed.
pub fn handle_row_actions<K: Copy + PartialEq>(
    items: impl IntoIterator<Item = (K, WidgetRef)>,
    is_scrolling: bool,
    actions: &Actions,
    hovered: &mut Option<K>,
) -> (Option<K>, bool) {
    let mut tapped = None;
    let mut hover_changed = false;
    for (key, item) in items {
        let row = item.as_view();
        // A touch has no hover, so we highlight the row it presses until the finger lifts.
        if row.finger_down(actions).is_some() && *hovered != Some(key) {
            *hovered = Some(key);
            hover_changed = true;
        }
        if let Some(fe) = row.finger_up(actions) {
            // Don't treat a touch that drags (a scroll motion) as a regular tap/click.
            if !is_scrolling && fe.is_over && fe.is_primary_hit() && fe.was_tap() {
                tapped = Some(key);
            }
            // Only a mouse that's still over the row keeps it hovered.
            if !(fe.is_over && fe.device.has_hovers()) && *hovered == Some(key) {
                *hovered = None;
                hover_changed = true;
            }
        }
        if row.finger_hover_in(actions).is_some() {
            *hovered = Some(key);
            hover_changed = true;
        }
        if row.finger_hover_out(actions).is_some() && *hovered == Some(key) {
            *hovered = None;
            hover_changed = true;
        }
    }
    (tapped, hover_changed)
}

/// Returns a `ListLoadingRow` or `ListEmptyRow` (from the list's `loading_row` or `empty_row` template)
/// at the given index of the given list, showing the given text.
pub fn status_row(cx: &mut Cx, list: &mut PortalList, index: usize, is_loading: bool, text: &str) -> WidgetRef {
    let (template, label) = status_row_template(is_loading);
    let row = list.item(cx, index, template);
    row.child_by_path(label).set_text(cx, text);
    row
}

/// Draws a `ListLoadingRow` or `ListEmptyRow` (from a `FlatList`'s `loading_row` or `empty_row` template)
/// with the given ID, showing the given text.
pub fn draw_flat_list_status_row(cx: &mut Cx2d, scope: &mut Scope, list: &mut FlatList, id: LiveId, is_loading: bool, text: &str) {
    let (template, label) = status_row_template(is_loading);
    let Some(row) = list.item(cx, id, template) else { return };
    row.child_by_path(label).set_text(cx, text);
    row.draw_all(cx, scope);
}

/// Returns the template and label of a `ListLoadingRow` if `is_loading`, otherwise of a `ListEmptyRow`.
fn status_row_template(is_loading: bool) -> (LiveId, &'static [LiveId]) {
    if is_loading {
        (id!(loading_row), ids!(loading_label))
    } else {
        (id!(empty_row), ids!(empty_label))
    }
}
