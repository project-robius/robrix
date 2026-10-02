//! Handles grouping a contiguous series of small state events into a collapsible/expandable group
//! with a summary of what happened across those state events (like joins, leaves, profile changes, etc).
//!
//! The timeline's PortalList always maintains the invariant that `item id == timeline index`,
//! so a group's first item contains its summary (and an expand button) when collapsed.
//! When expanded, it contains the group's first event.
//!
//! For perf reasons, when collapsed, the portallist actually skips drawing the rest of
//! the groups' events (because they're not even visible).

use std::ops::Range;
use hashbrown::HashMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{EventId, OwnedEventId, OwnedUserId, UserId};
use matrix_sdk_ui::timeline::{Profile, TimelineDetails};
use crate::{
    LivePtr, widget_ref_from_live_ptr,
    home::room_read_receipt::{AvatarRowRef, AvatarRowWidgetRefExt},
    shared::{avatar::{AvatarRef, AvatarWidgetRefExt}, expand_arrow::ExpandArrow, hover_highlight::handle_hover_hit, styles::{COLOR_ROBRIX_PURPLE, SMALL_STATE_TEXT_COLOR}},
    sliding_sync::TimelineKind,
};

/// How much each avatar in an [`AvatarStack`] overlaps the one before it.
const STACKED_AVATAR_OVERLAP: f64 = 5.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*


    // The expand/collapse buttons for a group of state events.
    // This consists of an arrow at the beginning followed by an "Expand" or "Collapse" label.
    mod.widgets.GroupToggle = set_type_default() do #(GroupToggle::register_widget(vm)) {
        width: Fit, height: Fit
        flow: Right,
        spacing: 1.5

        toggle_arrow := mod.widgets.ExpandArrow {
            width: 16, height: 16,
            margin: Inset{left: 0.5}
            draw_bg.color: (SMALL_STATE_TEXT_COLOR)
        }

        label_view := View {
            width: Fit, height: Fit
            margin: Inset{top: 1.0}

            toggle_label := Label {
                width: Fit, height: Fit
                flow: Flow.Right { wrap: false },
                padding: 0
                draw_text +: {
                    text_style: SMALL_STATE_TEXT_STYLE {},
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: "Expand"
            }
        }
    }

    // A line with just a collapse button, under the last event of an expanded group.
    mod.widgets.GroupToggleLine = set_type_default() do #(GroupToggleLine::register_widget(vm)) {
        width: Fill,
        height: Fit,
        margin: Inset{top: 4.0}
        padding: Inset{ left: 82.0, top: 2.0, bottom: 2.0 }
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            hover: instance(0.0)
            color: instance((COLOR_PRIMARY))
            color_hover: instance(COLOR_LIST_ITEM_BG_HOVER)

            pixel: fn() {
                return Pal.premul(mix(self.color, self.color_hover, self.hover))
            }
        }

        animator: Animator{
            bg_hover: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 1.0} }
                }
            }
        }

        toggle := mod.widgets.GroupToggle {}
    }

    // The view used for each state event (non-messages) in a room's timeline.
    // It's called a small event because the timestamp, profile picture, and text are all smaller than messages.
    mod.widgets.SmallStateEvent = set_type_default() do #(SmallStateEvent::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,
        margin: Inset{ top: 4.0, bottom: 4.0}
        spacing: 0.0
        cursor: MouseCursor.Default

        // See-through, except for a blue flash (like a message's) after a jump to it.
        show_bg: true
        draw_bg +: {
            highlight: instance(0.0)
            color_highlight: instance(#c5d6fa)

            pixel: fn() {
                return Pal.premul(vec4(self.color_highlight.xyz, self.color_highlight.w * self.highlight))
            }
        }

        animator: Animator{
            highlight: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 2.0} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Forward {duration: 0.5} }
                    ease: ExpDecay {d1: 0.80, d2: 0.97}
                    apply: { draw_bg: {highlight: 1.0} }
                }
            }
        }

        body := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{ left: 7.0, top: 3.0, bottom: 3.0, right: 10.0 }
            spacing: 5.0

            left_container := View {
                align: Align{x: 0.5, y: 0}
                width: 70.0,
                height: Fit

                timestamp := Timestamp {
                    margin: Inset{top: 3}
                }
            }

            avatar := Avatar {
                width: 19.,
                height: 19.,
                margin: 0

                text_view +: {
                    text +: {
                        draw_text +: {
                            text_style: TITLE_TEXT { font_size: 7.0 }
                        }
                    }
                }
            }

            // Only knocks that haven't been answered show an invite button.
            // We also do not put those knocks into a collapsed group.
            invite_user_button := RobrixPositiveIconButton {
                visible: false
                margin: Inset{ top: -1.5, left: 2, right: 2}
                padding: Inset{top: 4, bottom: 4, left: 9, right: 9}
                draw_bg +: {
                    border_size: 0.75
                }
                draw_icon.svg: (ICON_ADD_USER)
                draw_text.text_style: SMALL_STATE_TEXT_STYLE {}
                icon_walk: Walk{width: 15, height: Fit, margin: Inset{right: -4}}
                text: "Invite to Room"
            }

            content := Label {
                width: Fill,
                height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset{top: 2.5}
                padding: Inset{ top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 }
                draw_text +: {
                    text_style: SMALL_STATE_TEXT_STYLE {},
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            avatar_row := mod.widgets.AvatarRow {}
        }

        // We also show a line with just the collapse button after the last event in a group,
        // (when it's expanded).
        // This makes it easy for the user to collapse it without having to scroll up.
        collapse_line := mod.widgets.GroupToggleLine { visible: false }
    }

    // The avatars of the people involved in a group of state events.
    mod.widgets.AvatarStack = #(AvatarStack::register_widget(vm)) {
        width: Fit,
        height: Fit,
        margin: Inset{top: 2.0}
        avatar_template: Avatar {
            width: 16.0,
            height: 16.0,
            margin: 0
            text_view +: {
                text +: {
                    draw_text +: {
                        text_style: TITLE_TEXT { font_size: 6.5 }
                    }
                }
            }
        }
    }

    // The header of a group of state events, which includes the expand/collapse button and the text summary.
    mod.widgets.StateEventGroupHeader = set_type_default() do #(StateEventGroupHeader::register_widget(vm)) {
        width: Fill,
        height: Fit,
        flow: Down,
        margin: Inset{ top: 4.0, bottom: 4.0}
        padding: Inset{ top: 1.0, bottom: 1.0 }
        spacing: 0.0
        cursor: MouseCursor.Hand

        show_bg: true
        draw_bg +: {
            hover: instance(0.0)
            color: instance((COLOR_PRIMARY))
            color_hover: instance(COLOR_LIST_ITEM_BG_HOVER)

            pixel: fn() {
                return Pal.premul(mix(self.color, self.color_hover, self.hover))
            }
        }

        animator: Animator{
            bg_hover: {
                default: @off
                off: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 0.0} }
                }
                on: AnimatorState{
                    redraw: true,
                    from: { all: Snap }
                    apply: { draw_bg: {hover: 1.0} }
                }
            }
        }

        toggle_row := View {
            width: Fill,
            height: Fit
            // Align it with the avatar/content column (left padding that's the same width as a timestamp).
            padding: Inset{ left: 82.0, top: 1.0, bottom: 1.0 }

            toggle := mod.widgets.GroupToggle {}
        }

        // The summary, laid out like a SmallStateEvent so it lines up with the items around it.
        body := View {
            width: Fill,
            height: Fit
            flow: Right,
            padding: Inset{ left: 7.0, top: 2.0, bottom: 2.0, right: 10.0 }
            spacing: 5.0

            left_container := View {
                align: Align{x: 0.5, y: 0}
                width: 70.0,
                height: Fit

                timestamp := Timestamp {
                    margin: Inset{top: 3}
                }
            }

            participants := mod.widgets.AvatarStack {}

            summary := Label {
                width: Fill,
                height: Fit
                flow: Flow.Right{wrap: true},
                margin: Inset{top: 2.5}
                padding: 0
                max_lines: 3
                text_overflow: TextOverflow.Ellipsis
                draw_text +: {
                    text_style: SMALL_STATE_TEXT_STYLE {},
                    color: (SMALL_STATE_TEXT_COLOR)
                }
                text: ""
            }

            avatar_row := mod.widgets.AvatarRow {}
        }
    }

    // A group's summary item.
    // When collapsed this contains its header plus the group's summary.
    // When expanded, this contains just the first event in the group, as normal.
    mod.widgets.GroupSummaryItem = View {
        width: Fill,
        height: Fit,
        flow: Down,

        header := mod.widgets.StateEventGroupHeader {}

        first_event := mod.widgets.SmallStateEvent {
            visible: false
            margin: Inset{ top: 0.0, bottom: 4.0 }
        }
    }
}


/// The kind of grouping that a timeline item can be a part of.
#[derive(Clone, Copy, Debug)]
pub enum GroupingKind<'a> {
    /// A visible state event that can be part of a group.
    Groupable(GroupableEvent<'a>),
    /// A hidden (zero-height) item: it sits inside a group without splitting it, but isn't counted.
    Hidden,
    /// A day divider, which can be spanned by or hidden within a group.
    ///
    /// A collapsed group still shows the last day divider it spans if that day goes on after the group
    /// (see [`StateEventGroups::day_shows_after()`]).
    Divider,
    /// Anything else that's visible (a message, an undecryptable event, etc) that ends a group.
    Breaker,
    /// Something visible that ends a group but isn't part of any day, like the read marker.
    Marker,
}

/// What we needs to know about a state event to determine if it can go into a group.
#[derive(Clone, Copy, Debug)]
pub struct GroupableEvent<'a> {
    pub event_id: Option<&'a EventId>,
    pub sender: &'a UserId,
    /// If this event is a membership or profile change, this contains the user it's about.
    pub user_about: Option<&'a UserId>,
    /// Whether this is the room's initial creation event.
    pub is_room_create: bool,
    /// Whether this only goes in a group as part of the room's setup, and is shown on its own anywhere else.
    pub only_grouped_in_room_setup: bool,
}

/// The timeline items that groups get computed over.
pub trait GroupItems {
    type Item;

    fn num_items(&self) -> usize;
    fn get(&self, index: usize) -> Option<&Self::Item>;
    fn grouping_kind<'a>(&'a self, item: &'a Self::Item) -> GroupingKind<'a>;
    /// Iterates over the items from `start` to the end.
    fn iter_from(&self, start: usize) -> impl Iterator<Item = &Self::Item>;
}

/// A contiguous group of 2+ state events, shown as one collapsible summary item.
///
/// All of its positions (`range` and the dividers) are timeline item indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateEventGroup {
    /// The timeline indices in the group; `start` is its summary item.
    pub range: Range<usize>,
    /// How many visible state events the group holds (hidden items in its range aren't counted).
    pub num_events: usize,
    pub is_expanded: bool,
    /// The last day divider inside the group, if it spans multiple days.
    ///
    /// While the group is collapsed, this divider is only shown if more events from that day
    /// come after the group, ensuring that whatever comes next is shown under the correct day.
    pub last_divider: Option<usize>,
    /// The day divider this group sits under, if any.
    ///
    /// While the group is collapsed, this divider will show the whole date range of the group
    /// (if it spans multiple days) just to make it clear to the user how many days were collapsed.
    pub preceding_divider: Option<usize>,
}

/// A group of state events that's still being assembled while scanning the timeline.
///
/// It only becomes a real group if it ends up having 2 or more events.
/// All of its positions (`start`, `last_grouped_item`, and the dividers) are timeline item indices.
struct PendingGroup<'a> {
    /// The first grouped item, where the summary will be shown.
    start: usize,
    /// The last grouped item so far. The group ends right after it.
    last_grouped_item: usize,
    /// How many state events are in this group so far (hidden items and day dividers don't count).
    num_events: usize,
    /// The newest expand/collapse choice that any of its events remembers, which decides whether it's expanded.
    latest_choice: Option<ExpandChoice>,
    /// The last day divider that has another item that is within this group after it.
    last_divider: Option<usize>,
    /// A day divider after the last grouped item so far, which isn't in the group yet.
    ///
    /// This will only join the group (becoming `last_divider`) if another grouped item
    /// comes after it later.
    trailing_divider: Option<usize>,
    /// The nearest day divider before the group (the day it starts on), if any.
    preceding_divider: Option<usize>,
    /// The room's creator, if this group starts with the room's creation.
    creator: Option<&'a UserId>,
}

impl<'a> PendingGroup<'a> {
    fn new(start: usize, creator: Option<&'a UserId>, preceding_divider: Option<usize>) -> Self {
        Self {
            start,
            last_grouped_item: start,
            num_events: 0,
            latest_choice: None,
            last_divider: None,
            trailing_divider: None,
            preceding_divider,
            creator,
        }
    }

    /// Returns whether the given event is the end of the room creation/setup group.
    fn ends_room_setup(&self, event: &GroupableEvent) -> bool {
        self.creator.is_some_and(|creator|
            event.sender != creator
                || event.user_about.is_some_and(|about| about != creator)
        )
    }

    /// Returns whether the given item is part of the room's creation/setup group.
    fn continues_room_setup(&self, event: &GroupableEvent) -> bool {
        self.creator.is_some() && !self.ends_room_setup(event)
    }

    /// Tries to turn this pending group into a real group, if it's long enough to be one.
    ///
    /// Returns `None` if it's too short (fewer than 2 items).
    fn into_group(self) -> Option<StateEventGroup> {
        (self.num_events >= 2).then(|| StateEventGroup {
            range: self.start..self.last_grouped_item + 1,
            num_events: self.num_events,
            is_expanded: self.latest_choice.is_some_and(|choice| choice.is_expanded),
            last_divider: self.last_divider,
            preceding_divider: self.preceding_divider,
        })
    }
}

impl StateEventGroup {
    /// Returns the group's items after its summary item (its first item).
    ///
    /// While the group is collapsed, these are all hidden, except for
    /// its last day divider if that day still continues after the end of this group.
    pub fn items_after_summary(&self) -> Range<usize> {
        self.range.start + 1 .. self.range.end
    }

    /// Returns this group with each of its indices at or after `first_shifted_index` shifted by `len_change`.
    ///
    /// That's where the group's items end up after an update adds `len_change` items (or removes some, if it's negative)
    /// right before `first_shifted_index`. For example, adding 3 items before index 6 turns a group at 8..11 into 11..14,
    /// but a group at 1..4 stays the same.
    fn with_indices_shifted(&self, len_change: isize, first_shifted_index: usize) -> Self {
        let shifted = |index: usize| if index < first_shifted_index { index } else { index.saturating_add_signed(len_change) };
        Self {
            // The range's end is one past its last item, so the last item decides whether the end shifts.
            range: shifted(self.range.start)..shifted(self.range.end - 1) + 1,
            last_divider: self.last_divider.map(shifted),
            preceding_divider: self.preceding_divider.map(shifted),
            ..self.clone()
        }
    }

    /// Returns the ranges of items to redraw when this group is toggled or changes.
    ///
    /// This includes its summary item and possibly also the day divider right before the group.
    pub fn ranges_to_redraw(&self) -> impl Iterator<Item = Range<usize>> {
        let summary_item = self.range.start..self.range.start + 1;
        let divider = self.preceding_divider.map(|d| d..d + 1);
        std::iter::once(summary_item).chain(divider)
    }
}

/// The user's choice to expand or collapse a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExpandChoice {
    /// Whether the user expanded the group (`true`) or collapsed it (`false`).
    is_expanded: bool,
    /// The sequence number of this choice, where a higher one is newer.
    ///
    /// This is incremented upon each new choice, which helps us determine
    /// whether the user's choice to expand or collapse a group is more recent than a prior one.
    sequence_number: u32,
}

/// Where an event should show up with respect to its group, if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupPlacement {
    /// On its own: it's not in a group, or it comes after the first event of an expanded group.
    OnItsOwn,
    /// The first event of an expanded group, shown under the group's header.
    UnderHeader,
    /// The first event of a collapsed group, whose summary item is shown instead.
    Summary,
    /// Hidden in a collapsed group.
    Hidden,
}

/// All state event groups within a timeline (in index order), plus which ones are expanded.
#[derive(Default)]
pub struct StateEventGroups {
    groups: Vec<StateEventGroup>,
    /// The latest expand/collapse choice for each event in a group the user toggled.
    ///
    /// This is just here to handle the case when multiple groups get merged together.
    /// Basically, the latest expand or collapse choice for either group will win.
    expand_choices: HashMap<OwnedEventId, ExpandChoice>,
    /// The sequence number of the user's latest choice.
    latest_sequence_number: u32,
    /// What's hidden in collapsed groups, see [`Self::collapsed_ranges()`].
    collapsed_ranges: Vec<Range<usize>>,
}

impl StateEventGroups {
    /// Recomputes how state events should be grouped after the given timeline items have changed.
    ///
    /// `changed` is the range of item indices that changed in the timeline.
    /// If it ends before the list does, that means nothing after it changed,
    /// though everything did move by `len_change` indices, which is the number of
    /// items added minus the number removed.
    /// Any items outside of `changed` that also changed in place need a call to
    /// [`Self::regroup_around()`] after this.
    ///
    /// Returns the ranges of items to redraw beyond `changed` itself: the summary item and day divider
    /// of each group that changed or had something in it change.
    pub fn rebuild<I: GroupItems>(
        &mut self,
        items: &I,
        changed: Range<usize>,
        len_change: isize,
    ) -> Vec<Range<usize>> {
        self.rebuild_ranges(items, [(changed, len_change)])
    }

    /// Recomputes the groups after items changed in several separate ranges,
    /// like [`Self::rebuild()`] does for a single one.
    ///
    /// Each one is the range of items that changed and its `len_change`, in order.
    pub fn rebuild_ranges<I: GroupItems>(
        &mut self,
        items: &I,
        changed_ranges: impl IntoIterator<Item = (Range<usize>, isize)>,
    ) -> Vec<Range<usize>> {
        let mut to_redraw = Vec::new();
        let mut any_changed = false;
        for (changed, len_change) in changed_ranges {
            to_redraw.extend(self.regroup_range(items, changed, len_change));
            any_changed = true;
        }
        if any_changed {
            self.update_collapsed_ranges(items);
        }
        to_redraw
    }

    /// Regroups one range of changed items like [`Self::rebuild()`] does,
    /// except for updating the collapsed ranges.
    fn regroup_range<I: GroupItems>(
        &mut self,
        items: &I,
        changed: Range<usize>,
        len_change: isize,
    ) -> Vec<Range<usize>> {
        let len = items.num_items();
        // Start one item before the changed range such that a changed item
        // can be considered as part of the group of items that came right before it.
        let mut scan_from = changed.start.min(len).saturating_sub(1);
        while scan_from > 0
            && items.get(scan_from).is_some_and(|item|
                matches!(items.grouping_kind(item), GroupingKind::Hidden | GroupingKind::Divider)
            )
        {
            scan_from -= 1;
        }
        let num_groups_before_scan = self.groups.partition_point(|g| g.range.end <= scan_from);
        if let Some(g) = self.groups.get(num_groups_before_scan) && g.range.start <= scan_from {
            scan_from = g.range.start;
        }
        let old_groups = self.groups.split_off(num_groups_before_scan);

        // We already know the bounds of the changed item(s), so we don't need to keep looking
        // once we've hit the ending boundary of a change.
        let is_bounded = changed.end < len;
        let mut scan_end = len;
        let mut pending: Option<PendingGroup> = None;
        // The last day divider before `scan_from`.
        let (unknown_from, last_known_divider) = match old_groups.first() {
            Some(g) if g.range.start == scan_from => (scan_from, g.preceding_divider),
            _ => self.groups.last().map_or((0, None), |g| (g.range.end, g.last_divider.or(g.preceding_divider))),
        };
        let last_divider_before_scan = || (unknown_from..scan_from).rev()
            .find(|&i| items.get(i).is_some_and(|item| matches!(items.grouping_kind(item), GroupingKind::Divider)))
            .or(last_known_divider);
        // The last day divider before the item being scanned.
        let mut divider_above: Option<Option<usize>> = None;
        for (offset, item) in items.iter_from(scan_from).enumerate() {
            let index = scan_from + offset;
            match items.grouping_kind(item) {
                GroupingKind::Groupable(event) => {
                    // Some changes only go in a group as part of the room's setup.
                    if event.only_grouped_in_room_setup
                        && !pending.as_ref().is_some_and(|pending| pending.continues_room_setup(&event))
                    {
                        self.add_group_if_long_enough(items, pending.take());
                        continue;
                    }
                    // A room's creation (the create event, then the creator setting it up) is its own special group:
                    // it ends at the first state event by or about anyone else.
                    if event.is_room_create || pending.as_ref().is_some_and(|pending| pending.ends_room_setup(&event)) {
                        self.add_group_if_long_enough(items, pending.take());
                    }
                    let pending = pending.get_or_insert_with(|| {
                        let divider = *divider_above.get_or_insert_with(last_divider_before_scan);
                        PendingGroup::new(index, event.is_room_create.then_some(event.sender), divider)
                    });
                    pending.num_events += 1;
                    pending.last_grouped_item = index;
                    if let Some(divider) = pending.trailing_divider.take() {
                        pending.last_divider = Some(divider);
                    }
                    if let Some(&choice) = event.event_id.and_then(|id| self.expand_choices.get(id))
                        && pending.latest_choice.is_none_or(|latest| latest.sequence_number < choice.sequence_number)
                    {
                        pending.latest_choice = Some(choice);
                    }
                }
                GroupingKind::Hidden => { }
                GroupingKind::Divider => {
                    divider_above = Some(Some(index));
                    if let Some(pending) = &mut pending {
                        pending.trailing_divider = Some(index);
                    }
                }
                GroupingKind::Marker => {
                    self.add_group_if_long_enough(items, pending.take());
                }
                GroupingKind::Breaker => {
                    self.add_group_if_long_enough(items, pending.take());
                    if is_bounded && index >= changed.end {
                        scan_end = index;
                        break;
                    }
                }
            }
        }
        self.add_group_if_long_enough(items, pending.take());
        let (still_valid, old_rescanned): (Vec<_>, Vec<_>) = old_groups.into_iter()
            .partition(|g| is_bounded && g.range.start > scan_end.saturating_add_signed(-len_change));
        // The old groups that overlapped the changed items have now changed with them.
        // To compare the others with the new groups, we have to shift the indices of the ones
        // that came after the changed items to where those items are now.
        let old_change_end = changed.end.saturating_add_signed(-len_change);
        let (old_overlapping, mut old_unchanged): (Vec<_>, Vec<_>) = old_rescanned.into_iter()
            .partition(|g| g.range.start < old_change_end && changed.start < g.range.end);
        for g in old_unchanged.iter_mut() {
            *g = g.with_indices_shifted(len_change, old_change_end);
        }
        let new_groups = &self.groups[num_groups_before_scan..];

        // A closure to determine if the given group depends on the change that happened
        // and thus needs to be redrawn.
        let depends_on_change = |g: &StateEventGroup| {
            let divider_shows_dates = !g.is_expanded && g.last_divider.is_some();
            let first_item_it_depends_on = match g.preceding_divider {
                Some(divider) if divider_shows_dates => divider,
                _ => g.range.start,
            };
            first_item_it_depends_on < changed.end && changed.start < g.range.end
        };

        // If everything changed, there's nothing else to redraw.
        let to_redraw = if changed.start == 0 && !is_bounded {
            Vec::new()
        } else {
            old_unchanged.iter().filter(|&g| !contains_group(new_groups, g))
                .flat_map(StateEventGroup::ranges_to_redraw)
                // An old group that overlapped the changed items is still where it was, unless it was among the items that moved.
                .chain(old_overlapping.iter().filter(|&g| !contains_group(new_groups, g))
                    .flat_map(StateEventGroup::ranges_to_redraw)
                    .filter(|range| len_change == 0 || range.start < changed.start)
                )
                .chain(new_groups.iter().filter(|&g| !contains_group(&old_unchanged, g) || depends_on_change(g))
                    .flat_map(StateEventGroup::ranges_to_redraw)
                )
                .collect()
        };

        // The groups after the rescanned items didn't change, they just moved.
        self.groups.extend(still_valid.iter().map(|old| {
            let mut g = old.with_indices_shifted(len_change, old_change_end);
            if old.preceding_divider.is_none_or(|divider| divider < old_change_end) {
                g.preceding_divider = match divider_above {
                    Some(divider) => divider,
                    None if old.preceding_divider.is_none_or(|divider| divider < scan_from) => old.preceding_divider,
                    None => *divider_above.get_or_insert_with(last_divider_before_scan),
                };
            }
            g
        }));
        to_redraw
    }

    /// Regroups the items around the given ones, which changed in place outside of `rebuild()`'s range.
    ///
    /// For example, a knock that just got answered can go in a group now, even though its answer
    /// came in further down the timeline. Returns the ranges of items to redraw, like `rebuild()` does.
    pub fn regroup_around<I: GroupItems>(&mut self, items: &I, indices: &[usize]) -> Vec<Range<usize>> {
        self.rebuild_ranges(items, indices.iter().map(|&index| (index..index + 1, 0)))
    }

    /// Adds the given pending group to `self.groups`, if it has at least 2 events.
    ///
    /// All of its events also get the newest expand/collapse choice that any of them remembers,
    /// so the whole group stays the way the user left it.
    fn add_group_if_long_enough<I: GroupItems>(&mut self, items: &I, pending: Option<PendingGroup>) {
        let Some(pending) = pending else { return };
        if let Some(choice) = pending.latest_choice {
            remember_choice(&mut self.expand_choices, grouped_event_ids(items, pending.start..pending.last_grouped_item + 1), choice);
        }
        self.groups.extend(pending.into_group());
    }

    /// Returns the last item of the date range that the day divider at `divider_index` shows, if any.
    ///
    /// A divider shows a date range when the first thing under it is a collapsed group that spans
    /// multiple days, and that range ends at the group's last item.
    /// Otherwise, the divider just shows its own singular date.
    pub fn collapsed_span_end<I: GroupItems>(
        &self,
        items: &I,
        divider_index: usize,
    ) -> Option<usize> {
        // Hidden items and the read marker don't count.
        let first_shown = (divider_index + 1..items.num_items()).find(|&i| {
            items.get(i).is_some_and(|item| !matches!(items.grouping_kind(item), GroupingKind::Hidden | GroupingKind::Marker))
        })?;
        let group = self.containing(first_shown).filter(|g| g.range.start == first_shown)?;
        (!group.is_expanded && group.last_divider.is_some()).then(|| group.range.end - 1)
    }

    /// Returns whether anything shows under the day divider at `index`, before the next day divider.
    ///
    /// Everything under it may be hidden, or in a collapsed group (which only shows its summary item),
    /// in which case the divider itself doesn't need to show either.
    pub fn day_shows_after<I: GroupItems>(
        &self,
        items: &I,
        index: usize,
    ) -> bool {
        let mut i = index + 1;
        while let Some(item) = items.get(i) {
            if let Some(group) = self.containing(i).filter(|g| !g.is_expanded && g.range.start != i) {
                if group.last_divider.is_some_and(|divider| divider >= i) {
                    return false;
                }
                i = group.range.end;
                continue;
            }
            match items.grouping_kind(item) {
                GroupingKind::Divider => return false,
                GroupingKind::Hidden | GroupingKind::Marker => i += 1,
                GroupingKind::Groupable(_) | GroupingKind::Breaker => return true,
            }
        }
        false
    }

    /// Returns the group that the timeline item at `index` belongs to, if any.
    pub fn containing(&self, index: usize) -> Option<&StateEventGroup> {
        let i = self.groups.partition_point(|g| g.range.end <= index);
        self.groups.get(i).filter(|g| g.range.start <= index)
    }

    /// Returns where the event at `index` should show up within its group.
    pub fn placement_of(&self, index: usize) -> GroupPlacement {
        match self.containing(index) {
            None => GroupPlacement::OnItsOwn,
            Some(group) if group.range.start == index && group.is_expanded => GroupPlacement::UnderHeader,
            Some(group) if group.range.start == index => GroupPlacement::Summary,
            Some(group) if group.is_expanded => GroupPlacement::OnItsOwn,
            Some(_) => GroupPlacement::Hidden,
        }
    }

    /// Returns the ranges of items that are hidden in collapsed groups, in order.
    ///
    /// A collapsed group only shows its summary item, plus its last day divider if that day
    /// continues after the group ends (see [`Self::day_shows_after()`]).
    ///
    /// The timeline's list skips drawing these ranges, since they're not visible anyway.
    pub fn collapsed_ranges(&self) -> &[Range<usize>] {
        &self.collapsed_ranges
    }

    /// Returns the summary item for the collapsed group that ends right before `index`, if there is one.
    ///
    /// Day dividers, hidden items, and the read marker in between don't count.
    pub fn collapsed_group_right_before<I: GroupItems>(&self, items: &I, index: usize) -> Option<usize> {
        let last_shown = (0..index.min(items.num_items())).rev().find(|&i| items.get(i).is_some_and(|item|
            matches!(items.grouping_kind(item), GroupingKind::Groupable(_) | GroupingKind::Breaker)
        ))?;
        self.summary_item_if_collapsed(last_shown)
    }

    /// Returns the index of the next item after `index` that the timeline's list draws.
    ///
    /// This skips items hidden in collapsed groups (see [`Self::collapsed_ranges()`]),
    /// but it never skips the timeline's last item, even if it's hidden.
    /// This is because we always must draw the last item such that the portallist knows it's at the end.
    pub fn next_drawn_after(&self, index: usize, num_items: usize) -> usize {
        let next = index + 1;
        let i = self.collapsed_ranges.partition_point(|range| range.end <= next);
        let next = self.collapsed_ranges.get(i).filter(|range| range.start <= next).map_or(next, |range| range.end);
        next.min(num_items.saturating_sub(1)).max(index + 1)
    }

    /// Recomputes which items are hidden in collapsed groups (see [`Self::collapsed_ranges()`]).
    ///
    /// This always starts from scratch, since a collapsed group's last day divider
    /// depends on what comes after that group, which itself can change even if the group doesn't change.
    fn update_collapsed_ranges<I: GroupItems>(&mut self, items: &I) {
        let mut collapsed_ranges = std::mem::take(&mut self.collapsed_ranges);
        collapsed_ranges.clear();
        for group in self.groups.iter().filter(|group| !group.is_expanded) {
            match group.last_divider.filter(|&divider| self.day_shows_after(items, divider)) {
                Some(divider) => collapsed_ranges.extend([group.range.start + 1..divider, divider + 1..group.range.end]),
                None => collapsed_ranges.push(group.items_after_summary()),
            }
        }
        collapsed_ranges.retain(|range| !range.is_empty());
        self.collapsed_ranges = collapsed_ranges;
    }

    /// Returns the summary item of the collapsed group that contains the item at `index`,
    /// unless `index` *is* that summary item, in which case it returns `None`.
    pub fn summary_item_if_collapsed(&self, index: usize) -> Option<usize> {
        self.containing(index)
            .filter(|group| !group.is_expanded && group.range.start != index)
            .map(|group| group.range.start)
    }

    /// Expands or collapses the group containing the item at `index`, returning it.
    ///
    /// All of the group's events remember this choice (see [`ExpandChoice`]).
    pub fn toggle<I: GroupItems>(&mut self, index: usize, items: &I) -> Option<StateEventGroup> {
        let i = self.groups.partition_point(|g| g.range.end <= index);
        let group = self.groups.get_mut(i).filter(|g| g.range.start <= index)?;
        group.is_expanded = !group.is_expanded;
        self.latest_sequence_number += 1;
        let choice = ExpandChoice { is_expanded: group.is_expanded, sequence_number: self.latest_sequence_number };
        remember_choice(&mut self.expand_choices, grouped_event_ids(items, group.range.clone()), choice);
        let group = group.clone();
        self.update_collapsed_ranges(items);
        Some(group)
    }

    /// Expands the group containing the item at `index` and returns it, if that group is collapsed.
    pub fn expand_containing<I: GroupItems>(&mut self, index: usize, items: &I) -> Option<StateEventGroup> {
        if self.containing(index).is_none_or(|g| g.is_expanded) {
            return None;
        }
        self.toggle(index, items)
    }
}

/// Returns whether anything before `index` shows up as its own item in the timeline.
///
/// Day dividers, hidden items, and the read marker don't count.
pub fn shows_anything_before<I: GroupItems>(items: &I, index: usize) -> bool {
    items.iter_from(0)
        .take(index)
        .any(|item| matches!(items.grouping_kind(item), GroupingKind::Groupable(_) | GroupingKind::Breaker))
}

/// Returns whether the sorted `groups` contain the given `group`, unchanged.
fn contains_group(groups: &[StateEventGroup], group: &StateEventGroup) -> bool {
    groups.binary_search_by_key(&group.range.start, |g| g.range.start).is_ok_and(|i| groups[i] == *group)
}

/// Makes the given list of events remember the expand/collapse choice.
fn remember_choice<'a>(
    expand_choices: &mut HashMap<OwnedEventId, ExpandChoice>,
    event_ids: impl Iterator<Item = &'a EventId>,
    choice: ExpandChoice,
) {
    for id in event_ids {
        match expand_choices.get_mut(id) {
            Some(its_choice) => *its_choice = choice,
            None => { expand_choices.insert(id.to_owned(), choice); }
        }
    }
}

/// Returns the event IDs of the grouped events among the items in `range`.
fn grouped_event_ids<I: GroupItems>(items: &I, range: Range<usize>) -> impl Iterator<Item = &EventId> {
    items.iter_from(range.start)
        .take(range.len())
        .filter_map(move |item| match items.grouping_kind(item) {
            GroupingKind::Groupable(info) => info.event_id,
            _ => None,
        })
}


/// The avatars of the (distinct) people involved in a group of state events.
#[derive(Script, ScriptHook, WidgetRef, WidgetSet, WidgetRegister)]
pub struct AvatarStack {
    #[uid] uid: WidgetUid,
    #[rust] area: Area,
    #[layout] layout: Layout,
    #[walk] walk: Walk,
    #[live] avatar_template: Option<LivePtr>,
    /// One avatar per shown user, in order of the events.
    #[rust] avatars: Vec<StackedAvatar>,
    #[rust] timeline_kind: Option<TimelineKind>,
}

/// Info about a user to show in an [`AvatarStack`].
#[derive(Clone, PartialEq)]
pub struct StackedUser {
    pub user_id: OwnedUserId,
    pub profile: Option<Profile>,
}

/// One avatar in an [`AvatarStack`].
struct StackedAvatar {
    user: StackedUser,
    avatar: AvatarRef,
    /// Whether its image is fully drawn yet.
    is_drawn: bool,
}

impl WidgetNode for AvatarStack {
    fn widget_uid(&self) -> WidgetUid { self.uid }
    fn walk(&mut self, _cx: &mut Cx) -> Walk { self.walk }
    fn area(&self) -> Area { self.area }
    fn redraw(&mut self, cx: &mut Cx) { self.area.redraw(cx) }
    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        for (i, StackedAvatar { avatar, .. }) in self.avatars.iter().enumerate() {
            visit(live_id_num!(avatar, i as u64), WidgetRef::clone(avatar));
        }
    }
    fn skip_widget_tree_search(&self) -> bool { true }
    fn cancel_children_impl(&self, _visit: &mut dyn FnMut(LiveId, WidgetRef)) -> bool { true }
}

impl Widget for AvatarStack {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // The avatars come from a template, so nothing sends them events unless we do.
        // They only get actions here, mostly so they can show their images once loaded.
        if let Event::Actions(_) = event {
            for StackedAvatar { avatar, .. } in &self.avatars {
                avatar.handle_event(cx, event, scope);
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.avatars.is_empty() {
            self.area = Area::Empty;
            return DrawStep::done();
        }
        // Avatars show a text placeholder while being fetched; keep drawing them until their image arrives.
        self.update_undrawn_avatars(cx);
        cx.begin_turtle(walk, self.layout);
        for (i, StackedAvatar { avatar, .. }) in self.avatars.iter_mut().enumerate() {
            // Each avatar slightly overlaps the one before it.
            let mut avatar_walk = avatar.walk(cx);
            if i > 0 {
                avatar_walk.margin.left = -STACKED_AVATAR_OVERLAP;
            }
            let _ = avatar.draw_walk(cx, scope, avatar_walk);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl AvatarStack {
    /// Shows avatars for the given users.
    pub fn set_users(&mut self, cx: &mut Cx, timeline_kind: &TimelineKind, users: &[StackedUser]) {
        if !self.avatars.iter().map(|a| &a.user).eq(users) {
            self.avatars = users.iter()
                .map(|user| StackedAvatar {
                    user: user.clone(),
                    avatar: widget_ref_from_live_ptr(cx, self.avatar_template).as_avatar(),
                    is_drawn: false,
                })
                .collect();
            // Tell the widget tree to pick up the list of new avatars
            cx.widget_tree_mark_dirty(self.uid);
        }
        self.timeline_kind = Some(timeline_kind.clone());
        self.update_undrawn_avatars(cx);
    }

    /// Populates any avatars that aren't fully drawn yet.
    fn update_undrawn_avatars(&mut self, cx: &mut Cx) {
        let Some(timeline_kind) = &self.timeline_kind else { return };
        for StackedAvatar { user, avatar, is_drawn } in &mut self.avatars {
            if !*is_drawn {
                let profile = user.profile.clone().map(TimelineDetails::Ready);
                *is_drawn = avatar.set_avatar_and_get_username(cx, timeline_kind, &user.user_id, profile.as_ref(), None, false).1;
            }
        }
    }
}

impl AvatarStackRef {
    /// See [`AvatarStack::set_users()`].
    pub fn set_users(&self, cx: &mut Cx, timeline_kind: &TimelineKind, users: &[StackedUser]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_users(cx, timeline_kind, users);
        }
    }
}


#[derive(Clone, Debug, Default)]
enum StateEventGroupAction {
    /// A group's expand/collapse button was clicked.
    Toggled,
    #[default]
    None,
}

/// Handles a pointer event on a widget that toggles its group when clicked anywhere on it.
///
/// This includes hover highlights, animating the text/icon color and the background hover state.
///
/// Returns `true` if it was clicked/tapped.
fn handle_toggle_hit<W>(widget: &mut W, cx: &mut Cx, event: &Event, claim_before: Area) -> bool
where
    W: AnimatorImpl + std::ops::Deref<Target = View>,
{
    let area = widget.area();
    let was_hovered = widget.animator_in_state(cx, ids!(bg_hover.on));
    let hit = handle_hover_hit(widget, cx, event, area, claim_before, false);
    let hovered = widget.animator_in_state(cx, ids!(bg_hover.on));
    if hovered != was_hovered
        && let Some(mut toggle) = widget.widget(cx, ids!(toggle)).borrow_mut::<GroupToggle>()
    {
        toggle.set_highlighted(cx, hovered);
    }
    match hit {
        Hit::FingerDown(_) => {
            cx.set_key_focus(area);
            false
        }
        Hit::FingerUp(fe) => fe.is_over && fe.is_primary_hit() && fe.was_tap(),
        _ => false,
    }
}

/// The expand/collapse button in a group's header or toggle line.
#[derive(Script, ScriptHook, Widget)]
pub struct GroupToggle {
    #[deref] view: View,
    #[rust] highlighted: bool,
}

impl Widget for GroupToggle {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let color = if self.highlighted { COLOR_ROBRIX_PURPLE } else { SMALL_STATE_TEXT_COLOR };
        self.view.label(cx, ids!(toggle_label)).set_text_color(cx, color);
        if let Some(mut arrow) = self.view.widget(cx, ids!(toggle_arrow)).borrow_mut::<ExpandArrow>() {
            arrow.set_color(cx, color);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl GroupToggle {
    fn set_highlighted(&mut self, cx: &mut Cx, highlighted: bool) {
        self.highlighted = highlighted;
        self.redraw(cx);
    }
}

/// Returns whether the widget with the given `uid` was just clicked to toggle its group.
fn was_toggled(uid: WidgetUid, actions: &Actions) -> bool {
    actions.filter_widget_actions(uid)
        .any(|action| matches!(action.cast(), StateEventGroupAction::Toggled))
}

/// The header of a group of state events: its expand/collapse button, followed by its summary while collapsed.
///
/// Clicking anywhere on it toggles (expands/collapses) the group.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct StateEventGroupHeader {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
    #[rust] expanded: bool,
    /// Users whose names in the summary are still being looked up.
    #[rust] pending_names: Vec<(OwnedUserId, Option<String>)>,
}

impl Widget for StateEventGroupHeader {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        // Children (e.g., the timestamp's tooltip) go first, but remember who'd claimed
        // the pointer before them so a child hover doesn't turn off our highlight.
        let claim_before = event.pointer_claimed_area();
        self.view.handle_event(cx, event, scope);

        if handle_toggle_hit(self, cx, event, claim_before) {
            self.set_expanded(cx, !self.expanded, Animate::Yes);
            self.redraw(cx);
            cx.widget_action(self.widget_uid(), StateEventGroupAction::Toggled);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl StateEventGroupHeader {
    fn set_expanded(&mut self, cx: &mut Cx, expanded: bool, animate: Animate) {
        self.expanded = expanded;
        self.view.label(cx, ids!(toggle_label)).set_text(cx, if expanded { "Collapse" } else { "Expand" });
        if let Some(mut arrow) = self.view.widget(cx, ids!(toggle_arrow)).borrow_mut::<ExpandArrow>() {
            match animate {
                Animate::Yes => arrow.set_is_open(cx, expanded, Animate::Yes),
                Animate::No => arrow.set_is_open_no_animate(expanded),
            }
        }
        // Once expanded, the events themselves are on show, so the summary goes away.
        // Flip the flag directly: this gets drawn right after, so no redraw is needed.
        if let Some(mut body) = self.view.view(cx, ids!(body)).borrow_mut() {
            body.visible = !expanded;
        }
    }
}

impl StateEventGroupHeaderRef {
    /// Sets whether the group is expanded, which hides the summary.
    pub fn set_expanded(&self, cx: &mut Cx, expanded: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_expanded(cx, expanded, Animate::No);
        }
    }

    /// Sets the summary text, plus whose names in it are still being looked up.
    ///
    /// `pending_names` pairs each of those users with the name the summary shows for them
    /// in the meantime (`None` means it shows their user ID). See [`Self::names_still_pending()`].
    pub fn set_summary(&self, cx: &mut Cx, summary: &str, pending_names: Vec<(OwnedUserId, Option<String>)>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.label(cx, ids!(summary)).set_text(cx, summary);
            inner.pending_names = pending_names;
        }
    }

    /// Returns whether the summary's pending names are all still being looked up, with nothing new to show.
    ///
    /// The `look_up` closure is invoked to get a user's name and whether it's still being looked up.
    ///
    /// Returns false if no more names were pending.
    pub fn names_still_pending(&self, mut look_up: impl FnMut(&UserId) -> (Option<String>, bool)) -> bool {
        self.borrow().is_some_and(|inner| {
            !inner.pending_names.is_empty() && inner.pending_names.iter().all(|(user_id, shown)| {
                let (name, pending) = look_up(user_id);
                pending && name == *shown
            })
        })
    }

    /// Returns whether this header was just clicked to toggle its group.
    pub fn toggled(&self, actions: &Actions) -> bool {
        was_toggled(self.widget_uid(), actions)
    }
}

/// A line with just the expand/collapse control, after the last event of an expanded group.
///
/// Clicking anywhere on it collapses the group.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct GroupToggleLine {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
}

impl Widget for GroupToggleLine {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.view.visible && event.requires_visibility() {
            return;
        }
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        let claim_before = event.pointer_claimed_area();
        self.view.handle_event(cx, event, scope);

        if handle_toggle_hit(self, cx, event, claim_before) {
            cx.widget_action(self.widget_uid(), StateEventGroupAction::Toggled);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // This line sits under the group it collapses, so its arrow always points up at the group.
        self.view.label(cx, ids!(toggle_label)).set_text(cx, "Collapse");
        if let Some(mut arrow) = self.view.widget(cx, ids!(toggle_arrow)).borrow_mut::<ExpandArrow>() {
            arrow.set_pointing_up_no_animate();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl GroupToggleLineRef {
    /// Shows or hides this line. Meant for right before it gets drawn, so it doesn't redraw anything.
    pub fn set_shown(&self, shown: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.visible = shown;
        }
    }

    /// Returns whether this line was just clicked to collapse its group.
    pub fn toggled(&self, actions: &Actions) -> bool {
        was_toggled(self.widget_uid(), actions)
    }
}

/// A small state event in the timeline, e.g., a membership, profile, or room setting change.
///
/// It can flash a highlight, e.g., after a jump to it.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct SmallStateEvent {
    #[source] source: ScriptObjectRef,
    #[deref] view: View,
    #[apply_default] animator: Animator,
}

impl Widget for SmallStateEvent {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.redraw(cx);
        }
        // Once the highlight is fully on, fade it back out.
        if !self.animator.is_track_animating(id!(highlight)) && self.animator_in_state(cx, ids!(highlight.on)) {
            self.animator_play(cx, ids!(highlight.off));
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl SmallStateEventRef {
    /// Shows or hides this event without redrawing anything.
    pub fn set_shown(&self, shown: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.visible = shown;
        }
    }

    /// Flashes this event's highlight, like a message's after a jump to it.
    pub fn highlight(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.animator_play(cx, ids!(highlight.on));
            inner.redraw(cx);
        }
    }
}

/// Returns whether a group's expand/collapse control in the given timeline item was just clicked.
///
/// That's either a group's header, or the collapse line under an expanded group's last event.
pub fn group_toggled(cx: &mut Cx, item: &WidgetRef, actions: &Actions) -> bool {
    item.state_event_group_header(cx, ids!(header)).toggled(actions)
        || item.group_toggle_line(cx, ids!(collapse_line)).toggled(actions)
}

/// Returns the read receipt rows in the given timeline item.
///
/// A group's summary item has two: one next to the summary, and one on the group's first event,
/// which only shows once the group is expanded. For any other item, the second one is an empty ref.
pub fn avatar_rows(cx: &mut Cx, item: &WidgetRef) -> [AvatarRowRef; 2] {
    [item.avatar_row(cx, ids!(avatar_row)), item.avatar_row(cx, ids!(first_event.avatar_row))]
}

/// Flashes the highlight on the given timeline item, if it's a small state event.
///
/// For a group's summary item, this flashes the group's first event.
pub fn highlight_small_state_event(cx: &mut Cx, item: &WidgetRef) {
    let first_event = item.small_state_event(cx, ids!(first_event));
    let event = if first_event.is_empty() { item.as_small_state_event() } else { first_event };
    event.highlight(cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in timeline item, so the grouping can be tested without real SDK items.
    enum Item {
        Message,
        Hidden,
        Divider,
        ReadMarker,
        State { id: OwnedEventId, sender: OwnedUserId, about: Option<OwnedUserId>, is_create: bool, only_in_setup: bool },
        /// A knock that's still waiting on an answer, which is shown on its own like a message is.
        ///
        /// Once it's answered, it's a `State` event by and about whoever knocked.
        PendingKnock { id: OwnedEventId, sender: OwnedUserId },
    }

    impl GroupItems for Vec<Item> {
        type Item = Item;
        fn num_items(&self) -> usize { self.len() }
        fn get(&self, index: usize) -> Option<&Item> { self.as_slice().get(index) }
        fn iter_from(&self, start: usize) -> impl Iterator<Item = &Item> { self[start.min(self.len())..].iter() }
        fn grouping_kind<'a>(&'a self, item: &'a Item) -> GroupingKind<'a> {
            match item {
                Item::Message | Item::PendingKnock { .. } => GroupingKind::Breaker,
                Item::Hidden => GroupingKind::Hidden,
                Item::Divider => GroupingKind::Divider,
                Item::ReadMarker => GroupingKind::Marker,
                Item::State { id, sender, about, is_create, only_in_setup } => GroupingKind::Groupable(GroupableEvent {
                    event_id: Some(id),
                    sender,
                    user_about: about.as_deref(),
                    is_room_create: *is_create,
                    only_grouped_in_room_setup: *only_in_setup,
                }),
            }
        }
    }

    fn user(name: &str) -> OwnedUserId {
        OwnedUserId::try_from(format!("@{name}:example.org")).unwrap()
    }

    /// Returns a state event by `sender` about `about`, or about nobody.
    ///
    /// An event about someone stands in for a membership or profile change,
    /// and an event about nobody stands in for a room change.
    fn event(n: usize, sender: &str, about: Option<&str>) -> Item {
        Item::State {
            id: OwnedEventId::try_from(format!("$ev{n}")).unwrap(),
            sender: user(sender),
            about: about.map(user),
            is_create: false,
            only_in_setup: false,
        }
    }

    /// Returns a state event by alice about herself (the common case).
    fn state(n: usize) -> Item {
        event(n, "alice", Some("alice"))
    }

    /// Returns a knock by `sender` that's still waiting on an answer.
    fn pending_knock(n: usize, sender: &str) -> Item {
        Item::PendingKnock { id: OwnedEventId::try_from(format!("$ev{n}")).unwrap(), sender: user(sender) }
    }

    /// Returns what the knock `item` turns into once it's answered, or once its answer goes away.
    ///
    /// Here, any state event by and about the same person counts as an answered knock. Returns `None` for anything else.
    fn flip_knock(item: &Item) -> Option<Item> {
        match item {
            Item::PendingKnock { id, sender } => Some(Item::State {
                id: id.clone(),
                sender: sender.clone(),
                about: Some(sender.clone()),
                is_create: false,
                only_in_setup: false,
            }),
            Item::State { id, sender, about: Some(about), is_create: false, only_in_setup: false } if about == sender => {
                Some(Item::PendingKnock { id: id.clone(), sender: sender.clone() })
            }
            _ => None,
        }
    }

    fn create(n: usize, sender: &str) -> Item {
        let Item::State { id, sender, about, .. } = event(n, sender, None) else { unreachable!() };
        Item::State { id, sender, about, is_create: true, only_in_setup: false }
    }

    /// Returns a change by `sender` to who can join the room (like its join rules).
    ///
    /// Such a change only goes in a group as part of the room's setup.
    fn access_change(n: usize, sender: &str) -> Item {
        let Item::State { id, sender, about, .. } = event(n, sender, None) else { unreachable!() };
        Item::State { id, sender, about, is_create: false, only_in_setup: true }
    }

    fn rebuild_all(items: &Vec<Item>) -> StateEventGroups {
        let mut groups = StateEventGroups::default();
        groups.rebuild(items, 0..usize::MAX, 0);
        groups
    }

    /// A group's range, event count, and last day divider.
    #[derive(Debug, PartialEq)]
    struct GroupShape {
        range: Range<usize>,
        num_events: usize,
        last_divider: Option<usize>,
    }

    fn shape(range: Range<usize>, num_events: usize, last_divider: Option<usize>) -> GroupShape {
        GroupShape { range, num_events, last_divider }
    }

    /// Returns the [`GroupShape`] of every group, for terse assertions.
    fn group_shapes(groups: &StateEventGroups) -> Vec<GroupShape> {
        groups.groups.iter().map(|g| shape(g.range.clone(), g.num_events, g.last_divider)).collect()
    }

    #[test]
    fn collapsed_groups_extend_the_day_divider_above_them() {
        let items = vec![
            Item::Divider, Item::Hidden, state(2), Item::Divider, state(4), Item::Message,
            Item::Divider, state(7), state(8),
        ];
        let mut groups = rebuild_all(&items);
        assert_eq!(groups.containing(2).unwrap().preceding_divider, Some(0));
        assert_eq!(groups.containing(7).unwrap().preceding_divider, Some(6));
        // The first group continues into the next day, so the divider above it covers its last event too.
        assert_eq!(groups.collapsed_span_end(&items, 0), Some(4));
        assert_eq!(groups.collapsed_span_end(&items, 6), None);
        assert_eq!(groups.containing(2).unwrap().ranges_to_redraw().collect::<Vec<_>>(), vec![2..3, 0..1]);
        // Not once it's expanded: its own dividers show then.
        groups.toggle(2, &items);
        assert_eq!(groups.collapsed_span_end(&items, 0), None);
        assert_eq!(groups.containing(2).unwrap().ranges_to_redraw().collect::<Vec<_>>(), vec![2..3, 0..1]);
        // A rebuild that starts partway through a day still finds the divider above.
        let to_redraw = groups.rebuild(&items, 8..9, 0);
        assert!(to_redraw.contains(&(7..8)) && to_redraw.contains(&(6..7)));
        assert_eq!(groups.containing(7).unwrap().preceding_divider, Some(6));
    }

    #[test]
    fn two_or_more_contiguous_state_events_become_a_group() {
        let items = vec![state(0), state(1), Item::Message, state(3), Item::Message, state(5), state(6), state(7)];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..2, 2, None), shape(5..8, 3, None)]);
    }

    #[test]
    fn hidden_items_are_absorbed_but_not_counted() {
        let items = vec![Item::Hidden, state(1), Item::Hidden, state(3), Item::Hidden, Item::Message, state(6), Item::Hidden, Item::Message];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(1..4, 2, None)]);
    }

    #[test]
    fn day_dividers_are_spanned() {
        let items = vec![state(0), Item::Divider, state(2), Item::Divider, state(4), Item::Divider, Item::Message];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..5, 3, Some(3))]);
    }

    #[test]
    fn room_creation_is_its_own_group() {
        let items = vec![
            create(0, "alice"),
            event(1, "alice", Some("alice")),   // alice joins her new room
            event(2, "alice", None),            // power levels
            event(3, "alice", None),            // room name
            event(4, "bob", Some("bob")),       // bob joins: a new group starts here
            event(5, "carol", Some("carol")),
            event(6, "alice", None),            // alice changing the topic later is a normal change
        ];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..4, 4, None), shape(4..7, 3, None)]);
    }

    #[test]
    fn room_setup_can_span_days() {
        let items = vec![
            create(0, "alice"),
            event(1, "alice", Some("alice")),
            Item::Divider,
            event(3, "alice", None),            // alice changes the topic the next day
            event(4, "bob", Some("bob")),
            event(5, "carol", Some("carol")),
        ];
        // However long it takes, it's all the room's setup until someone else shows up.
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..4, 3, Some(2)), shape(4..6, 2, None)]);
    }

    #[test]
    fn creator_inviting_someone_ends_room_setup() {
        let items = vec![
            create(0, "alice"),
            event(1, "alice", Some("alice")),
            event(2, "alice", Some("bob")),     // alice invites bob
            event(3, "bob", Some("bob")),
        ];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..2, 2, None), shape(2..4, 2, None)]);
    }

    #[test]
    fn changes_to_who_can_join_only_go_in_the_rooms_setup() {
        let items = vec![
            create(0, "alice"),
            event(1, "alice", Some("alice")),
            access_change(2, "alice"),          // part of setting up the room
            event(3, "bob", Some("bob")),
            access_change(4, "alice"),          // like alice making the room public later: that's shown on its own
            event(5, "carol", Some("carol")),
            event(6, "dave", Some("dave")),
        ];
        assert_eq!(group_shapes(&rebuild_all(&items)), vec![shape(0..3, 3, None), shape(5..7, 2, None)]);

        // The items after the create event are only part of the setup while it's there, so replacing it regroups them.
        let mut items = vec![create(0, "alice"), access_change(1, "alice"), event(2, "alice", None), event(3, "bob", Some("bob"))];
        let mut groups = rebuild_all(&items);
        assert_eq!(group_shapes(&groups), vec![shape(0..3, 3, None)]);
        items[0] = event(0, "alice", Some("alice"));
        groups.rebuild(&items, 0..1, 0);
        assert_eq!(group_shapes(&groups), vec![shape(2..4, 2, None)]);
        items[0] = create(0, "alice");
        groups.rebuild(&items, 0..1, 0);
        assert_eq!(group_shapes(&groups), vec![shape(0..3, 3, None)]);
    }

    #[test]
    fn lookup_by_index() {
        let items = vec![Item::Message, state(1), state(2), Item::Message, state(4), state(5)];
        let groups = rebuild_all(&items);
        assert_eq!(groups.containing(0), None);
        assert_eq!(groups.containing(2).map(|g| g.range.clone()), Some(1..3));
        assert_eq!(groups.containing(3), None);
        assert_eq!(groups.containing(5).map(|g| g.range.clone()), Some(4..6));
        assert_eq!(groups.containing(1).unwrap().items_after_summary(), 2..3);
    }

    #[test]
    fn bounded_change_only_redoes_the_group_it_touches() {
        let items = vec![state(0), state(1), Item::Message, state(3), state(4), Item::Message, state(6), state(7)];
        let mut groups = rebuild_all(&items);
        // Something about item 3 changed (e.g. a read receipt), but the list is the same shape.
        let to_redraw = groups.rebuild(&items, 3..4, 0);
        assert_eq!(to_redraw, vec![3..4]);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None), shape(3..5, 2, None), shape(6..8, 2, None)]);
    }

    #[test]
    fn appended_event_groups_with_the_state_event_before_it() {
        let mut items = vec![state(0), state(1), Item::Message, state(3)];
        let mut groups = rebuild_all(&items);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None)]);
        items.push(Item::Hidden);
        items.push(state(5));
        let to_redraw = groups.rebuild(&items, 4..6, 0);
        assert_eq!(to_redraw, vec![3..4]);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None), shape(3..6, 2, None)]);
    }

    #[test]
    fn unbounded_change_rebuilds_everything_after_it() {
        let mut items = vec![state(0), state(1), Item::Message, state(3), state(4)];
        let mut groups = rebuild_all(&items);
        // A message got inserted at index 3, shifting everything after it.
        items.insert(3, Item::Message);
        let to_redraw = groups.rebuild(&items, 3..usize::MAX, 0);
        // The old group's summary item, then the new one's.
        assert_eq!(to_redraw, vec![3..4, 4..5]);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None), shape(4..6, 2, None)]);
        // And a shorter list can't keep stale groups around.
        let items = vec![state(0)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert_eq!(group_shapes(&groups), vec![]);
    }

    #[test]
    fn a_dissolved_group_still_gets_redrawn() {
        let mut items = vec![Item::Divider, state(1), Item::Divider, state(3), Item::Message];
        let mut groups = rebuild_all(&items);
        assert_eq!(groups.collapsed_span_end(&items, 0), Some(3));
        // The group's first event gets hidden (e.g. redacted into a no-op), leaving just one event.
        items[1] = Item::Hidden;
        let to_redraw = groups.rebuild(&items, 1..2, 0);
        assert_eq!(group_shapes(&groups), vec![]);
        // The divider above was showing the group's dates, so it needs a redraw.
        assert!(to_redraw.contains(&(0..1)));
    }

    #[test]
    fn an_answered_knock_joins_the_groups_around_it() {
        // A knock still waiting on an answer is shown on its own, so it splits up the state events around it.
        let mut items = vec![state(0), state(1), pending_knock(2, "alice"), state(3), state(4)];
        let mut groups = rebuild_all(&items);
        groups.toggle(3, &items);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None), shape(3..5, 2, None)]);
        // Its answer comes in at the end. Once it's answered, it can go in a group,
        // so it joins both groups into one, which the user left expanded.
        items.push(Item::Message);
        items[2] = flip_knock(&items[2]).unwrap();
        let mut to_redraw = groups.rebuild(&items, 5..6, 0);
        to_redraw.extend(groups.regroup_around(&items, &[2]));
        assert_eq!(group_shapes(&groups), vec![shape(0..5, 5, None)]);
        assert!(groups.containing(0).unwrap().is_expanded);
        assert!(to_redraw.contains(&(0..1)) && to_redraw.contains(&(3..4)));
        // If it goes back to waiting on an answer, it splits them up again, and both stay expanded.
        items[2] = flip_knock(&items[2]).unwrap();
        groups.regroup_around(&items, &[2]);
        assert_eq!(group_shapes(&groups), vec![shape(0..2, 2, None), shape(3..5, 2, None)]);
        assert!(groups.containing(0).unwrap().is_expanded && groups.containing(3).unwrap().is_expanded);
    }

    #[test]
    fn a_change_right_after_a_group_leaves_it_alone() {
        let items = vec![Item::Message, state(1), state(2), Item::Message, Item::Message];
        let mut groups = rebuild_all(&items);
        // e.g. a reaction on the message right after it
        assert!(groups.rebuild(&items, 3..4, 0).is_empty());
        // unlike a change to one of its events, which its summary sums up
        assert_eq!(groups.rebuild(&items, 2..3, 0), vec![1..2]);
        // still nothing to redraw once it's expanded
        groups.toggle(1, &items);
        assert!(groups.rebuild(&items, 3..4, 0).is_empty());
    }

    #[test]
    fn expanded_state_survives_rebuilds() {
        let items = vec![state(0), state(1), state(2), Item::Message, state(4), state(5)];
        let mut groups = rebuild_all(&items);
        assert!(groups.toggle(2, &items).is_some_and(|g| g.is_expanded && g.range == (0..3)));
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(groups.containing(0).unwrap().is_expanded);
        assert!(!groups.containing(4).unwrap().is_expanded);
        // Expanding via a jump is a no-op on an already expanded group.
        assert!(groups.expand_containing(1, &items).is_none());
        assert_eq!(groups.expand_containing(5, &items).map(|g| g.range.start), Some(4));
        assert!(groups.toggle(0, &items).is_some_and(|g| !g.is_expanded));
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(!groups.containing(0).unwrap().is_expanded);
        assert!(groups.containing(4).unwrap().is_expanded);
    }

    #[test]
    fn events_that_join_an_expanded_group_stay_expanded() {
        let mut items = vec![Item::Message, state(1), state(2)];
        let mut groups = rebuild_all(&items);
        groups.toggle(1, &items);
        items.push(state(3));
        groups.rebuild(&items, 3..4, 0);
        items.push(state(4));
        groups.rebuild(&items, 4..5, 0);
        // The timeline gets reset with only the events that joined after the group was expanded.
        let items = vec![state(3), state(4)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(groups.containing(0).unwrap().is_expanded);

        // Same for older events paginated in above the group, even after the group's original events
        // get hidden (e.g. redacted into no-ops).
        let items = vec![state(2), state(3)];
        let mut groups = rebuild_all(&items);
        groups.toggle(0, &items);
        let mut items = vec![state(0), state(1), state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        items[2] = Item::Hidden;
        items[3] = Item::Hidden;
        groups.rebuild(&items, 2..4, 0);
        assert!(groups.containing(0).unwrap().is_expanded);
    }

    #[test]
    fn collapsing_a_group_sticks_once_its_older_events_come_back() {
        // Expanded, then reset with only its newest events, then collapsed, then the rest paged back in.
        let mut items = vec![Item::Message, state(1), state(2)];
        let mut groups = rebuild_all(&items);
        groups.toggle(1, &items);
        items.push(state(3));
        groups.rebuild(&items, 3..4, 0);
        let items = vec![state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        let items = vec![state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(groups.containing(0).unwrap().is_expanded);
        groups.toggle(0, &items);
        let items = vec![state(1), state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(!groups.containing(0).unwrap().is_expanded);
        let items = vec![Item::Message, state(1), state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(!groups.containing(1).unwrap().is_expanded);

        // Same for two groups that got expanded separately and then merged.
        let mut items = vec![Item::Message, state(1), state(2), Item::Message, state(4), state(5)];
        let mut groups = rebuild_all(&items);
        groups.toggle(1, &items);
        groups.toggle(4, &items);
        items[3] = Item::Hidden;
        groups.rebuild(&items, 3..4, 0);
        assert_eq!(groups.containing(1).map(|g| (g.range.clone(), g.is_expanded)), Some((1..6, true)));
        let items = vec![state(4), state(5)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        groups.toggle(0, &items);
        let items = vec![Item::Message, state(1), state(2), Item::Hidden, state(4), state(5)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(!groups.containing(1).unwrap().is_expanded);
    }

    #[test]
    fn expanding_a_group_sticks_once_its_older_events_come_back() {
        // Expanded and collapsed again, then reset with only its newest events, expanded, then the rest paged back in.
        let items = vec![Item::Message, state(1), state(2)];
        let mut groups = rebuild_all(&items);
        groups.toggle(1, &items);
        groups.toggle(1, &items);
        let items = vec![state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(!groups.containing(0).unwrap().is_expanded);
        groups.toggle(0, &items);
        let items = vec![Item::Message, state(1), state(2), state(3)];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert!(groups.containing(1).unwrap().is_expanded);
    }

    #[test]
    fn older_state_events_can_move_a_groups_summary_item() {
        let items = vec![Item::Divider, state(2), state(3), Item::Message];
        let mut groups = rebuild_all(&items);
        assert_eq!(groups.summary_item_if_collapsed(1), None);
        assert_eq!(groups.summary_item_if_collapsed(2), Some(1));
        assert_eq!(groups.summary_item_if_collapsed(3), None);
        // Older state events get paginated in right above the group and join it,
        // so its old summary item (now at index 5) gets hidden too.
        let items = vec![Item::Divider, state(10), Item::Hidden, state(11), Item::Divider, state(2), state(3), Item::Message];
        groups.rebuild(&items, 0..usize::MAX, 0);
        assert_eq!(groups.summary_item_if_collapsed(1), None);
        assert_eq!(groups.summary_item_if_collapsed(2), Some(1));
        assert_eq!(groups.summary_item_if_collapsed(5), Some(1));
        assert_eq!(groups.summary_item_if_collapsed(7), None);
        // Nothing's hidden once it's expanded.
        groups.toggle(5, &items);
        assert_eq!(groups.summary_item_if_collapsed(5), None);
    }

    /// Returns the indices of the day dividers in `items` that have anything to show under them.
    fn shown_dividers(groups: &StateEventGroups, items: &Vec<Item>) -> Vec<usize> {
        (0..items.len())
            .filter(|&i| matches!(items[i], Item::Divider) && groups.day_shows_after(items, i))
            .collect()
    }

    #[test]
    fn a_collapsed_group_only_keeps_its_last_divider_if_that_day_goes_on() {
        // A group spanning 3 days, then the next day's message.
        let items = vec![
            Item::Divider, state(1), Item::Divider, state(3), Item::Divider, state(5),
            Item::Divider, Item::Message,
        ];
        let mut groups = rebuild_all(&items);
        assert_eq!(group_shapes(&groups), vec![shape(1..6, 3, Some(4))]);
        // Collapsed, only the divider above the summary shows: nothing else happened on the last day.
        assert_eq!(shown_dividers(&groups, &items), vec![0, 6]);
        groups.toggle(1, &items);
        assert_eq!(shown_dividers(&groups, &items), vec![0, 2, 4, 6]);

        // With a message on the group's last day after it, that day's divider stays.
        let items = vec![Item::Divider, state(1), Item::Divider, state(3), Item::Message];
        let groups = rebuild_all(&items);
        assert_eq!(shown_dividers(&groups, &items), vec![0, 2]);

        // Nor does it dangle at the end of the timeline, or over just the read marker.
        let items = vec![Item::Divider, state(1), Item::Divider, state(3)];
        assert_eq!(shown_dividers(&rebuild_all(&items), &items), vec![0]);
        let items = vec![Item::Divider, state(1), Item::Divider, state(3), Item::ReadMarker, Item::Divider, Item::Message];
        assert_eq!(shown_dividers(&rebuild_all(&items), &items), vec![0, 5]);
    }

    #[test]
    fn a_divider_over_something_else_first_keeps_its_own_date() {
        // A message comes before the group under this divider, which a range would mislabel.
        let items = vec![Item::Divider, Item::Message, state(2), Item::Divider, state(4), Item::Divider, Item::Message];
        let groups = rebuild_all(&items);
        assert_eq!(group_shapes(&groups), vec![shape(2..5, 2, Some(3))]);
        assert_eq!(groups.collapsed_span_end(&items, 0), None);
        // With nothing but the read marker before it, the group is still first under the divider.
        let items = vec![Item::Divider, Item::ReadMarker, state(2), Item::Divider, state(4)];
        assert_eq!(rebuild_all(&items).collapsed_span_end(&items, 0), Some(4));
    }

    #[test]
    fn days_with_nothing_visible_have_no_divider() {
        // A day of only hidden events (e.g. thread replies in the main timeline).
        let items = vec![Item::Divider, Item::Message, Item::Divider, Item::Hidden, Item::Hidden, Item::Divider, Item::Message];
        assert_eq!(shown_dividers(&rebuild_all(&items), &items), vec![0, 5]);
        // Hidden items or the read marker before something visible don't hide it.
        let items = vec![Item::Divider, Item::Hidden, Item::ReadMarker, Item::Message];
        assert_eq!(shown_dividers(&rebuild_all(&items), &items), vec![0]);
        // A collapsed group's summary item counts as something to show.
        let items = vec![Item::Divider, Item::Hidden, state(2), state(3)];
        assert_eq!(shown_dividers(&rebuild_all(&items), &items), vec![0]);
    }

    #[test]
    fn older_events_only_regroup_the_items_before_the_ones_already_there() {
        let mut items = vec![Item::Divider, state(1), state(2), Item::Message, state(4), state(5)];
        let mut groups = rebuild_all(&items);
        // Back pagination adds older events right after the top day divider, which changes to their day,
        // and the events that were there before get a divider of their own.
        items.splice(1..1, [state(10), state(11), Item::Message, Item::Divider]);
        groups.rebuild(&items, 0..5, 4);
        assert_eq!(groups.groups, rebuild_all(&items).groups);
        // The groups that were there before moved down, under their new divider.
        assert_eq!(groups.containing(8).map(|g| (g.range.clone(), g.preceding_divider)), Some((8..10, Some(4))));
    }

    #[test]
    fn where_events_show_up_in_their_groups() {
        use GroupPlacement::*;
        let items = vec![Item::Message, state(1), state(2), state(3), Item::Message, state(5), state(6)];
        let mut groups = rebuild_all(&items);
        // Both groups start out collapsed.
        assert_eq!([0, 1, 2, 3, 5, 6].map(|i| groups.placement_of(i)), [OnItsOwn, Summary, Hidden, Hidden, Summary, Hidden]);
        groups.toggle(1, &items);
        assert_eq!([1, 2, 3, 4].map(|i| groups.placement_of(i)), [UnderHeader, OnItsOwn, OnItsOwn, OnItsOwn]);
    }

    #[test]
    fn finding_the_collapsed_group_right_before_an_item() {
        let items = vec![Item::Divider, state(1), state(2), Item::Divider, Item::Hidden, Item::Message];
        let groups = rebuild_all(&items);
        assert_eq!(groups.collapsed_group_right_before(&items, 5), Some(1));
        assert_eq!(groups.collapsed_group_right_before(&items, 2), None);
        assert_eq!(groups.collapsed_group_right_before(&items, 0), None);
    }

    #[test]
    fn drawing_skips_over_collapsed_items() {
        // A collapsed group over two days, the second of which goes on after the group.
        let items = vec![Item::Divider, state(1), Item::Hidden, state(3), Item::Divider, state(5), state(6), Item::Message];
        let mut groups = rebuild_all(&items);
        assert_eq!(group_shapes(&groups), vec![shape(1..7, 4, Some(4))]);
        assert_eq!(groups.collapsed_ranges(), [2..4, 5..7]);
        let next = |i| groups.next_drawn_after(i, items.len());
        assert_eq!((next(0), next(1), next(4), next(5), next(6), next(7)), (1, 4, 7, 7, 7, 8));
        // Nothing gets skipped once it's expanded.
        groups.toggle(1, &items);
        assert!(groups.collapsed_ranges().is_empty());

        // That day doesn't go on past the group, so its divider gets skipped over too.
        let items = vec![Item::Divider, state(1), Item::Divider, state(3), Item::Divider, Item::Message];
        assert_eq!(rebuild_all(&items).collapsed_ranges(), vec![2..4]);

        // A group at the very end still gets its last item drawn.
        let items = vec![Item::Divider, state(1), state(2), state(3), state(4)];
        let groups = rebuild_all(&items);
        assert_eq!(groups.collapsed_ranges(), vec![2..5]);
        assert_eq!((groups.next_drawn_after(1, items.len()), groups.next_drawn_after(3, items.len())), (4, 4));
    }

    #[test]
    fn whats_collapsed_can_change_without_any_group_changing() {
        let mut items = vec![Item::Divider, state(1), Item::Divider, state(3), Item::ReadMarker, Item::Hidden];
        let mut groups = rebuild_all(&items);
        assert_eq!(groups.collapsed_ranges(), vec![2..4]);
        // Something shows up on the group's last day past the read marker, so that day's divider shows.
        items[5] = Item::Message;
        assert!(groups.rebuild(&items, 5..6, 0).is_empty());
        assert_eq!(groups.collapsed_ranges(), vec![3..4]);
    }

    #[test]
    fn whether_anything_shows_before_an_item() {
        let items = vec![Item::Divider, Item::Hidden, Item::ReadMarker, state(3), state(4), Item::Message];
        assert!(!shows_anything_before(&items, 0));
        assert!(!shows_anything_before(&items, 3));
        // The group's first item shows, as its summary item.
        assert!(shows_anything_before(&items, 4));
        assert!(shows_anything_before(&items, 6));
    }

    #[test]
    fn a_divider_gets_new_dates_when_whats_first_under_it_changes() {
        // The message under the divider gets hidden, leaving the collapsed group under the
        // read marker first under it, so the divider now shows that group's dates.
        let mut items = vec![Item::Divider, Item::Message, Item::ReadMarker, state(3), Item::Divider, state(5), Item::Message];
        let mut groups = rebuild_all(&items);
        assert_eq!(groups.collapsed_span_end(&items, 0), None);
        items[1] = Item::Hidden;
        let to_redraw = groups.rebuild(&items, 1..2, 0);
        assert_eq!(groups.collapsed_span_end(&items, 0), Some(5));
        assert!(to_redraw.contains(&(0..1)));
    }

    /// A tiny deterministic PRNG (xorshift64), so the randomized test below is reproducible.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    const PEOPLE: [&str; 3] = ["alice", "bob", "carol"];

    /// Returns a random event with a fresh ID.
    ///
    /// That's a message, a hidden event, a pending knock, or a state event by and about a few people
    /// (now and then a room's creation, or a change to who can join it).
    fn random_event(rng: &mut Rng, next_id: &mut usize) -> Item {
        *next_id += 1;
        let sender = PEOPLE[rng.below(PEOPLE.len())];
        match rng.below(10) {
            0 | 1 => Item::Message,
            2 => Item::Hidden,
            3 => create(*next_id, sender),
            4 => access_change(*next_id, sender),
            5 => pending_knock(*next_id, sender),
            _ => {
                let about = match rng.below(3) {
                    0 => None,
                    1 => Some(sender),
                    _ => Some(PEOPLE[rng.below(PEOPLE.len())]),
                };
                event(*next_id, sender, about)
            }
        }
    }

    fn random_item(rng: &mut Rng, next_id: &mut usize) -> Item {
        match rng.below(12) {
            0 | 1 => Item::Divider,
            2 => Item::ReadMarker,
            _ => random_event(rng, next_id),
        }
    }

    /// Applies a random diff to `items`, keeping track of what changed like the timeline subscriber in `sliding_sync.rs` does.
    ///
    /// That's widening `first..last`, and counting how many items at the end the diffs didn't touch.
    /// An insert or remove at index 0 clears the cache there, which counts as everything changing from index 0 on.
    fn random_diff(
        rng: &mut Rng,
        next_id: &mut usize,
        items: &mut Vec<Item>,
        first: &mut usize,
        last: &mut usize,
        num_unchanged_at_end: &mut usize,
    ) {
        let len = items.len();
        match rng.below(7) {
            // Set. The SDK only ever swaps a day divider for another one.
            0 | 1 if len > 0 => {
                let i = rng.below(len);
                let new_item = match items[i] {
                    Item::Divider => Item::Divider,
                    Item::ReadMarker => Item::ReadMarker,
                    _ => random_event(rng, next_id),
                };
                items[i] = new_item;
                *first = (*first).min(i);
                *last = (*last).max(i + 1);
                *num_unchanged_at_end = (*num_unchanged_at_end).min(len - i - 1);
            }
            // Append.
            2 => {
                *first = (*first).min(len);
                for _ in 0..=rng.below(3) {
                    items.push(random_item(rng, next_id));
                }
                *last = (*last).max(items.len());
                *num_unchanged_at_end = 0;
            }
            // Insert, which back pagination does at the start.
            3 if len > 0 => {
                let i = rng.below(len);
                items.insert(i, random_item(rng, next_id));
                *first = (*first).min(i);
                *last = usize::MAX;
                *num_unchanged_at_end = (*num_unchanged_at_end).min(len - i);
            }
            // Remove.
            4 if len > 0 => {
                let i = rng.below(len);
                items.remove(i);
                *first = (*first).min(i.saturating_sub(1));
                *last = usize::MAX;
                *num_unchanged_at_end = (*num_unchanged_at_end).min(len - i - 1);
            }
            // Truncate.
            5 if len > 1 => {
                let new_len = 1 + rng.below(len - 1);
                items.truncate(new_len);
                *first = (*first).min(new_len - 1);
                *last = usize::MAX;
                *num_unchanged_at_end = 0;
            }
            // PopBack.
            6 if len > 1 => {
                items.pop();
                *first = (*first).min(items.len());
                *last = usize::MAX;
                *num_unchanged_at_end = 0;
            }
            _ => {}
        }
    }

    /// Checks `groups` against what they should be for `items`, worked out the slow way.
    fn check_groups(groups: &StateEventGroups, items: &Vec<Item>) {
        let is_state = |i: usize| matches!(items[i], Item::State { .. });
        let is_divider = |i: usize| matches!(items[i], Item::Divider);
        // Whether the item at `i` comes while a room's being set up: after its creation,
        // with nothing since but the creator's own changes.
        let in_setup = |i: usize| {
            let Item::State { sender, .. } = &items[i] else { return false };
            (0..i).rev()
                .find_map(|j| match &items[j] {
                    Item::Message | Item::PendingKnock { .. } | Item::ReadMarker => Some(false),
                    Item::State { is_create: true, sender: creator, .. } => Some(creator == sender),
                    Item::State { sender: s, about, .. } if s != sender || about.as_ref().is_some_and(|a| a != sender) => Some(false),
                    _ => None,
                })
                .unwrap_or(false)
        };
        let is_access_change = |i: usize| matches!(items[i], Item::State { only_in_setup: true, .. });
        let breaks_groups = |i: usize| matches!(items[i], Item::Message | Item::PendingKnock { .. } | Item::ReadMarker) || (is_access_change(i) && !in_setup(i));
        let collapsed_into = |i: usize| groups.groups.iter()
            .find(|g| !g.is_expanded && g.range.start < i && i < g.range.end)
            .map(|g| g.range.start);

        let mut prev_end = 0;
        for g in &groups.groups {
            assert!(prev_end <= g.range.start && g.range.end <= items.len(), "{g:?} overlaps or goes past the end");
            prev_end = g.range.end;
            assert!(is_state(g.range.start) && is_state(g.range.end - 1), "{g:?} doesn't start and end with a state event");
            assert!(!g.range.clone().any(breaks_groups), "{g:?} spans a message, the read marker, or a change to who can join");
            assert_eq!(g.num_events, g.range.clone().filter(|&i| is_state(i)).count(), "{g:?}");
            assert!(g.num_events >= 2, "{g:?}");
            assert_eq!(g.last_divider, g.range.clone().rev().find(|&i| is_divider(i)), "{g:?}");
            assert_eq!(g.preceding_divider, (0..g.range.start).rev().find(|&i| is_divider(i)), "{g:?}");
            // Every event in a group remembers the group's latest choice, if any,
            // and that choice decides whether the group is expanded.
            let choices: Vec<_> = g.range.clone()
                .filter_map(|i| match &items[i] { Item::State { id, .. } => Some(groups.expand_choices.get(id).copied()), _ => None })
                .collect();
            assert!(choices.windows(2).all(|pair| pair[0] == pair[1]), "{g:?} has events with different choices: {choices:?}");
            assert_eq!(g.is_expanded, choices[0].is_some_and(|choice| choice.is_expanded), "{g:?}");
        }
        // Without a room's creation among them, all the state events between two items that break groups are one group.
        let mut after_prev_breaker = 0;
        for next_breaker in (0..=items.len()).filter(|&i| i == items.len() || breaks_groups(i)) {
            let states: Vec<usize> = (after_prev_breaker..next_breaker).filter(|&i| is_state(i)).collect();
            let has_create = states.iter().any(|&i| matches!(items[i], Item::State { is_create: true, .. }));
            if states.len() >= 2 && !has_create {
                let expected_group = states[0]..states[states.len() - 1] + 1;
                assert!(groups.groups.iter().any(|g| g.range == expected_group), "{expected_group:?} isn't one group");
            }
            after_prev_breaker = next_breaker + 1;
        }
        // A change to who can join only goes in a group as part of the room's setup.
        for i in (0..items.len()).filter(|&i| is_access_change(i)) {
            let in_setup_group = groups.containing(i).is_some_and(|g| matches!(items[g.range.start], Item::State { is_create: true, .. }));
            assert_eq!(in_setup_group, in_setup(i), "item {i}");
        }
        for i in 0..items.len() {
            assert_eq!(groups.summary_item_if_collapsed(i), collapsed_into(i), "item {i}");
            if !is_divider(i) {
                continue;
            }
            // A day shows if anything that isn't hidden in a collapsed group comes before the next day divider.
            let day_shows = (i + 1..items.len())
                .find_map(|j| match items[j] {
                    Item::Divider => Some(false),
                    Item::Message | Item::PendingKnock { .. } => Some(true),
                    Item::State { .. } if collapsed_into(j).is_none() => Some(true),
                    _ => None,
                })
                .unwrap_or(false);
            assert_eq!(groups.day_shows_after(items, i), day_shows, "divider {i}");
            let span_end = (i + 1..items.len())
                .find(|&j| !matches!(items[j], Item::Hidden | Item::ReadMarker))
                .and_then(|first| groups.groups.iter().find(|g| g.range.start == first))
                .filter(|g| !g.is_expanded && g.range.clone().any(is_divider))
                .map(|g| g.range.end - 1);
            assert_eq!(groups.collapsed_span_end(items, i), span_end, "divider {i}");
        }
        // Drawing jumps straight over whatever in a collapsed group doesn't show,
        // which is everything but its summary item and maybe its last day divider...
        let skipped = |j: usize| collapsed_into(j).is_some() && !(is_divider(j) && groups.day_shows_after(items, j));
        let collapsed = groups.collapsed_ranges();
        assert!(collapsed.iter().all(|range| !range.is_empty()) && collapsed.windows(2).all(|pair| pair[0].end < pair[1].start), "{collapsed:?}");
        assert_eq!(collapsed.iter().flat_map(|range| range.clone()).collect::<Vec<_>>(), (0..items.len()).filter(|&j| skipped(j)).collect::<Vec<_>>());
        for i in 0..items.len() {
            // ...except for the last item, which always gets drawn.
            let next = (i + 1..items.len()).find(|&j| j + 1 == items.len() || !skipped(j)).unwrap_or(i + 1);
            assert_eq!(groups.next_drawn_after(i, items.len()), next, "drawing down from {i}");
        }
    }

    #[test]
    fn incremental_rebuilds_match_full_ones() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut next_id = 0;
        for _ in 0..200 {
            let mut items: Vec<Item> = (0..rng.below(30)).map(|_| random_item(&mut rng, &mut next_id)).collect();
            let mut groups = rebuild_all(&items);
            check_groups(&groups, &items);
            for _ in 0..15 {
                // Now and then, expand or collapse something like a click would.
                if !items.is_empty() && rng.below(3) == 0 {
                    groups.toggle(rng.below(items.len()), &items);
                    check_groups(&groups, &items);
                }
                let old_groups = groups.groups.clone();
                let old_spans: Vec<_> = (0..items.len()).map(|i| groups.collapsed_span_end(&items, i)).collect();
                let old_len = items.len();
                let (mut first, mut last, mut num_unchanged_at_end) = (usize::MAX, 0, old_len);
                for _ in 0..=rng.below(3) {
                    random_diff(&mut rng, &mut next_id, &mut items, &mut first, &mut last, &mut num_unchanged_at_end);
                }
                if first == usize::MAX {
                    continue;
                }
                let changed = first..last;
                // Now and then, a knock outside of that change gets answered (or loses its answer)
                // because of it, which changes whether that knock can go in a group.
                let mut flipped = Vec::new();
                for _ in 0..rng.below(3) {
                    let Some(i) = (!items.is_empty()).then(|| rng.below(items.len())) else { break };
                    if !changed.contains(&i) && !flipped.contains(&i) && let Some(item) = flip_knock(&items[i]) {
                        items[i] = item;
                        flipped.push(i);
                    }
                }
                flipped.sort_unstable();
                // Like `process_timeline_updates()`, only regroup the items up to the ones at the end that the diffs didn't touch.
                let first_change = first.min(old_len).min(items.len());
                let num_unchanged_at_end = num_unchanged_at_end.min(old_len - first_change).min(items.len() - first_change);
                let len_change = items.len() as isize - old_len as isize;
                let regrouped = first_change..items.len() - num_unchanged_at_end;
                let mut to_redraw = groups.rebuild(&items, regrouped.clone(), len_change);
                to_redraw.extend(groups.regroup_around(&items, &flipped));
                // The old groups after the regrouped items are the same ones as before, just moved along with them.
                let old_groups: Vec<_> = old_groups.iter().map(|g| g.with_indices_shifted(len_change, old_len - num_unchanged_at_end)).collect();

                // Same as working it all out again (with the same events expanded)...
                let mut full = StateEventGroups { expand_choices: groups.expand_choices.clone(), ..Default::default() };
                full.rebuild(&items, 0..usize::MAX, 0);
                assert_eq!(groups.groups, full.groups, "after changing {changed:?} and flipping {flipped:?}");
                check_groups(&groups, &items);

                // ...and whatever looks different gets redrawn: the summary of a group that changed (or had
                // an event change), and the dates in the divider above a collapsed one that spans days.
                // Each flipped knock counts as a change of its own. The caller redraws the whole range the diffs
                // reported (which goes to the end once items move), but only the regrouped items really changed.
                let changes: Vec<Range<usize>> = std::iter::once(changed.clone()).chain(flipped.iter().map(|&i| i..i + 1)).collect();
                let real_changes: Vec<Range<usize>> = std::iter::once(regrouped.clone()).chain(flipped.iter().map(|&i| i..i + 1)).collect();
                let redrawn = |i: usize| changes.iter().any(|c| c.contains(&i)) || to_redraw.iter().any(|range| range.contains(&i));
                let overlaps_change = |g: &StateEventGroup| real_changes.iter().any(|c| g.range.start < c.end && c.start < g.range.end);
                let shows_dates = |g: &StateEventGroup| !g.is_expanded && g.last_divider.is_some();
                // A group that went under another day divider after the change is still the same group, since that divider
                // can't show its dates: a group only gets moved there if something visible comes between them.
                let still_there = |g: &StateEventGroup, others: &[StateEventGroup]| others.iter().any(|other|
                    StateEventGroup { preceding_divider: other.preceding_divider, ..g.clone() } == *other
                );
                for g in groups.groups.iter().filter(|&g| overlaps_change(g) || !still_there(g, &old_groups)) {
                    assert!(redrawn(g.range.start), "{g:?}'s summary after changing {changed:?} and flipping {flipped:?}");
                    assert!(!shows_dates(g) || g.preceding_divider.is_none_or(redrawn), "{g:?}'s dates after changing {changed:?} and flipping {flipped:?}");
                }
                // ...and nothing else: a group that came out the same only gets redrawn if one of its items
                // changed. If it's collapsed and spans days, a change between it and the divider above counts too.
                let depends_on_change = |g: &StateEventGroup| {
                    let first_item_it_depends_on = g.preceding_divider.filter(|_| shows_dates(g)).unwrap_or(g.range.start);
                    real_changes.iter().any(|c| first_item_it_depends_on < c.end && c.start < g.range.end)
                };
                for g in groups.groups.iter().filter(|&g| old_groups.contains(g) && !depends_on_change(g)) {
                    assert!(!to_redraw.iter().any(|range| range.contains(&g.range.start)), "{g:?} was redrawn for nothing after changing {changed:?} and flipping {flipped:?}");
                }
                for g in old_groups.iter().filter(|&g| shows_dates(g) && !still_there(g, &groups.groups)) {
                    assert!(g.preceding_divider.is_none_or(redrawn), "old {g:?}'s dates after changing {changed:?} and flipping {flipped:?}");
                }
                // ...and so does a divider whose date range changed. Only the indices before the
                // change still line up, unless it all changed in place.
                let num_unmoved = if changed.end < items.len() { items.len() } else { changed.start.min(items.len()) };
                for d in (0..num_unmoved).filter(|&d| matches!(items[d], Item::Divider)) {
                    assert!(old_spans[d] == groups.collapsed_span_end(&items, d) || redrawn(d), "divider {d}'s dates after changing {changed:?}");
                }
            }
        }
    }
}
