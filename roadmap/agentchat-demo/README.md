# agentchat-demo — ready-to-drop artifacts

Concrete files implementing the **plain-text** robrix2 × agent-chat demo on local
Palpo (design: [`../robrix-agentchat-demo-integration-zh.md`](../robrix-agentchat-demo-integration-zh.md)).

All interface facts below were verified against the agent-chat repo and the
installed `agent-spec 0.2.7`.

## Files

| File | What it is | Where it actually goes |
|---|---|---|
| `issue-workflow/SKILL.md` | **The only new "code".** Shared skill; behavior branches on `whoami` name (wf_coordinator/wf_implementer/wf_reviewer/**wf_final_reviewer**). Read by Claude AND Codex agents (verified: Codex loads it from `~/.codex/skills`). | Symlinked into `~/.claude/skills/issue-workflow` + `~/.codex/skills/issue-workflow` (via `link-skill.sh`). Optionally copy into `agent-chat/skills/issue-workflow/`. |
| `agent-chat.env.demo` | Matrix/Palpo env block | Merge into `agent-chat/.env` |
| `register-accounts.mjs` | Pre-creates the bot + agent Matrix accounts via Palpo's `m.login.dummy` flow (bridge can't self-register here). Derives agent passwords exactly like the bridge. Idempotent. | run before first start (start-demo.sh calls it) |
| `link-skill.sh` | Symlinks the shared skill into agent homes | run as-is |
| `start-demo.sh` | Brings up backend+bridge+relay+**dashboard**, links skill, launches 4 agents (3 Claude + 1 **Codex** final-reviewer), prints robrix2 steps | run as-is (edit the 3 vars at top) |
| `nudge.sh` | Wake an agent whose inbox has a message push-relay's idle-gate didn't inject (`./nudge.sh wf_coordinator`) | run when a command shows "no response" |
| `workflow-board.mjs` | **Independent** zero-dep viewer (`http://127.0.0.1:8086`) for the DEMO_REPO's project/issues/specs/plans/notes, dark-card style, click-to-read. Does NOT touch agent-chat source. | started by start-demo.sh |

## Two dashboards

- **Agent Monitor** `http://127.0.0.1:8084` — agent-chat's own (`server.js`): live agents,
  queue, message flow, tasks. (We start it; we don't modify it.)
- **Workflow Board** `http://127.0.0.1:8086` — ours: the issue→spec→plan→implement→review
  artifacts the agents write into the sandbox repo, rendered as status-colored cards.

## Verified facts this is built on

- **MCP tools** (`agent-chat/lib/mcp-server-core.js`): `whoami()`,
  `send_message(to, summary, full, type?, priority?, reply_to?, attachments?, schema?)`,
  `check_inbox(kinds?) → {dm:[], group:[]}`, `post(group, summary, full, ...)`,
  `check_group(group, ...)`. The skill uses only these, by exact name/params.
- **No role flag.** `agent-up-v1` has no `--role`/`--system-prompt`; role = agent
  **name**, behavior = shared skill branching on `whoami`. Flags it DOES accept:
  `claude|codex`, `--project`, `--project-mode copy|symlink`, `--project-name`,
  `--home`, `--agent-id`, `--model`, `--extra-args`, `--fresh`, `--attach`,
  `--allow-shared-workspace`.
- **Bridge is started separately** — `node bridge-matrix.js` (or the
  `bridge-matrix.service` unit). It is **not** an `agent-up-v1` flag.
  (`--with-bridge` is an *install-time* flag of `install-full.sh`, not a runtime
  agent flag.)
- **Skill sync** (`bin/agentchat-sync-skills`) only links the single
  `skills/agent-chat` dir → so we link `issue-workflow` ourselves (`link-skill.sh`).
- **Agent accounts auto-register** (`@ac_*`) on first bridge use; only the
  **bot account** must be pre-created on Palpo (open registration is on).
- **Routing**: in a group room only **@mentioned** agents get the message in
  inbox; a **DM** room delivers every message to that agent with no mention.
  The mention token matches the agent's **short name** (`@wf_coordinator`), not the
  `ac_`-prefixed MXID.
- **agent-spec** (v0.2.7): **positional** file arg — `agent-spec lint <f> --min-score 0.7`,
  `agent-spec parse <f>` (NO `--path` flag). Frontmatter starts on line 1 with `spec:`
  (NO leading `---` fence — a leading `---` triggers a misleading "missing 'spec:'
  field"), closed by one `---`. Task specs need Given/When/Then scenarios in the body
  to score ≥0.7; a project contract scoring ~0% is normal.

## Post-audit corrections (adversarial pass over these files)

The artifacts were adversarially audited against the agent-chat code; fixes applied:
- **Groups, not raw rooms (was a dead-end):** a robrix2-created room is **not** an
  agent-chat group, and agents **cannot** create groups (needs `MATRIX_BRIDGE_SECRET`).
  → the human creates the group with the bridge command
  **`!mkgroup demoboard wf_coordinator wf_implementer wf_reviewer`** (`lib/bot-commands.js:626`),
  which makes the backend group **and a new Matrix room** with everyone invited.
  The wf_coordinator learns the group name at runtime from the inbound message's
  `group` field (never hardcoded).
- **`MATRIX_BRIDGE_SECRET` was missing** from `.env` (blocking) — `!mkgroup` →
  `POST /api/groups` needs it. Added (name per `.env.example:74`).
- **`[NOTIFICATION]` is a terminal string**, not an MCP event
  (`lib/push-relay-core.js:624`). The skill now says: when you see it, call
  `check_inbox()` voluntarily.
- **`send_message(to=...)` takes an agent NAME**, not a human — wf_coordinator replies
  to the human via `post(group, ...)` instead. `type` defaults to `inform`; use
  `type="request"` for delegations, `type="reply"` for answers.
- **Role = agent name** (no `--role` flag) → launched as
  `wf_coordinator`/`wf_implementer`/`wf_reviewer` (Claude) + `wf_final_reviewer` (**Codex**);
  the skill self-checks `whoami()` and matches `final` before `reviewer` (the codex name
  contains both).
- **start-demo.sh** now polls `GET /health` instead of a blind sleep, and passes
  `--allow-shared-workspace` to **all** agents (shared symlinked workspace).
- The bridge **auto-registers** `@agent-bridge` on first start; the manual
  register curl is only a fallback.

## Quick start

```bash
cd roadmap/agentchat-demo
# 1. Fill agent-chat/.env from agent-chat.env.demo
#    (set API_TOKEN, MATRIX_BRIDGE_SECRET, MATRIX_BOT_PASSWORD, MATRIX_AGENT_PASSWORD_SECRET).
# 2. Preflight (verifies Palpo :8128, tooling, env keys; re-run after backend is up to test groups):
AC_DIR=/Users/zhangalex/Work/Projects/consult/agent-chat ./preflight.sh
# 3. Launch backend→bridge→relay, link skill, start 4 agents (3 Claude + 1 Codex):
DEMO_REPO=/path/to/the/repo/agents/should/work/in ./start-demo.sh
# 4. In robrix2: invite @agent-bridge, then send:
#       !mkgroup demoboard wf_coordinator wf_implementer wf_reviewer wf_final_reviewer
#    Accept the invite to the new "demoboard" room, then:
#       @wf_coordinator /create-issue <title> | <description>
#       approve
```

> **robrix2 prerequisite for the `/create-issue` / `/go` / `/review` / `/status` workflow
> slash-commands:** build robrix2 with `cargo run --features agent_chat`, then turn on
> **Settings → Preferences → "Agent-chat support (experimental)"**. Both the Cargo feature
> and the runtime toggle are **off by default**. Details:
> [`../../docs/robrix-with-agentchat/README.md`](../../docs/robrix-with-agentchat/README.md) §2.

Full acceptance steps: see **`CHECKLIST.md`**.

## Open items to confirm on your machine (see design §8)

1. Palpo on host = `http://127.0.0.1:8128` (compose maps `8128:8008`; server_name +
   well-known both `127.0.0.1:8128`). VERIFIED reachable under OrbStack docker
   (`/_matrix/client/versions` responds). `preflight.sh` re-checks this + login.
2. Bot account: the bridge auto-registers `@agent-bridge` on first start; if it
   fails, use the fallback register curl in `start-demo.sh`.
3. `approve` is honored only from the issue opener — confirm the wf_coordinator reads
   the human's identity from the inbox message `from` field on your setup.
4. `!mkgroup` needs `MATRIX_BRIDGE_SECRET` to match bridge↔backend, else
   `POST /api/groups` is rejected (`backend-v2.js:9495 requireBridgeSecret`).
