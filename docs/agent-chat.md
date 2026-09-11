# Agent-chat / hagency support

Robrix can act as the human-facing client for the
[hagency](https://github.com/hagency-org/hagency) control plane (a fork of
[agent-chat](https://github.com/shisuiki/agent-chat)), which runs Claude Code
and Codex coding agents and exposes them in Matrix rooms through a bridge bot.

Everything described here is compiled only with the `agent_chat` Cargo
feature. Default builds contain none of it: the timeline and settings DSL
reference placeholder widgets from `src/agent_chat_dummy.rs` that render as
empty views.

```bash
cargo run --features agent_chat
cargo build --release --features agent_chat
```

## What it does

| Capability | Gate | Where |
|---|---|---|
| **Owner approval cards.** Requests sent by the bridge as `com.agentchat.approval.request.v1` render as a card with the tool, command preview, expiry, and the decision buttons the bridge offered (`approve_once`, optional `approve_task` / `approve_always`, `deny`). Clicking one sends `com.agentchat.approval.verdict.v1` echoing every binding field. Status and verdict notices render as plain text and never carry buttons. | feature only | `src/agent_chat/approval.rs`, `approval_card.rs`; hooks in `src/home/room_screen.rs` |
| **Verdict delivery.** Before sending, the client refreshes the bridge bot's device keys and rotates the room's outbound Megolm session so a bridge device registered after the session began can decrypt the verdict. The verdict is sent directly, bypassing the send queue. | feature only | `MatrixRequest::SendAgentChatApprovalVerdict` in `src/sliding_sync.rs` |
| **Timeline filter.** The SDK's default event filter drops unknown msgtypes; the three approval msgtypes are allowed through. | feature only | `robrix_timeline_event_filter` in `src/sliding_sync.rs` |
| **Companion bridge invites.** Inviting `@ac_<team>_<role>:server` also invites `@agent-bridge-<team>:server` and the legacy `@agent-bridge:server`, best-effort. | feature only | `src/agent_chat/agents.rs`; `MatrixRequest::InviteUser` |
| **Workflow slash commands.** `/create-issue`, `/go`, `/review`, `/status` are offered in the `/` popup when the room contains a `*_coordinator` agent, and are sent as plain text for that agent to interpret. | feature + Settings toggle + coordinator present | `src/agent_chat/workflow.rs`; `src/shared/mentionable_text_input.rs` |
| **Thread-session slash commands.** `/task @agent …` and `/thread model|mode …` are offered when the room contains any agent puppet; the hagency backend parses them. | feature + Settings toggle + agent present | same |
| **Agent message presentation.** Messages from `@ac_*` accounts get a badge after the sender name with the agent's workflow role (from its account name) and the message kind the bridge stamped (`📋` request, `↩️` reply, `ℹ️` info). The kind marker and the trailing `🔗 permalink` line are stripped from the body. | feature only | `src/agent_chat/presentation.rs`; hooks in `src/home/room_screen.rs` |
| **Settings toggle.** Settings → Preferences → "Agent-chat (experimental)". Persisted in `AppPreferences::agent_chat_enabled`. | feature only | `src/agent_chat/preferences.rs`, `src/settings/` |

## Security model

Robrix is a presentation surface only. It never decides authorization:

- Binding fields (`agent`, `project`, `project_room_id`, `request_id`,
  `input_digest`) are read only from the **original** event content, never
  from an `m.replace` edit, and are echoed verbatim in the verdict.
- A request whose payload fails validation (bad ids, unknown actions, scoped
  actions without a `reusable_scope`, a status notice smuggling buttons) renders
  with **no** buttons. Fail closed.
- Expiry is enforced locally with a timer so a stale card cannot be clicked,
  and again server-side.
- hagency validates the verdict's real `event.sender`, the room, every
  binding field, expiry, and single-use consumption. Text replies are not
  approvals; the card says so.

## Wire format

Request (`com.agentchat.approval.request.v1`), under the `com.agentchat.approval` key:

```json
{
  "version": 1, "kind": "request",
  "agent": "wf_codex", "project": "robrix2", "project_room_id": "!board:example.org",
  "request_id": "approval_<32 hex>", "upstream_request_id": "…",
  "input_digest": "<64 hex>", "runtime": "claude|codex",
  "tool_name": "Bash", "description": "…", "input_preview": "…",
  "expires_at": 1757500000000,
  "reusable_scope": { "description": "…", "workspace": "…", "task_id": "…" },
  "actions": [
    { "id": "approve_once",   "label": "Approve once",                "style": "primary" },
    { "id": "approve_task",   "label": "Allow for this task",         "style": "secondary" },
    { "id": "approve_always", "label": "Always allow this operation", "style": "secondary" },
    { "id": "deny",           "label": "Deny",                        "style": "danger" }
  ]
}
```

Verdict (`com.agentchat.approval.verdict.v1`) sent by Robrix:

```json
{
  "msgtype": "com.agentchat.approval.verdict.v1",
  "body": "<button label>",
  "com.agentchat.approval": {
    "version": 1, "kind": "verdict",
    "agent": "…", "project": "…", "project_room_id": "…",
    "request_id": "…", "input_digest": "…", "action": "approve_once"
  },
  "m.relates_to": { "m.in_reply_to": { "event_id": "<request event>" } }
}
```

## Verifying

Unit tests cover the protocol logic:

```bash
cargo test --lib --features agent_chat agent_chat
```

Manual checks against a running hagency stack (see hagency's
`docs/E2E-RUNBOOK-macos.md`):

1. Build with the feature, log in, and open the `Approval: <agent>` room the
   bridge created for you. A pending request shows the card with live buttons.
2. Click **Approve once**. The card shows "Sending…" then "Decided" with the
   chosen button as a receipt; the agent's runtime continues.
3. Let a request expire. The badge flips to **Expired** and the buttons vanish
   without any interaction.
4. In the project room, the redacted "waiting for approval" status renders as
   plain text with no buttons.
5. In a room containing `wf_coordinator`, with the toggle on, typing `/` lists
   the four workflow commands plus `/task` and `/thread`; `/status` sends as
   plain text. With the toggle off, or in a room without agents, none appear.
6. Inviting `@ac_wf_coordinator:server` also invites `@agent-bridge-wf:server`.
7. A message from `@ac_wf_reviewer` whose body starts with `↩️` shows a
   `reviewer · reply` badge after the name, without the emoji or the trailing
   permalink line in the bubble.

## Not yet ported from robrix2

- Long-reply folding and the MSC4357 streaming animation for agent messages.
- `com.hagency.agent_ops.*` client sessions (hagency still marks these as
  development-only, and its namespace is still in flux).
- Octos AppService and BotFather tooling, which is unrelated to hagency.
