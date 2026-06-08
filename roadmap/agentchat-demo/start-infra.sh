#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Shared infrastructure for the multi-team agent-chat × robrix2 demo. Run ONCE.
#
# Starts: agent-chat backend (:8090) + matrix bridge + push-relay + Agent Monitor
# (:8084) + ONE Workflow Board (:8086, multi-project switcher), links the shared
# issue-workflow skill, and ensures the @agent-bridge bot account exists.
#
# Teams are added separately and repeatedly:  ./add-team.sh   (see its header).
#
# PREREQS: Palpo at http://127.0.0.1:8128 (open registration); agent-chat at $AC_DIR
# with .env filled from agent-chat.env.demo.
# ─────────────────────────────────────────────────────────────────────────────

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$DEMO_DIR/common.sh"

cd "$AC_DIR"
mkdir -p "$AC_DIR/.demo-logs"

echo "== clean prior INFRA processes (team agents are left alone) =============="
pkill -f "backend-v2.js"   2>/dev/null || true
pkill -f "bridge-matrix.js" 2>/dev/null || true
pkill -f "push-relay.js"   2>/dev/null || true
pkill -f "node server.js"  2>/dev/null || true   # Agent Monitor (:8084)
pkill -f "workflow-board.mjs" 2>/dev/null || true # Workflow Board (:8086)
sleep 2
if lsof -nP -iTCP:"$BACKEND_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "  ⚠ port $BACKEND_PORT still held — waiting 3s more"; sleep 3
fi
echo "  infra cleaned (port $BACKEND_PORT free)"
echo

load_env || exit 1
echo "loaded .env (API_TOKEN ${API_TOKEN:+set}${API_TOKEN:-MISSING}, HS=${MATRIX_HOMESERVER:-?})"
echo

echo "== ensure agent-chat npm deps =========================================="
if [ ! -d "$AC_DIR/node_modules/express" ]; then
  echo "  installing (one-time)…"; ( cd "$AC_DIR" && npm install ) || { echo "  ✗ npm install failed"; exit 1; }
else
  echo "  deps present ✓"
fi
echo

echo "== ensure the @agent-bridge bot account (teams create their own agents) =="
# AGENTS="" → register-accounts creates ONLY the bot. Bridge can't self-register on
# Palpo (needs the dummy flow), so the bot must exist before the bridge logs in.
AGENTS="" AC_DIR="$AC_DIR" ENV_FILE="$AC_DIR/.env" node "$DEMO_DIR/register-accounts.mjs" || {
  echo "  ✗ bot account creation failed — check .env (MATRIX_BOT_PASSWORD)"; exit 1; }
echo

echo "== start backend → wait /health → bridge + push-relay + monitor + board =="
node backend-v2.js >"$AC_DIR/.demo-logs/backend.log" 2>&1 &  echo "  backend   pid $!"
echo -n "  waiting for backend $BACKEND_URL/health "
for i in $(seq 1 30); do
  if curl -fsS "$BACKEND_URL/health" >/dev/null 2>&1; then echo "ok"; break; fi
  echo -n "."; sleep 1
  [ "$i" -eq 30 ] && { echo " TIMEOUT — see .demo-logs/backend.log"; exit 1; }
done
node bridge-matrix.js >"$AC_DIR/.demo-logs/bridge.log" 2>&1 &  echo "  bridge    pid $!"
PUSH_RELAY_MODE=local node push-relay.js >"$AC_DIR/.demo-logs/relay.log" 2>&1 & echo "  relay     pid $!"
node server.js >"$AC_DIR/.demo-logs/dashboard.log" 2>&1 & echo "  monitor   pid $! → http://127.0.0.1:${WEB_PORT}"
# ONE Workflow Board for ALL teams — reads $PROJECTS_JSON and offers a project switcher.
PROJECTS_JSON="$PROJECTS_JSON" PORT="$WORKFLOW_BOARD_PORT" \
  node "$DEMO_DIR/workflow-board.mjs" >"$AC_DIR/.demo-logs/workflow-board.log" 2>&1 & \
  echo "  wf-board  pid $! → http://127.0.0.1:${WORKFLOW_BOARD_PORT}  (switch projects in-UI)"
sleep 2
echo

echo "== link the shared issue-workflow skill into agent homes ================"
SKILL_SRC="$SKILL_SRC" "$DEMO_DIR/link-skill.sh"
"$AC_DIR/bin/agentchat-sync-skills" || true
echo

echo "Infra up. Add one team per project, e.g.:"
echo "  TEAM=alpha DEMO_REPO=/abs/path/projA ./add-team.sh"
echo "  TEAM=beta  DEMO_REPO=/abs/path/projB ./add-team.sh"
echo "Logs: $AC_DIR/.demo-logs/   ·   Board: http://127.0.0.1:${WORKFLOW_BOARD_PORT}"
