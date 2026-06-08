#!/usr/bin/env bash
set -euo pipefail

# Symlink the shared `issue-workflow` skill into the Claude Code AND Codex skill
# dirs so every agent-chat agent (wf_coordinator/wf_implementer/wf_reviewer) loads it.
#
# WHY THIS EXISTS: agent-chat's own `agentchat-sync-skills` is hardcoded to sync
# ONLY skills/agent-chat (TEMPLATE=skills/agent-chat/SKILL.md). It does not pick
# up additional skills, so we link this one ourselves, mirroring its approach.
#
# Usage:
#   ./link-skill.sh                 # link from this repo's copy
#   SKILL_SRC=/abs/path ./link-skill.sh   # link from a custom location
#
# Recommended: copy roadmap/agentchat-demo/issue-workflow into the agent-chat
# repo (agent-chat/skills/issue-workflow) so it lives beside the other skill,
# then point SKILL_SRC at it. But linking directly from here works too.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_SRC="${SKILL_SRC:-$SCRIPT_DIR/issue-workflow}"

if [[ ! -f "$SKILL_SRC/SKILL.md" ]]; then
  echo "[link-skill] ERROR: $SKILL_SRC/SKILL.md not found" >&2
  exit 1
fi

link_one() {
  local target="$1"   # e.g. $HOME/.claude/skills/issue-workflow
  mkdir -p "$(dirname "$target")"
  [[ -L "$target" || -e "$target" ]] && rm -rf "$target"
  ln -s "$SKILL_SRC" "$target"
  echo "[link-skill] linked $target -> $SKILL_SRC"
}

link_one "$HOME/.claude/skills/issue-workflow"
link_one "$HOME/.codex/skills/issue-workflow"

echo "[link-skill] done. Re-run after moving the source."
