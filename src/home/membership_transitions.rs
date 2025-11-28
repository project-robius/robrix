use std::{collections::HashMap, ops::Range, sync::Arc};
use matrix_sdk::ruma::{OwnedEventId, OwnedUserId};
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, MsgLikeContent, MsgLikeKind, TimelineItem, TimelineItemContent, TimelineItemKind};
use ruma::events::FullStateEventContent;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionType {
    CreateRoom,
    ConfigureRoom,
    Joined,
    Left,
    JoinedAndLeft,
    LeftAndJoined,
    InviteReject,
    InviteWithdrawal,
    Invited,
    Banned,
    Unbanned,
    Kicked,
    ChangedName,
    ChangedAvatar,
    #[default]
    NoChange,
    ServerAcl,
    ChangedPins,
    MessageRemoved,
    UnableToDecrypt,
    HiddenEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserEvent {    
    pub transition: TransitionType,
    pub index: usize,
    pub is_redacted: bool,
    pub should_show: bool,
    pub sender: String,
    pub display_name: String,
    pub state_key: Option<String>,
    pub membership: Option<String>,
}

#[derive(Default, Debug)]
pub struct CreationCollapsibleList {
    pub range: std::ops::Range<usize>,
    pub opened: bool,
    pub username: String,
}

/// Canonicalize neighbouring transitions, e.g. [Joined, Left] -> [JoinedAndLeft]
fn canonicalize_transitions(transitions: &[TransitionType]) -> Vec<TransitionType> {
    let mut res = Vec::new();
    let mut i = 0;
    while i < transitions.len() {
        let t = transitions[i];
        if i + 1 < transitions.len() {
            let t2 = transitions[i + 1];
            match (t, t2) {
                (TransitionType::Joined, TransitionType::Left) => {
                    res.push(TransitionType::JoinedAndLeft);
                    i += 2;
                    continue;
                }
                (TransitionType::Left, TransitionType::Joined) => {
                    res.push(TransitionType::LeftAndJoined);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        res.push(t);
        i += 1;
    }
    res
}

/// Combine repeated transitions (e.g. [JoinedAndLeft, JoinedAndLeft] -> 2×JoinedAndLeft)
fn coalesce_repeated_transitions(transitions: &[TransitionType]) -> Vec<(TransitionType, usize)> {
    let mut res = Vec::new();
    for &t in transitions {
        if let Some((last, count)) = res.last_mut() {
            if *last == t {
                *count += 1;
                continue;
            }
        }
        res.push((t, 1));
    }
    res
}

/// Determine the textual description for a transition
fn describe_transition(t: TransitionType, user_count: usize, repeats: usize) -> String {
    let plural = user_count > 1;
    match t {
        TransitionType::CreateRoom => "created and configured the room".to_string(),
        TransitionType::ConfigureRoom => "".to_string(),
        TransitionType::Joined => {
            if repeats > 1 {
                if plural { format!("joined (×{})", repeats) } else { format!("joined the room (×{})", repeats) }
            } else {
                if plural { "joined".to_string() } else { "joined the room".to_string() }
            }
        }
        TransitionType::Left => {
            if repeats > 1 {
                if plural { format!("left (×{})", repeats) } else { format!("left the room (×{})", repeats) }
            } else {
                if plural { "left".to_string() } else { "left the room".to_string() }
            }
        }
        TransitionType::JoinedAndLeft => if repeats > 1 { format!("joined and left (×{})", repeats) } else { "joined and left".to_string() },
        TransitionType::LeftAndJoined => if repeats > 1 { format!("left and rejoined (×{})", repeats) } else { "left and rejoined".to_string() },
        TransitionType::ChangedName => if repeats > 1 { format!("changed their name (×{})", repeats) } else { "changed their name".to_string() },
        TransitionType::ChangedAvatar => if repeats > 1 { format!("changed their profile picture (×{})", repeats) } else { "changed their profile picture".to_string() },
        TransitionType::Invited => if repeats > 1 { format!("was invited (×{})", repeats) } else { "was invited".to_string() },
        TransitionType::Banned => if repeats > 1 { format!("was banned (×{})", repeats) } else { "was banned".to_string() },
        TransitionType::Unbanned => if repeats > 1 { format!("was unbanned (×{})", repeats) } else { "was unbanned".to_string() },
        TransitionType::InviteReject => if repeats > 1 { format!("rejected invite (×{})", repeats) } else { "rejected invite".to_string() },
        TransitionType::InviteWithdrawal => if repeats > 1 { format!("invite withdrawn (×{})", repeats) } else { "invite withdrawn".to_string() },
        TransitionType::Kicked => if repeats > 1 { format!("was kicked (×{})", repeats) } else { "was kicked".to_string() },
        TransitionType::ServerAcl => if repeats > 1 { format!("updated server ACLs (×{})", repeats) } else { "updated server ACLs".to_string() },
        TransitionType::ChangedPins => if repeats > 1 { format!("changed pinned messages (×{})", repeats) } else { "changed pinned messages".to_string() },
        TransitionType::MessageRemoved => if repeats > 1 { format!("removed a message (×{})", repeats) } else { "removed a message".to_string() },
        TransitionType::HiddenEvent => if repeats > 1 { format!("did a hidden event (×{})", repeats) } else { "did a hidden event".to_string() },
        TransitionType::NoChange => if repeats > 1 { format!("made no changes (×{})", repeats) } else { "made no changes".to_string() },
        TransitionType::UnableToDecrypt => if repeats > 1 { format!("decryption failed (×{})", repeats) } else { "decryption failed".to_string() },
    }
}

/// Produce an English-readable name list, with "and N others"
fn format_name_list(names: &[String], max_names: usize) -> String {
    match names.len() {
        0 => "".into(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        n if n <= max_names => names.join(", "),
        n => format!("{}, and {} others", names[..max_names].join(", "), n - max_names),
    }
}

pub fn convert_to_timeline_event(event_item: EventTimelineItem, user_name: String, index: usize) -> UserEvent {
    use matrix_sdk_ui::timeline::{TimelineItemContent, MembershipChange};
    let (state_key, membership) = match event_item.content() {
        TimelineItemContent::MembershipChange(change) => {
            let state_key = Some(change.user_id().to_string());
            let membership = match change.change() {
                Some(MembershipChange::Joined) => Some("join"),
                Some(MembershipChange::Left) => Some("leave"),
                Some(MembershipChange::Invited) => Some("invite"),
                Some(MembershipChange::Banned) => Some("ban"),
                _ => None,
            }.map(|s| s.to_string());
            (state_key, membership)
        }
        TimelineItemContent::OtherState(other) => {
            (Some(other.state_key().to_string()), None)
        }
        _ => (None, None),
    };
    let (_is_small_state, transistion_type, _display_name_from_state) = is_small_state_event(&event_item);
    UserEvent {
        index,
        display_name: user_name,
        transition: transistion_type,
        is_redacted: matches!(event_item.content(), 
            TimelineItemContent::MsgLike(msg) if matches!(msg.kind, matrix_sdk_ui::timeline::MsgLikeKind::Redacted)
        ),
        should_show: true, // For now, assume all events should be shown
        sender: event_item.sender().to_string(),
        state_key,
        membership,
    }
}

/// Convert Matrix membership change → transition type
pub fn get_transition_from_membership_change(change: &MembershipChange) -> TransitionType {
    use matrix_sdk_ui::timeline::MembershipChange;
    match change {
        MembershipChange::Joined => TransitionType::Joined,
        MembershipChange::Left => TransitionType::Left,
        MembershipChange::Banned => TransitionType::Banned,
        MembershipChange::Unbanned => TransitionType::Unbanned,
        MembershipChange::Kicked => TransitionType::Kicked,
        MembershipChange::Invited => TransitionType::Invited,
        MembershipChange::KickedAndBanned => TransitionType::Banned,
        MembershipChange::InvitationAccepted => TransitionType::Joined,
        MembershipChange::InvitationRejected => TransitionType::InviteReject,
        MembershipChange::InvitationRevoked => TransitionType::InviteWithdrawal,
        MembershipChange::Knocked => TransitionType::HiddenEvent,
        MembershipChange::KnockAccepted => TransitionType::Joined,
        MembershipChange::KnockRetracted => TransitionType::HiddenEvent,
        MembershipChange::None => TransitionType::NoChange,
        MembershipChange::Error => TransitionType::NoChange,
        MembershipChange::NotImplemented => TransitionType::NoChange,
        MembershipChange::KnockDenied => TransitionType::HiddenEvent,
    }
}

/// Convert other room state events → transition type
pub fn get_transition_from_other_events(content: &AnyOtherFullStateEventContent, _state_key: &str) -> TransitionType {
    match content {
        AnyOtherFullStateEventContent::RoomServerAcl(_) => TransitionType::ServerAcl,
        AnyOtherFullStateEventContent::RoomPinnedEvents(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomName(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomTopic(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomAvatar(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomCanonicalAlias(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomCreate(_) => TransitionType::CreateRoom,
        AnyOtherFullStateEventContent::RoomEncryption(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomGuestAccess(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomHistoryVisibility(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomJoinRules(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomThirdPartyInvite(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomTombstone(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::RoomAliases(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::SpaceChild(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::SpaceParent(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::PolicyRuleRoom(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::PolicyRuleServer(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::PolicyRuleUser(_) => TransitionType::ConfigureRoom,
        AnyOtherFullStateEventContent::_Custom { .. } => TransitionType::HiddenEvent,
    }
}

/// Appends a new user event to the given list of user events.
///
/// If the given transition is `HiddenEvent`, this function does nothing.
///
/// Otherwise, it appends a new `UserEvent` to the list of user events for the given user ID.
/// If the user ID is not found in the list, a new entry is created.
///
/// If the user ID is found, but there is no existing `UserEvent` with the same index,
/// a new `UserEvent` is appended to the list of user events for that user ID.
///
/// The function prints debug messages to help with debugging.
pub fn append_user_event(user_event: UserEvent, user_events: &mut Vec<(String, Vec<UserEvent>)>) {
    if let TransitionType::HiddenEvent = user_event.transition {
        return;
    }
    if let Some((_, events)) = user_events.iter_mut().find(|(id, _)| id == &user_event.sender) {
        if events.iter().filter(|inner_user_event| inner_user_event.index == user_event.index).count() == 0 {
            events.push(user_event);
        }
    } else {
        user_events.push((user_event.sender.clone(), vec![user_event]));
    }
}

/// Appends a new user event to the given HashMap of user events.
/// This version handles the HashMap structure used in room_screen.rs
fn append_user_event_to_map(user_event: UserEvent, user_events: &mut HashMap<usize, (String, Vec<UserEvent>)>) {
    let item_id = user_event.index;
    let mut old_group_id = None;
    let user_state_key = user_event.state_key.clone().unwrap_or_default();
    let user_state_key = if user_state_key.is_empty() {
        user_event.sender.clone()
    } else {
        user_state_key
    };
    for (group_id, (user_id, user_events_vec)) in user_events.iter_mut() {
        if user_state_key == *user_id {
            if user_events_vec.iter().filter(|user_event| user_event.index == item_id).count() == 0 {
                user_events_vec.push(user_event.clone());
            }
            if &item_id >= group_id {
                return;
            }
            old_group_id = Some(group_id.clone());
        }
    }
    
    
    if let Some(old_group_id) = old_group_id {
        if let Some(data) = user_events.remove(&old_group_id) {
            user_events.insert(item_id, data);
            return;
        }
    }
    user_events.insert(item_id, (user_state_key.clone(), vec![user_event]));
}

/// Checks if a event timeline item is a small state event.
/// Returns true if the item is a small state event that can be grouped.
/// return Display name
pub fn is_small_state_event(
    event_tl_item: &EventTimelineItem,
) -> (bool, TransitionType, Option<String>) {
    match event_tl_item.content() {
        TimelineItemContent::MembershipChange(change) => (true, change.change().map(
            |f| get_transition_from_membership_change(&f)
            ).unwrap_or_default(), change.display_name()
        ),
        TimelineItemContent::ProfileChange(change) => {
            let transition_type = if let Some(_) = change.avatar_url_change() {
                TransitionType::ChangedAvatar
            } else if let Some(_) = change.displayname_change() {
                TransitionType::ChangedName
            } else {
                TransitionType::NoChange
            };
            (true, transition_type, None)
        }
        TimelineItemContent::OtherState(other_state) => {
            (true, get_transition_from_other_events(other_state.content(), other_state.state_key()), None)
        },
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::Poll(_),
            ..
        }) => (true, TransitionType::NoChange, None),
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::Redacted,
            ..
        }) => (true, TransitionType::MessageRemoved, None),
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::UnableToDecrypt(_),
            ..
        }) => (true, TransitionType::UnableToDecrypt, None),
        _ => {
            (false, TransitionType::NoChange, None)
        }
    }
}

/// Summarize all user transitions into a single string
pub fn generate_summary(
    user_events: &HashMap<usize, (String, Vec<UserEvent>)>,
    summary_length: usize,
) -> String {
    // Aggregate by transition sequence
    let mut aggregates: Vec<(Vec<TransitionType>, Vec<String>)> = Vec::new();
    // Sort keys in ascending order for consistent iteration
    let mut sorted_keys: Vec<&usize> = user_events.keys().collect();
    sorted_keys.sort();
    
    for &index in sorted_keys {
        let (user_id, events) = &user_events[&index];
        let mut events = events.clone();
        events.sort_by_key(|e|e.index);
        let mut transitions: Vec<_> = events.iter().map(|e| e.transition).collect();

        // Filter out Joined transitions for room creators
        if transitions.contains(&TransitionType::CreateRoom) {
            transitions.retain(|&t| t != TransitionType::Joined);
        }
        let canonical = canonicalize_transitions(&transitions);
        let name = events.first().map(|e| e.display_name.clone()).unwrap_or(user_id.clone());
        if let Some((_, names)) = aggregates.iter_mut().find(|(seq, _)| seq == &canonical) {
            names.push(name);
        } else {
            aggregates.push((canonical, vec![name]));
        }
    }
    // Build text
    let mut summary_parts = Vec::new();
    for (canonical, names) in aggregates {
        let coalesced = coalesce_repeated_transitions(&canonical);

        let descs: Vec<String> = coalesced
            .into_iter()
            .map(|(t, repeats)| describe_transition(t, names.len(), repeats))
            .collect();
        let name_list = format_name_list(&names, summary_length);
        let transition_text = descs.join(", ");
        summary_parts.push(format!("{} {}", name_list, transition_text));
    }

    summary_parts.join(", ")
}

/// Dynamically updates small state groups as timeline items are processed.
/// This function is called during populate_small_state_event to build groups on-demand.
/// Since iteration starts from the biggest item_id and goes backwards, we handle reverse grouping.
/// Returns a tuple whether to display the message, and whether to display the collapsible button and whether collapsible list is expanded and range to redraw
pub fn update_small_state_groups_for_item(
    item_id: usize,
    username: String,
    current_item: &EventTimelineItem,
    previous_item: Option<&Arc<TimelineItem>>,
    next_item: Option<&Arc<TimelineItem>>,
    room_creation: &mut Option<(OwnedUserId, OwnedEventId)>,
    small_state_groups: &mut Vec<(std::ops::Range<usize>, bool, HashMap<usize, (String, Vec<UserEvent>)>)>,
    creation_collapsible_list: &mut CreationCollapsibleList,
) -> (bool, bool, bool, Range<usize>) { //opened, show_collapsible_button, expanded, redraw_range
    let (current_item_is_small_state, _transition, display_name) = is_small_state_event(current_item);
    if !current_item_is_small_state {
        return (true, false, false, Range::default()); // Not a small state event, draw as individual item, no debug button
    }

    // check if the next item (item_id + 1) is a small state event to continue grouping
    let (next_item_is_small_state, _, _) = next_item
        .and_then(|timeline_item| match timeline_item.kind() {
            TimelineItemKind::Event(event_tl_item) => Some(event_tl_item),
            _ => None,
        })
        .map(is_small_state_event)
        .unwrap_or((false, TransitionType::NoChange, None));
    let (previous_item_is_small_state, _, _) = previous_item
        .and_then(|timeline_item| match timeline_item.kind() {
            TimelineItemKind::Event(event_tl_item) => Some(event_tl_item),
            _ => None,
        })
        .map(is_small_state_event)
        .unwrap_or((false, TransitionType::NoChange, None));
    if !previous_item_is_small_state && !next_item_is_small_state {
        return (true, false, false,  Range::default()); // Isolated small state event, no debug button
    }
    if room_creation.is_none() {
        if let TimelineItemContent::OtherState(other_state) = current_item.content() {
            if let AnyOtherFullStateEventContent::RoomCreate(FullStateEventContent::Original { .. }) = other_state.content() {
                let creator_id = current_item.sender().to_owned();
                if let Some(event_id) = current_item.event_id() {
                    *room_creation = Some((creator_id, event_id.to_owned()));
                    creation_collapsible_list.range.start = item_id;
                    if creation_collapsible_list.range.end <= item_id {
                        creation_collapsible_list.range.end = item_id + 1;
                    }
                    return (true, true, creation_collapsible_list.opened, item_id..item_id + 1);
                }                
            }
        }
    }
    creation_collapsible_list.username = username.clone();
    let user_event = convert_to_timeline_event(current_item.clone(), display_name.unwrap_or(username), item_id);
    if item_id == creation_collapsible_list.range.start && creation_collapsible_list.range.len() > 0 {
        return (true, true, creation_collapsible_list.opened, creation_collapsible_list.range.clone());
    }
    if matches!(user_event.transition, 
        TransitionType::ConfigureRoom | TransitionType::Joined
    ) {
        if creation_collapsible_list.range.len()== 0 {
            return (true, false, true, Range::default());
        }
        if creation_collapsible_list.range.end == item_id {
            creation_collapsible_list.range.end = item_id + 1;
            if matches!(user_event.transition, 
                TransitionType::ConfigureRoom | TransitionType::Joined
            ) {
                return (creation_collapsible_list.opened, false, creation_collapsible_list.opened, item_id..item_id + 2);
            } else {
                return (creation_collapsible_list.opened, false, creation_collapsible_list.opened, item_id..item_id + 1);
            }
        }
        if creation_collapsible_list.range.contains(&item_id) {
            return (creation_collapsible_list.opened, false, creation_collapsible_list.opened, Range::default());
        }
    }
    // Check if this item is already part of an existing group or can extend one
    let group_keys: Vec<Range<usize>> = small_state_groups.iter().map(|f| f.0.clone()).collect();
    'outer: for (range, is_open, user_events_map) in small_state_groups.iter_mut() {
        if range.start == item_id {
            // Add user event to the HashMap using item_id as key with userId and Vec<UserEvent>
            append_user_event_to_map(user_event, user_events_map);
            return (true, range.len() > 2, *is_open, Range::default()); // Start of group, show debug button
        }
        if range.contains(&item_id) {
            // Add user event to the HashMap using item_id as key with userId and Vec<UserEvent>
            append_user_event_to_map(user_event, user_events_map);
            return (*is_open || range.len() <= 2, false, *is_open,  Range::default()); // Item is in group but not at start, no debug button
        }
        
        // Since we're iterating backwards (from highest to lowest item_id),
        if range.start == item_id + 1 {
            for r in group_keys.iter() {
                if r.contains(&item_id) {
                    continue 'outer;
                }
            }
            // Extend this group backwards to include current item
            *range = item_id..range.end;
            append_user_event_to_map(user_event, user_events_map);
            return (*is_open || range.len() <= 2, false, false,  range.clone()); // Extended group, no debug button for this item
        }
    }

    if next_item_is_small_state {
        let mut user_events_map = HashMap::new();
        append_user_event_to_map(user_event, &mut user_events_map);
        // Plus 2 to include the next item into the group.        
        small_state_groups.push((item_id..(item_id + 2), false, user_events_map));
        return (false, false, false, item_id..(item_id + 2));
    }
    (false, false, false,  Range::default()) // Return collapsed state, no debug button
}

#[cfg(test)]
mod tests {
    use super::*;

    //#[test]
    // fn test_generate_summary() {
    //     let mut user_events: HashMap<usize, (String, Vec<UserEvent>)> = HashMap::new();
        
    //     user_events.insert(0, ("alice".into(), vec![
    //         UserEvent { display_name: "Alice".into(), transition: TransitionType::Joined, index: 0, state_key: None },
    //         UserEvent { display_name: "Alice".into(), transition: TransitionType::Left, index: 1, state_key: None },
    //     ]));
        
    //     user_events.insert(2, ("bob".into(), vec![
    //         UserEvent { display_name: "Bob".into(), transition: TransitionType::ChangedAvatar, index: 2, state_key: None },
    //     ]));
        
    //     user_events.insert(3, ("charlie".into(), vec![
    //         UserEvent { display_name: "Charlie".into(), transition: TransitionType::Joined, index: 3, state_key: None },
    //     ]));

    //     let summary = generate_summary(&user_events, 2);
    //     assert_eq!(summary, "Alice joined and left, Bob changed their profile picture, Charlie joined the room");
    // }
}