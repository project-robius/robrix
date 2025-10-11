use std::collections::HashMap;
use matrix_sdk_ui::timeline::{AnyOtherFullStateEventContent, MembershipChange, RoomMembershipChange};

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

#[derive(Debug, Clone)]
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
            if plural { format!("joined{}", count_str) } else { format!("joined{}", count_str) }
        }
        TransitionType::Left => {
            if plural { format!("left{}", count_str) } else { format!("left{}", count_str) }
        }
        TransitionType::JoinedAndLeft => format!("joined and left{}", count_str),
        TransitionType::LeftAndJoined => format!("left and rejoined{}", count_str),
        TransitionType::ChangedName => format!("changed name{}", count_str),
        TransitionType::ChangedAvatar => format!("changed avatar{}", count_str),
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

/// Summarize all user transitions into a single string
pub fn generate_summary(
    user_events: &HashMap<String, Vec<UserEvent>>,
    summary_length: usize,
) -> String {
    // Aggregate by transition sequence
    let mut aggregates: HashMap<String, Vec<String>> = HashMap::new();
    for (user_id, events) in user_events {
        let mut transitions: Vec<_> = events.iter().map(|e| e.transition).collect();
        transitions.reverse();
        
        // Filter out Joined transitions for room creators
        if transitions.contains(&TransitionType::CreateRoom) {
            transitions.retain(|&t| t != TransitionType::Joined);
        }
        
        let canonical = canonicalize_transitions(&transitions);
        let sequence_key = canonical.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>().join(",");

        let name = events.first().map(|e| e.display_name.clone()).unwrap_or(user_id.clone());
        aggregates.entry(sequence_key).or_default().push(name);
    }

    // Order by first appearance (optional; just lexical sort for now)
    let mut sequences: Vec<_> = aggregates.into_iter().collect();
    sequences.sort_by_key(|(k, _)| k.clone());

    // Build text
    let mut summary_parts = Vec::new();
    for (seq, names) in sequences {
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
        let mut user_events: HashMap<String, Vec<UserEvent>> = HashMap::new();

        user_events.insert("alice".into(), vec![
            UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Joined, index: 0 },
            UserEvent { user_id: "alice".into(), display_name: "Alice".into(), transition: TransitionType::Left, index: 1 },
        ]);

        user_events.insert("bob".into(), vec![
            UserEvent { user_id: "bob".into(), display_name: "Bob".into(), transition: TransitionType::ChangedAvatar, index: 2 },
        ]);

        user_events.insert("charlie".into(), vec![
            UserEvent { user_id: "charlie".into(), display_name: "Charlie".into(), transition: TransitionType::Joined, index: 3 },
        ]);

        let summary = generate_summary(&user_events, 2);
        assert!(summary.contains("Alice"));
        assert!(summary.contains("Bob"));
        assert!(summary.contains("Charlie"));
    }
}