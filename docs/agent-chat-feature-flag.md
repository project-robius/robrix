# Agent-chat workflow support — enabling & local verification

Robrix can show **agent-chat workflow slash-commands** (`/create-issue`, `/go`,
`/review`, `/status`) in rooms that contain a coordinator agent (a member whose
name is `coordinator` or ends with `_coordinator`, e.g. `wf_coordinator`,
`alpha_coordinator`). These let you drive an [agent-chat](https://github.com/) AI
agent team through an `issue → spec → plan → implement → review` workflow from a
normal Matrix chat.

The feature is **double-gated and off by default**, so default Robrix builds are
unchanged:

1. **Compile-time** — Cargo feature `agent_chat`. Without it, none of the
   workflow-command code is compiled in.
2. **Runtime** — a persisted toggle in **Settings → Preferences →
   "Agent-chat support (experimental)"** (off by default), only shown in
   `agent_chat`-feature builds.

The commands only appear when **both** gates are on **and** the room contains a
`*_coordinator` member.

---

## Build

```bash
# Default build — feature OFF (no agent-chat workflow support compiled in):
cargo run

# Agent-chat build — feature ON:
cargo run --features agent_chat
# release:
cargo build --release --features agent_chat
```

## Verify — lightweight (no agent-chat stack needed)

These check the gating itself; they only need a Matrix login + one room whose
member list contains a `*_coordinator`-named user.

| # | Build | Action | Expected |
|---|-------|--------|----------|
| 1 | `cargo run` (feature OFF) | open **Settings → Preferences** | **No** "Agent-chat support" toggle |
| 2 | `cargo run` (feature OFF) | in any room, type `/` | workflow commands **never** appear (upstream-native behavior) |
| 3 | `cargo run --features agent_chat` | open **Settings → Preferences** | "Agent-chat support (experimental)" toggle present, **off** by default |
| 4 | feature ON, toggle **off** | in a room with a `*_coordinator` member, type `/` | workflow commands **do not** appear |
| 5 | feature ON, toggle **on** | same room, type `/` | popup shows **`/create-issue` `/go` `/review` `/status`** under a "Workflow Commands" header |
| 6 | feature ON, toggle **on** | a room **without** any `*_coordinator` member, type `/` | workflow commands **do not** appear |

The toggle state persists across restarts (stored in `AppPreferences`).

## Verify — full end-to-end (drives a real agent team)

To exercise the commands against real agents (the agent actually files the issue,
writes the spec, implements, etc.), follow the complete local setup in
[`docs/robrix-with-agentchat/README.md`](./robrix-with-agentchat/README.md)
(local Palpo homeserver + agent-chat + tmux + Claude Code). Build Robrix with
`--features agent_chat` and turn the Settings toggle on, then in a room
containing `wf_coordinator` send e.g.:

```
@wf_coordinator /create-issue Title | description…
```

## Automated checks

```bash
cargo check                          # feature OFF (default / production)
cargo check --features agent_chat    # feature ON
cargo test  --lib                    # feature OFF
cargo test  --lib --features agent_chat   # feature ON (runs the cfg-gated workflow test)
```
All four pass. (Note: `cargo test` also needs the unrelated upstream test fixes
tracked in `issues/011`; see that issue if you build the test target from a merge
base that lacks them.)

## Where the gate lives

| Concern | Location |
|---|---|
| Cargo feature | `Cargo.toml` `[features] agent_chat = []` |
| Workflow command table + coordinator match (cfg-gated) | `src/shared/mentionable_text_input.rs` (`WORKFLOW_SLASH_COMMANDS`, `name_is_workflow_coordinator`, the workflow branch of `update_slash_command_list`) |
| Runtime setting (persisted) | `src/settings/app_preferences.rs` (`agent_chat_enabled`), persisted via `src/app.rs` |
| Settings toggle UI + i18n | `src/settings/app_settings.rs`, `resources/i18n/{en,zh-CN}.json` |
