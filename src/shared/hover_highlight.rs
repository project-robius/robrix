//! Shared logic for handling gray hover/press highlights for list items.

use makepad_widgets::*;

/// Hit-tests `area`, drives the widget's `bg_hover` animator, and returns the hit.
/// `claim_before` is `event.pointer_claimed_area()` snapshotted at the widget's handle_event entry.
/// Pass `true` for `keep_hovered` to leave the hover on even if it should be off.
pub fn handle_hover_hit<W: AnimatorImpl>(
    widget: &mut W,
    cx: &mut Cx,
    event: &Event,
    area: Area,
    claim_before: Area,
    keep_hovered: bool,
) -> Hit {
    handle_hover_hit_with_test(widget, cx, event, area, claim_before, keep_hovered,
        |abs, rect, inset| Inset::rect_contains_with_inset(abs, rect, inset),
    )
}

/// Same as [`handle_hover_hit()`], but with a custom hit test.
///
/// The test only narrows the returned hit; the highlight still covers the whole
/// `area`, so a child that owns the pointer can't darken us.
pub fn handle_hover_hit_with_test<W: AnimatorImpl, F>(
    widget: &mut W,
    cx: &mut Cx,
    event: &Event,
    area: Area,
    claim_before: Area,
    keep_hovered: bool,
    hit_test: F,
) -> Hit
where
    F: Fn(Vec2d, &Rect, &Option<Inset>) -> bool,
{
    // Hover hits go to exactly one widget, so a child claiming them would leave
    // us dark (or stuck lit). Instead, light up whenever the pointer rests inside
    // us and any claim on it appeared during our own dispatch (us or a child).
    if let Event::MouseMove(mm) = event {
        let rect = area.clipped_rect(cx);
        if !rect.contains(mm.abs) {
            if !keep_hovered {
                widget.animator_play(cx, ids!(bg_hover.off));
            }
        } else if cx.fingers.first_mouse_button.is_none() {
            if claim_before.is_empty() {
                widget.animator_play(cx, ids!(bg_hover.on));
            } else if !keep_hovered {
                // A claim predating our dispatch means an overlay above us owns the pointer.
                widget.animator_play(cx, ids!(bg_hover.off));
            }
        }
    }

    if let Event::ClearHover = event
        && !keep_hovered
    {
        widget.animator_play(cx, ids!(bg_hover.off));
    }

    let hit = event.hits_with_test(cx, area, hit_test);
    match &hit {
        Hit::FingerHoverIn(_) | Hit::FingerDown(_) | Hit::FingerLongPress(_) => {
            widget.animator_play(cx, ids!(bg_hover.on));
        }
        // Touch sends no mouse moves, so a drag off of us is handled here.
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
