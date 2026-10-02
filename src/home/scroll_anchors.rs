//! Determines which items in a roomscreen's portallist that the view should stay anchored on.
//!
//! The whole point of this is to ensure that the viewport doesn't jump around
//! when items get added or removed above the ones on screen, e.g., back pagination, collapsing groups, etc.

use std::{ops::Range, sync::Arc};
use imbl::Vector;
use makepad_widgets::PortalListRef;
use matrix_sdk_ui::timeline::{TimelineItem, TimelineItemContent};
use ruma::{EventId, OwnedEventId};
use crate::home::{
    state_event_group::{GroupPlacement, StateEventGroups},
    timeline_items::{index_of_event, uses_compact_view},
};

/// A timeline item that was on screen in the list's most recent draw
/// that should be kept at the same spot on the screen after the next draw.
///
/// This can only be a real event, as we need to use its unique event ID to find it
/// after an update to the timeline's items has been processed.
#[derive(Clone)]
struct ScrollAnchor {
    /// The item's event ID.
    event_id: OwnedEventId,
    /// The item's index in the timeline's current items.
    index: usize,
    /// How many items without an event ID (like a day divider) the list starts with right before this item.
    ///
    /// Only an anchor that stands in for the list's first item has these (see [`ScrollAnchors::at_list_position()`]).
    items_before: usize,
    /// The distance from the top of the list's viewport to the top of the item.
    scroll_offset: f64,
    /// Whether the item was drawn with a non-zero height, unlike a hidden one.
    ///
    /// A zero-height item has no spot on screen of its own, so it only gets kept in place if nothing else can be.
    had_nonzero_height: bool,
    /// How the item was laid out in that draw, or since its group got toggled if it's the group's first item.
    layout: ItemLayout,
}

impl ScrollAnchor {
    /// Returns the index that the list should start at to keep this item in place.
    fn list_start(&self, items: &Vector<Arc<TimelineItem>>) -> usize {
        let has_no_event_id = |index: usize| items.get(index).is_some_and(|item| item.as_event().and_then(|event| event.event_id()).is_none());
        let num_still_before = (1..=self.items_before).take_while(|&n| self.index.checked_sub(n).is_some_and(has_no_event_id)).count();
        self.index - num_still_before
    }
}

/// The items to keep in place on screen through timeline updates, until the timeline's list draws again.
///
/// The list's item positions come from its most recent draw, so they only match the timeline's items
/// until the next change.
pub(super) struct ScrollAnchors {
    /// The items to keep in place, from top to bottom.
    anchors: Vec<ScrollAnchor>,
    /// The list's first item and scroll offset as of the last draw or update, to catch any scrolling since then.
    list_position: (usize, f64),
}

impl ScrollAnchors {
    /// Captures the positions of items on screen after the list's latest draw.
    ///
    /// The timeline's items must not have changed since that draw. If the list didn't draw its first item
    /// (like when it hasn't drawn this timeline yet), this falls back to [`Self::at_list_position()`].
    pub(super) fn capture(portal_list: &PortalListRef, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups) -> Self {
        let first_id = portal_list.first_id();
        let Some(first_slot) = portal_list.drawn_slot(first_id) else {
            return Self::at_list_position(portal_list, items, groups);
        };
        // The list draws everything down from its first item, so any scrolling since the draw moves every item.
        let scrolled_since_draw = portal_list.scroll_position() - first_slot.start;
        // +1 for the list's first item, which is often off-screen.
        let max_events = portal_list.visible_items() + 1;
        let mut anchors = Vec::with_capacity(max_events);
        let mut best_off_screen: Option<ScrollAnchor> = None;
        let mut num_events = 0;
        let mut index = first_id;
        while let Some(item) = items.get(index) && num_events < max_events {
            if let Some(event_id) = item.as_event().and_then(|event| event.event_id()) {
                num_events += 1;
                // Not all items got drawn, e.g., ones past the bottom of the list.
                if let Some(slot) = portal_list.drawn_slot(index) {
                    let anchor = ScrollAnchor {
                        event_id: event_id.to_owned(),
                        index,
                        items_before: 0,
                        scroll_offset: slot.start + scrolled_since_draw,
                        had_nonzero_height: slot.size > 0.0,
                        layout: ItemLayout::of_item_at(items, groups, index),
                    };
                    if anchor.had_nonzero_height && slot.start + slot.size > 0.0 {
                        anchors.push(anchor);
                    } else if best_off_screen.as_ref().is_none_or(|best| !best.had_nonzero_height && anchor.had_nonzero_height) {
                        best_off_screen = Some(anchor);
                    }
                }
            }
            index = groups.next_drawn_after(index, items.len());
        }
        if anchors.is_empty() {
            anchors.extend(best_off_screen);
        }
        Self { anchors, list_position: (first_id, portal_list.scroll_position()) }
    }

    /// Returns the items to keep in place right now, taking them out of `saved`.
    pub(super) fn take(saved: &mut Option<Self>, portal_list: &PortalListRef, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups) -> Self {
        match saved.take() {
            Some(anchors) => anchors.follow_list(portal_list, items, groups),
            None => Self::capture(portal_list, items, groups),
        }
    }

    /// Returns the list's first item, at the list's scroll offset, as the only item to keep in place.
    ///
    /// That's exactly where the list will draw it, so this works without any positions from a draw.
    /// If that item has no event ID (like a day divider), the anchor is the first real event with an ID
    /// that comes after it.
    pub(super) fn at_list_position(portal_list: &PortalListRef, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups) -> Self {
        let first_id = portal_list.first_id();
        let scroll = portal_list.scroll_position();
        let anchor = (first_id..items.len())
            .find_map(|index| Some((index, items.get(index)?.as_event()?.event_id()?)))
            .map(|(index, event_id)| ScrollAnchor {
                event_id: event_id.to_owned(),
                index,
                items_before: index - first_id,
                scroll_offset: scroll,
                had_nonzero_height: true,
                layout: ItemLayout::of_item_at(items, groups, index),
            });
        Self { anchors: anchor.into_iter().collect(), list_position: (first_id, scroll) }
    }

    /// Returns these anchors minus the ones that expanding or collapsing the group over `group_range` moves.
    ///
    /// The list draws downwards from its first item, so a toggle moves everything *after* the group's first item,
    /// unless the list now starts after the group (like when it got collapsed from its last item).
    /// If no available anchors exist any more, this falls back to [`Self::at_list_position()`].
    pub(super) fn through_toggle(mut self, portal_list: &PortalListRef, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups, group_range: Range<usize>) -> Self {
        let first_id = portal_list.first_id();
        self.anchors.retain(|anchor| anchor.index >= first_id && (anchor.index <= group_range.start || first_id >= group_range.end));
        if self.anchors.is_empty() {
            return Self::at_list_position(portal_list, items, groups);
        }
        // The group's first item stays where it starts, but it's laid out the way the toggle left it.
        if let Some(anchor) = self.anchors.iter_mut().find(|anchor| anchor.index == group_range.start) {
            anchor.layout = ItemLayout::of_item_at(items, groups, anchor.index);
        }
        self.list_position = (first_id, portal_list.scroll_position());
        self
    }

    /// Returns these anchors, moved along with any scrolling the list did since the last update.
    ///
    /// If something else put the list at a different first item, only that item gets kept in place from then on.
    fn follow_list(mut self, portal_list: &PortalListRef, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups) -> Self {
        let (first_id, scroll) = self.list_position;
        if portal_list.first_id() != first_id {
            return Self::at_list_position(portal_list, items, groups);
        }
        let scrolled = portal_list.scroll_position() - scroll;
        for anchor in &mut self.anchors {
            anchor.scroll_offset += scrolled;
        }
        self.list_position.1 += scrolled;
        self
    }

    /// Finds the anchored items again in `new_items`, which replace `old_items`, and returns whether any of them moved.
    ///
    /// Items that are gone get dropped, and so do items that moved past other items (see [`keep_anchors_in_order()`]).
    /// If the update `added_at_front`, so do items that the SDK replaced with a copy in the items it added.
    pub(super) fn refind(&mut self, old_items: &Vector<Arc<TimelineItem>>, new_items: &Vector<Arc<TimelineItem>>, added_at_front: bool) -> bool {
        /// How far around its old index an item gets looked for before searching the whole timeline.
        const NEARBY: usize = 64;
        let is_at = |index: usize, event_id: &EventId| new_items.get(index).and_then(|item| item.as_event()?.event_id()) == Some(event_id);
        // Items added or removed above the anchored items move them all by the same amount, so look there before searching.
        let len_change = new_items.len() as isize - old_items.len() as isize;
        let mut last_move = len_change;
        let num_anchors = self.anchors.len();
        let mut old_indices = Vec::with_capacity(num_anchors);
        self.anchors.retain_mut(|anchor| {
            let new_index = [0, last_move, len_change].into_iter()
                .filter_map(|change| anchor.index.checked_add_signed(change))
                .find(|&index| is_at(index, &anchor.event_id))
                .or_else(|| (anchor.index.saturating_sub(NEARBY)..anchor.index + NEARBY).find(|&index| is_at(index, &anchor.event_id)))
                .or_else(|| index_of_event(new_items, &anchor.event_id, new_items.len(), usize::MAX));
            let Some(new_index) = new_index else { return false };
            last_move = new_index as isize - anchor.index as isize;
            old_indices.push(anchor.index);
            anchor.index = new_index;
            true
        });
        // A page of older events can include an anchored event, which the SDK then moves into that page.
        // Keeping that one in place would make the view jump into the page, so drop it if there are other anchors.
        if added_at_front && len_change > 0 {
            let is_page_copy: Vec<bool> = self.anchors.iter().zip(&old_indices)
                .map(|(anchor, &old)| (anchor.index as isize - old as isize) < len_change
                    && !old_items.get(old).zip(new_items.get(anchor.index)).is_some_and(|(old_item, new_item)| Arc::ptr_eq(old_item, new_item))
                    && !still_above_same_event(old_items, new_items, old, anchor.index)
                )
                .collect();
            if is_page_copy.contains(&false) {
                let mut is_copy = is_page_copy.iter().copied();
                self.anchors.retain(|_| !is_copy.next().unwrap_or(false));
                let mut is_copy = is_page_copy.into_iter();
                old_indices.retain(|_| !is_copy.next().unwrap_or(false));
            }
        }
        let moved = self.anchors.len() < num_anchors || self.anchors.iter().zip(&old_indices).any(|(anchor, &old)| anchor.index != old);
        keep_anchors_in_order(&mut self.anchors, &old_indices);
        moved
    }

    /// Returns where the list should start to keep these items in place: an item index and its scroll offset.
    ///
    /// That's at the first item that's still laid out the way it was drawn, so nothing moved within its slot.
    /// If there isn't one, the first item stays where it was, or its group's summary item does if it's hidden in one.
    pub(super) fn pin(&self, items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups) -> Option<(usize, f64)> {
        let laid_out_as_drawn = self.anchors.iter().find(|anchor|
            anchor.had_nonzero_height && ItemLayout::of_item_at(items, groups, anchor.index) == anchor.layout
        );
        if let Some(anchor) = laid_out_as_drawn {
            return Some((anchor.list_start(items), anchor.scroll_offset));
        }
        let first = self.anchors.first()?;
        let start = groups.summary_item_if_collapsed(first.index).unwrap_or_else(|| first.list_start(items));
        Some((start, first.scroll_offset))
    }

    /// Remembers where the list is now, so that only scrolling after this moves these items (see [`Self::follow_list()`]).
    pub(super) fn remember_list_position(&mut self, portal_list: &PortalListRef) {
        self.list_position = (portal_list.first_id(), portal_list.scroll_position());
    }
}

/// Drops the anchors that moved past other anchors, keeping as many anchors as possible that are still in draw order.
fn keep_anchors_in_order(anchors: &mut Vec<ScrollAnchor>, old_indices: &[usize]) {
    if anchors.windows(2).all(|pair| pair[0].index < pair[1].index) {
        return;
    }
    /// The best set of anchors that are still in order, ending at a given anchor.
    struct AnchorsInOrder {
        num_anchors: usize,
        total_moved: usize,
        prev_anchor: Option<usize>,
    }
    let rank = |in_order: &AnchorsInOrder| (in_order.num_anchors, std::cmp::Reverse(in_order.total_moved));
    let mut best_ending_at: Vec<AnchorsInOrder> = Vec::with_capacity(anchors.len());
    for (i, anchor) in anchors.iter().enumerate() {
        let moved = anchor.index.abs_diff(old_indices[i]);
        let alone = AnchorsInOrder { num_anchors: 1, total_moved: moved, prev_anchor: None };
        let best = (0..i)
            .filter(|&prev| anchors[prev].index < anchor.index)
            .map(|prev| {
                let before = &best_ending_at[prev];
                AnchorsInOrder { num_anchors: before.num_anchors + 1, total_moved: before.total_moved + moved, prev_anchor: Some(prev) }
            })
            .fold(alone, |best, in_order| if rank(&in_order) > rank(&best) { in_order } else { best });
        best_ending_at.push(best);
    }
    let mut keep = vec![false; anchors.len()];
    let mut next = (0..anchors.len()).max_by_key(|&i| rank(&best_ending_at[i]));
    while let Some(i) = next {
        keep[i] = true;
        next = best_ending_at[i].prev_anchor;
    }
    let mut keep = keep.into_iter();
    anchors.retain(|_| keep.next().unwrap_or(false));
}

/// Returns whether the item that moved from `old_index` to `new_index` is still right above the same event.
fn still_above_same_event(
    old_items: &Vector<Arc<TimelineItem>>,
    new_items: &Vector<Arc<TimelineItem>>,
    mut old_index: usize,
    mut new_index: usize,
) -> bool {
    /// How many replaced events to follow down before giving up.
    const MAX_REPLACED: usize = 64;
    /// Returns the first event with an ID after `index`, and that ID.
    fn next_event(items: &Vector<Arc<TimelineItem>>, index: usize) -> Option<(usize, &EventId)> {
        (index + 1..items.len()).find_map(|i| Some((i, items.get(i)?.as_event()?.event_id()?)))
    }
    for _ in 0..MAX_REPLACED {
        match (next_event(old_items, old_index), next_event(new_items, new_index)) {
            (None, None) => return true,
            (Some((next_old, old_id)), Some((next_new, new_id))) if old_id == new_id => {
                if Arc::ptr_eq(&old_items[next_old], &new_items[next_new]) {
                    return true;
                }
                (old_index, new_index) = (next_old, next_new);
            }
            _ => return false,
        }
    }
    true
}

/// Layout info that decides where a timeline item's content starts within the portallist.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ItemLayout {
    placement: GroupPlacement,
    is_compact: bool,
}

impl ItemLayout {
    fn of_item_at(items: &Vector<Arc<TimelineItem>>, groups: &StateEventGroups, index: usize) -> Self {
        let is_compact = items.get(index).and_then(|item| item.as_event()).is_some_and(|event|
            matches!(event.content(), TimelineItemContent::MsgLike(_))
                && uses_compact_view(index.checked_sub(1).and_then(|prev| items.get(prev)), event)
        );
        Self { placement: groups.placement_of(index), is_compact }
    }
}
