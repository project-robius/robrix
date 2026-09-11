//! Support for the agent-chat / [hagency](https://github.com/hagency-org/hagency)
//! control plane for coding agents.
//!
//! hagency runs Claude Code and Codex agents and exposes them in Matrix rooms
//! through a bridge bot. This module makes Robrix a first-class client for
//! that bridge:
//!
//! * [`approval`] — the `com.agentchat.approval.*` owner-approval protocol
//!   (parsing, expiry, verdict building), and [`approval_card`], the timeline
//!   card that renders a request with its decision buttons.
//! * [`agents`] — recognising agent puppet accounts (`@ac_<team>_<role>`) and
//!   their companion bridge bots, so inviting an agent also invites its bridge.
//! * [`presentation`] — role / message-kind badges for agent-sent messages,
//!   and stripping the bridge's type marker and permalink from their bodies.
//! * [`workflow`] — the `/create-issue`, `/go`, `/review`, `/status` slash
//!   commands offered in rooms that contain a `*_coordinator` agent.
//!
//! Everything here is compiled only with the `agent_chat` Cargo feature, and
//! most of it is additionally gated at runtime by the
//! `AppPreferences::agent_chat_enabled` setting. Approval cards are the
//! exception: they render whenever the feature is compiled in, because a
//! request sitting undecoded in an encrypted approval room is worse than one
//! the user can see and deny.
//!
//! Security note: this client never decides authorization. hagency validates
//! the verdict's real `event.sender`, room, binding fields, expiry, and
//! single-use consumption server-side. See [`approval`] for details.

use makepad_widgets::*;
use matrix_sdk::ruma::events::{AnyMessageLikeEventContent, AnySyncTimelineEvent};

pub mod agents;
pub mod approval;
pub mod approval_card;
pub mod preferences;
pub mod presentation;
pub mod workflow;

use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};
use matrix_sdk_ui::timeline::EventTimelineItem;

/// Returns `true` for `m.room.message` events whose msgtype is one of the
/// agent-chat approval msgtypes. Used to extend the SDK's default timeline
/// event filter, which would otherwise drop custom msgtypes.
pub fn is_approval_timeline_event(event: &AnySyncTimelineEvent) -> bool {
    let AnySyncTimelineEvent::MessageLike(message) = event else {
        return false;
    };
    message.original_content().is_some_and(|content| {
        matches!(
            content,
            AnyMessageLikeEventContent::RoomMessage(message)
                if approval::is_approval_msgtype(message.msgtype.msgtype())
        )
    })
}

/// Classifies a timeline event as an agent-chat approval message, if it is one.
///
/// Reads the **original** event content only, so an `m.replace` edit can never
/// change the binding fields of a card that has already been shown.
pub fn approval_message_of(event_tl_item: &EventTimelineItem) -> Option<approval::ApprovalMessage> {
    let content = event_tl_item
        .original_json()
        .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok())
        .flatten()?;
    approval::ApprovalMessage::from_original_content(&content)
}

/// The outcome of sending an approval verdict, posted back to the UI thread.
#[derive(Clone, Debug)]
pub enum ApprovalVerdictResult {
    /// The verdict was accepted by the homeserver.
    Sent { room_id: OwnedRoomId, source_event_id: OwnedEventId },
    /// The verdict could not be sent; the card's buttons are re-enabled.
    Failed { room_id: OwnedRoomId, source_event_id: OwnedEventId, error: String },
}

/// Registers the agent-chat widgets with the script VM.
pub fn script_mod(vm: &mut ScriptVm) {
    approval_card::script_mod(vm);
    preferences::script_mod(vm);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_event(msgtype: &str) -> AnySyncTimelineEvent {
        serde_json::from_value(serde_json::json!({
            "type": "m.room.message",
            "event_id": "$req:example.org",
            "sender": "@agent-bridge:example.org",
            "origin_server_ts": 1_757_500_000_000u64,
            "content": {
                "msgtype": msgtype,
                "body": "Agent wf_codex is waiting for approval from its owner.",
                "com.agentchat.approval": { "version": 1, "kind": "status" }
            }
        }))
        .unwrap()
    }

    #[test]
    fn approval_msgtypes_are_recognised_by_the_timeline_filter() {
        for ns in approval::Namespace::ALL {
            assert!(is_approval_timeline_event(&sync_event(&ns.request_msgtype())));
            assert!(is_approval_timeline_event(&sync_event(&ns.status_msgtype())));
            assert!(is_approval_timeline_event(&sync_event(&ns.verdict_msgtype())));
        }
        // Unrelated custom msgtypes stay subject to the SDK's default filter.
        assert!(!is_approval_timeline_event(&sync_event("com.agentchat.something.else")));
        assert!(!is_approval_timeline_event(&sync_event("m.text")));
    }
}
