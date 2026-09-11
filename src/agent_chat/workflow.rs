//! The agent-chat slash commands.
//!
//! Two sets are offered in the message input's `/` popup, and both are sent
//! as **plain text** for the agents (or the hagency backend) to interpret.
//! Robrix does not act on them itself; listing them here is an autocomplete
//! convenience and keeps the client's own command parser from rejecting them
//! as unknown.
//!
//! * **Workflow commands** (`/create-issue`, `/go`, `/review`, `/status`) drive
//!   the `issue-workflow` skill and are offered when the room contains a
//!   `*_coordinator` agent.
//! * **Thread-session commands** (`/task`, `/thread`) are parsed by the hagency
//!   backend and are offered when the room contains any agent puppet.
//!
//! Double-gated: the `agent_chat` Cargo feature (compile time) and the
//! `AppPreferences::agent_chat_enabled` toggle (run time), plus the room
//! membership conditions above.

use makepad_widgets::Cx;
use matrix_sdk::room::RoomMember;

use crate::settings::app_preferences::AppPreferencesGlobal;
use crate::shared::slash_commands::SlashCommand;

use super::agents::{is_agent_localpart, room_has_coordinator};

/// The workflow commands, in display order.
pub static WORKFLOW_SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "create-issue",
        aliases: &[],
        description: "Ask the coordinator agent to file an issue and draft its spec",
        usage: "/create-issue <title> | <description>",
    },
    SlashCommand {
        name: "go",
        aliases: &[],
        description: "Ask the coordinator agent to plan and implement an issue",
        usage: "/go <issue>",
    },
    SlashCommand {
        name: "review",
        aliases: &[],
        description: "Ask the coordinator agent to run an adversarial review",
        usage: "/review <issue>",
    },
    SlashCommand {
        name: "status",
        aliases: &[],
        description: "Ask the coordinator agent for the status of every issue",
        usage: "/status",
    },
];

/// The thread-session commands handled by the hagency backend, in display order.
pub static THREAD_SESSION_SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "task",
        aliases: &[],
        description: "Create a task for one agent; its replies land in a thread",
        usage: "/task @agent <what to do>",
    },
    SlashCommand {
        name: "thread",
        aliases: &[],
        description: "Change the model or mode of this thread's agent session (operators only)",
        usage: "@agent /thread model <name|default> | mode <plan|auto>",
    },
];

/// Which agent-chat command sets a room currently qualifies for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnabledCommandSets {
    /// The room contains a coordinator agent.
    pub workflow: bool,
    /// The room contains at least one agent puppet.
    pub thread_sessions: bool,
}

impl EnabledCommandSets {
    /// No agent-chat commands.
    pub const NONE: Self = Self { workflow: false, thread_sessions: false };

    /// Whether any command set is enabled.
    pub fn any(self) -> bool {
        self.workflow || self.thread_sessions
    }

    /// The commands in the enabled sets.
    fn commands(self) -> impl Iterator<Item = &'static SlashCommand> {
        self.workflow
            .then_some(WORKFLOW_SLASH_COMMANDS)
            .into_iter()
            .chain(self.thread_sessions.then_some(THREAD_SESSION_SLASH_COMMANDS))
            .flat_map(|set| set.iter())
    }

    /// Whether `name` (without the leading slash) is an enabled agent-chat command.
    pub fn contains(self, name: &str) -> bool {
        self.commands().any(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Returns the enabled commands whose name starts with `query`.
    pub fn matching(self, query: &str) -> impl Iterator<Item = &'static SlashCommand> {
        let query = query.to_lowercase();
        self.commands().filter(move |c| c.name.starts_with(&query))
    }
}

/// Which agent-chat command sets to offer for a room with `members`: none
/// unless the runtime toggle is on, then by room membership.
pub fn enabled_command_sets(cx: &mut Cx, members: Option<&[RoomMember]>) -> EnabledCommandSets {
    if !cx.global::<AppPreferencesGlobal>().0.agent_chat_enabled {
        return EnabledCommandSets::NONE;
    }
    let Some(members) = members else {
        return EnabledCommandSets::NONE;
    };
    EnabledCommandSets {
        workflow: room_has_coordinator(members),
        thread_sessions: members.iter().any(|m| is_agent_localpart(m.user_id().localpart())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_sets_match_by_prefix() {
        let all = EnabledCommandSets { workflow: true, thread_sessions: true };
        let names: Vec<&str> = all.matching("").map(|c| c.name).collect();
        assert_eq!(names, ["create-issue", "go", "review", "status", "task", "thread"]);
        let names: Vec<&str> = all.matching("T").map(|c| c.name).collect();
        assert_eq!(names, ["task", "thread"]);
        assert!(all.matching("x").next().is_none());
        assert!(all.contains("Go"));
        assert!(!all.contains("leave"));

        let threads_only = EnabledCommandSets { workflow: false, thread_sessions: true };
        assert!(threads_only.contains("task"));
        assert!(!threads_only.contains("go"));
        assert!(!EnabledCommandSets::NONE.any());
        assert!(EnabledCommandSets::NONE.matching("").next().is_none());
    }
}
