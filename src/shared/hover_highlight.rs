//! Shared logic for handling gray hover/press highlights for list items.

use makepad_widgets::*;
use makepad_widgets::makepad_platform::event::finger::TouchState;

/// Hit-tests `area`, drives the widget's `hover` animator, and returns the hit.
///
/// Pass `true` for `keep_hovered` to leave the hover on even if it should be off.
///
/// Note the animator group is `bg_hover`, not `hover`: `View` drives its own
/// `hover` group off any hit a child steals, which would fight us.
pub fn handle_hover_hit<W: AnimatorImpl>(
    widget: &mut W,
    cx: &mut Cx,
    event: &Event,
    area: Area,
    keep_hovered: bool,
) -> Hit {
    // Drive from the mouse position, not from hover hits: a child widget steals those,
    // and scrolling slides us off the area the hover was recorded against.
    if let Event::MouseMove(mm) = event {
        let rect = area.clipped_rect(cx);
        if !rect.contains(mm.abs) {
            if !keep_hovered {
                widget.animator_play(cx, ids!(bg_hover.off));
            }
        }
        // Whatever claimed a move inside our rect is us or a child, unless it's
        // layered on top, which is always bigger. Stay dark under a menu or modal.
        else {
            let claimed = mm.handled.get().rect(cx).size;
            if claimed.x <= rect.size.x && claimed.y <= rect.size.y {
                widget.animator_play(cx, ids!(bg_hover.on));
            }
        }
    }

    // Touch has no mouse moves, and a child may have captured the press, so our own
    // FingerUp below can't be relied on. Any release ends the hover.
    if !keep_hovered
        && let Event::TouchUpdate(tu) = event
        && tu.touches.iter().any(|t| t.state == TouchState::Stop)
    {
        widget.animator_play(cx, ids!(bg_hover.off));
    }

    let hit = event.hits(cx, area);
    match &hit {
        Hit::FingerHoverIn(_) | Hit::FingerDown(_) | Hit::FingerLongPress(_) => {
            widget.animator_play(cx, ids!(bg_hover.on));
        }
        // Touch sends no mouse moves, so it needs the drag-off case handled here.
        Hit::FingerMove(fe) if !keep_hovered && !fe.is_over => {
            widget.animator_play(cx, ids!(bg_hover.off));
        }
        // A released hit clears all hovers, unless the mouse is still hovering over us.
        Hit::FingerUp(fe) if !keep_hovered => {
            widget.animator_toggle(
                cx,
                fe.device.has_hovers() && fe.is_over,
                Animate::Yes,
                ids!(bg_hover.on),
                ids!(bg_hover.off),
            );
        }
        _ => { }
    }
    hit
}
