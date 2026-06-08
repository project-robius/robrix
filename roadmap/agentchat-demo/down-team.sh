#!/usr/bin/env bash
set -euo pipefail

# Tear down ONE team: stop its 4 agents, kill their tmux sessions, and remove the
# project from the Workflow Board registry. Leaves the shared infra running.
#
# Usage:  TEAM=alpha ./down-team.sh

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$DEMO_DIR/common.sh"

TEAM="${TEAM:-}"
[ -n "$TEAM" ] || { echo "✗ TEAM is required (e.g. TEAM=alpha)"; exit 2; }

echo "== tearing down team '$TEAM' ==========================================="
while IFS= read -r name; do
  [ -n "$name" ] || continue
  "$AC_DIR/bin/agentchat" down "$name" 2>/dev/null && echo "  down $name" || echo "  (down $name: not running / already gone)"
  tmux kill-session -t "$name" 2>/dev/null || true
done < <(team_agents "$TEAM")

PROJECTS_JSON="$PROJECTS_JSON" node "$DEMO_DIR/projects-registry.mjs" remove "$TEAM" || true
echo "  (cowork registrations persist harmlessly — no unregister command; agent-ids are team-scoped)"
echo "Done. Shared infra still up; other teams unaffected."
