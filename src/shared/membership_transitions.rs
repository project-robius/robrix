use std::collections::HashMap;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, MembershipChange};
use ruma::UserId;

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

#[derive(Debug, Clone, PartialEq)]
pub struct UserEvent {
    pub user_id: String,
    pub display_name: String,
    pub transition: TransitionType,
    pub index: usize,
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
    let count_str = if repeats > 1 {
        format!(" (×{})", repeats)
    } else {
        "".to_string()
    };
    match t {
        TransitionType::CreateRoom => format!("created and configured the room"),
        TransitionType::Joined => {
            if plural { format!("joined{}", count_str) } else { format!("joined the room{}", count_str) }
        }
        TransitionType::Left => {
            if plural { format!("left{}", count_str) } else { format!("left the room{}", count_str) }
        }
        TransitionType::JoinedAndLeft => format!("joined and left{}", count_str),
        TransitionType::LeftAndJoined => format!("left and rejoined{}", count_str),
        TransitionType::ChangedName => format!("changed their name{}", count_str),
        TransitionType::ChangedAvatar => format!("changed their profile picture{}", count_str),
        TransitionType::Invited => format!("was invited{}", count_str),
        TransitionType::Banned => format!("was banned{}", count_str),
        TransitionType::Unbanned => format!("was unbanned{}", count_str),
        TransitionType::InviteReject => format!("rejected invite{}", count_str),
        TransitionType::InviteWithdrawal => format!("invite withdrawn{}", count_str),
        TransitionType::Kicked => format!("was kicked{}", count_str),
        TransitionType::ServerAcl => format!("updated server ACLs{}", count_str),
        TransitionType::ChangedPins => format!("changed pinned messages{}", count_str),
        TransitionType::MessageRemoved => format!("removed a message{}", count_str),
        TransitionType::HiddenEvent => format!("did a hidden event{}", count_str),
        TransitionType::NoChange => format!("made no changes{}", count_str),
        TransitionType::UnableToDecrypt => format!("decryption failed{}", count_str),
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
pub fn append_user_event(index: usize, user_id: &UserId, username: String, transition: TransitionType, user_events: &mut Vec<(String, Vec<UserEvent>)>) {
    if let TransitionType::HiddenEvent = transition {
        return;
    }
    let user_event = UserEvent{
        user_id: user_id.to_string(),
        display_name: username.clone(),
        transition,
        index,
    };
    println!("appending user event: user_id: {}, display_name: {}, transition: {:?}, index: {}", user_id, username, transition, index);
    let user_id_str = user_id.to_string();
    if let Some((_, events)) = user_events.iter_mut().find(|(id, _)| id == &user_id_str) {
        if events.iter().filter(|user_event| user_event.index == index).count() == 0 {
            events.push(user_event);
        }
    } else {
        user_events.push((user_id_str, vec![user_event]));
    }
    println!("user_events after append: {:#?}", user_events.into_iter().map(|(u,_e)|{u.clone()}).collect::<Vec<_>>());
}

/// Summarize all user transitions into a single string
pub fn generate_summary(
    user_events: &HashMap<usize, (String, Vec<UserEvent>)>,
    summary_length: usize,
) -> String {
    // Aggregate by transition sequence
    let mut aggregates: Vec<(String, Vec<String>)> = Vec::new();
    // Sort keys in ascending order for consistent iteration
    let mut sorted_keys: Vec<&usize> = user_events.keys().collect();
    sorted_keys.sort();
    
    for &index in sorted_keys {
        let (user_id, events) = &user_events[&index];
        let mut transitions: Vec<_> = events.iter().map(|e| e.transition).collect();
        transitions.reverse();
        
        // Filter out Joined transitions for room creators
        if transitions.contains(&TransitionType::CreateRoom) {
            transitions.retain(|&t| t != TransitionType::Joined);
        }
        println!("transitions user_id {:?} transitions {:?}", user_id, transitions);
        let canonical = canonicalize_transitions(&transitions);
        println!("canonical for {}: {}", user_id, canonical.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>().join(","));
        let sequence_key = canonical.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>().join(",");
        println!("sequence_key for {}: {}", user_id, sequence_key);
        let name = events.first().map(|e| e.display_name.clone()).unwrap_or(user_id.clone());
        if let Some((_, names)) = aggregates.iter_mut().find(|(id, _)| id == &sequence_key) {
            names.push(name);
        } else {
            aggregates.push((sequence_key.clone(), vec![name]));
        }
    }

    // Build text
    let mut summary_parts = Vec::new();
    for (seq, names) in aggregates {
        let transitions: Vec<_> = seq
            .split(',')
            .filter_map(|s| match s.trim() {
                "CreateRoom" => Some(TransitionType::CreateRoom),
                "Joined" => Some(TransitionType::Joined),
                "Left" => Some(TransitionType::Left),
                "JoinedAndLeft" => Some(TransitionType::JoinedAndLeft),
                "LeftAndJoined" => Some(TransitionType::LeftAndJoined),
                "ChangedName" => Some(TransitionType::ChangedName),
                "ChangedAvatar" => Some(TransitionType::ChangedAvatar),
                _ => None,
            })
            .collect();

        let canonical = canonicalize_transitions(&transitions);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_summary() {
        let mut user_events: HashMap<usize, (String, Vec<UserEvent>)> = HashMap::new();
        
        user_events.insert(0, ("alice".into(), vec![
            UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Joined, index: 0 },
            UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Left, index: 1 },
        ]));
        
        user_events.insert(2, ("bob".into(), vec![
            UserEvent { user_id: "bob".into(), display_name: "Bob".into(), transition: TransitionType::ChangedAvatar, index: 2 },
        ]));
        
        user_events.insert(3, ("charlie".into(), vec![
            UserEvent { user_id: "charlie".into(), display_name: "Charlie".into(), transition: TransitionType::Joined, index: 3 },
        ]));

        let summary = generate_summary(&user_events, 2);
        assert_eq!(summary, "Alice joined and left, Bob changed their profile picture, Charlie joined the room");
    }
}