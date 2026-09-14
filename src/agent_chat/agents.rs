//! Recognising agent-chat agent accounts and their companion bridge bots.
//!
//! hagency registers one Matrix "puppet" account per agent, named
//! `@ac_<team>_<role>:server` (the `ac_` prefix is the bridge's default
//! `MATRIX_AGENT_PREFIX`), and one bridge bot per instance, named
//! `@agent-bridge-<team>:server` (or the legacy shared `@agent-bridge:server`).
//! Nothing here is an identity check: the bridge decides who is an agent, and
//! the agent-chat server decides who owns one. These helpers only drive UI
//! conveniences such as role badges, coordinator detection for the workflow
//! commands, and auto-inviting the bridge alongside its agents.

use std::collections::HashSet;

use matrix_sdk::{room::RoomMember, ruma::{OwnedUserId, UserId}};

/// The localpart prefix the bridge gives every agent puppet account.
pub const AGENT_LOCALPART_PREFIX: &str = "ac_";
/// The localpart of the bridge bot (`agent-bridge-<team>`, or bare `agent-bridge`).
pub const BRIDGE_BOT_LOCALPART: &str = "agent-bridge";

/// The role an agent plays in an agent-chat workflow, derived from its Matrix
/// localpart (`ac_<team>_<role>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Coordinator,
    Implementer,
    Reviewer,
    FinalReviewer,
}

impl AgentRole {
    /// Every role suffix. `final_reviewer` must be tested before `reviewer`,
    /// which is a suffix of it.
    const ALL: [(&'static str, Self); 4] = [
        ("final_reviewer", Self::FinalReviewer),
        ("coordinator", Self::Coordinator),
        ("implementer", Self::Implementer),
        ("reviewer", Self::Reviewer),
    ];

    /// Derives the role from a localpart or display name, e.g. `ac_wf_reviewer`
    /// or `wf_coordinator`. A bare role name (`coordinator`) also matches.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL.iter().find_map(|(suffix, role)| {
            (name == *suffix || name.ends_with(&format!("_{suffix}"))).then_some(*role)
        })
    }

    /// Short badge text for this role.
    pub fn label(self) -> &'static str {
        match self {
            Self::Coordinator => "coordinator",
            Self::Implementer => "implementer",
            Self::Reviewer => "reviewer",
            Self::FinalReviewer => "final review",
        }
    }
}

/// Whether `localpart` looks like an agent-chat agent puppet: either it carries
/// the bridge's `ac_` prefix, or it ends in one of the workflow role names.
pub fn is_agent_localpart(localpart: &str) -> bool {
    localpart.starts_with(AGENT_LOCALPART_PREFIX) || AgentRole::from_name(localpart).is_some()
}

/// Whether `user_id` is an agent-chat bridge bot.
pub fn is_bridge_bot(user_id: &UserId) -> bool {
    let localpart = user_id.localpart();
    localpart == BRIDGE_BOT_LOCALPART
        || localpart.strip_prefix(BRIDGE_BOT_LOCALPART).is_some_and(|rest| rest.starts_with('-'))
}

/// Whether `name` names a workflow coordinator agent: bare `coordinator` or
/// `<team>_coordinator`. Matched against both display names and localparts,
/// so `ac_wf_coordinator` and a friendly display name both qualify.
pub fn is_coordinator_name(name: &str) -> bool {
    AgentRole::from_name(name) == Some(AgentRole::Coordinator)
}

/// Whether any of `members` is a workflow coordinator agent.
pub fn room_has_coordinator(members: &[RoomMember]) -> bool {
    members.iter().any(|member| {
        member.display_name().is_some_and(is_coordinator_name)
            || is_coordinator_name(member.user_id().localpart())
    })
}

/// The companion bridge bots to invite alongside an agent puppet.
///
/// Multi-instance topology: each team's bridge bot is `agent-bridge-<team>`,
/// derived from the invitee's `ac_<team>_<role>` localpart. The legacy shared
/// `agent-bridge` name is kept as a best-effort fallback so single-instance
/// deployments keep working; inviting a nonexistent account fails harmlessly.
pub fn companion_bridge_bots(user_id: &UserId) -> Vec<OwnedUserId> {
    let Some(rest) = user_id.localpart().strip_prefix(AGENT_LOCALPART_PREFIX) else {
        return Vec::new();
    };
    let mut plan = Vec::new();
    let team = AgentRole::ALL
        .iter()
        .find_map(|(suffix, _)| rest.strip_suffix(suffix)?.strip_suffix('_'))
        .filter(|team| !team.is_empty());
    if let Some(team) = team {
        let derived = format!("@{BRIDGE_BOT_LOCALPART}-{team}:{}", user_id.server_name());
        if let Ok(user) = OwnedUserId::try_from(derived) {
            plan.push(user);
        }
    }
    let legacy = format!("@{BRIDGE_BOT_LOCALPART}:{}", user_id.server_name());
    if let Ok(user) = OwnedUserId::try_from(legacy) {
        plan.push(user);
    }
    plan
}

/// Expands a list of invitees with the companion bridge bots of any agent
/// puppets among them, de-duplicated and in invitation order.
pub fn invite_plan<I>(invitees: I) -> Vec<OwnedUserId>
where
    I: IntoIterator<Item = OwnedUserId>,
{
    let mut seen = HashSet::new();
    let mut plan = Vec::new();
    for invitee in invitees {
        if !seen.insert(invitee.clone()) {
            continue;
        }
        let bots = companion_bridge_bots(&invitee);
        plan.push(invitee);
        for bot in bots {
            if seen.insert(bot.clone()) {
                plan.push(bot);
            }
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str) -> OwnedUserId {
        OwnedUserId::try_from(id).unwrap()
    }

    #[test]
    fn roles_from_names() {
        assert_eq!(AgentRole::from_name("ac_wf_coordinator"), Some(AgentRole::Coordinator));
        assert_eq!(AgentRole::from_name("wf_implementer"), Some(AgentRole::Implementer));
        assert_eq!(AgentRole::from_name("ac_tyrese_reviewer"), Some(AgentRole::Reviewer));
        assert_eq!(AgentRole::from_name("ac_wf_final_reviewer"), Some(AgentRole::FinalReviewer));
        assert_eq!(AgentRole::from_name("Coordinator"), Some(AgentRole::Coordinator));
        assert_eq!(AgentRole::from_name("alice"), None);
        assert_eq!(AgentRole::from_name("coordinators"), None);
    }

    #[test]
    fn coordinator_and_agent_detection() {
        assert!(is_coordinator_name("wf_coordinator"));
        assert!(is_coordinator_name("coordinator"));
        assert!(!is_coordinator_name("wf_reviewer"));
        assert!(is_agent_localpart("ac_anything"));
        assert!(is_agent_localpart("wf_reviewer"));
        assert!(!is_agent_localpart("alice"));
        assert!(is_bridge_bot(&user("@agent-bridge:example.org")));
        assert!(is_bridge_bot(&user("@agent-bridge-wf:example.org")));
        assert!(!is_bridge_bot(&user("@agent-bridged:example.org")));
    }

    #[test]
    fn invite_plan_adds_team_and_legacy_bridge_bots() {
        let plan = invite_plan([user("@ac_wf_coordinator:example.org")]);
        assert_eq!(
            plan,
            vec![
                user("@ac_wf_coordinator:example.org"),
                user("@agent-bridge-wf:example.org"),
                user("@agent-bridge:example.org"),
            ]
        );
        // A puppet with no recognisable role still gets the legacy bot.
        let plan = invite_plan([user("@ac_helper:example.org")]);
        assert_eq!(plan, vec![user("@ac_helper:example.org"), user("@agent-bridge:example.org")]);
        // Humans get nothing extra, and duplicates collapse.
        let plan = invite_plan([
            user("@alice:example.org"),
            user("@ac_wf_reviewer:example.org"),
            user("@ac_wf_implementer:example.org"),
            user("@alice:example.org"),
        ]);
        assert_eq!(
            plan,
            vec![
                user("@alice:example.org"),
                user("@ac_wf_reviewer:example.org"),
                user("@agent-bridge-wf:example.org"),
                user("@agent-bridge:example.org"),
                user("@ac_wf_implementer:example.org"),
            ]
        );
    }
}
