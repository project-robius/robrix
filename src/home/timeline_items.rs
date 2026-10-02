//! Decides what each timeline item gets drawn as: a message, a small state event,
//! a collapsed group's summary item, a day divider, the read marker, or nothing at all.
//! Also builds the text that small state events and day dividers show.

use std::{ops::Range, sync::Arc};

use chrono::{DateTime, Datelike, Local};
use hashbrown::{HashMap, HashSet};
use imbl::Vector;
use matrix_sdk_ui::timeline::{
    self, EncryptedMessage, EventTimelineItem, MemberProfileChange, MsgLikeContent, MsgLikeKind, OtherMessageLike, PollState, RoomMembershipChange, TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem,
};
use ruma::{MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedUserId, UserId, events::{StateEventContentChange, StateEventType, room::member::{MembershipState, RoomMemberEventContent}}};

use crate::{
    event_preview::{membership_and_reason, membership_of, membership_transition_of, plaintext_body_of_timeline_item, text_preview_of_encrypted_message, text_preview_of_member_profile_change, text_preview_of_membership_transition, text_preview_of_other_message_like, text_preview_of_other_state},
    home::{
        state_event_group::{GroupItems, GroupedItemInfo, GroupingKind, StateEventGroup, StateEventGroups},
        state_event_summary::Membership,
    },
    sliding_sync::TimelineKind,
    utils::unix_time_millis_to_datetime,
};

/// A timeline's items, plus what kind of timeline it is and its pending knocks.
///
/// These all affect how each item gets drawn (see [`item_draw()`]) and grouped.
#[derive(Clone, Copy)]
pub(super) struct TimelineInfo<'a> {
    pub(super) items: &'a Vector<Arc<TimelineItem>>,
    pub(super) kind: &'a TimelineKind,
    /// The knocks in `items` that are still waiting on an answer, which never go in a group.
    pub(super) pending_knocks: &'a PendingKnocks,
}

impl GroupItems for TimelineInfo<'_> {
    type Item = Arc<TimelineItem>;
    fn num_items(&self) -> usize { self.items.len() }
    fn get(&self, index: usize) -> Option<&Self::Item> { self.items.get(index) }
    fn iter_from(&self, start: usize) -> impl Iterator<Item = &Self::Item> {
        self.items.focus().narrow(start.min(self.items.len())..).into_iter()
    }
    fn grouping_kind<'a>(&'a self, item: &'a Self::Item) -> GroupingKind<'a> {
        item_display_kind(item, self.kind).grouping_kind(self.pending_knocks)
    }
}

/// What a timeline item gets drawn as, with its group (if any) taken into account.
pub(super) enum ItemDraw<'a> {
    /// Nothing: it's hidden on its own or in a collapsed group, or it's a day divider with nothing under it.
    Empty,
    /// The first item of a group, which draws the group's header (and its first event once expanded).
    GroupHead { group: &'a StateEventGroup, event: &'a EventTimelineItem, content: SmallStateContent<'a> },
    /// A small state event on its own row.
    ///
    /// The last one of an expanded group also shows a line with a button to collapse that group,
    /// and a knock still waiting on an answer shows a button to invite whoever knocked.
    SmallState { event: &'a EventTimelineItem, content: SmallStateContent<'a>, shows_collapse_line: bool, shows_invite_button: bool },
    Message(&'a EventTimelineItem, &'a MsgLikeContent),
    DateDivider(MilliSecondsSinceUnixEpoch),
    ReadMarker,
}

/// Decides how to draw the timeline item at `index`, or returns `None` if there isn't one.
pub(super) fn item_draw<'a>(
    timeline: TimelineInfo<'a>,
    groups: &'a StateEventGroups,
    index: usize,
) -> Option<ItemDraw<'a>> {
    let item = timeline.items.get(index)?;
    // Sometimes, a membership event needs to know what came before it in order to
    // determine the end result of that membership event.
    let display = item_display_kind(item, timeline.kind).with_history(timeline.items, index);
    let group = groups.containing(index);
    Some(match (group, display) {
        // If it's the first item (the header) of a small state events group, show the group's summary in it.
        (Some(group), ItemDisplayKind::SmallState(event, content)) if group.range.start == index => {
            ItemDraw::GroupHead { group, event, content }
        }
        // Everything else in a collapsed group is hidden, since the group's summary item shows instead,
        // except for day dividers, which are handled below.
        (Some(group), display) if !group.is_expanded && !matches!(display, ItemDisplayKind::DateDivider(_)) => ItemDraw::Empty,
        (_, ItemDisplayKind::Message(event, content)) => ItemDraw::Message(event, content),
        (group, ItemDisplayKind::SmallState(event, content)) => ItemDraw::SmallState {
            event,
            content,
            // The last event of an expanded group should also display another line with a collapse button.
            shows_collapse_line: group.is_some_and(|g| g.is_expanded && index + 1 == g.range.end),
            shows_invite_button: timeline.pending_knocks.contains(event, &content),
        },
        // If there is a date divider with nothing to show within it (because it's hidden or part of a collapsed group),
        // then it doesn't need to be shown at all.
        (_, ItemDisplayKind::DateDivider(_)) if !groups.day_shows_after(&timeline, index) => ItemDraw::Empty,
        (_, ItemDisplayKind::DateDivider(millis)) => ItemDraw::DateDivider(millis),
        (_, ItemDisplayKind::ReadMarker) => ItemDraw::ReadMarker,
        (_, ItemDisplayKind::Hidden | ItemDisplayKind::TimelineStart) => ItemDraw::Empty,
    })
}

/// The knocks in a timeline that are still waiting on an answer.
///
/// This is a map keyed by the user ID who knocked
#[derive(Default)]
pub(super) struct PendingKnocks(HashMap<OwnedUserId, OwnedEventId>);

impl PendingKnocks {
    /// Works out which knocks in the given items are still waiting on an answer.
    pub(super) fn new(items: &Vector<Arc<TimelineItem>>) -> Self {
        Self(pending_knocks_in(items))
    }

    /// Returns whether the given event is a knock that's still waiting on an answer.
    fn contains(&self, event_tl_item: &EventTimelineItem, content: &SmallStateContent) -> bool {
        let SmallStateContent::Membership(change, _) = content else { return false };
        self.0.get(change.user_id()).is_some_and(|knock| event_tl_item.event_id() == Some(&**knock))
    }

    /// Catches up after the items in `changed` went from `old_items` to `new_items`.
    ///
    /// Returns the indices of knocks that just got an answer (or lost one), which need to be
    /// regrouped (only answered knocks can go in a group) and redrawn (to show or hide their invite button).
    pub(super) fn update(
        &mut self,
        old_items: &Vector<Arc<TimelineItem>>,
        new_items: &Vector<Arc<TimelineItem>>,
        changed: Range<usize>,
    ) -> Vec<usize> {
        // Only a member event about someone who's knocking, or a new knock, can change anything.
        let affects_knocks = items_in(old_items, &changed)
            .chain(items_in(new_items, &changed))
            .filter_map(|item| member_event_about(item))
            .any(|(user_id, is_knock)| is_knock || self.0.contains_key(user_id));
        if !affects_knocks {
            return Vec::new();
        }
        let now_pending = pending_knocks_in(new_items);
        let flipped = new_items.iter().enumerate()
            .filter_map(|(index, item)| {
                let (user_id, _) = member_event_about(item)?;
                let event_id = item.as_event()?.event_id()?;
                let was_pending = self.0.get(user_id).is_some_and(|knock| &**knock == event_id);
                let is_pending = now_pending.get(user_id).is_some_and(|knock| &**knock == event_id);
                (was_pending != is_pending).then_some(index)
            })
            .collect();
        self.0 = now_pending;
        flipped
    }
}

/// Returns which knocks in the given items are still waiting on an answer.
///
/// A knock is still waiting if it's the latest member event about the person who knocked.
/// That means nobody invited them or rejected their know, and they haven't given up or knocked again.
fn pending_knocks_in(items: &Vector<Arc<TimelineItem>>) -> HashMap<OwnedUserId, OwnedEventId> {
    let mut seen = HashSet::new();
    let mut pending = HashMap::new();
    for item in items.iter().rev() {
        let Some((user_id, is_knock)) = member_event_about(item) else { continue };
        if seen.insert(user_id)
            && is_knock
            && let Some(event_id) = item.as_event().and_then(|event| event.event_id())
        {
            pending.insert(user_id.to_owned(), event_id.to_owned());
        }
    }
    pending
}

/// Returns who the given item is about and whether it's a knock, if it's a member event.
fn member_event_about(item: &TimelineItem) -> Option<(&UserId, bool)> {
    match item.as_event()?.content() {
        TimelineItemContent::MembershipChange(change) => {
            Some((change.user_id(), *membership_and_reason(change).0 == MembershipState::Knock))
        }
        // The SDK couldn't parse this one, but we can still extract who it's about.
        TimelineItemContent::FailedToParseState { event_type: StateEventType::RoomMember, state_key, .. } => {
            Some((<&UserId>::try_from(state_key.as_str()).ok()?, false))
        }
        _ => None,
    }
}

/// Iterates over the given items within `range`, ignoring any part of `range` past the last item.
fn items_in<'a>(items: &'a Vector<Arc<TimelineItem>>, range: &Range<usize>) -> impl Iterator<Item = &'a Arc<TimelineItem>> {
    items.focus().narrow(range.start.min(items.len())..range.end.min(items.len())).into_iter()
}

/// Returns when the collapsed group right under the day divider at `index` ends, if it spans multiple days.
///
/// This lets the divider show the group's whole date range.
pub(super) fn divider_span_end(timeline: TimelineInfo, groups: &StateEventGroups, index: usize) -> Option<MilliSecondsSinceUnixEpoch> {
    let end = groups.collapsed_span_end(&timeline, index)?;
    Some(timeline.items.get(end)?.as_event()?.timestamp())
}

/// Returns the text of a day divider, like "Sun Sep 5, 2021".
///
/// If `span_end` falls on a later day than `millis`, the text becomes a date range instead,
/// like "Fri Sep 25 – Mon Sep 28, 2026".
pub(super) fn date_divider_text(millis: MilliSecondsSinceUnixEpoch, span_end: Option<MilliSecondsSinceUnixEpoch>) -> String {
    let Some(start) = unix_time_millis_to_datetime(millis) else { return format!("{millis:?}") };
    match later_day(start, span_end.and_then(unix_time_millis_to_datetime)) {
        Some(end) if end.year() == start.year() => {
            format!("{} – {}", start.format("%a %b %-d"), end.format("%a %b %-d, %Y"))
        }
        Some(end) => format!("{} – {}", start.format("%a %b %-d, %Y"), end.format("%a %b %-d, %Y")),
        None => start.format("%a %b %-d, %Y").to_string(),
    }
}

/// Returns `end` if it falls within a later day than `start`.
pub(super) fn later_day(start: DateTime<Local>, end: Option<DateTime<Local>>) -> Option<DateTime<Local>> {
    end.filter(|end| end.date_naive() > start.date_naive())
}

/// The possible kinds of timeline items that can get drawn in a timeline.
#[derive(Clone, Copy)]
pub(super) enum ItemDisplayKind<'a> {
    /// A message, sticker, or redacted message.
    Message(&'a EventTimelineItem, &'a MsgLikeContent),
    /// Anything drawn as a small one-line event (like state changes).
    SmallState(&'a EventTimelineItem, SmallStateContent<'a>),
    /// An item that exists but shouldn't be shown at all.
    ///
    /// These are drawn as zero-height items via the `ZeroHeightItem` widget.
    Hidden,
    DateDivider(MilliSecondsSinceUnixEpoch),
    ReadMarker,
    TimelineStart,
}

impl<'a> ItemDisplayKind<'a> {
    /// Returns how this item takes part in a group of state events.
    fn grouping_kind(&self, pending_knocks: &PendingKnocks) -> GroupingKind<'a> {
        match self {
            Self::SmallState(event_tl_item, content) if content.is_groupable(event_tl_item, pending_knocks) => GroupingKind::GroupedItem(GroupedItemInfo {
                event_id: event_tl_item.event_id(),
                sender: event_tl_item.sender(),
                user_about: match content {
                    SmallStateContent::Membership(change, _) => Some(change.user_id()),
                    SmallStateContent::Profile(change) => Some(change.user_id()),
                    _ => None,
                },
                is_room_create: matches!(content, SmallStateContent::OtherState(other) if is_room_create(other)),
                only_grouped_in_room_setup: matches!(content, SmallStateContent::OtherState(other) if changes_who_can_join_or_read(other)),
            }),
            Self::Hidden => GroupingKind::Neutral,
            Self::DateDivider(_) => GroupingKind::Divider,
            Self::ReadMarker | Self::TimelineStart => GroupingKind::Marker,
            Self::Message(..) | Self::SmallState(..) => GroupingKind::Breaker,
        }
    }

    /// Returns this item, plus where it is in `items` if it's a membership event.
    ///
    /// If a membership event doesn't say what it did (e.g., it got redacted), we can work that out
    /// by looking at what came before it in the timeline (see [`previous_membership()`]).
    pub(super) fn with_history(self, items: &'a Vector<Arc<TimelineItem>>, index: usize) -> Self {
        match self {
            Self::SmallState(event_tl_item, SmallStateContent::Membership(change, _)) => Self::SmallState(
                event_tl_item,
                SmallStateContent::Membership(change, Some(TimelineHistory { items, index })),
            ),
            other => other,
        }
    }
}

/// Returns whether the given state event is the room's creation event.
fn is_room_create(other: &timeline::OtherState) -> bool {
    matches!(other.content(), timeline::AnyOtherStateEventContentChange::RoomCreate(_))
}

/// Returns whether the given state event changes who can join the room or read its history.
///
/// Everyone should see a change like that, and what it changed to. So it gets its own row,
/// unless it's part of the room's setup.
fn changes_who_can_join_or_read(other: &timeline::OtherState) -> bool {
    use timeline::AnyOtherStateEventContentChange as C;
    matches!(other.content(), C::RoomJoinRules(_) | C::RoomHistoryVisibility(_) | C::RoomGuestAccess(_))
}

/// Where an item is in its timeline, for looking back at what came before it.
#[derive(Clone, Copy)]
pub(super) struct TimelineHistory<'a> {
    items: &'a Vector<Arc<TimelineItem>>,
    index: usize,
}

/// How far back to look for a user's previous membership.
const MAX_ITEMS_TO_SEARCH_FOR_MEMBERSHIP: usize = 500;

impl TimelineHistory<'_> {
    /// Returns the given user's membership right before this item.
    ///
    /// This goes by the latest loaded event before it that shows what that membership was.
    fn membership_before(self, user_id: &UserId) -> Option<Membership> {
        self.items.focus()
            .narrow(..self.index.min(self.items.len()))
            .into_iter()
            .rev()
            .take(MAX_ITEMS_TO_SEARCH_FOR_MEMBERSHIP)
            .find_map(|item| {
                let event = item.as_event()?;
                match event.content() {
                    TimelineItemContent::MembershipChange(change) if change.user_id() == user_id => {
                        Some(Some(membership_of(membership_and_reason(change).0)))
                    }
                    // Profiles only change while joined.
                    TimelineItemContent::ProfileChange(change) if change.user_id() == user_id => Some(Some(Membership::Join)),
                    // One the SDK couldn't parse; the membership itself is usually still fine.
                    TimelineItemContent::FailedToParseState { event_type: StateEventType::RoomMember, state_key, .. }
                        if state_key == user_id.as_str() => Some(membership_in_raw_member_event(event)),
                    // Back to the room's creation without seeing them, so they'd never been in it.
                    // (Not the timeline start though, which can just be as far back as we can see.)
                    TimelineItemContent::OtherState(other) if is_room_create(other) => Some(Some(Membership::Leave)),
                    // Only members can send anything else.
                    _ if event.sender() == user_id => Some(Some(Membership::Join)),
                    _ => None,
                }
            })
            .flatten()
    }
}

/// Returns the user's membership right before the given membership event.
///
/// This is for when the SDK can't tell what the event did (e.g., it arrived redacted).
/// The event itself may still say, and if it doesn't, the timeline before it might.
pub(super) fn previous_membership(
    event_tl_item: &EventTimelineItem,
    change: &RoomMembershipChange,
    history: Option<TimelineHistory>,
) -> Option<Membership> {
    #[derive(serde::Deserialize)]
    struct Unsigned {
        prev_content: Option<MemberContent>,
    }
    // The SDK drops this for a redacted event, but the raw event may still have it.
    let from_event = event_tl_item.original_json()
        .and_then(|json| json.get_field::<Unsigned>("unsigned").ok().flatten())
        .and_then(|unsigned| unsigned.prev_content)
        .map(|prev| membership_of(&prev.membership));
    from_event.or_else(|| history?.membership_before(change.user_id()))
}

/// Just the membership of a member event's content.
#[derive(serde::Deserialize)]
struct MemberContent {
    membership: MembershipState,
}

/// Returns the membership in a member event that the SDK couldn't parse, straight from its raw JSON.
fn membership_in_raw_member_event(event_tl_item: &EventTimelineItem) -> Option<Membership> {
    let content = event_tl_item.original_json()?.get_field::<MemberContent>("content").ok()??;
    Some(membership_of(&content.membership))
}

/// Decides how the given timeline item should be drawn, including whether it's hidden.
pub(super) fn item_display_kind<'a>(item: &'a TimelineItem, timeline_kind: &TimelineKind) -> ItemDisplayKind<'a> {
    let event_tl_item = match item.kind() {
        TimelineItemKind::Event(event_tl_item) => event_tl_item,
        TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(millis)) => return ItemDisplayKind::DateDivider(*millis),
        TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => return ItemDisplayKind::ReadMarker,
        TimelineItemKind::Virtual(VirtualTimelineItem::TimelineStart) => return ItemDisplayKind::TimelineStart,
    };
    let content = match event_tl_item.content() {
        TimelineItemContent::MsgLike(msg_like_content) => {
            // Hide threaded replies from the main room timeline UI.
            if timeline_kind.thread_root_event_id().is_none() && msg_like_content.thread_root.is_some() {
                return ItemDisplayKind::Hidden;
            }
            match &msg_like_content.kind {
                MsgLikeKind::Message(_)
                | MsgLikeKind::Sticker(_)
                | MsgLikeKind::Redacted => return ItemDisplayKind::Message(event_tl_item, msg_like_content),
                // TODO: properly implement `Poll` as a regular Message-like timeline item.
                MsgLikeKind::Poll(poll_state) => SmallStateContent::Poll(poll_state),
                MsgLikeKind::UnableToDecrypt(utd) => SmallStateContent::UnableToDecrypt(utd),
                MsgLikeKind::LiveLocation(_) => SmallStateContent::LiveLocation,
                MsgLikeKind::Other(other) => SmallStateContent::OtherMessageLike(other),
            }
        }
        TimelineItemContent::MembershipChange(membership_change) => {
            // Hide timeline entries that show duplicate join/leaves.
            let by_self = event_tl_item.sender() == membership_change.user_id();
            if is_no_op_membership_change(membership_change.change(), membership_change.content(), by_self) {
                return ItemDisplayKind::Hidden;
            }
            SmallStateContent::Membership(membership_change, None)
        }
        TimelineItemContent::ProfileChange(profile_change) => SmallStateContent::Profile(profile_change),
        TimelineItemContent::OtherState(other) => {
            // Don't show noisy updates like policy rules, server ACLs, space links, custom state events, etc.
            // We could always make this configurable, e.g., some kind of dev mode.
            let should_hide = matches!(
                other.content(),
                timeline::AnyOtherStateEventContentChange::PolicyRuleRoom(_)
                | timeline::AnyOtherStateEventContentChange::PolicyRuleServer(_)
                | timeline::AnyOtherStateEventContentChange::PolicyRuleUser(_)
                | timeline::AnyOtherStateEventContentChange::RoomServerAcl(_)
                | timeline::AnyOtherStateEventContentChange::SpaceChild(_)
                | timeline::AnyOtherStateEventContentChange::SpaceParent(_)
                | timeline::AnyOtherStateEventContentChange::_Custom { .. }
            );
            if should_hide {
                return ItemDisplayKind::Hidden;
            }
            SmallStateContent::OtherState(other)
        }
        _unhandled => SmallStateContent::Unhandled,
    };
    ItemDisplayKind::SmallState(event_tl_item, content)
}

/// Returns whether a member event surely changed nothing (like a duplicate join).
///
/// An event like that gets hidden. This only goes by the event itself, so the timeline
/// and its groups always agree on whether it's hidden.
fn is_no_op_membership_change(
    change: Option<timeline::MembershipChange>,
    content: &StateEventContentChange<RoomMemberEventContent>,
    by_self: bool,
) -> bool {
    if change != Some(timeline::MembershipChange::None) {
        return false;
    }
    match content {
        StateEventContentChange::Original { content, prev_content } => match content.membership {
            MembershipState::Join => true,
            MembershipState::Leave => !by_self
                && content.reason.is_none()
                && prev_content.as_ref().is_some_and(|prev| prev.membership == MembershipState::Leave),
            _ => false,
        },
        StateEventContentChange::Redacted(content) => content.membership == MembershipState::Join,
    }
}


/// The kinds of content that get drawn with the `SmallStateEvent` widget.
#[derive(Clone, Copy)]
pub(super) enum SmallStateContent<'a> {
    Poll(&'a PollState),
    UnableToDecrypt(&'a EncryptedMessage),
    LiveLocation,
    OtherMessageLike(&'a OtherMessageLike),
    /// A membership change, plus where it is in the timeline if known.
    ///
    /// See [`ItemDisplayKind::with_history()`].
    Membership(&'a RoomMembershipChange, Option<TimelineHistory<'a>>),
    Profile(&'a MemberProfileChange),
    OtherState(&'a timeline::OtherState),
    /// Anything else (calls, unparseable events, ...), shown as its plaintext body.
    Unhandled,
}

impl SmallStateContent<'_> {
    /// Returns whether this event can go in a group.
    ///
    /// Only membership, profile and other state changes can, since the user shouldn't lose sight of
    /// the rest, like undecryptable messages or unparseable events. But a knock still waiting on an
    /// answer can't, since a collapsed group would hide its invite button. And changes to who can join
    /// or read history only get grouped with the room's setup (see [`changes_who_can_join_or_read()`]).
    fn is_groupable(&self, event_tl_item: &EventTimelineItem, pending_knocks: &PendingKnocks) -> bool {
        match self {
            Self::Membership(..) => !pending_knocks.contains(event_tl_item, self),
            Self::Profile(_) | Self::OtherState(_) => true,
            Self::Poll(_)
            | Self::UnableToDecrypt(_)
            | Self::LiveLocation
            | Self::OtherMessageLike(_)
            | Self::Unhandled => false,
        }
    }

    /// Returns the text describing this event, where `username` is the sender's name.
    pub(super) fn text(&self, event_tl_item: &EventTimelineItem, username: &str) -> String {
        match self {
            // TODO: once we properly display polls, we should remove this,
            //       because polls shouldn't be displayed using the SmallStateEvent widget.
            Self::Poll(poll) => poll.fallback_text().unwrap_or_else(|| poll.results().question),
            Self::UnableToDecrypt(encrypted) => text_preview_of_encrypted_message(encrypted).format_with(username, false),
            Self::LiveLocation => format!("{username} shared a live location."),
            Self::OtherMessageLike(other) => text_preview_of_other_message_like(other).format_with(username, false),
            Self::Membership(change, history) => {
                let sender = event_tl_item.sender();
                let transition = membership_transition_of(change, sender, || previous_membership(event_tl_item, change, *history));
                text_preview_of_membership_transition(change, sender, transition, false).format_with(username, false)
            }
            Self::Profile(change) => text_preview_of_member_profile_change(change, username, false).format_with(username, false),
            Self::OtherState(other) => text_preview_of_other_state(other, false).format_with(username, false),
            Self::Unhandled => plaintext_body_of_timeline_item(event_tl_item),
        }
    }
}
