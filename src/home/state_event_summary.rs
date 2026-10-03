//! Distills/summarizes a contiguous group of small state events down to one short summary.
//!
//! Example: "Alice and Bob joined. Carol joined and left twice. Dave changed the room name and topic."
//!
//! Each event becomes a [`SummaryEntry`], which contains who it's about and what happened.
//! We combine similar verbs/actions together, e.g., "Alice, Bob, Carol, and 37 others joined."

use std::collections::HashMap;
use crate::utils::{distinct_user_labels, join_with_and};

/// How many people get named in one sentence before "and N others".
const MAX_NAMES_PER_SENTENCE: usize = 3;

/// How many room settings get named in one phrase before "and N other settings".
const MAX_SETTINGS_PER_PHRASE: usize = 3;

/// The most sentences a summary can have while maintaining their original order.
///
/// Beyond that, everyone who did the same state action will be combined into one sentence
/// so that we don't get a ton of repetitive sentences.
const MAX_CHRONOLOGICAL_SENTENCES: usize = 4;

/// One state event, reduced to who it's about and what happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryEntry {
    pub who: Who,
    /// Their display name, or `None` if the event didn't have one (`summarize()` then looks it up).
    pub name: Option<String>,
    pub change: StateChange,
}

/// Who a state event is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    /// A user, by their user ID.
    User(String),
    /// Someone invited by email, known only by the partial email address in their invitation, like `"ali...@exa..."`.
    EmailInvitee(String),
}

/// What a single small state event did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateChange {
    Membership(MembershipTransition),
    Profile(ProfileChange),
    Room(RoomChange),
}

/// A membership state, as far as telling membership changes apart goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Membership {
    Join,
    Leave,
    Invite,
    Ban,
    Knock,
    /// One the spec doesn't define.
    Custom,
}

/// What a membership event did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MembershipTransition {
    Joined,
    Left,
    Invited,
    InvitationAccepted,
    InvitationRejected,
    InvitationRevoked,
    Banned,
    Unbanned,
    Kicked,
    KickedAndBanned,
    Knocked,
    KnockAccepted,
    KnockRetracted,
    KnockDenied,
    /// The membership stayed the same, e.g. a ban whose reason got updated.
    Unchanged(Membership),
    /// Someone else set their state to left, which could've been a kick,
    /// an unban, a revoked invite, a denied knock, or even something else.
    ///
    /// We can't tell which, e.g. because the event got redacted.
    Removed,
    /// A redacted join by someone who'd already joined, which was most likely a profile change.
    ProfileChanged,
    /// A redacted join by someone whose history we don't know,
    /// implying that they either joined or changed their profile.
    JoinedOrChangedProfile,
    /// A membership state the spec doesn't define.
    Custom,
}

/// What a profile change did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileChange {
    Name,
    RemovedName,
    Avatar,
    RemovedAvatar,
    NameAndAvatar,
    /// We can't tell what changed, e.g. because it got redacted.
    Unknown,
}

/// A change to the room itself rather than to a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoomChange {
    Created,
    Upgraded,
    EnabledEncryption,
    RevokedEmailInvite,
    /// An email invitation whose details got redacted.
    EmailInvite,
    Setting(RoomSetting),
}

/// A room setting that someone changed.
///
/// Settings changed back to back by one person get listed together,
/// in the order that these variants are listed.
/// If there are many, only the first few get named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoomSetting {
    Name,
    Topic,
    Avatar,
    Address,
    GuestAccess,
    HistoryVisibility,
    JoinRules,
    PinnedMessages,
    PowerLevels,
    /// Any other setting.
    Other,
}

impl RoomSetting {
    fn noun(self) -> &'static str {
        match self {
            RoomSetting::Name              => "room name",
            RoomSetting::Topic             => "topic",
            RoomSetting::Avatar            => "room picture",
            RoomSetting::Address           => "room address",
            RoomSetting::GuestAccess       => "guest access",
            RoomSetting::HistoryVisibility => "history visibility",
            RoomSetting::JoinRules         => "join rules",
            RoomSetting::PinnedMessages    => "pinned messages",
            RoomSetting::PowerLevels       => "power levels",
            RoomSetting::Other             => "room settings",
        }
    }
}

/// Returns what a membership event did, given the user's membership before and after it.
///
/// This is for events the SDK couldn't classify, e.g. redacted ones.
/// Pass `None` for `previous` if we don't know the user's earlier membership,
/// or `Some(Membership::Leave)` if they had none, like ruma does.
/// The transitions mostly follow ruma's own classification too.
pub fn membership_transition(previous: Option<Membership>, now: Membership, by_self: bool) -> MembershipTransition {
    use Membership as M;
    use MembershipTransition as MT;
    let Some(previous) = previous else {
        // A redacted self join with no history is either a join or a profile change.
        if now == M::Join && by_self {
            return MT::JoinedOrChangedProfile;
        }
        return guess_membership_transition(now, by_self);
    };
    match (previous, now) {
        (_, M::Custom) => MT::Custom,
        (M::Leave | M::Knock, M::Join) => MT::Joined,
        (M::Invite, M::Join) => MT::InvitationAccepted,
        (M::Invite, M::Leave) if by_self => MT::InvitationRejected,
        (M::Invite, M::Leave) => MT::InvitationRevoked,
        (M::Invite | M::Leave | M::Knock, M::Ban) => MT::Banned,
        // Joining while already joined is generally always a profile change.
        (M::Join, M::Join) if by_self => MT::ProfileChanged,
        (M::Join, M::Leave) if by_self => MT::Left,
        (M::Join, M::Leave) => MT::Kicked,
        (M::Join, M::Ban) => MT::KickedAndBanned,
        (M::Leave, M::Invite) => MT::Invited,
        (M::Ban, M::Leave) => MT::Unbanned,
        (M::Leave, M::Knock) => MT::Knocked,
        (M::Knock, M::Invite) => MT::KnockAccepted,
        (M::Knock, M::Leave) if by_self => MT::KnockRetracted,
        (M::Knock, M::Leave) => MT::KnockDenied,
        // Nobody can leave twice on their own, so what we know about before is out of date.
        (M::Leave, M::Leave) if by_self => MT::Left,
        (a, b) if a == b => MT::Unchanged(a),
        // Anything else (e.g. ban to join) isn't allowed, so the previous state tells us nothing.
        _ => guess_membership_transition(now, by_self),
    }
}

/// Guesses what a membership event did, going only by the new membership and who sent it.
///
/// This is for when we don't know the user's earlier membership,
/// or when the spec doesn't allow the change (e.g. ban to join).
fn guess_membership_transition(now: Membership, by_self: bool) -> MembershipTransition {
    use Membership as M;
    use MembershipTransition as MT;
    match now {
        M::Join             => MT::Joined,
        M::Leave if by_self => MT::Left,
        M::Leave            => MT::Removed,
        M::Invite           => MT::Invited,
        M::Ban              => MT::Banned,
        M::Knock            => MT::Knocked,
        M::Custom           => MT::Custom,
    }
}

/// An activity within a person's state changes, either one change, or a pair of back-to-back changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Activity {
    One(StateChange),
    JoinedAndLeft,
    LeftAndRejoined,
    KnockedAndInvited,
    KnockedAndDenied,
    KnockedAndWithdrew,
}

fn times(count: usize) -> String {
    match count {
        0 | 1 => String::new(),
        2 => String::from(" twice"),
        n => format!(" {n} times"),
    }
}

fn membership_phrase(transition: MembershipTransition, plural: bool) -> String {
    use MembershipTransition as MT;
    let was = if plural { "were" } else { "was" };
    let s = if plural { "s" } else { "" };
    match transition {
        MT::Joined => "joined".to_string(),
        MT::Left => "left".to_string(),
        MT::Invited => format!("{was} invited"),
        MT::InvitationAccepted => format!("accepted their invitation{s}"),
        MT::InvitationRejected => format!("declined their invitation{s}"),
        MT::InvitationRevoked => format!("had their invitation{s} revoked"),
        MT::Banned => format!("{was} banned"),
        MT::Unbanned => format!("{was} unbanned"),
        MT::Kicked => format!("{was} kicked"),
        MT::KickedAndBanned => format!("{was} kicked and banned"),
        MT::Knocked => "asked to join".to_string(),
        MT::KnockAccepted => format!("had their request{s} to join accepted"),
        MT::KnockRetracted => format!("withdrew their request{s} to join"),
        MT::KnockDenied => format!("had their request{s} to join denied"),
        MT::Unchanged(Membership::Ban) => format!("had their ban{s} updated"),
        MT::Unchanged(Membership::Invite) => format!("{was} invited again"),
        MT::Unchanged(Membership::Knock) => "asked to join again".to_string(),
        MT::Unchanged(_) => format!("had their membership{s} updated"),
        MT::Removed => format!("{was} removed"),
        MT::ProfileChanged => format!("changed their profile{s}"),
        MT::JoinedOrChangedProfile => format!("joined or changed their profile{s}"),
        MT::Custom => format!("changed their membership{s}"),
    }
}

fn profile_phrase(change: ProfileChange, plural: bool) -> String {
    let s = if plural { "s" } else { "" };
    match change {
        ProfileChange::Name => format!("changed their name{s}"),
        ProfileChange::RemovedName => format!("removed their name{s}"),
        ProfileChange::Avatar => format!("changed their profile picture{s}"),
        ProfileChange::RemovedAvatar => format!("removed their profile picture{s}"),
        ProfileChange::NameAndAvatar => format!("changed their name{s} and profile picture{s}"),
        ProfileChange::Unknown => format!("changed their profile{s}"),
    }
}

fn room_action_phrase(change: RoomChange, count: usize) -> String {
    match change {
        RoomChange::Created => format!("created the room{}", times(count)),
        RoomChange::Upgraded => format!("upgraded the room{}", times(count)),
        RoomChange::EnabledEncryption => format!("enabled encryption{}", times(count)),
        RoomChange::RevokedEmailInvite if count > 1 => format!("revoked {count} email invitations"),
        RoomChange::RevokedEmailInvite => "revoked an email invitation".to_string(),
        RoomChange::EmailInvite if count > 1 => format!("updated {count} email invitations"),
        RoomChange::EmailInvite => "updated an email invitation".to_string(),
        RoomChange::Setting(setting) => format!("changed the {}{}", setting.noun(), times(count)),
    }
}

fn settings_list(settings: &[RoomSetting]) -> String {
    let shown = settings.len().min(MAX_SETTINGS_PER_PHRASE);
    let mut items: Vec<String> = settings[..shown].iter().map(|setting| setting.noun().to_string()).collect();
    match settings.len() - shown {
        0 => {}
        1 => items.push(String::from("1 other setting")),
        n => items.push(format!("{n} other settings")),
    }
    join_with_and(&items)
}

/// Joins one person's phrases in order, like "joined, changed their name, then left".
fn join_with_then(phrases: &[String]) -> String {
    match phrases {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{}, then {last}", rest.join(", ")),
    }
}

/// The changes about one person in a group, in order, with repeats counted.
#[derive(Default)]
struct PersonChanges {
    activities: Vec<(Activity, usize)>,
}

impl PersonChanges {
    fn add(&mut self, change: StateChange) {
        use MembershipTransition as MT;
        // A profile change can reach us as either kind of change, depending on how it got redacted.
        let change = match change {
            StateChange::Membership(MT::ProfileChanged) => StateChange::Profile(ProfileChange::Unknown),
            other => other,
        };
        // Knocking (or being invited) again right after the first time just counts as one more time,
        // e.g. "asked to join 3 times" rather than "asked to join, then asked to join again twice".
        let change = match (self.activities.last(), change) {
            (Some((Activity::One(StateChange::Membership(MT::Knocked)), _)), StateChange::Membership(MT::Unchanged(Membership::Knock))) => {
                StateChange::Membership(MT::Knocked)
            }
            (Some((Activity::One(StateChange::Membership(MT::Invited)), _)), StateChange::Membership(MT::Unchanged(Membership::Invite))) => {
                StateChange::Membership(MT::Invited)
            }
            (_, change) => change,
        };
        // Combine a leave+join or join+leave into one activity to make it even less verbose.
        // Do the same for name changes and profile avatar changes that are adjacent too.
        let activity = match (self.activities.last(), change) {
            (Some((Activity::One(StateChange::Membership(MT::Joined)), 1)), StateChange::Membership(MT::Left)) => {
                self.activities.pop();
                Activity::JoinedAndLeft
            }
            (Some((Activity::One(StateChange::Membership(MT::Left)), 1)), StateChange::Membership(MT::Joined)) => {
                self.activities.pop();
                Activity::LeftAndRejoined
            }
            // Same for a knock and its answer, so someone knocking over and over gets counted too.
            (Some((Activity::One(StateChange::Membership(MT::Knocked)), 1)), StateChange::Membership(answer @ (MT::KnockAccepted | MT::KnockDenied | MT::KnockRetracted))) => {
                self.activities.pop();
                match answer {
                    MT::KnockAccepted => Activity::KnockedAndInvited,
                    MT::KnockDenied => Activity::KnockedAndDenied,
                    _ => Activity::KnockedAndWithdrew,
                }
            }
            (Some((Activity::One(StateChange::Profile(ProfileChange::Name)), 1)), StateChange::Profile(ProfileChange::Avatar))
            | (Some((Activity::One(StateChange::Profile(ProfileChange::Avatar)), 1)), StateChange::Profile(ProfileChange::Name)) => {
                self.activities.pop();
                Activity::One(StateChange::Profile(ProfileChange::NameAndAvatar))
            }
            _ => Activity::One(change),
        };
        match self.activities.last_mut() {
            Some((last, count)) if *last == activity => *count += 1,
            _ => self.activities.push((activity, 1)),
        }
    }

    fn phrases(&self, plural: bool) -> Vec<String> {
        // Several people doing something to the room each did it; they didn't do it together.
        let each = if plural { "each " } else { "" };
        let mut phrases = Vec::new();
        let mut settings: Vec<RoomSetting> = Vec::new();
        let mut num_setting_changes = 0;
        let flush_settings = |settings: &mut Vec<RoomSetting>, num_changes: &mut usize, phrases: &mut Vec<String>| {
            if settings.is_empty() {
                return;
            }
            // Settings changed back to back have no meaningful order
            settings.sort_unstable();
            let count = if settings.len() == 1 { *num_changes } else { 1 };
            phrases.push(format!("{each}changed the {}{}", settings_list(settings), times(count)));
            settings.clear();
            *num_changes = 0;
        };
        for &(activity, count) in &self.activities {
            if let Activity::One(StateChange::Room(RoomChange::Setting(setting))) = activity {
                if !settings.contains(&setting) {
                    settings.push(setting);
                }
                num_setting_changes += count;
                continue;
            }
            flush_settings(&mut settings, &mut num_setting_changes, &mut phrases);
            phrases.push(match activity {
                Activity::JoinedAndLeft => format!("joined and left{}", times(count)),
                Activity::LeftAndRejoined => format!("left and rejoined{}", times(count)),
                Activity::KnockedAndInvited => format!("asked to join and {} invited{}", if plural { "were" } else { "was" }, times(count)),
                Activity::KnockedAndDenied => format!("asked to join and had their request{} denied{}", if plural { "s" } else { "" }, times(count)),
                Activity::KnockedAndWithdrew => format!("asked to join and withdrew their request{}{}", if plural { "s" } else { "" }, times(count)),
                Activity::One(StateChange::Membership(transition)) => format!("{}{}", membership_phrase(transition, plural), times(count)),
                Activity::One(StateChange::Profile(change)) => format!("{}{}", profile_phrase(change, plural), times(count)),
                Activity::One(StateChange::Room(change)) => format!("{each}{}", room_action_phrase(change, count)),
            });
        }
        flush_settings(&mut settings, &mut num_setting_changes, &mut phrases);
        phrases
    }
}

/// How the creator set up a room whose creation starts the group.
#[derive(Clone, Copy)]
enum RoomSetup {
    Created,
    CreatedAndConfigured,
}

/// Someone in a group, with every change that's about them.
struct Person<'a> {
    who: &'a Who,
    /// Their latest known name.
    name: Option<&'a str>,
    changes: PersonChanges,
    /// Set for the creator of a room whose creation starts the group.
    room_setup: Option<RoomSetup>,
}

impl Person<'_> {
    fn phrases(&self, plural: bool) -> Vec<String> {
        let mut phrases = Vec::with_capacity(self.changes.activities.len() + 1);
        if let Some(setup) = self.room_setup {
            phrases.push(String::from(match setup {
                RoomSetup::Created => "created the room",
                RoomSetup::CreatedAndConfigured => "created and configured the room",
            }));
        }
        phrases.extend(self.changes.phrases(plural));
        phrases
    }
}

fn collect_people(entries: &[SummaryEntry]) -> Vec<Person<'_>> {
    let creator = entries.first()
        .filter(|entry| entry.change == StateChange::Room(RoomChange::Created))
        .map(|entry| &entry.who);

    let mut index_of: HashMap<&Who, usize> = HashMap::new();
    let mut people: Vec<Person> = Vec::new();
    let mut creator_joined = false;
    for entry in entries {
        let index = *index_of.entry(&entry.who).or_insert_with(|| {
            people.push(Person { who: &entry.who, name: None, changes: PersonChanges::default(), room_setup: None });
            people.len() - 1
        });
        let person = &mut people[index];
        // Use their latest available name, e.g. the new one after a rename.
        if let Some(name) = entry.name.as_deref() {
            person.name = Some(name);
        }
        if creator == Some(&entry.who) {
            let is_setup = match entry.change {
                StateChange::Room(RoomChange::Created) => {
                    person.room_setup.get_or_insert(RoomSetup::Created);
                    true
                }
                // Settings only count as setup until the creator does anything else.
                StateChange::Room(RoomChange::Setting(_) | RoomChange::EnabledEncryption) if person.changes.activities.is_empty() => {
                    person.room_setup = Some(RoomSetup::CreatedAndConfigured);
                    true
                }
                StateChange::Membership(MembershipTransition::Joined) if !creator_joined && person.changes.activities.is_empty() => {
                    creator_joined = true;
                    true
                }
                _ => false,
            };
            if is_setup {
                continue;
            }
        }
        person.changes.add(entry.change);
    }
    people
}

/// Returns the name to show in the summary for each of the given people.
///
/// A user without a name shows their user ID, and users who'd show the same name get their user IDs
/// added to tell them apart (see `distinct_user_labels()`). Someone invited by email shows whatever
/// name their invitation has, if any.
fn names_to_show(subjects: &[&Who], names: &[Option<String>]) -> Vec<Option<String>> {
    let users: Vec<(&str, Option<&str>)> = subjects.iter().zip(names)
        .filter_map(|(who, name)| match who {
            Who::User(user_id) => Some((user_id.as_str(), name.as_deref())),
            Who::EmailInvitee(_) => None,
        })
        .collect();
    let mut user_labels = distinct_user_labels(&users).into_iter();
    subjects.iter().zip(names)
        .map(|(who, name)| match who {
            Who::User(_) => user_labels.next(),
            Who::EmailInvitee(_) => name.clone(),
        })
        .collect()
}

/// Summarizes the given entries (in timeline order) into one line of text.
///
/// The `name_of` callback gets invoked for anyone about to be named whose entries didn't include a name.
/// This prevents the caller from having to look up lots of names for people that'll never get used.
pub fn summarize(entries: &[SummaryEntry], mut name_of: impl FnMut(&Who) -> Option<String>) -> String {
    let people = collect_people(entries);
    let phrases_per_person: Vec<Vec<String>> = people.iter().map(|person| person.phrases(false)).collect();

    let mut sentences: Vec<Vec<usize>> = Vec::new();
    for (index, phrases) in phrases_per_person.iter().enumerate() {
        match sentences.last_mut() {
            Some(sentence) if phrases_per_person[sentence[0]] == *phrases => sentence.push(index),
            _ => sentences.push(vec![index]),
        }
    }
    if sentences.len() > MAX_CHRONOLOGICAL_SENTENCES {
        let mut sentence_with_phrases: HashMap<&[String], usize> = HashMap::new();
        sentences.clear();
        for (index, phrases) in phrases_per_person.iter().enumerate() {
            match sentence_with_phrases.get(phrases.as_slice()) {
                Some(&sentence_index) => sentences[sentence_index].push(index),
                None => {
                    sentence_with_phrases.insert(phrases, sentences.len());
                    sentences.push(vec![index]);
                }
            }
        }
    }

    // Only the first few people in each sentence get named; the rest are just counted.
    let named_people: Vec<&Person> = sentences.iter()
        .flat_map(|sentence| sentence.iter().take(MAX_NAMES_PER_SENTENCE).map(|&index| &people[index]))
        .collect();
    let names: Vec<Option<String>> = named_people.iter()
        .map(|person| person.name.map(ToOwned::to_owned).or_else(|| name_of(person.who)))
        .collect();
    let subjects: Vec<&Who> = named_people.iter().map(|person| person.who).collect();
    let mut shown_names = names_to_show(&subjects, &names).into_iter();

    sentences.iter()
        .map(|sentence| {
            let mut items: Vec<String> = shown_names.by_ref()
                .take(sentence.len().min(MAX_NAMES_PER_SENTENCE))
                .enumerate()
                .map(|(i, name)| name.unwrap_or_else(|| String::from(if i == 0 { "Someone" } else { "someone" })))
                .collect();
            match sentence.len().saturating_sub(MAX_NAMES_PER_SENTENCE) {
                0 => {}
                1 => items.push(String::from("1 other")),
                n => items.push(format!("{n} others")),
            }
            let phrases = people[sentence[0]].phrases(sentence.len() > 1);
            format!("{} {}.", join_with_and(&items), join_with_then(&phrases))
        })
        .collect::<Vec<_>>()
        .join(" ")
}


#[cfg(test)]
mod tests {
    use super::*;
    use MembershipTransition as MT;

    /// Summarizes the given entries using only the names in them.
    fn text(entries: &[SummaryEntry]) -> String {
        summarize(entries, |_| None)
    }

    fn user_id(name: &str) -> String {
        format!("@{}:example.org", name.to_lowercase())
    }

    fn entry(name: &str, change: StateChange) -> SummaryEntry {
        SummaryEntry { who: Who::User(user_id(name)), name: Some(name.to_string()), change }
    }

    fn membership(name: &str, transition: MembershipTransition) -> SummaryEntry {
        entry(name, StateChange::Membership(transition))
    }

    fn profile(name: &str, change: ProfileChange) -> SummaryEntry {
        entry(name, StateChange::Profile(change))
    }

    fn setting(name: &str, setting: RoomSetting) -> SummaryEntry {
        entry(name, StateChange::Room(RoomChange::Setting(setting)))
    }

    fn room(name: &str, change: RoomChange) -> SummaryEntry {
        entry(name, StateChange::Room(change))
    }

    #[test]
    fn single_join() {
        assert_eq!(text(&[membership("Alice", MT::Joined)]), "Alice joined.");
    }

    #[test]
    fn empty_input() {
        assert_eq!(text(&[]), "");
    }

    #[test]
    fn neighbors_doing_the_same_thing_share_a_sentence() {
        let entries = [membership("Alice", MT::Joined), membership("Bob", MT::Joined), membership("Carol", MT::Left)];
        assert_eq!(text(&entries), "Alice and Bob joined. Carol left.");
    }

    #[test]
    fn sentences_stay_in_order() {
        let entries = [membership("Alice", MT::Joined), membership("Bob", MT::Left), membership("Carol", MT::Joined)];
        assert_eq!(text(&entries), "Alice joined. Bob left. Carol joined.");
    }

    #[test]
    fn long_summaries_merge_everyone_who_did_the_same_thing() {
        let entries: Vec<_> = ["Alice", "Bob", "Carol", "Dave", "Eve", "Frank"].iter().enumerate()
            .map(|(i, name)| membership(name, if i % 2 == 0 { MT::Joined } else { MT::Left }))
            .collect();
        assert_eq!(text(&entries), "Alice, Carol, and Eve joined. Bob, Dave, and Frank left.");
    }

    #[test]
    fn join_storm_truncates_names() {
        let entries: Vec<_> = ["Alice", "Bob", "Carol", "Dave", "Eve", "Frank"].iter()
            .map(|name| membership(name, MT::Joined))
            .collect();
        assert_eq!(text(&entries), "Alice, Bob, Carol, and 3 others joined.");
    }

    #[test]
    fn three_names_use_the_oxford_comma() {
        let entries: Vec<_> = ["Alice", "Bob", "Carol"].iter().map(|name| membership(name, MT::Invited)).collect();
        assert_eq!(text(&entries), "Alice, Bob, and Carol were invited.");
    }

    #[test]
    fn join_and_leave_pairs_are_counted() {
        let entries = [
            membership("Alice", MT::Joined),
            membership("Alice", MT::Left),
            membership("Alice", MT::Joined),
            membership("Alice", MT::Left),
            membership("Alice", MT::Joined),
        ];
        assert_eq!(text(&entries), "Alice joined and left twice, then joined.");
    }

    #[test]
    fn leave_then_rejoin() {
        let entries: Vec<_> = (0..6)
            .map(|i| membership("Alice", if i % 2 == 0 { MT::Left } else { MT::Joined }))
            .collect();
        assert_eq!(text(&entries), "Alice left and rejoined 3 times.");
    }

    #[test]
    fn changes_read_in_order() {
        let entries = [
            membership("Alice", MT::Invited),
            membership("Alice", MT::InvitationAccepted),
            profile("Alice", ProfileChange::Avatar),
            membership("Alice", MT::Left),
        ];
        assert_eq!(
            text(&entries),
            "Alice was invited, accepted their invitation, changed their profile picture, then left.",
        );
    }

    #[test]
    fn identical_histories_merge_with_counts() {
        let entries = [
            membership("Alice", MT::Joined),
            membership("Bob", MT::Joined),
            membership("Alice", MT::Left),
            membership("Bob", MT::Left),
            membership("Carol", MT::Joined),
            membership("Alice", MT::Joined),
            membership("Alice", MT::Left),
            membership("Bob", MT::Joined),
            membership("Bob", MT::Left),
            membership("Carol", MT::Left),
        ];
        assert_eq!(text(&entries), "Alice and Bob joined and left twice. Carol joined and left.");
    }

    #[test]
    fn identical_text_shares_a_sentence() {
        // These differ in how often the topic changed, but that doesn't show once it's
        // listed with another setting, so they read the same.
        let entries = [
            setting("Alice", RoomSetting::Topic),
            setting("Alice", RoomSetting::Name),
            setting("Bob", RoomSetting::Topic),
            setting("Bob", RoomSetting::Topic),
            setting("Bob", RoomSetting::Name),
        ];
        assert_eq!(text(&entries), "Alice and Bob each changed the room name and topic.");
    }

    #[test]
    fn repeated_changes_are_counted() {
        let entries = [
            profile("Alice", ProfileChange::Name),
            profile("Alice", ProfileChange::Name),
            profile("Alice", ProfileChange::Avatar),
        ];
        assert_eq!(text(&entries), "Alice changed their name twice, then changed their profile picture.");
    }

    #[test]
    fn name_and_picture_changes_pair_up() {
        let entries = [
            profile("Alice", ProfileChange::Avatar),
            profile("Alice", ProfileChange::Name),
            profile("Bob", ProfileChange::Name),
            profile("Bob", ProfileChange::Avatar),
            profile("Bob", ProfileChange::Avatar),
        ];
        assert_eq!(
            text(&entries),
            "Alice changed their name and profile picture. Bob changed their name and profile picture, then changed their profile picture.",
        );
    }

    #[test]
    fn plural_profile_phrases() {
        let entries = [
            profile("Alice", ProfileChange::Name),
            profile("Bob", ProfileChange::Name),
            profile("Carol", ProfileChange::RemovedName),
            profile("Dave", ProfileChange::RemovedName),
            profile("Eve", ProfileChange::RemovedAvatar),
        ];
        assert_eq!(
            text(&entries),
            "Alice and Bob changed their names. Carol and Dave removed their names. Eve removed their profile picture.",
        );
    }

    #[test]
    fn plural_membership_phrases() {
        let entries = [
            membership("Alice", MT::Invited),
            membership("Bob", MT::Invited),
            membership("Carol", MT::Kicked),
            membership("Dave", MT::KnockDenied),
            membership("Eve", MT::KnockDenied),
            membership("Frank", MT::InvitationAccepted),
            membership("Grace", MT::InvitationAccepted),
        ];
        assert_eq!(
            text(&entries),
            "Alice and Bob were invited. Carol was kicked. Dave and Eve had their requests to join denied. Frank and Grace accepted their invitations.",
        );
    }

    #[test]
    fn knocks_pair_up_with_their_answers() {
        let entries = [
            membership("Alice", MT::Knocked),
            membership("Alice", MT::KnockAccepted),
            membership("Alice", MT::InvitationAccepted),
        ];
        assert_eq!(text(&entries), "Alice asked to join and was invited, then accepted their invitation.");
        let entries: Vec<_> = (0..6).map(|i| membership("Mallory", if i % 2 == 0 { MT::Knocked } else { MT::KnockRetracted })).collect();
        assert_eq!(text(&entries), "Mallory asked to join and withdrew their request 3 times.");
        let entries = [
            membership("Dave", MT::Knocked),
            membership("Dave", MT::KnockDenied),
            membership("Eve", MT::Knocked),
            membership("Eve", MT::KnockDenied),
        ];
        assert_eq!(text(&entries), "Dave and Eve asked to join and had their requests denied.");
    }

    #[test]
    fn knocking_or_being_invited_again_counts_up() {
        let entries = [
            membership("Alice", MT::Knocked),
            membership("Alice", MT::Unchanged(Membership::Knock)),
            membership("Alice", MT::Unchanged(Membership::Knock)),
            membership("Alice", MT::KnockAccepted),
        ];
        assert_eq!(text(&entries), "Alice asked to join 3 times, then had their request to join accepted.");
        let entries = [
            membership("Bob", MT::Invited),
            membership("Bob", MT::Unchanged(Membership::Invite)),
            membership("Bob", MT::InvitationAccepted),
        ];
        assert_eq!(text(&entries), "Bob was invited twice, then accepted their invitation.");
    }

    #[test]
    fn membership_updates_that_change_nothing() {
        let entries = [
            membership("Alice", MT::Unchanged(Membership::Ban)),
            membership("Bob", MT::Unchanged(Membership::Invite)),
            membership("Carol", MT::Unchanged(Membership::Knock)),
            membership("Dave", MT::Unchanged(Membership::Leave)),
        ];
        assert_eq!(
            text(&entries),
            "Alice had their ban updated. Bob was invited again. Carol asked to join again. Dave had their membership updated.",
        );
    }

    #[test]
    fn ambiguous_membership_phrases() {
        let entries = [
            membership("Alice", MT::Removed),
            membership("Bob", MT::Removed),
            membership("Carol", MT::ProfileChanged),
        ];
        assert_eq!(text(&entries), "Alice and Bob were removed. Carol changed their profile.");
    }

    #[test]
    fn rename_uses_the_latest_name() {
        let mut renamed = profile("Alice", ProfileChange::Name);
        renamed.name = Some("Alicia".to_string());
        assert_eq!(text(&[membership("Alice", MT::Joined), renamed]), "Alicia joined, then changed their name.");
    }

    #[test]
    fn unknown_names_fall_back_to_the_user_id() {
        let mut unnamed = membership("Bob", MT::Left);
        unnamed.name = None;
        // A name from another of their events fills in for one we don't know.
        assert_eq!(text(&[membership("Bob", MT::Joined), unnamed.clone()]), "Bob joined and left.");
        assert_eq!(text(&[unnamed]), "@bob:example.org left.");
    }

    #[test]
    fn duplicate_names_get_their_user_ids() {
        let entries = [
            SummaryEntry { who: Who::User("@alice:a.org".into()), name: Some("Alice".into()), change: StateChange::Membership(MT::Joined) },
            SummaryEntry { who: Who::User("@alice:b.org".into()), name: Some("Alice".into()), change: StateChange::Membership(MT::Joined) },
        ];
        assert_eq!(text(&entries), "Alice (@alice:a.org) and Alice (@alice:b.org) joined.");
    }

    #[test]
    fn room_changes_keep_their_order() {
        let entries = [
            setting("Alice", RoomSetting::Name),
            setting("Alice", RoomSetting::Topic),
            setting("Alice", RoomSetting::Topic),
            room("Alice", RoomChange::EnabledEncryption),
            setting("Bob", RoomSetting::PinnedMessages),
        ];
        assert_eq!(
            text(&entries),
            "Alice changed the room name and topic, then enabled encryption. Bob changed the pinned messages.",
        );
    }

    #[test]
    fn one_setting_changed_repeatedly() {
        let entries = [setting("Alice", RoomSetting::Topic), setting("Alice", RoomSetting::Topic), setting("Alice", RoomSetting::Topic)];
        assert_eq!(text(&entries), "Alice changed the topic 3 times.");
    }

    #[test]
    fn many_settings_get_truncated() {
        let settings = [RoomSetting::Name, RoomSetting::Topic, RoomSetting::Avatar, RoomSetting::PowerLevels, RoomSetting::JoinRules];
        let four: Vec<_> = settings[..4].iter().map(|&s| setting("Alice", s)).collect();
        assert_eq!(text(&four), "Alice changed the room name, topic, room picture, and 1 other setting.");
        let five: Vec<_> = settings.iter().map(|&s| setting("Alice", s)).collect();
        assert_eq!(text(&five), "Alice changed the room name, topic, room picture, and 2 other settings.");
    }

    #[test]
    fn several_people_changing_the_room_each_did_it() {
        let entries = [
            membership("Alice", MT::Joined),
            setting("Alice", RoomSetting::Topic),
            membership("Bob", MT::Joined),
            setting("Bob", RoomSetting::Topic),
        ];
        assert_eq!(text(&entries), "Alice and Bob joined, then each changed the topic.");
        let entries = [room("Alice", RoomChange::EnabledEncryption), room("Bob", RoomChange::EnabledEncryption)];
        assert_eq!(text(&entries), "Alice and Bob each enabled encryption.");
    }

    #[test]
    fn email_invitations() {
        let invitee = |token: &str, name: &str| SummaryEntry {
            who: Who::EmailInvitee(token.into()),
            name: Some(name.into()),
            change: StateChange::Membership(MT::Invited),
        };
        let entries = [membership("Bob", MT::Invited), invitee("t1", "c...@example.org")];
        assert_eq!(text(&entries), "Bob and c...@example.org were invited.");

        let revoked = [room("Alice", RoomChange::RevokedEmailInvite)];
        assert_eq!(text(&revoked), "Alice revoked an email invitation.");
        let revoked = [room("Alice", RoomChange::RevokedEmailInvite), room("Alice", RoomChange::RevokedEmailInvite)];
        assert_eq!(text(&revoked), "Alice revoked 2 email invitations.");

        let unnamed = SummaryEntry { who: Who::EmailInvitee("t2".into()), name: None, change: StateChange::Membership(MT::Invited) };
        assert_eq!(text(std::slice::from_ref(&unnamed)), "Someone was invited.");
        // Only capitalized when it starts a sentence.
        assert_eq!(text(&[membership("Bob", MT::Invited), unnamed.clone()]), "Bob and someone were invited.");
        assert_eq!(text(&[unnamed, membership("Bob", MT::Invited)]), "Someone and Bob were invited.");
    }

    #[test]
    fn names_keep_their_own_case() {
        let entries = [SummaryEntry { who: Who::User(user_id("alice")), name: Some("alice".into()), change: StateChange::Membership(MT::Joined) }];
        assert_eq!(text(&entries), "alice joined.");
    }

    #[test]
    fn room_creation_reads_as_one_phrase() {
        let entries = [
            room("Alice", RoomChange::Created),
            membership("Alice", MT::Joined),
            setting("Alice", RoomSetting::PowerLevels),
            setting("Alice", RoomSetting::Name),
        ];
        assert_eq!(text(&entries), "Alice created and configured the room.");

        let entries = [room("Alice", RoomChange::Created), membership("Alice", MT::Joined)];
        assert_eq!(text(&entries), "Alice created the room.");
    }

    #[test]
    fn room_creation_keeps_the_creators_other_actions() {
        let mut renamed = profile("Alice", ProfileChange::Name);
        renamed.name = Some("Alicia".into());
        // Their leave carries the name they had by then.
        let mut left = membership("Alice", MT::Left);
        left.name = Some("Alicia".into());
        let entries = [
            room("Alice", RoomChange::Created),
            membership("Alice", MT::Joined),
            setting("Alice", RoomSetting::PowerLevels),
            renamed,
            left,
        ];
        assert_eq!(
            text(&entries),
            "Alicia created and configured the room, changed their name, then left.",
        );
    }

    #[test]
    fn room_creation_with_email_invites() {
        let entries = [
            room("Alice", RoomChange::Created),
            membership("Alice", MT::Joined),
            setting("Alice", RoomSetting::JoinRules),
            SummaryEntry {
                who: Who::EmailInvitee("t1".into()),
                name: Some("b...@example.org".into()),
                change: StateChange::Membership(MT::Invited),
            },
        ];
        assert_eq!(text(&entries), "Alice created and configured the room. b...@example.org was invited.");
    }

    #[test]
    fn settings_changed_in_a_different_order_still_match() {
        let entries = [
            setting("Alice", RoomSetting::Topic),
            setting("Alice", RoomSetting::Name),
            setting("Bob", RoomSetting::Name),
            setting("Bob", RoomSetting::Topic),
        ];
        assert_eq!(text(&entries), "Alice and Bob each changed the room name and topic.");
    }

    #[test]
    fn creators_later_settings_keep_their_place() {
        let entries = [
            room("Alice", RoomChange::Created),
            membership("Alice", MT::Joined),
            membership("Alice", MT::Left),
            membership("Alice", MT::Joined),
            setting("Alice", RoomSetting::Topic),
        ];
        assert_eq!(text(&entries), "Alice created the room, left and rejoined, then changed the topic.");
    }

    #[test]
    fn redacted_profile_changes_count_together() {
        let entries = [membership("Alice", MT::ProfileChanged), profile("Alice", ProfileChange::Unknown)];
        assert_eq!(text(&entries), "Alice changed their profile twice.");
    }

    #[test]
    fn joins_with_unknown_history() {
        let entries = [membership("Alice", MT::JoinedOrChangedProfile), membership("Bob", MT::JoinedOrChangedProfile)];
        assert_eq!(text(&entries), "Alice and Bob joined or changed their profiles.");
    }

    #[test]
    fn names_are_only_looked_up_for_people_who_get_named() {
        let entries: Vec<_> = ["a", "b", "c", "d", "e"].iter()
            .map(|name| SummaryEntry { who: Who::User(user_id(name)), name: None, change: StateChange::Membership(MT::Joined) })
            .collect();
        let mut asked = Vec::new();
        let summary = summarize(&entries, |who| {
            asked.push(who.clone());
            let Who::User(id) = who else { return None };
            Some(id[1..2].to_uppercase())
        });
        assert_eq!(summary, "A, B, C, and 2 others joined.");
        assert_eq!(asked.len(), 3);
    }

    #[test]
    fn names_cant_pass_for_someone_else() {
        let mallory = SummaryEntry {
            who: Who::User("@mallory:example.org".into()),
            name: Some("@bob:example.org".into()),
            change: StateChange::Membership(MT::Joined),
        };
        let mut bob = membership("Bob", MT::Joined);
        bob.name = None;
        assert_eq!(text(&[bob, mallory]), "@bob:example.org and @bob:example.org (@mallory:example.org) joined.");
        // Duplicate names are told apart across sentences too.
        let entries = [
            SummaryEntry { who: Who::User("@alice:a.org".into()), name: Some("Alice".into()), change: StateChange::Membership(MT::Joined) },
            membership("Bob", MT::Left),
            SummaryEntry { who: Who::User("@alice:b.org".into()), name: Some("Alice".into()), change: StateChange::Membership(MT::Kicked) },
        ];
        assert_eq!(text(&entries), "Alice (@alice:a.org) joined. Bob left. Alice (@alice:b.org) was kicked.");
    }

    #[test]
    fn repeat_email_invitations_to_one_address() {
        let invite = || SummaryEntry {
            who: Who::EmailInvitee("joh...@cor...".into()),
            name: Some("joh...@cor...".into()),
            change: StateChange::Membership(MT::Invited),
        };
        assert_eq!(text(&[invite(), invite()]), "joh...@cor... was invited twice.");
        let revoked = SummaryEntry {
            who: Who::EmailInvitee("joh...@cor...".into()),
            name: None,
            change: StateChange::Membership(MT::InvitationRevoked),
        };
        assert_eq!(text(&[invite(), revoked]), "joh...@cor... was invited, then had their invitation revoked.");
    }

    #[test]
    fn membership_transitions_from_known_history() {
        use Membership as M;
        let cases = [
            (M::Leave, M::Join, true, MT::Joined),
            (M::Knock, M::Join, true, MT::Joined),
            (M::Invite, M::Join, true, MT::InvitationAccepted),
            (M::Invite, M::Leave, true, MT::InvitationRejected),
            (M::Invite, M::Leave, false, MT::InvitationRevoked),
            (M::Leave, M::Ban, false, MT::Banned),
            (M::Join, M::Join, true, MT::ProfileChanged),
            (M::Join, M::Join, false, MT::Unchanged(M::Join)),
            (M::Join, M::Leave, true, MT::Left),
            (M::Join, M::Leave, false, MT::Kicked),
            (M::Join, M::Ban, false, MT::KickedAndBanned),
            (M::Leave, M::Invite, false, MT::Invited),
            (M::Ban, M::Leave, false, MT::Unbanned),
            (M::Leave, M::Knock, true, MT::Knocked),
            (M::Knock, M::Invite, false, MT::KnockAccepted),
            (M::Knock, M::Leave, true, MT::KnockRetracted),
            (M::Knock, M::Leave, false, MT::KnockDenied),
            (M::Ban, M::Ban, false, MT::Unchanged(M::Ban)),
            (M::Invite, M::Invite, false, MT::Unchanged(M::Invite)),
            (M::Leave, M::Leave, false, MT::Unchanged(M::Leave)),
            (M::Leave, M::Leave, true, MT::Left),
            (M::Join, M::Custom, true, MT::Custom),
            // Not allowed by the spec, so it's read like an unknown history.
            (M::Ban, M::Join, true, MT::Joined),
            (M::Join, M::Invite, false, MT::Invited),
        ];
        for (previous, now, by_self, expected) in cases {
            assert_eq!(membership_transition(Some(previous), now, by_self), expected, "{previous:?} -> {now:?}, by self: {by_self}");
        }
    }

    #[test]
    fn membership_transitions_without_history() {
        use Membership as M;
        assert_eq!(membership_transition(None, M::Join, true), MT::JoinedOrChangedProfile);
        assert_eq!(membership_transition(None, M::Join, false), MT::Joined);
        assert_eq!(membership_transition(None, M::Leave, true), MT::Left);
        assert_eq!(membership_transition(None, M::Leave, false), MT::Removed);
        assert_eq!(membership_transition(None, M::Invite, false), MT::Invited);
        assert_eq!(membership_transition(None, M::Ban, false), MT::Banned);
        assert_eq!(membership_transition(None, M::Knock, true), MT::Knocked);
        assert_eq!(membership_transition(None, M::Custom, true), MT::Custom);
    }
}
