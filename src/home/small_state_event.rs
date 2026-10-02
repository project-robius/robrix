//! Populates small state events in the timeline, plus the summary items of state event groups.

use hashbrown::HashMap;
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk_ui::timeline::{self, EventTimelineItem, MemberProfileChange, Profile, TimelineDetails};
use ruma::{OwnedRoomId, OwnedUserId, UserId, events::StateEventContentChange};

use crate::{
    event_preview::{is_email_invite_revocation, membership_transition_of},
    home::{
        room_read_receipt::{AvatarRowWidgetRefExt, populate_read_receipts},
        room_screen::ItemDrawnStatus,
        state_event_group::{AvatarStackWidgetRefExt, GroupToggleLineWidgetRefExt, SmallStateEventWidgetRefExt, StackedUser, StateEventGroup, StateEventGroupHeaderWidgetRefExt},
        state_event_summary::{MembershipTransition, ProfileChange, RoomChange, RoomSetting, StateChange, SummaryEntry, Who, summarize},
        timeline_items::{ItemDisplayKind, SmallStateContent, TimelineInfo, item_display_kind, end_if_later_day, previous_membership},
    },
    profile::user_profile_cache::{self, RoomMemberEntry},
    shared::{avatar::AvatarWidgetRefExt, timestamp::TimestampWidgetRefExt},
    sliding_sync::TimelineKind,
    utils::{self, unix_time_millis_to_datetime},
};

/// How many participant avatars a summary item shows; everyone else is only in the text.
const MAX_STACKED_AVATARS: usize = 3;

/// Creates, populates, and adds a group's summary item (a `GroupSummaryItem` widget) at `item_id`.
///
/// `item_id` must be the first item of `group`.
/// While collapsed, the widget sums up every event in the group.
/// Once expanded, it shows the group's first event instead,
/// because the rest are each drawn as their own timeline items.
pub(super) fn populate_group_summary_item(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    timeline: TimelineInfo,
    group: &StateEventGroup,
    first_event_tl_item: &EventTimelineItem,
    first_event_content: &SmallStateContent,
    item_drawn_status: ItemDrawnStatus,
) -> (WidgetRef, ItemDrawnStatus) {
    let TimelineInfo { items, kind: timeline_kind, .. } = timeline;
    let room_id = timeline_kind.room_id();
    let (item, existed) = list.item_with_existed(cx, item_id, id!(GroupSummaryItem));
    let content_already_drawn = existed && item_drawn_status.content_drawn;
    if content_already_drawn && item_drawn_status.profile_drawn {
        return (item, item_drawn_status);
    }
    let header = item.state_event_group_header(cx, ids!(header));
    // If some of the summary's names were still loading when we built it, and we still don't have
    // anything better to show for any of them, just skip rebuilding it.
    // Some names may never load, so we don't want to endlessly rebuild the summary on every draw.
    if content_already_drawn && !group.is_expanded && header.names_still_pending(|user_id| known_display_name(cx, room_id, user_id)) {
        return (item, item_drawn_status);
    }
    header.set_expanded(cx, group.is_expanded);

    // The group's first event is technically a part of this item,
    // but we only actually show it when the group is expanded.
    // Its cached drawn status is only correct if it hasn't been expanded/collapsed since.
    let first_event = item.small_state_event(cx, ids!(first_event));
    let first_event_was_shown = existed && first_event.visible();
    first_event.set_shown(group.is_expanded);
    if group.is_expanded {
        // The summary is hidden while expanded, so there's no point building it.
        populate_read_receipts(&first_event, cx, timeline_kind, first_event_tl_item);
        let (_, first_event_status) = populate_small_state_event_widget(
            cx,
            (*first_event).clone(),
            first_event_was_shown,
            timeline_kind,
            first_event_tl_item,
            first_event_content,
            item_drawn_status,
            false,
            false,
        );
        return (item, first_event_status);
    }

    // Now for the summary, we need to distill every event in the group down to
    // a short blurb about who did what.
    // The summary also shows the read receipts of everything collapsed within it.
    let mut entries = Vec::with_capacity(group.num_events);
    let mut participants: Vec<StackedUser> = Vec::with_capacity(MAX_STACKED_AVATARS);
    let mut read_receipts = IndexMap::new();
    let mut invitee_by_token = HashMap::new();
    for (offset, tl_item) in items.focus().narrow(group.range.clone()).into_iter().enumerate() {
        let display_kind = item_display_kind(tl_item, timeline_kind).with_history(items, group.range.start + offset);
        let ItemDisplayKind::SmallState(event_tl_item, content) = display_kind else { continue };
        let Some((entry, user_id)) = summary_entry_of(event_tl_item, &content, &mut invitee_by_token) else { continue };
        entries.push(entry);
        // Only the first few relevant users get an avatar.
        if let Some(user_id) = user_id
            && participants.len() < MAX_STACKED_AVATARS
            && !participants.iter().any(|participant| participant.user_id == user_id)
        {
            participants.push(StackedUser { profile: profile_in(event_tl_item, &content), user_id });
        }
        read_receipts.extend(event_tl_item.read_receipts().iter().map(|(u, r)| (u.clone(), r.clone())));
    }

    // Only look up names for the users that will actually get shown in the summary.
    let mut pending_names = Vec::new();
    let summary = summarize(&entries, |who| {
        let Who::User(user_id) = who else { return None };
        let user_id = <&UserId>::try_from(user_id.as_str()).ok()?;
        let (name, pending) = known_display_name(cx, room_id, user_id);
        if pending {
            pending_names.push((user_id.to_owned(), name.clone()));
        }
        name
    });
    let no_names_pending = pending_names.is_empty();
    header.set_summary(cx, &summary, pending_names);
    header.avatar_stack(cx, ids!(participants)).set_users(cx, timeline_kind, &participants);
    if let Some(start) = unix_time_millis_to_datetime(first_event_tl_item.timestamp()) {
        // If a collapsed group spans multiple days, we include that date range in its tooltip.
        let end = items.get(group.range.end - 1).and_then(
            |item| unix_time_millis_to_datetime(item.as_event()?.timestamp())
        );
        header.timestamp(cx, ids!(timestamp)).set_date_time_span(cx, start, end_if_later_day(start, end));
    }
    header.avatar_row(cx, ids!(avatar_row)).set_avatar_row(cx, timeline_kind, &read_receipts);
    (item, ItemDrawnStatus { profile_drawn: no_names_pending, content_drawn: true })
}

/// Returns the profile that the given event shows for the user it's about, if it shows one.
///
/// A membership event has its user's name and avatar (or the ones it replaced), unless it got redacted,
/// and a profile change has the new avatar if that changed. Other events go by their sender's profile.
fn profile_in(event_tl_item: &EventTimelineItem, content: &SmallStateContent) -> Option<Profile> {
    let sender_profile = || match event_tl_item.sender_profile() {
        TimelineDetails::Ready(profile) => Some(profile.clone()),
        _ => None,
    };
    match content {
        SmallStateContent::Membership(change, _) => matches!(change.content(), StateEventContentChange::Original { .. })
            .then(|| Profile { display_name: change.display_name(), display_name_ambiguous: false, avatar_url: change.avatar_url() }),
        SmallStateContent::Profile(change) => match change.avatar_url_change() {
            Some(avatar) => Some(Profile {
                display_name: change.displayname_change().and_then(|name| name.new.clone())
                    .or_else(|| sender_profile().and_then(|profile| profile.display_name)),
                display_name_ambiguous: false,
                avatar_url: avatar.new.clone(),
            }),
            None => sender_profile(),
        },
        _ => sender_profile(),
    }
}

/// Creates a summary part for the given single state event.
///
/// Returns `None` if this isn't a state event (a membership, profile, or other state change).
fn summary_entry_of(
    event_tl_item: &EventTimelineItem,
    content: &SmallStateContent,
    invitee_by_token: &mut HashMap<String, String>,
) -> Option<(SummaryEntry, Option<OwnedUserId>)> {
    let sender = event_tl_item.sender();
    let sender_name = || utils::non_blank(get_profile_display_name(event_tl_item));
    let about_user = |user_id: &UserId, name: Option<String>, change: StateChange| (
        SummaryEntry { who: Who::User(user_id.to_string()), name, change },
        Some(user_id.to_owned()),
    );
    let about_invitee = |invitee: String, name: Option<String>, transition: MembershipTransition| (
        SummaryEntry { who: Who::EmailInvitee(invitee), name, change: StateChange::Membership(transition) },
        None,
    );
    Some(match content {
        SmallStateContent::Membership(change, history) => {
            let target = change.user_id();
            let name = utils::non_blank(change.display_name())
                .or_else(|| if target == sender { sender_name() } else { None });
            let transition = membership_transition_of(change, sender, || previous_membership(event_tl_item, change, *history));
            about_user(target, name, StateChange::Membership(transition))
        }
        SmallStateContent::Profile(change) => {
            let name = change.displayname_change()
                .and_then(|c| utils::non_blank(c.new.clone()).or_else(|| utils::non_blank(c.old.clone())))
                .or_else(sender_name);
            about_user(change.user_id(), name, StateChange::Profile(profile_change_of(change)))
        }
        SmallStateContent::OtherState(other) => match other.content() {
            timeline::AnyOtherStateEventContentChange::RoomThirdPartyInvite(StateEventContentChange::Original { content, prev_content }) => {
                let token = other.state_key();
                if !is_email_invite_revocation(content) {
                    let name = utils::non_blank(Some(content.display_name.clone()));
                    let invitee = name.clone().unwrap_or_default();
                    invitee_by_token.insert(token.to_owned(), invitee.clone());
                    about_invitee(invitee, name, MembershipTransition::Invited)
                } else if let Some(invitee) = invitee_by_token.get(token) {
                    about_invitee(invitee.clone(), None, MembershipTransition::InvitationRevoked)
                } else if let Some(name) = prev_content.as_ref().and_then(|prev| utils::non_blank(Some(prev.display_name.clone()))) {
                    about_invitee(name.clone(), Some(name), MembershipTransition::InvitationRevoked)
                } else {
                    about_user(sender, sender_name(), StateChange::Room(room_change_of(other)))
                }
            }
            _ => about_user(sender, sender_name(), StateChange::Room(room_change_of(other))),
        },
        _ => return None,
    })
}

/// Returns what the given profile change did.
fn profile_change_of(change: &MemberProfileChange) -> ProfileChange {
    // Setting an empty display name is the same as removing it.
    let name_set = change.displayname_change()
        .map(|c| c.new.as_deref().is_some_and(|name| !name.trim().is_empty()));
    let avatar_set = change.avatar_url_change().map(|c| c.new.is_some());
    match (name_set, avatar_set) {
        (Some(_), Some(_)) => ProfileChange::NameAndAvatar,
        (Some(true), None) => ProfileChange::Name,
        (Some(false), None) => ProfileChange::RemovedName,
        (None, Some(true)) => ProfileChange::Avatar,
        (None, Some(false)) => ProfileChange::RemovedAvatar,
        (None, None) => ProfileChange::Unknown,
    }
}

/// Returns what the given change to the room's own state did.
fn room_change_of(other: &timeline::OtherState) -> RoomChange {
    use timeline::AnyOtherStateEventContentChange as SEC;
    match other.content() {
        SEC::RoomCreate(_) => RoomChange::Created,
        SEC::RoomTombstone(_) => RoomChange::Upgraded,
        SEC::RoomEncryption(_) => RoomChange::EnabledEncryption,
        SEC::RoomThirdPartyInvite(StateEventContentChange::Original { content, .. })
            if is_email_invite_revocation(content) => RoomChange::RevokedEmailInvite,
        SEC::RoomThirdPartyInvite(_) => RoomChange::EmailInvite,
        SEC::RoomName(_) => RoomChange::Setting(RoomSetting::Name),
        SEC::RoomTopic(_) => RoomChange::Setting(RoomSetting::Topic),
        SEC::RoomAvatar(_) => RoomChange::Setting(RoomSetting::Avatar),
        SEC::RoomCanonicalAlias(_) => RoomChange::Setting(RoomSetting::Address),
        SEC::RoomGuestAccess(_) => RoomChange::Setting(RoomSetting::GuestAccess),
        SEC::RoomHistoryVisibility(_) => RoomChange::Setting(RoomSetting::HistoryVisibility),
        SEC::RoomJoinRules(_) => RoomChange::Setting(RoomSetting::JoinRules),
        SEC::RoomPinnedEvents(_) => RoomChange::Setting(RoomSetting::PinnedMessages),
        SEC::RoomPowerLevels(_) => RoomChange::Setting(RoomSetting::PowerLevels),
        _ => RoomChange::Setting(RoomSetting::Other),
    }
}

/// Returns the given user's display name in this room, or the global one if we know it.
///
/// The returned bool is true if a better name for them is still being fetched.
fn known_display_name(cx: &mut Cx, room_id: &OwnedRoomId, user_id: &UserId) -> (Option<String>, bool) {
    user_profile_cache::with_user_profile(cx, user_id.to_owned(), Some(room_id), true, |profile, rooms| {
        match rooms.get(room_id) {
            // Their name in this room is the one that should always be used, even if they don't have one.
            Some(RoomMemberEntry::Loaded(member)) => (utils::non_blank(member.display_name().map(ToOwned::to_owned)), false),
            Some(RoomMemberEntry::Requested) => (utils::non_blank(profile.username.clone()), true),
            _ => (utils::non_blank(profile.username.clone()), false),
        }
    })
    .unwrap_or((None, true))
}


/// Creates, populates, and adds a `SmallStateEvent` widget to the given `PortalList` at the given `item_id`.
pub(super) fn populate_small_state_event(
    cx: &mut Cx,
    list: &mut PortalList,
    item_id: usize,
    timeline_kind: &TimelineKind,
    event_tl_item: &EventTimelineItem,
    event_content: &SmallStateContent,
    item_drawn_status: ItemDrawnStatus,
    show_collapse_line: bool,
    show_invite_button: bool,
) -> (WidgetRef, ItemDrawnStatus) {
    let (item, existed) = list.item_with_existed(cx, item_id, id!(SmallStateEvent));
    populate_read_receipts(&item, cx, timeline_kind, event_tl_item);
    populate_small_state_event_widget(
        cx, item, existed, timeline_kind, event_tl_item, event_content, item_drawn_status, show_collapse_line, show_invite_button,
    )
}

/// Populates the given `SmallStateEvent` widget with the event's profile and content, but not its read receipts.
///
/// ## Arguments
/// * `existed`: whether this widget was already showing this event.
/// * `show_collapse_line`: true for the last event of an expanded group, so it can collapse the group.
/// * `show_invite_button`: true for a knock that's still waiting on an answer.
fn populate_small_state_event_widget(
    cx: &mut Cx,
    item: WidgetRef,
    existed: bool,
    timeline_kind: &TimelineKind,
    event_tl_item: &EventTimelineItem,
    event_content: &SmallStateContent,
    item_drawn_status: ItemDrawnStatus,
    show_collapse_line: bool,
    show_invite_button: bool,
) -> (WidgetRef, ItemDrawnStatus) {
    let mut new_drawn_status = item_drawn_status;
    // The text content of a small state event may depend on the profile info,
    // so we can only mark the content as drawn after the profile has been fully drawn and cached.
    let skip_redrawing_profile = existed && item_drawn_status.profile_drawn;
    let skip_redrawing_content = skip_redrawing_profile && item_drawn_status.content_drawn;
    // This one's cheap and depends on the group, not the event, so always keep it up to date.
    item.group_toggle_line(cx, ids!(collapse_line)).set_shown(show_collapse_line);
    if skip_redrawing_content {
        return (item, new_drawn_status);
    }

    // If the profile has been drawn, we can just quickly grab the user's display name
    // instead of having to call `set_avatar_and_get_username` again.
    let username_opt = skip_redrawing_profile
        .then(|| get_profile_display_name(event_tl_item))
        .flatten();

    let username = username_opt.unwrap_or_else(|| {
        // As a fallback, call `set_avatar_and_get_username` to get the user's display name.
        let avatar_ref = item.avatar(cx, ids!(avatar));

        let (username, profile_drawn) = avatar_ref.set_avatar_and_get_username(
            cx,
            timeline_kind,
            event_tl_item.sender(),
            Some(event_tl_item.sender_profile()),
            event_tl_item.event_id(),
            true,
        );
        // Draw the timestamp as part of the profile.
        if let Some(dt) = unix_time_millis_to_datetime(event_tl_item.timestamp()) {
            item.timestamp(cx, ids!(left_container.timestamp)).set_date_time(cx, dt);
        }
        new_drawn_status.profile_drawn = profile_drawn;
        username
    });

    // Only a knock still waiting on an answer should show the invite button.
    // See the `PendingKnocks` struct for more details on how that gets updated.
    item.button(cx, ids!(invite_user_button)).set_visible(cx, show_invite_button);
    item.label(cx, ids!(content)).set_text(cx, &event_content.text(event_tl_item, &username));
    new_drawn_status.content_drawn = true;
    (item, new_drawn_status)
}

/// Returns the display name of the sender of the given `event_tl_item`, if available.
fn get_profile_display_name(event_tl_item: &EventTimelineItem) -> Option<String> {
    if let TimelineDetails::Ready(profile) = event_tl_item.sender_profile() {
        profile.display_name.clone()
    } else {
        None
    }
}
