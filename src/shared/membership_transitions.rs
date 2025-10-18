use std::collections::HashMap;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, EventTimelineItem, MembershipChange, MsgLikeContent, MsgLikeKind, TimelineItemContent};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionType {
    CreateRoom,
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

impl TransitionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            TransitionType::CreateRoom => "CreateRoom",
            TransitionType::Joined => "Joined",
            TransitionType::Left => "Left",
            TransitionType::JoinedAndLeft => "JoinedAndLeft",
            TransitionType::LeftAndJoined => "LeftAndJoined",
            TransitionType::InviteReject => "InviteReject",
            TransitionType::InviteWithdrawal => "InviteWithdrawal",
            TransitionType::Invited => "Invited",
            TransitionType::Banned => "Banned",
            TransitionType::Unbanned => "Unbanned",
            TransitionType::Kicked => "Kicked",
            TransitionType::ChangedName => "ChangedName",
            TransitionType::ChangedAvatar => "ChangedAvatar",
            TransitionType::NoChange => "NoChange",
            TransitionType::ServerAcl => "ServerAcl",
            TransitionType::ChangedPins => "ChangedPins",
            TransitionType::MessageRemoved => "MessageRemoved",
            TransitionType::UnableToDecrypt => "UnableToDecrypt",
            TransitionType::HiddenEvent => "HiddenEvent",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "CreateRoom" => Some(TransitionType::CreateRoom),
            "Joined" => Some(TransitionType::Joined),
            "Left" => Some(TransitionType::Left),
            "JoinedAndLeft" => Some(TransitionType::JoinedAndLeft),
            "LeftAndJoined" => Some(TransitionType::LeftAndJoined),
            "InviteReject" => Some(TransitionType::InviteReject),
            "InviteWithdrawal" => Some(TransitionType::InviteWithdrawal),
            "Invited" => Some(TransitionType::Invited),
            "Banned" => Some(TransitionType::Banned),
            "Unbanned" => Some(TransitionType::Unbanned),
            "Kicked" => Some(TransitionType::Kicked),
            "ChangedName" => Some(TransitionType::ChangedName),
            "ChangedAvatar" => Some(TransitionType::ChangedAvatar),
            "NoChange" => Some(TransitionType::NoChange),
            "ServerAcl" => Some(TransitionType::ServerAcl),
            "ChangedPins" => Some(TransitionType::ChangedPins),
            "MessageRemoved" => Some(TransitionType::MessageRemoved),
            "UnableToDecrypt" => Some(TransitionType::UnableToDecrypt),
            "HiddenEvent" => Some(TransitionType::HiddenEvent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserEvent {    
    pub event_type: String,
    pub transition: TransitionType,
    pub index: usize,
    pub is_state: bool,
    pub is_redacted: bool,
    pub should_show: bool,
    pub sender: String,
    pub display_name: String,
    pub state_key: Option<String>,
    pub membership: Option<String>,
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
    
    let event_type = match event_item.content() {
        TimelineItemContent::MsgLike(msg_like) => {
            use matrix_sdk_ui::timeline::MsgLikeKind;
            match &msg_like.kind {
                MsgLikeKind::Message(_) => "m.room.message",
                MsgLikeKind::Poll(_) => "m.room.poll",
                MsgLikeKind::Redacted => "m.room.redacted",
                MsgLikeKind::UnableToDecrypt(_) => "m.room.encrypted",
                _ => "m.room.message",
            }
        }
        TimelineItemContent::MembershipChange(_) => "m.room.member",
        TimelineItemContent::ProfileChange(_) => "m.room.member", // Profile changes are member events
        TimelineItemContent::OtherState(other) => {
            if !other.state_key().is_empty() {
                println!("OtherState event with state_key: {}", other.state_key()); 
            }
            // Extract the actual event type from the state event
            match other.content() {
                matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomCreate(_) => "m.room.create",
                matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomName(_) => "m.room.name",
                matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomTopic(_) => "m.room.topic",
                matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomServerAcl(_) => "m.room.server_acl",
                matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomPinnedEvents(_) => "m.room.pinned_events",
                // matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomJoinRules(_) => "m.room.join_rules",
                // matrix_sdk_ui::timeline::AnyOtherFullStateEventContent::RoomHistoryVisibility(_) => "m.room.history_visibility",
                _ => "m.room.state",
            }
        }
        _ => "unknown",
    };

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
            if !other.state_key().is_empty() {
                println!("OtherState event with state_key: {}", other.state_key());
            }
            (Some(other.state_key().to_string()), None)
        }
        _ => (None, None),
    };
    let (_, transistion_type, _, _display_name_from_state) = is_small_state_event(&event_item);
    //println!("state_key: {:?}, membership: {:?}, transistion_type: {:?}, event_type: {}", state_key, membership, transistion_type, event_type);
    UserEvent {
        index,
        display_name: user_name,
        event_type: event_type.to_string(),
        transition: transistion_type,
        is_state: matches!(event_item.content(), 
            TimelineItemContent::MembershipChange(_) | 
            TimelineItemContent::ProfileChange(_) | 
            TimelineItemContent::OtherState(_)
        ),
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
        AnyOtherFullStateEventContent::RoomPinnedEvents(_) => TransitionType::ChangedPins,
        AnyOtherFullStateEventContent::RoomName(_) => TransitionType::NoChange,
        AnyOtherFullStateEventContent::RoomTopic(_) => TransitionType::NoChange,
        AnyOtherFullStateEventContent::RoomAvatar(_) => TransitionType::NoChange,
        AnyOtherFullStateEventContent::RoomCanonicalAlias(_) => TransitionType::NoChange,
        AnyOtherFullStateEventContent::RoomCreate(_) => TransitionType::CreateRoom,
        AnyOtherFullStateEventContent::RoomEncryption(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomGuestAccess(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomHistoryVisibility(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomJoinRules(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomThirdPartyInvite(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomTombstone(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::RoomAliases(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::SpaceChild(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::SpaceParent(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::PolicyRuleRoom(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::PolicyRuleServer(_) => TransitionType::HiddenEvent,
        AnyOtherFullStateEventContent::PolicyRuleUser(_) => TransitionType::HiddenEvent,
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

/// Checks if a event timeline item is a small state event.
/// Returns true if the item is a small state event that can be grouped.
/// return state key
/// return Display name
pub fn is_small_state_event(
    event_tl_item: &EventTimelineItem,
) -> (bool, TransitionType, Option<String>, Option<String>) {
    match event_tl_item.content() {
        TimelineItemContent::MembershipChange(change) => (true, change.change().map(
            |f| get_transition_from_membership_change(&f)
            ).unwrap_or_default(), None, change.display_name()
        ),
        TimelineItemContent::ProfileChange(change) => {
            let transition_type = if let Some(_) = change.avatar_url_change() {
                TransitionType::ChangedAvatar
            } else if let Some(_) = change.displayname_change() {
                TransitionType::ChangedName
            } else {
                TransitionType::NoChange
            };
            (true, transition_type, None, None)
        }
        TimelineItemContent::OtherState(other_state) => (true, get_transition_from_other_events(other_state.content(), other_state.state_key()), Some(other_state.state_key().to_string()), None),
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::Poll(_),
            ..
        }) => (true, TransitionType::NoChange, None, None),
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::Redacted,
            ..
        }) => (true, TransitionType::MessageRemoved, None, None),
        TimelineItemContent::MsgLike(MsgLikeContent {
            kind: MsgLikeKind::UnableToDecrypt(_),
            ..
        }) => (true, TransitionType::UnableToDecrypt, None, None),
        _ => {
            (false, TransitionType::NoChange, None, None)
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
    println!("Aggregates: {:?}", aggregates);
    // Build text
    let mut summary_parts = Vec::new();
    for (canonical, names) in aggregates {
        let coalesced = coalesce_repeated_transitions(&canonical);

        let descs: Vec<String> = coalesced
            .into_iter()
            .map(|(t, repeats)| describe_transition(t, names.len(), repeats))
            .collect();
        println!("descs: {:?}", descs);
        let name_list = format_name_list(&names, summary_length);
        let transition_text = descs.join(", ");
        summary_parts.push(format!("{} {}", name_list, transition_text));
    }

    summary_parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn test_generate_summary() {
    //     let mut user_events: HashMap<usize, (String, Vec<UserEvent>)> = HashMap::new();
        
    //     user_events.insert(0, ("alice".into(), vec![
    //         UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Joined, index: 0, state_key: None },
    //         UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Left, index: 1, state_key: None },
    //     ]));
        
    //     user_events.insert(2, ("bob".into(), vec![
    //         UserEvent { user_id: "bob".into(), display_name: "Bob".into(), transition: TransitionType::ChangedAvatar, index: 2, state_key: None },
    //     ]));
        
    //     user_events.insert(3, ("charlie".into(), vec![
    //         UserEvent { user_id: "charlie".into(), display_name: "Charlie".into(), transition: TransitionType::Joined, index: 3, state_key: None },
    //     ]));

    //     let summary = generate_summary(&user_events, 2);
    //     assert_eq!(summary, "Alice joined and left, Bob changed their profile picture, Charlie joined the room");
    // }
}