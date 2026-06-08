#!/usr/bin/env bash
# Shared config + helpers for the multi-team agent-chat demo scripts.
# SOURCE this file — do not run it. Sourced by start-infra.sh / add-team.sh / down-team.sh.
#
# Model: ONE shared agent-chat backend/bridge/push-relay/Agent-Monitor + ONE Workflow
# Board serve ALL teams. Each TEAM is a 4-agent squad (coordinator/implementer/reviewer
# on Claude + final_reviewer on Codex) bound to its own project repo, in its own group.

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AC_DIR="${AC_DIR:-/Users/zhangalex/Work/Projects/consult/agent-chat}"
SKILL_SRC="${SKILL_SRC:-$DEMO_DIR/issue-workflow}"
BACKEND_PORT="${AGENT_CHAT_BACKEND_PORT:-8090}"
BACKEND_URL="http://127.0.0.1:${BACKEND_PORT}"
WORKFLOW_BOARD_PORT="${WORKFLOW_BOARD_PORT:-8086}"
WEB_PORT="${AGENT_CHAT_WEB_PORT:-8084}"
# Registry the single Workflow Board reads to list/switch projects. add-team upserts,
# down-team removes. The board (workflow-board.mjs) reads the same path.
export PROJECTS_JSON="${PROJECTS_JSON:-$DEMO_DIR/projects.json}"

# Every team has these roles. final_reviewer runs on Codex; the rest on Claude.
# Arrays (not space-strings) so iteration never depends on unquoted word-splitting.
TEAM_ROLES_CLAUDE=(coordinator implementer reviewer)
TEAM_ROLE_CODEX=final_reviewer

# Print the 4 agent names for a team, one per line:
#   team_agents alpha → alpha_coordinator / alpha_implementer / alpha_reviewer / alpha_final_reviewer
team_agents() {
  local t="$1" r
  for r in "${TEAM_ROLES_CLAUDE[@]}" "$TEAM_ROLE_CODEX"; do printf '%s\n' "${t}_${r}"; done
}

# Export valid KEY=VALUE lines from $AC_DIR/.env (agent-chat has no dotenv; its node
# entrypoints rely on systemd EnvironmentFile, so we export by hand). We do NOT `source`
# the file: lines like `SLACK_BLOCKS=(disabled)` are valid systemd but break shell
# parsing, and `<...>` placeholders break agent-up's raw `source .env`. Sanitize once.
load_env() {
  if [ ! -f "$AC_DIR/.env" ]; then
    echo "✗ $AC_DIR/.env not found — copy from $DEMO_DIR/agent-chat.env.demo first" >&2
    return 1
  fi
  if grep -qE '^[A-Za-z_][A-Za-z0-9_]*=.*[<>]' "$AC_DIR/.env"; then
    cp -n "$AC_DIR/.env" "$AC_DIR/.env.bak.predemo" 2>/dev/null || true
    grep -vE '^[A-Za-z_][A-Za-z0-9_]*=.*[<>]' "$AC_DIR/.env" > "$AC_DIR/.env.tmp" \
      && mv "$AC_DIR/.env.tmp" "$AC_DIR/.env"
  fi
  while IFS= read -r _line || [ -n "$_line" ]; do
    case "$_line" in
      ''|'#'*) continue ;;
      [A-Za-z_]*=*) export "$_line" ;;
      *) : ;;
    esac
  done < "$AC_DIR/.env"
}
