//! The agent-chat owner-approval protocol: parsing, expiry, and verdict building.
//!
//! This module is pure protocol logic with no UI dependencies so it can be
//! unit-tested exhaustively. The wire format is the `com.agentchat.approval.*`
//! family of `m.room.message` msgtypes emitted by the hagency Matrix bridge:
//!
//! * `com.agentchat.approval.request.v1` — sent by the bridge bot into the
//!   owner's private, end-to-end encrypted approval room. Carries the full
//!   request details and the list of decision buttons to offer.
//! * `com.agentchat.approval.status.v1` — a redacted "waiting for owner"
//!   notice posted into the public project room. Never actionable.
//! * `com.agentchat.approval.verdict.v1` — sent by *this client* in reply to a
//!   request. It echoes every binding field of the request verbatim so the
//!   server can validate it against its stored record.
//!
//! Security model: the client is a presentation surface only. hagency
//! validates the real Matrix `event.sender`, the room, every binding field,
//! expiry, and single-use consumption server-side. Nothing here grants
//! authority; a well-formed verdict from the wrong sender is still rejected.
//! Accordingly, binding fields are only ever read from the **original** event
//! content, never from an `m.replace` edit.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use matrix_sdk::ruma::{EventId, OwnedEventId};

/// The content key holding the structured approval payload.
pub const APPROVAL_EVENT_KEY: &str = "com.agentchat.approval";
/// msgtype of an owner approval request (sent by the bridge into the approval room).
pub const APPROVAL_REQUEST_MSGTYPE: &str = "com.agentchat.approval.request.v1";
/// msgtype of the redacted public status notice (sent by the agent into the project room).
pub const APPROVAL_STATUS_MSGTYPE: &str = "com.agentchat.approval.status.v1";
/// msgtype of the verdict this client sends in reply to a request.
pub const APPROVAL_VERDICT_MSGTYPE: &str = "com.agentchat.approval.verdict.v1";

/// The most decision buttons a request may carry:
/// `approve_once`, `approve_task`, `approve_always`, `deny`.
pub const MAX_APPROVAL_ACTIONS: usize = 4;

/// Returns `true` if `msgtype` is one of the three agent-chat approval msgtypes.
pub fn is_approval_msgtype(msgtype: &str) -> bool {
    matches!(
        msgtype,
        APPROVAL_REQUEST_MSGTYPE | APPROVAL_STATUS_MSGTYPE | APPROVAL_VERDICT_MSGTYPE
    )
}

/// The visual style the bridge asked for on a decision button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStyle {
    Primary,
    Secondary,
    Danger,
}

impl ActionStyle {
    fn parse(style: Option<&str>) -> Self {
        match style {
            Some("primary") => Self::Primary,
            Some("danger") => Self::Danger,
            _ => Self::Secondary,
        }
    }
}

/// One decision button offered by an approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalAction {
    /// The action id echoed back in the verdict, e.g. `approve_once`.
    pub id: String,
    /// The human-readable button label supplied by the bridge.
    pub label: String,
    pub style: ActionStyle,
}

/// The reusable scope that `approve_task` / `approve_always` would grant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReusableScope {
    pub description: String,
    pub workspace: String,
    pub task_id: Option<String>,
}

/// A fully-validated owner approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub agent: String,
    pub project: String,
    pub project_room_id: String,
    pub request_id: String,
    pub upstream_request_id: String,
    pub input_digest: String,
    pub runtime: String,
    pub tool_name: String,
    pub description: String,
    pub input_preview: String,
    /// Absolute expiry deadline, in milliseconds since the Unix epoch.
    pub expires_at_millis: u64,
    pub reusable_scope: Option<ReusableScope>,
    /// The decision buttons, in the order the bridge listed them.
    pub actions: Vec<ApprovalAction>,
}

impl ApprovalRequest {
    /// The card title: the tool being invoked, e.g. `Bash`.
    pub fn title(&self) -> String {
        if self.runtime.is_empty() {
            self.tool_name.clone()
        } else {
            format!("{} · {}", self.tool_name, self.runtime)
        }
    }

    /// The card body: what the agent wants to do, plus the command preview
    /// and any reusable scope the wider approvals would grant.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(4);
        parts.push(format!("Agent {} · project {}", self.agent, self.project));
        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }
        if !self.input_preview.is_empty() {
            parts.push(self.input_preview.clone());
        }
        if let Some(scope) = &self.reusable_scope {
            let mut line = String::from("Reusable scope: ");
            line.push_str(&scope.description);
            if !scope.workspace.is_empty() {
                line.push_str(&format!("\nWorkspace: {}", scope.workspace));
            }
            parts.push(line);
        }
        parts.join("\n")
    }

    /// Whether the request has passed its deadline at `now_millis`.
    pub fn is_expired(&self, now_millis: u64) -> bool {
        now_millis >= self.expires_at_millis
    }

    /// Looks up one of this request's actions by id.
    pub fn action(&self, id: &str) -> Option<&ApprovalAction> {
        self.actions.iter().find(|action| action.id == id)
    }

    /// Builds the `com.agentchat.approval.verdict.v1` message content for `action`.
    ///
    /// Every binding field is copied verbatim from the request; the server
    /// compares them field-by-field against its stored record.
    pub fn verdict_content(
        &self,
        action: &ApprovalAction,
        source_event_id: &EventId,
    ) -> serde_json::Value {
        serde_json::json!({
            "msgtype": APPROVAL_VERDICT_MSGTYPE,
            "body": action.label,
            APPROVAL_EVENT_KEY: {
                "version": 1,
                "kind": "verdict",
                "agent": self.agent,
                "project": self.project,
                "project_room_id": self.project_room_id,
                "request_id": self.request_id,
                "input_digest": self.input_digest,
                "action": action.id,
            },
            "m.relates_to": {
                "m.in_reply_to": {
                    "event_id": source_event_id.as_str(),
                }
            }
        })
    }
}

/// The current wall-clock time in milliseconds since the Unix epoch.
pub fn current_unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(u64::MAX)
}

fn is_lowercase_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn trimmed_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().map(str::trim)
}

/// Parses and validates the `actions` array of a request payload.
///
/// hagency always lists `approve_once` first and `deny` last, with the optional
/// scoped grants (`approve_task`, `approve_always`) in between. Anything else
/// is rejected wholesale so a malformed card never renders live buttons.
fn parse_actions(approval: &serde_json::Value) -> Option<Vec<ApprovalAction>> {
    let actions = approval.get("actions")?.as_array()?;
    if actions.len() < 2 || actions.len() > MAX_APPROVAL_ACTIONS {
        return None;
    }
    let mut parsed = Vec::with_capacity(actions.len());
    let mut seen_ids = HashSet::new();
    for action in actions {
        let id = trimmed_str(action, "id")?;
        let label = trimmed_str(action, "label")?;
        if id.is_empty() || label.is_empty() || !seen_ids.insert(id) {
            return None;
        }
        parsed.push(ApprovalAction {
            id: id.to_owned(),
            label: label.to_owned(),
            style: ActionStyle::parse(action.get("style").and_then(|value| value.as_str())),
        });
    }
    let first = parsed.first()?;
    let last = parsed.last()?;
    if first.id != "approve_once" || first.style != ActionStyle::Primary {
        return None;
    }
    if last.id != "deny" || last.style != ActionStyle::Danger {
        return None;
    }
    let middles_valid = parsed[1..parsed.len() - 1]
        .iter()
        .all(|action| matches!(action.id.as_str(), "approve_task" | "approve_always"));
    middles_valid.then_some(parsed)
}

fn parse_reusable_scope(approval: &serde_json::Value) -> Option<ReusableScope> {
    let scope = approval.get("reusable_scope")?;
    if !scope.is_object() {
        return None;
    }
    Some(ReusableScope {
        description: trimmed_str(scope, "description").unwrap_or_default().to_owned(),
        workspace: trimmed_str(scope, "workspace").unwrap_or_default().to_owned(),
        task_id: trimmed_str(scope, "task_id")
            .filter(|task_id| !task_id.is_empty())
            .map(str::to_owned),
    })
}

/// Parses an approval request from the **original** content of an
/// `m.room.message` event. Returns `None` for anything that isn't a
/// well-formed `com.agentchat.approval.request.v1`.
pub fn parse_approval_request(content: &serde_json::Value) -> Option<ApprovalRequest> {
    if content.get("msgtype").and_then(|value| value.as_str()) != Some(APPROVAL_REQUEST_MSGTYPE) {
        return None;
    }
    let approval = content.get(APPROVAL_EVENT_KEY)?;
    if approval.get("version").and_then(|value| value.as_u64()) != Some(1)
        || approval.get("kind").and_then(|value| value.as_str()) != Some("request")
    {
        return None;
    }
    let actions = parse_actions(approval)?;

    let agent = trimmed_str(approval, "agent")?;
    let project = trimmed_str(approval, "project")?;
    let project_room_id = trimmed_str(approval, "project_room_id")?;
    let request_id = trimmed_str(approval, "request_id")?;
    let upstream_request_id = trimmed_str(approval, "upstream_request_id")?;
    let input_digest = trimmed_str(approval, "input_digest")?;
    let runtime = trimmed_str(approval, "runtime")?;
    let tool_name = trimmed_str(approval, "tool_name")?;
    let description = trimmed_str(approval, "description")?;
    let input_preview = trimmed_str(approval, "input_preview")?;
    let expires_at_millis = approval.get("expires_at")?.as_u64()?;

    let request_suffix = request_id.strip_prefix("approval_")?;
    if agent.is_empty()
        || project.is_empty()
        || !project_room_id.starts_with('!')
        || !project_room_id.contains(':')
        || !is_lowercase_hex(request_suffix, 32)
        || upstream_request_id.is_empty()
        || !is_lowercase_hex(input_digest, 64)
        || !matches!(runtime, "claude" | "codex")
        || tool_name.is_empty()
        || expires_at_millis == 0
    {
        return None;
    }

    let reusable_scope = parse_reusable_scope(approval);
    // A scoped grant button without a scope to grant is a malformed card.
    let has_scoped_action = actions
        .iter()
        .any(|action| matches!(action.id.as_str(), "approve_task" | "approve_always"));
    if has_scoped_action && reusable_scope.is_none() {
        return None;
    }
    if actions.iter().any(|action| action.id == "approve_task")
        && reusable_scope.as_ref().is_none_or(|scope| scope.task_id.is_none())
    {
        return None;
    }

    Some(ApprovalRequest {
        agent: agent.to_owned(),
        project: project.to_owned(),
        project_room_id: project_room_id.to_owned(),
        request_id: request_id.to_owned(),
        upstream_request_id: upstream_request_id.to_owned(),
        input_digest: input_digest.to_owned(),
        runtime: runtime.to_owned(),
        tool_name: tool_name.to_owned(),
        description: description.to_owned(),
        input_preview: input_preview.to_owned(),
        expires_at_millis,
        reusable_scope,
        actions,
    })
}

/// For any of the three approval msgtypes, returns the plain-text `body`
/// to display, provided the structured payload is at least self-consistent
/// (`version == 1` and a `kind` matching the msgtype). Used to render
/// status/verdict notices as ordinary text, and as the fallback body for
/// a request whose card could not be parsed.
pub fn custom_message_body(content: &serde_json::Value) -> Option<&str> {
    let msgtype = content.get("msgtype")?.as_str()?;
    let expected_kind = match msgtype {
        APPROVAL_REQUEST_MSGTYPE => "request",
        APPROVAL_STATUS_MSGTYPE => "status",
        APPROVAL_VERDICT_MSGTYPE => "verdict",
        _ => return None,
    };
    let approval = content.get(APPROVAL_EVENT_KEY)?;
    if approval.get("version").and_then(|value| value.as_u64()) != Some(1)
        || approval.get("kind").and_then(|value| value.as_str()) != Some(expected_kind)
    {
        return None;
    }
    content.get("body")?.as_str()
}

/// What an approval message looks like once parsed for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalMessage {
    /// A well-formed request: render the card.
    Request(Box<ApprovalRequest>),
    /// A request whose structured payload failed validation: render the body
    /// text only, never any buttons (fail closed).
    MalformedRequest { body: String },
    /// A status or verdict notice: render the body text only.
    Notice { body: String },
}

impl ApprovalMessage {
    /// Classifies the original content of an `m.room.message` event.
    /// Returns `None` if the msgtype is not an agent-chat approval msgtype.
    pub fn from_original_content(content: &serde_json::Value) -> Option<Self> {
        let msgtype = content.get("msgtype")?.as_str()?;
        if !is_approval_msgtype(msgtype) {
            return None;
        }
        let body = custom_message_body(content)
            .or_else(|| content.get("body")?.as_str())
            .unwrap_or_default()
            .to_owned();
        if msgtype == APPROVAL_REQUEST_MSGTYPE {
            Some(match parse_approval_request(content) {
                Some(request) => Self::Request(Box::new(request)),
                None => Self::MalformedRequest { body },
            })
        } else {
            Some(Self::Notice { body })
        }
    }

    /// The plain text to show in the message bubble above (or instead of) the card.
    pub fn bubble_text(&self) -> String {
        match self {
            Self::Request(request) => format!("Agent {} is asking for approval.", request.agent),
            Self::MalformedRequest { body } | Self::Notice { body } => body.clone(),
        }
    }
}

/// The lifecycle of one approval card as far as this client is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecisionState {
    /// The user has not decided yet and the deadline has not passed.
    Pending,
    /// The deadline passed before the user decided.
    Expired,
    /// The user clicked a button and the verdict is being sent.
    Sending(ApprovalAction),
    /// The verdict was sent successfully.
    Sent(ApprovalAction),
}

/// Per-timeline UI state for approval cards: which requests have been
/// decided locally, and which are still in flight.
#[derive(Debug, Default)]
pub struct ApprovalUiState {
    /// Requests with live (undecided, unexpired) buttons currently drawn,
    /// keyed by the request event id. Used to schedule the expiry timer.
    live_requests: HashMap<OwnedEventId, u64>,
    /// Decisions the user has made in this session, keyed by request event id.
    decisions: HashMap<OwnedEventId, ApprovalDecisionState>,
}

impl ApprovalUiState {
    /// Records that a request with the given deadline is being drawn with live buttons.
    pub fn track_live(&mut self, event_id: &EventId, expires_at_millis: u64) {
        self.live_requests.insert(event_id.to_owned(), expires_at_millis);
    }

    /// Stops tracking a request (it was decided, expired, or scrolled away).
    pub fn untrack_live(&mut self, event_id: &EventId) {
        self.live_requests.remove(event_id);
    }

    /// The earliest deadline among live requests, if any.
    pub fn earliest_live_deadline_millis(&self) -> Option<u64> {
        self.live_requests.values().copied().min()
    }

    /// Removes every live request whose deadline has passed, returning their event ids.
    pub fn expire_live(&mut self, now_millis: u64) -> Vec<OwnedEventId> {
        let expired: Vec<OwnedEventId> = self
            .live_requests
            .iter()
            .filter(|(_, deadline)| now_millis >= **deadline)
            .map(|(event_id, _)| event_id.clone())
            .collect();
        for event_id in &expired {
            self.live_requests.remove(event_id);
        }
        expired
    }

    /// The locally-recorded decision for a request, if any.
    pub fn decision(&self, event_id: &EventId) -> Option<&ApprovalDecisionState> {
        self.decisions.get(event_id)
    }

    /// Records that the user chose `action` and the verdict is being sent.
    pub fn mark_sending(&mut self, event_id: &EventId, action: ApprovalAction) {
        self.live_requests.remove(event_id);
        self.decisions.insert(event_id.to_owned(), ApprovalDecisionState::Sending(action));
    }

    /// Records that the in-flight verdict for a request was sent successfully.
    pub fn mark_sent(&mut self, event_id: &EventId) {
        if let Some(ApprovalDecisionState::Sending(action)) = self.decisions.remove(event_id) {
            self.decisions.insert(event_id.to_owned(), ApprovalDecisionState::Sent(action));
        }
    }

    /// Records that the in-flight verdict failed to send, re-enabling the buttons.
    pub fn mark_send_failed(&mut self, event_id: &EventId) {
        if matches!(self.decisions.get(event_id), Some(ApprovalDecisionState::Sending(_))) {
            self.decisions.remove(event_id);
        }
    }

    /// Resolves the state to draw for `request` at `now_millis`.
    pub fn decision_state(
        &self,
        event_id: &EventId,
        request: &ApprovalRequest,
        now_millis: u64,
    ) -> ApprovalDecisionState {
        match self.decisions.get(event_id) {
            Some(state) => state.clone(),
            None if request.is_expired(now_millis) => ApprovalDecisionState::Expired,
            None => ApprovalDecisionState::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST_ID: &str = "approval_0123456789abcdef0123456789abcdef";
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn two_action_request(expires_at: u64) -> serde_json::Value {
        serde_json::json!({
            "msgtype": APPROVAL_REQUEST_MSGTYPE,
            "body": "Agent wf_codex wants to run Bash",
            APPROVAL_EVENT_KEY: {
                "version": 1,
                "kind": "request",
                "agent": "wf_codex",
                "project": "robrix2",
                "project_room_id": "!board:example.org",
                "request_id": REQUEST_ID,
                "upstream_request_id": "codex-42",
                "input_digest": DIGEST,
                "runtime": "codex",
                "tool_name": "Bash",
                "description": "Run the library tests",
                "input_preview": "cargo test --lib",
                "expires_at": expires_at,
                "actions": [
                    { "id": "approve_once", "label": "Approve once", "style": "primary" },
                    { "id": "deny", "label": "Deny", "style": "danger" }
                ]
            }
        })
    }

    fn four_action_request(expires_at: u64) -> serde_json::Value {
        let mut content = two_action_request(expires_at);
        let approval = content.get_mut(APPROVAL_EVENT_KEY).unwrap();
        approval["actions"] = serde_json::json!([
            { "id": "approve_once", "label": "Approve once", "style": "primary" },
            { "id": "approve_task", "label": "Allow for this task", "style": "secondary" },
            { "id": "approve_always", "label": "Always allow this operation", "style": "secondary" },
            { "id": "deny", "label": "Deny", "style": "danger" }
        ]);
        approval["reusable_scope"] = serde_json::json!({
            "description": "cargo test in this workspace",
            "workspace": "/srv/robrix2",
            "task_id": "task_7"
        });
        content
    }

    #[test]
    fn parses_a_two_action_request() {
        let request = parse_approval_request(&two_action_request(1_000)).unwrap();
        assert_eq!(request.agent, "wf_codex");
        assert_eq!(request.request_id, REQUEST_ID);
        assert_eq!(request.expires_at_millis, 1_000);
        assert_eq!(request.actions.len(), 2);
        assert_eq!(request.actions[0].id, "approve_once");
        assert_eq!(request.actions[1].id, "deny");
        assert!(request.reusable_scope.is_none());
        assert_eq!(request.title(), "Bash · codex");
        assert!(request.summary().contains("cargo test --lib"));
    }

    #[test]
    fn parses_a_four_action_scoped_request() {
        let request = parse_approval_request(&four_action_request(1_000)).unwrap();
        let ids: Vec<&str> = request.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["approve_once", "approve_task", "approve_always", "deny"]);
        let scope = request.reusable_scope.as_ref().unwrap();
        assert_eq!(scope.task_id.as_deref(), Some("task_7"));
        assert!(request.summary().contains("Reusable scope"));
    }

    #[test]
    fn rejects_scoped_actions_without_a_scope() {
        let mut content = four_action_request(1_000);
        content[APPROVAL_EVENT_KEY].as_object_mut().unwrap().remove("reusable_scope");
        assert!(parse_approval_request(&content).is_none());
    }

    #[test]
    fn rejects_approve_task_without_a_task_id() {
        let mut content = four_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["reusable_scope"].as_object_mut().unwrap().remove("task_id");
        assert!(parse_approval_request(&content).is_none());
    }

    #[test]
    fn rejects_unknown_or_misordered_actions() {
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["actions"] = serde_json::json!([
            { "id": "deny", "label": "Deny", "style": "danger" },
            { "id": "approve_once", "label": "Approve once", "style": "primary" }
        ]);
        assert!(parse_approval_request(&content).is_none());

        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["actions"] = serde_json::json!([
            { "id": "approve_once", "label": "Approve once", "style": "primary" },
            { "id": "run_anyway", "label": "Run anyway", "style": "secondary" },
            { "id": "deny", "label": "Deny", "style": "danger" }
        ]);
        assert!(parse_approval_request(&content).is_none());

        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["actions"] = serde_json::json!([
            { "id": "approve_once", "label": "Approve once", "style": "primary" }
        ]);
        assert!(parse_approval_request(&content).is_none());
    }

    #[test]
    fn rejects_padded_or_uppercase_ids() {
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["request_id"] = serde_json::json!("approval_0123456789ABCDEF0123456789abcdef");
        assert!(parse_approval_request(&content).is_none());
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["input_digest"] = serde_json::json!(format!("{DIGEST}0"));
        assert!(parse_approval_request(&content).is_none());
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["runtime"] = serde_json::json!("gemini");
        assert!(parse_approval_request(&content).is_none());
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["project_room_id"] = serde_json::json!("board:example.org");
        assert!(parse_approval_request(&content).is_none());
    }

    #[test]
    fn malformed_request_is_classified_fail_closed() {
        let mut content = two_action_request(1_000);
        content[APPROVAL_EVENT_KEY]["actions"] = serde_json::json!([]);
        match ApprovalMessage::from_original_content(&content).unwrap() {
            ApprovalMessage::MalformedRequest { body } => {
                assert_eq!(body, "Agent wf_codex wants to run Bash");
            }
            other => panic!("expected MalformedRequest, got {other:?}"),
        }
    }

    #[test]
    fn status_and_verdict_are_notices_without_actions() {
        let status = serde_json::json!({
            "msgtype": APPROVAL_STATUS_MSGTYPE,
            "body": "Agent wf_codex is waiting for approval from its owner.",
            APPROVAL_EVENT_KEY: {
                "version": 1, "kind": "status", "agent": "wf_codex", "project": "robrix2",
                "state": "waiting_for_owner",
                // Even if a status smuggled in buttons, they must never render.
                "actions": [
                    { "id": "approve_once", "label": "Approve once", "style": "primary" },
                    { "id": "deny", "label": "Deny", "style": "danger" }
                ]
            }
        });
        assert!(matches!(
            ApprovalMessage::from_original_content(&status).unwrap(),
            ApprovalMessage::Notice { .. }
        ));
        let verdict = serde_json::json!({
            "msgtype": APPROVAL_VERDICT_MSGTYPE,
            "body": "Approve once",
            APPROVAL_EVENT_KEY: { "version": 1, "kind": "verdict", "action": "approve_once" }
        });
        assert!(matches!(
            ApprovalMessage::from_original_content(&verdict).unwrap(),
            ApprovalMessage::Notice { body } if body == "Approve once"
        ));
        let plain = serde_json::json!({ "msgtype": "m.text", "body": "hi" });
        assert!(ApprovalMessage::from_original_content(&plain).is_none());
    }

    #[test]
    fn verdict_echoes_every_binding_field() {
        let request = parse_approval_request(&four_action_request(1_000)).unwrap();
        let action = request.action("approve_always").unwrap();
        let source = EventId::parse("$req:example.org").unwrap();
        let verdict = request.verdict_content(action, &source);
        assert_eq!(verdict["msgtype"], APPROVAL_VERDICT_MSGTYPE);
        assert_eq!(verdict["body"], "Always allow this operation");
        let detail = &verdict[APPROVAL_EVENT_KEY];
        assert_eq!(detail["version"], 1);
        assert_eq!(detail["kind"], "verdict");
        assert_eq!(detail["agent"], "wf_codex");
        assert_eq!(detail["project"], "robrix2");
        assert_eq!(detail["project_room_id"], "!board:example.org");
        assert_eq!(detail["request_id"], REQUEST_ID);
        assert_eq!(detail["input_digest"], DIGEST);
        assert_eq!(detail["action"], "approve_always");
        assert_eq!(verdict["m.relates_to"]["m.in_reply_to"]["event_id"], "$req:example.org");
        // No routing metadata or extra keys leak into the verdict.
        assert_eq!(verdict.as_object().unwrap().len(), 4);
    }

    #[test]
    fn expiry_and_decision_state_lifecycle() {
        let request = parse_approval_request(&two_action_request(1_000)).unwrap();
        let event_id = EventId::parse("$req:example.org").unwrap();
        let mut ui = ApprovalUiState::default();

        assert_eq!(ui.decision_state(&event_id, &request, 999), ApprovalDecisionState::Pending);
        assert_eq!(ui.decision_state(&event_id, &request, 1_000), ApprovalDecisionState::Expired);

        ui.track_live(&event_id, request.expires_at_millis);
        assert_eq!(ui.earliest_live_deadline_millis(), Some(1_000));
        assert!(ui.expire_live(999).is_empty());
        assert_eq!(ui.expire_live(1_000), vec![event_id.to_owned()]);
        assert_eq!(ui.earliest_live_deadline_millis(), None);

        let deny = request.action("deny").unwrap().clone();
        ui.track_live(&event_id, request.expires_at_millis);
        ui.mark_sending(&event_id, deny.clone());
        assert_eq!(ui.earliest_live_deadline_millis(), None);
        assert_eq!(
            ui.decision_state(&event_id, &request, 5_000),
            ApprovalDecisionState::Sending(deny.clone())
        );
        ui.mark_send_failed(&event_id);
        assert_eq!(ui.decision_state(&event_id, &request, 999), ApprovalDecisionState::Pending);

        ui.mark_sending(&event_id, deny.clone());
        ui.mark_sent(&event_id);
        assert_eq!(ui.decision_state(&event_id, &request, 5_000), ApprovalDecisionState::Sent(deny));
        // A late failure notification must not undo a recorded success.
        ui.mark_send_failed(&event_id);
        assert!(matches!(ui.decision(&event_id), Some(ApprovalDecisionState::Sent(_))));
    }
}
