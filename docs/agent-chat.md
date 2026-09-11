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
| **Rooms-list previews.** Approval events are custom msgtypes, so without help they fall through ruma's `_Custom` arm and print `[Custom message]: CustomMessageContent { msgtype: ... }` into the rooms list. The bridge's human-readable `body` is shown instead. | feature only | `src/event_preview.rs` |
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

## Wire namespace compatibility

The protocol was designed under `com.agentchat.*`, and that is what upstream
robrix2 and the hagency master parse. **Deployed forks rename the wire
namespace**, and a real soak against a live HAFleet deployment (v1.2.0) found it
emits `com.hafleet.approval.request.v1` with content key `com.hafleet.approval`
and no `com.agentchat.*` compatibility. A `com.agentchat.*`-only client renders
nothing against it.

This client is therefore namespace-agnostic (`Namespace` in
`src/agent_chat/approval.rs`): it accepts `com.agentchat.*`, `com.hafleet.*`, and
`com.hagency.*` for requests, status and verdicts, and **sends the verdict back
under the same namespace the request arrived in** — the wire name is the
bridge's to choose, not the client's.

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

Unit tests cover the protocol logic, including a fixture captured from a real
Palpo round-trip (`src/agent_chat/testdata/`):

```bash
cargo test --lib --features agent_chat agent_chat
```

### Driving the real app (`tools/agentchat-probe`)

`tools/agentchat-probe/probe_approval.py` drives a running Robrix against a real
homeserver through makepad's `--remote` HTTP control surface, which injects input
through the same path a human click takes. It asserts the card renders, clicks a
decision button, and then checks the homeserver for the resulting verdict event.

```bash
MAKEPAD_REMOTE=8099 ./target/debug/robrix <user> <password> <homeserver> &
python3 tools/agentchat-probe/probe_approval.py \
    --bridge 8099 --homeserver http://127.0.0.1:8128 \
    --room '!room:server' --request-id approval_<32hex> \
    --token <reader-token> --user <localpart>
```

`MAKEPAD_REMOTE=1` means *port 1*, not "enabled" — pass a real port. The probe
ends with `/gq` so it never leaves a test window behind.

> **Why not the headless renderer?** `MAKEPAD=headless` does not compile on macOS
> at the pinned makepad rev `493d23a`: `platform/src/os/cx_shared.rs:762` calls
> `crate::os::apple::metal::note_input_event()` under `#[cfg(target_vendor =
> "apple")]`, while `platform/src/os/mod.rs` gates `pub mod apple;` behind
> `not(headless)` — so the call survives and the module does not (E0433). The
> correct gate is `all(not(headless), target_vendor = "apple")`. This is an
> upstream makepad bug, not a robrix one; the remote bridge avoids it entirely.

### Soak results

Against a real Palpo homeserver with real accounts, a bridge account posting a
genuine 4-action request, and the client driven through the remote bridge:

- **17/17** wire checks on the request/verdict round-trip: every binding field
  preserved, `m.in_reply_to` intact, both senders stamped by the server.
- **15/15** end-to-end UI checks: room synced and listed, no debug-text leak in
  the rooms list, card showing all four buttons plus tool, command preview,
  Pending badge and the hint; clicking **Approve once** flipped the badge to
  Decided and put a real `com.agentchat.approval.verdict.v1` on the server,
  sent by the logged-in owner, echoing the `request_id` and `action`.
- An expired request rendered as **Expired** with its buttons withdrawn.

Against the **real HAFleet deployment on the remote mini** (2026-09-11):

- **6/6** server-side checks on the live backend's approval state machine: owner
  binding, forged-sender verdict rejected (`senderMxid_mismatch`), tampered-digest
  rejected (`inputDigest_mismatch`), owner verdict approved, replayed verdict
  rejected (`not_pending`), and verdict without the bridge secret refused (403).
- The live bridge's `buildOwnerApprovalRequest` emits the `com.hafleet.*`
  namespace; this client, made namespace-agnostic, renders it and approves it
  (**3/3** UI checks, verdict returned as `com.hafleet.approval.verdict.v1`).

Known cosmetic nit: the decided-state receipt uses `✓` (U+2713), which the
bundled font substitutes with a similar glyph.

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
