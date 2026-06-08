#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Add ONE project + its dedicated 4-agent team to the running demo. Repeatable.
# Requires the shared infra to be up first:  ./start-infra.sh
#
# Usage:
#   TEAM=alpha DEMO_REPO=/abs/path/to/repo [MODE=symlink|copy] ./add-team.sh
#
#   TEAM       short team/project id (a-z0-9_). Agents = ${TEAM}_{coordinator,
#              implementer,reviewer,final_reviewer}; group = ${TEAM}-board.
#   DEMO_REPO  absolute path to the project's git repo the team works in.
#   MODE       symlink (default; agents edit the REAL repo) | copy (agents edit a copy).
# ─────────────────────────────────────────────────────────────────────────────

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$DEMO_DIR/common.sh"

TEAM="${TEAM:-}"
DEMO_REPO="${DEMO_REPO:-}"
MODE="${MODE:-symlink}"

[ -n "$TEAM" ] || { echo "✗ TEAM is required (e.g. TEAM=alpha)"; exit 2; }
[ -n "$DEMO_REPO" ] || { echo "✗ DEMO_REPO is required (absolute path to the repo)"; exit 2; }
case "$TEAM" in *[!a-z0-9_]*) echo "✗ TEAM must be [a-z0-9_] (got '$TEAM')"; exit 2;; esac
[ -d "$DEMO_REPO" ] || { echo "✗ DEMO_REPO not a directory: $DEMO_REPO"; exit 2; }
case "$MODE" in symlink|copy) : ;; *) echo "✗ MODE must be symlink|copy"; exit 2;; esac

COWORK_CWD="$(cd "$DEMO_REPO" && pwd -P)"   # canonical real path = git root + cowork bus key
if ! git -C "$COWORK_CWD" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "⚠ $COWORK_CWD is not a git repo — the review step uses 'git diff'."
  echo "  init it first:  git -C \"$COWORK_CWD\" init && git -C \"$COWORK_CWD\" add -A && git -C \"$COWORK_CWD\" commit -m init"
  exit 2
fi

cd "$AC_DIR"
load_env || exit 1

# Fail fast if the shared infra isn't up.
if ! curl -fsS "$BACKEND_URL/health" >/dev/null 2>&1; then
  echo "✗ backend $BACKEND_URL/health not responding — run ./start-infra.sh first"; exit 1
fi

AGENT_LIST="$(team_agents "$TEAM" | tr '\n' ' ')"
echo "== team '$TEAM' → project $COWORK_CWD  (mode=$MODE) ====================="
echo "   agents: $AGENT_LIST"
echo

echo "== 1/4 pre-create the team's Matrix accounts (+ friendly display names) =="
AGENTS="$AGENT_LIST" AC_DIR="$AC_DIR" ENV_FILE="$AC_DIR/.env" node "$DEMO_DIR/register-accounts.mjs" || {
  echo "  ✗ account creation failed for team '$TEAM'"; exit 1; }
echo

echo "== 2/4 launch the 4 agents (3 Claude + 1 Codex final gate) =============="
launch_agent() {  # <name> <runtime>
  "$AC_DIR/bin/agentchat" up-v1 "$1" "$2" \
    --project "$DEMO_REPO" --project-mode "$MODE" --allow-shared-workspace --fresh
  echo "  launched $1 ($2)"
}
for r in "${TEAM_ROLES_CLAUDE[@]}"; do launch_agent "${TEAM}_${r}" claude; done
launch_agent "${TEAM}_${TEAM_ROLE_CODEX}" codex   # codex up-v1 is a bit slower (resume-id capture)
echo

echo "== 3/4 register the team into mempal cowork (peek + capture) ============"
if command -v mempal >/dev/null 2>&1; then
  reg_cowork() {  # <name> <tool>
    mempal cowork-register --agent-id "$1" --tool "$2" \
      --cwd "$COWORK_CWD" --transport tmux --tmux-target "$1:0.0" >/dev/null 2>&1 \
      && echo "  registered $1 ($2)" || echo "  ⚠ register $1 failed (peek disabled; flow ok)"
  }
  for r in "${TEAM_ROLES_CLAUDE[@]}"; do reg_cowork "${TEAM}_${r}" claude; done
  reg_cowork "${TEAM}_${TEAM_ROLE_CODEX}" codex
  mkdir -p "$COWORK_CWD/.agentchat-demo"
  printf '{ "cowork_cwd": "%s", "mempal_wing": "%s" }\n' \
    "$COWORK_CWD" "$(basename "$COWORK_CWD")" > "$COWORK_CWD/.agentchat-demo/cowork.json"
  echo "  cowork.json → $COWORK_CWD/.agentchat-demo/cowork.json"
else
  echo "  mempal not on PATH — peek/capture disabled (transport + workflow unaffected)"
fi
echo

echo "== 4/4 register the project on the Workflow Board (:$WORKFLOW_BOARD_PORT) "
PROJECTS_JSON="$PROJECTS_JSON" node "$DEMO_DIR/projects-registry.mjs" upsert "$TEAM" "$COWORK_CWD"
echo

echo "== team '$TEAM' ready. In robrix2: ====================================="
echo "  !mkgroup ${TEAM}-board $AGENT_LIST"
echo "  (accept the invite, then drive it:)"
echo "  @${TEAM}_coordinator /create-issue <title> | <description>"
echo "  approve"
echo "  Board: http://127.0.0.1:${WORKFLOW_BOARD_PORT}  → switch to project '${TEAM}'"
echo "  Nudge if quiet: ./nudge.sh ${TEAM}_coordinator"
echo "  Tear down:      TEAM=${TEAM} ./down-team.sh"
