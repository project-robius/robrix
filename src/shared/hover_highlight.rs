//! Shared logic for handling gray hover/press highlights for list items.

use makepad_widgets::*;

/// Hit-tests the given `area`, drives the widget's `bg_hover` animator, and returns the hit.
///
/// The `claim_before` area should come from `event.pointer_claimed_area()`, which should've been
/// obtained *before* the widget forwarded the event to its children (e.g., at the start of `handle_event()`).
///
/// If `keep_hovered` is `true`, the hover will be left on even if the hit-test says it should be off.
pub fn handle_hover_hit<W: AnimatorImpl>(
    widget: &mut W,
    cx: &mut Cx,
    event: &Event,
    area: Area,
    claim_before: Area,
    keep_hovered: bool,
) -> Hit {
    handle_hover_hit_with_test(
        widget,
        cx,
        event,
        area,
        claim_before,
        keep_hovered,
        Inset::rect_contains_with_inset
    )
}

/// Same as [`handle_hover_hit()`], but with a custom hit test.
///
/// The `hit_test` fn only narrows the returned hit, it doesn't affect
/// the area that may be hovered.
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
    // A hover hit can only be delivered to exactly one widget, so if a child claimed it
    // then that would leave the given `widget` not hovered.
    // Here, we want to activate the hover for the widget even if a child claimed it,
    // but not if an ancestor widget claimed it.
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
                widget.animator_play(cx, ids!(bg_hover.off));
            }
        }
    }

    if let Event::ClearHover = event && !keep_hovered {
        widget.animator_play(cx, ids!(bg_hover.off));
    }

    let hit = event.hits_with_test(cx, area, hit_test);
    match &hit {
        Hit::FingerHoverIn(_) | Hit::FingerDown(_) | Hit::FingerLongPress(_) => {
            widget.animator_play(cx, ids!(bg_hover.on));
        }
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
