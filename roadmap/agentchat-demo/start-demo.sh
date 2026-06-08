#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# robrix2 × agent-chat demo launcher (LOCAL PALPO, plain-text workflow)
#
# Brings up: agent-chat backend + matrix bridge + push-relay, links the shared
# issue-workflow skill, and launches wf_coordinator / wf_implementer / wf_reviewer agents
# bound to a target repo.
#
# PREREQUISITES (do once, by hand):
#   - Palpo running locally (CS-API at http://127.0.0.1:8128, open registration on).
#   - agent-chat installed at $AC_DIR with .env filled from agent-chat.env.demo
#     (MUST set API_TOKEN, MATRIX_BRIDGE_SECRET, MATRIX_BOT_PASSWORD, secrets).
#
# Edit the variables, then run:  ./start-demo.sh
# ─────────────────────────────────────────────────────────────────────────────

# Resolve this script's own dir BEFORE any cd (so helper paths stay correct).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

AC_DIR="${AC_DIR:-/Users/zhangalex/Work/Projects/consult/agent-chat}"
DEMO_REPO="${DEMO_REPO:-/Users/zhangalex/Work/Projects/FW/robius/robrix2}"  # repo agents work in
SKILL_SRC="${SKILL_SRC:-$SCRIPT_DIR/issue-workflow}"

BACKEND_PORT="${AGENT_CHAT_BACKEND_PORT:-8090}"
BACKEND_URL="http://127.0.0.1:${BACKEND_PORT}"

cd "$AC_DIR"
mkdir -p "$AC_DIR/.demo-logs"

# ── Clean slate ───────────────────────────────────────────────────────
# Re-running without cleanup leaves an OLD backend holding :8090, so the NEW
# backend hits EADDRINUSE and exits — then the bridge talks to the stale backend
# whose push-relay is gone, and agents never get the [NOTIFICATION] (inbox fills
# but nobody pokes the tmux pane). Kill prior services + agent tmux sessions first.
echo "== Step -1: clean up any prior demo processes ==========================="
pkill -f "backend-v2.js"   2>/dev/null || true
pkill -f "bridge-matrix.js" 2>/dev/null || true
pkill -f "push-relay.js"   2>/dev/null || true
pkill -f "node server.js"  2>/dev/null || true   # dashboard (:8084)
pkill -f "workflow-board.mjs" 2>/dev/null || true # workflow board (:8086)
for s in wf_coordinator wf_implementer wf_reviewer wf_final_reviewer; do
  tmux kill-session -t "$s" 2>/dev/null || true
done
sleep 2
if lsof -nP -iTCP:"${AGENT_CHAT_BACKEND_PORT:-8090}" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "  ⚠ port ${AGENT_CHAT_BACKEND_PORT:-8090} still held — waiting 3s more"; sleep 3
fi
echo "  cleaned (port ${AGENT_CHAT_BACKEND_PORT:-8090} free, old sessions gone)"
echo

# Load .env into the environment. agent-chat's node entrypoints have NO dotenv —
# they rely on systemd's EnvironmentFile=.env. Running them by hand bypasses that,
# so backend-v2.js would FATAL with "missing required API_TOKEN". Export every
# valid KEY=VALUE line. We do NOT `source` the file: .env.example-derived lines
# like `SLACK_BLOCKS=(disabled)` are valid systemd but break shell parsing
# (zsh treats `=(...)` as process substitution). Export only well-formed pairs.
if [ -f "$AC_DIR/.env" ]; then
  # Strip placeholder lines whose VALUE contains <...> (e.g.
  # `MATRIX_AGENT_PASSWORD_TEMPLATE=<your-bot-password>` copied from .env.example).
  # agent-chat's own bin/agent-up-v1 does a raw `source .env`, and a bare `<` is a
  # shell redirection → "syntax error near unexpected token `newline`" (Step 4).
  if grep -qE '^[A-Za-z_][A-Za-z0-9_]*=.*[<>]' "$AC_DIR/.env"; then
    echo "  sanitizing .env: removing placeholder line(s) with <…> that break 'source .env'"
    # back up once, then drop the offending lines in place
    cp -n "$AC_DIR/.env" "$AC_DIR/.env.bak.predemo" 2>/dev/null || true
    grep -vE '^[A-Za-z_][A-Za-z0-9_]*=.*[<>]' "$AC_DIR/.env" > "$AC_DIR/.env.tmp" && mv "$AC_DIR/.env.tmp" "$AC_DIR/.env"
  fi
  while IFS= read -r _line || [ -n "$_line" ]; do
    case "$_line" in
      ''|'#'*) continue ;;                       # skip blank + comments
      [A-Za-z_]*=*) export "$_line" ;;           # KEY=VALUE → export literally
      *) : ;;                                      # ignore anything else
    esac
  done < "$AC_DIR/.env"
  echo "loaded .env (API_TOKEN ${API_TOKEN:+set}${API_TOKEN:-MISSING}, HS=$MATRIX_HOMESERVER)"
else
  echo "✗ $AC_DIR/.env not found — copy from agent-chat.env.demo first"; exit 1
fi

echo "== Step 0: ensure agent-chat npm deps are installed ====================="
# Without node_modules, backend-v2.js dies on `import express` and /health never
# comes up (the Step 2 timeout you'd otherwise hit). Install once if missing.
if [ ! -d "$AC_DIR/node_modules/express" ]; then
  echo "  node_modules/express missing — running npm install (one-time)…"
  ( cd "$AC_DIR" && npm install ) || { echo "  ✗ npm install failed"; exit 1; }
else
  echo "  deps present ✓"
fi
echo

echo "== Step 1: pre-create Matrix accounts (bot + agents) ===================="
# Palpo registers via m.login.dummy (no token), but agent-chat's bridge only
# self-registers via the registration_token flow. So we create the accounts here
# with the dummy flow; the bridge then just LOGS IN. Idempotent.
AC_DIR="$AC_DIR" ENV_FILE="$AC_DIR/.env" node "$SCRIPT_DIR/register-accounts.mjs" || {
  echo "  ✗ account pre-creation failed — fix .env (MATRIX_BOT_PASSWORD / MATRIX_AGENT_PASSWORD_SECRET) and retry"; exit 1; }
echo

echo "== Step 2: start backend, then bridge + push-relay ======================"
node backend-v2.js >"$AC_DIR/.demo-logs/backend.log" 2>&1 &  echo "  backend  pid $!"

# Wait for backend /health (instead of a blind sleep — bridge needs it up).
echo -n "  waiting for backend $BACKEND_URL/health "
for i in $(seq 1 30); do
  if curl -fsS "$BACKEND_URL/health" >/dev/null 2>&1; then echo "ok"; break; fi
  echo -n "."; sleep 1
  [ "$i" -eq 30 ] && { echo " TIMEOUT — see .demo-logs/backend.log"; exit 1; }
done

node bridge-matrix.js >"$AC_DIR/.demo-logs/bridge.log" 2>&1 &  echo "  bridge   pid $!"
PUSH_RELAY_MODE=local node push-relay.js >"$AC_DIR/.demo-logs/relay.log" 2>&1 & echo "  relay    pid $!"
# Web dashboard (server.js → http://127.0.0.1:8084, proxies the backend).
node server.js >"$AC_DIR/.demo-logs/dashboard.log" 2>&1 & echo "  dashboard pid $! → http://127.0.0.1:${AGENT_CHAT_WEB_PORT:-8084}"
# Workflow Board (our independent viewer for issues/specs/plans in DEMO_REPO).
DEMO_REPO="$DEMO_REPO" PORT="${WORKFLOW_BOARD_PORT:-8086}" \
  node "$SCRIPT_DIR/workflow-board.mjs" >"$AC_DIR/.demo-logs/workflow-board.log" 2>&1 & \
  echo "  wf-board  pid $! → http://127.0.0.1:${WORKFLOW_BOARD_PORT:-8086}"
echo "  logs in $AC_DIR/.demo-logs/   (tail -f to watch)"
sleep 2

echo "== Step 3: link the shared issue-workflow skill into agent homes ========"
SKILL_SRC="$SKILL_SRC" "$SCRIPT_DIR/link-skill.sh"
"$AC_DIR/bin/agentchat-sync-skills" || true   # also (re)link agent-chat's own skill

echo "== Step 4: launch the agents (shared workspace) ========================="
# No --role flag exists; role = NAME, behavior = the shared skill branching on whoami.
# ALL need --allow-shared-workspace because they point at the same symlinked path.
# Three Claude agents + ONE Codex agent (wf_final_reviewer) — a different runtime/model
# on purpose, so the final sign-off is genuinely independent (adversarial diversity).
# (Verified live: codex boots, loads the issue-workflow skill from ~/.codex/skills,
#  receives push-relay [NOTIFICATION]s, and replies via the same MCP — full parity.)
launch_agent() {  # <name> <runtime>
  "$AC_DIR/bin/agentchat" up-v1 "$1" "$2" \
    --project "$DEMO_REPO" --project-mode symlink --allow-shared-workspace --fresh
  echo "  launched $1 ($2)"
}
for name in wf_coordinator wf_implementer wf_reviewer; do launch_agent "$name" claude; done
# Codex final gate. up-v1 for codex also captures a resume-id from ~/.codex/sessions,
# so this one can take a little longer than the claude launches — that's expected.
launch_agent wf_final_reviewer codex

echo "== Step 4.5: register agents into mempal cowork (peek + capture layer) ==="
# PRECISION LAYER (additive, optional). Lets reviewer READ implementer's live
# tmux session (cowork-tmux-peek) before judging, and SINK the verdict to durable
# memory (cowork-capture). We do NOT use cowork for messaging (push-relay owns the
# panes; cowork tmux-send would collide) — only peek (capture-pane, read-only) and
# capture (writes palace.db). Transport/messaging stays on agent-chat.
#
# ⚠ cwd identity (verified): cowork keys the bus by project_identity(--cwd), and a
# SYMLINK path hashes to a DIFFERENT bus than the real path. Agents run inside a
# symlinked workdir, so the skill MUST use the real repo root, not pwd. We register
# with the real $DEMO_REPO and persist that exact string to state.json for the skill.
if command -v mempal >/dev/null 2>&1; then
  COWORK_CWD="$(cd "$DEMO_REPO" && pwd -P)"   # canonical real path = the bus key
  reg_cowork() {  # <name> <tool>
    mempal cowork-register --agent-id "$1" --tool "$2" \
      --cwd "$COWORK_CWD" --transport tmux --tmux-target "$1:0.0" >/dev/null 2>&1 \
      && echo "  registered $1 ($2) → tmux $1:0.0" \
      || echo "  ⚠ register $1 failed (peek disabled for it; flow still works)"
  }
  for name in wf_coordinator wf_implementer wf_reviewer; do reg_cowork "$name" claude; done
  # peek/capture are runtime-agnostic CLI (mempal on PATH for codex too — verified);
  # --tool codex is just accurate bus metadata.
  reg_cowork wf_final_reviewer codex
  # Hand the skill the EXACT cwd string to pass to every cowork-* call (never pwd).
  mkdir -p "$DEMO_REPO/.agentchat-demo"
  printf '{ "cowork_cwd": "%s", "mempal_wing": "%s" }\n' \
    "$COWORK_CWD" "$(basename "$COWORK_CWD")" > "$DEMO_REPO/.agentchat-demo/cowork.json"
  echo "  cowork cwd → $COWORK_CWD  (written to .agentchat-demo/cowork.json)"
else
  echo "  mempal not on PATH — peek/capture disabled (transport + workflow unaffected)"
fi

echo
echo "== Step 5 (manual, in robrix2): ========================================"
cat <<'EOF'
  1. Log robrix2 into  http://127.0.0.1:8128  as your human account.
  2. Create a room and invite @agent-bridge (so the bot can take commands), OR
     DM the bot directly.
  3. Create the demo GROUP (a raw room is NOT an agent-chat group). In that room
     send the bridge command:
         !mkgroup demoboard wf_coordinator wf_implementer wf_reviewer wf_final_reviewer
     The bridge creates the backend group + a NEW Matrix room "demoboard" and
     invites you + the 4 agents. Accept the invite to "demoboard".
  4. In "demoboard", drive the workflow (@mention the wf_coordinator):
         @wf_coordinator /create-issue 登录闪退 | 点击登录按钮后崩溃
         approve
         @wf_coordinator /status
  5. Watch the pipeline post in "demoboard":
         wf_coordinator → wf_implementer → wf_reviewer (Claude, adversarial)
                        → wf_final_reviewer (CODEX, independent final gate) → done
     The Codex final-reviewer re-runs the build itself and signs off only after
     the first reviewer approves — a different model = a genuinely independent gate.

  Web dashboards:
    • Agent Monitor   http://127.0.0.1:8084   (agent-chat: agents, queue, messages)
    • Workflow Board  http://127.0.0.1:8086   (this demo: project, issues, specs, plans)
  If a command shows no response (agent busy → push-relay idle-gate held it),
  nudge it:   ./nudge.sh wf_coordinator   (works for wf_final_reviewer too)

  Stop:  kill the pids above, then
         agentchat down wf_coordinator|wf_implementer|wf_reviewer|wf_final_reviewer
EOF
echo "Done."
