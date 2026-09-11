//! Placeholder widgets for builds without the `agent_chat` feature.
//!
//! The room timeline DSL references `AgentApprovalCard` unconditionally; this
//! module defines it as an empty, invisible view so that default builds
//! contain no agent-chat UI at all. See `crate::agent_chat` for the real thing.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.AgentApprovalCard = View {
        visible: false,
        width: 0, height: 0
    }

    mod.widgets.AgentBadge = View {
        visible: false,
        width: 0, height: 0
    }

    mod.widgets.AgentChatPreferences = View {
        visible: false,
        width: 0, height: 0
    }
}
