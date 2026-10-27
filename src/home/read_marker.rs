//! A red divider that shows where the user's latest unread messages start.
//!
//! The reason for this module is that the matrix rust SDK abruptly moves the read marker
//! any time you send a read receipt, causing the timelin view to jerk around a bit 
//! when it hides the old one and shows the new one lower down (towards the bottom of the viewport).
//! I don't like that jerkiness, so we track the previous position of it and animate that out
//! nice and smoothly while animating in the new read marker position.

use std::sync::Arc;
use imbl::Vector;
use makepad_widgets::{animator::Ease, *};
use matrix_sdk_ui::timeline::{TimelineItem, TimelineItemKind, TimelineUniqueId, VirtualTimelineItem};
use crate::home::{state_event_group::StateEventGroups, timeline_items::{ChangedItems, items_in}};

const ANIMATION_DURATION_SECS: f64 = 0.3;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.DateDivider = View {
        width: Fill,
        height: Fit,
        margin: Inset{top: 7.0, bottom: 7.0}
        flow: Right,
        padding: Inset{left: 7.0, right: 7.0},
        spacing: 0.0,
        align: Align{x: 0.5, y: 0.5}

        left_line := LineH { }

        date := Label {
            padding: Inset{left: 7.0, right: 7.0}
            draw_text +: {
                text_style: TEXT_SUB {},
                color: (COLOR_DIVIDER_DARK)
            }
            text: "<date>"
        }

        right_line := LineH { }
    }

    mod.widgets.ReadMarker = #(ReadMarker::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,

        divider := mod.widgets.DateDivider {
            left_line := LineH {
                draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
            }

            date := Label {
                draw_text.color: (mod.widgets.COLOR_READ_MARKER)
                text: "New Messages"
            }

            right_line := LineH {
                draw_bg.color: (mod.widgets.COLOR_READ_MARKER)
            }
        }
    }
}

/// A divider showing where the unread messages start, which can be animated in or out.
#[derive(Script, ScriptHook, Widget)]
pub struct ReadMarker {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,

    /// How much of this marker's height is showing, from 0 (hidden) to 1 (fully shown).
    /// Values in between show only the middle of the marker, which is how it animates in and out.
    #[rust(1.0)] visible_fraction: f64,
    #[rust] full_height: f64,
}

impl Widget for ReadMarker {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.visible_fraction >= 1.0 {
            self.view.draw_walk(cx, scope, walk)?;
        } else {
            // Only show the visible part, keeping the marker's line in the middle.
            let height = self.full_height * self.visible_fraction;
            cx.begin_turtle(
                Walk { height: Size::Fixed(height), ..walk },
                Layout::flow_down().with_scroll(dvec2(0.0, (self.full_height - height) / 2.0)),
            );
            self.view.draw_walk_all(cx, scope, walk);
            cx.end_turtle();
        }
        self.full_height = self.view.area().rect(cx).size.y;
        DrawStep::done()
    }
}

impl ReadMarkerRef {
    /// Sets how much of this marker's height is showing, from 0 (none of it) to 1 (all of it).
    fn set_visible_fraction(&self, visible_fraction: f64) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.visible_fraction = visible_fraction;
    }

    /// Sets this marker's full height if it doesn't know it yet, since it can't before it's drawn.
    fn set_full_height_if_unknown(&self, full_height: f64) {
        let Some(mut inner) = self.borrow_mut() else { return };
        if inner.full_height <= 0.0 {
            inner.full_height = full_height;
        }
    }
}

/// How a timeline's read marker was last drawn, plus its animation if it just moved.
#[derive(Default)]
pub(super) struct ReadMarkerState {
    animation: Option<ReadMarkerAnimation>,
    /// This is `None` if the list hasn't drawn this timeline since it was shown.
    last_drawn: Option<DrawnReadMarker>,
    /// The list's read marker item, if the list's current draw included it.
    drawn_item: Option<ReadMarkerRef>,
    /// Whether the list's current draw included any of the animation.
    is_animation_drawn: bool,
}

impl ReadMarkerState {
    /// Starts animating the read marker if it moved within `changes` when `old_items` got replaced by `new_items`.
    ///
    /// If an outgoing marker is still animating out, it keeps going unless this move has its own.
    pub(super) fn animate_if_moved(
        &mut self,
        cx: &mut Cx,
        old_items: &Vector<Arc<TimelineItem>>,
        new_items: &Vector<Arc<TimelineItem>>,
        changes: &[ChangedItems],
        old_groups: &StateEventGroups,
    ) {
        let Some(drawn) = self.last_drawn else { return };
        let old_index = changes.iter().find_map(|change|
            items_in(old_items, &change.old).position(|item| is_read_marker(item)).map(|i| change.old.start + i)
        );
        let new_index = changes.iter().find_map(|change|
            items_in(new_items, &change.new).position(|item| is_read_marker(item)).map(|i| change.new.start + i)
        );
        // The marker only moved if the items on both sides of it changed.
        let is_same_spot = match (old_index, new_index) {
            (Some(old), Some(new)) => unique_id_at(old_items, old.checked_sub(1)) == unique_id_at(new_items, new.checked_sub(1))
                || unique_id_at(old_items, Some(old + 1)) == unique_id_at(new_items, Some(new + 1)),
            (None, None) => true,
            _ => false,
        };
        if is_same_spot {
            return;
        }
        // The outgoing marker goes right after the last item the list drew before the marker's old spot.
        let outgoing_marker = old_index
            .filter(|_| drawn.visible_fraction > 0.0)
            .and_then(|old| old.checked_sub(1))
            .and_then(|prev| old_items.get(old_groups.last_drawn_at_or_before(prev)))
            .map(|item| OutgoingMarker {
                after: item.unique_id().clone(),
                initial_visible_fraction: drawn.visible_fraction,
                marker: cx.with_vm(|vm| {
                    let marker = vm.eval(script! { mod.widgets.ReadMarker {} });
                    WidgetRef::script_from_value(vm, marker)
                }).as_read_marker(),
            })
            .or_else(|| self.animation.as_ref().and_then(|animation| animation.outgoing_marker.as_ref().map(|outgoing| OutgoingMarker {
                initial_visible_fraction: outgoing.get_visible_fraction(animation.progress),
                ..outgoing.clone()
            })));
        // A marker that left a spot that wasn't drawn has nothing to animate.
        if outgoing_marker.is_none() && new_index.is_none() {
            return;
        }
        self.animation = Some(ReadMarkerAnimation {
            started_at: Cx::monotonic_now(),
            progress: 0.0,
            full_height: drawn.full_height,
            outgoing_marker,
            next_frame: NextFrame::default(),
        });
        // The marker isn't where the list drew it anymore, so a later move can't animate it out there.
        self.last_drawn = Some(DrawnReadMarker { visible_fraction: 0.0, ..drawn });
    }

    /// Returns whether `event` is the animation's next frame, which the list needs to draw.
    pub(super) fn is_next_frame(&self, event: &Event) -> bool {
        self.animation.as_ref().is_some_and(|animation| animation.next_frame.is_event(event).is_some())
    }

    /// Works out how far along the animation is for this whole draw, so both markers agree.
    pub(super) fn prepare_for_draw(&mut self) {
        if let Some(animation) = self.animation.as_mut() {
            animation.progress = Ease::InOutQuad.map(((Cx::monotonic_now() - animation.started_at) / ANIMATION_DURATION_SECS).min(1.0));
        }
        self.drawn_item = None;
        self.is_animation_drawn = false;
    }

    /// Shows as much of the list's read marker `item` as has animated in so far.
    pub(super) fn populate_item(&mut self, item: &WidgetRef) {
        let marker = item.as_read_marker();
        if let Some(animation) = &self.animation {
            marker.set_full_height_if_unknown(animation.full_height);
            self.is_animation_drawn = true;
        }
        marker.set_visible_fraction(self.animation.as_ref().map_or(1.0, |animation| animation.progress));
        self.drawn_item = Some(marker);
    }

    /// Draws the outgoing marker if it goes right after the item at `index`, which the list just drew.
    pub(super) fn draw_outgoing_marker_after(&mut self, cx: &mut Cx2d, scope: &mut Scope, items: &Vector<Arc<TimelineItem>>, index: usize) {
        let Some(animation) = &self.animation else { return };
        let Some(outgoing) = &animation.outgoing_marker else { return };
        if items.get(index).is_some_and(|item| item.unique_id() == &outgoing.after) {
            outgoing.marker.set_full_height_if_unknown(animation.full_height);
            outgoing.marker.set_visible_fraction(outgoing.get_visible_fraction(animation.progress));
            outgoing.marker.draw_all(cx, scope);
            self.is_animation_drawn = true;
        }
    }

    /// Records how the list drew the read marker.
    pub(super) fn finish_draw(&mut self, cx: &mut Cx) {
        let progress = self.animation.as_ref().map_or(1.0, |animation| animation.progress);
        self.last_drawn = Some(self.drawn_item.take().map_or(
            DrawnReadMarker { visible_fraction: 0.0, ..self.last_drawn.unwrap_or_default() },
            |marker| DrawnReadMarker { full_height: marker.borrow().map_or(0.0, |inner| inner.full_height), visible_fraction: progress },
        ));
        // Keep animating until it's done, unless none of the animation is on screen.
        if progress >= 1.0 || !self.is_animation_drawn {
            self.animation = None;
        } else if let Some(animation) = self.animation.as_mut() {
            animation.next_frame = cx.new_next_frame();
        }
    }
}

/// How the read marker looked in the list's last draw of a timeline.
#[derive(Clone, Copy, Default)]
struct DrawnReadMarker {
    full_height: f64,
    /// How much of its height was showing, which is 0 if it wasn't drawn.
    visible_fraction: f64,
}

/// The read marker animating out of its old spot and into its new one.
struct ReadMarkerAnimation {
    /// When the animation started, from [`Cx::monotonic_now()`].
    started_at: f64,
    /// How far along the animation was in the list's latest draw, from 0 (just started) to 1 (done).
    progress: f64,
    /// The read marker's full height, for marker widgets that haven't been drawn yet.
    full_height: f64,
    /// The marker that animates out at the old spot, if it showed there.
    outgoing_marker: Option<OutgoingMarker>,
    next_frame: NextFrame,
}

/// A second read marker that animates out where the marker used to be.
#[derive(Clone)]
struct OutgoingMarker {
    /// The item that this marker gets drawn right after.
    after: TimelineUniqueId,
    /// How much of the marker was showing when it started animating out.
    initial_visible_fraction: f64,
    marker: ReadMarkerRef,
}

impl OutgoingMarker {
    /// Returns how much of this marker is showing at the given progress of its animation.
    fn get_visible_fraction(&self, progress: f64) -> f64 {
        self.initial_visible_fraction * (1.0 - progress)
    }
}

/// Returns the unique ID of the item at `index`, if there is one.
fn unique_id_at(items: &Vector<Arc<TimelineItem>>, index: Option<usize>) -> Option<&TimelineUniqueId> {
    items.get(index?).map(|item| item.unique_id())
}

pub(super) fn is_read_marker(item: &TimelineItem) -> bool {
    matches!(item.kind(), TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker))
}
