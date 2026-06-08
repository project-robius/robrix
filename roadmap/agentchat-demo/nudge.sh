#!/usr/bin/env bash
# Wake an agent that has unread inbox but wasn't poked by push-relay.
#
# WHY: push-relay has an IDLE-GATE (lib/push-relay-core.js ~860) — it HOLDS a
# notification for an agent that is "actively working" and only injects once the
# agent is idle enough. If you send a command (e.g. `approve`) while the agent is
# mid-turn, the message lands in its inbox but the tmux pane is never poked, so
# the agent looks unresponsive. This injects the [NOTIFICATION] yourself.
#
# Usage:
#   ./nudge.sh wf_coordinator
#   ./nudge.sh wf_coordinator "custom instruction text"
#
# With no custom text it just tells the agent to call check_inbox() and act.

set -euo pipefail
AGENT="${1:?usage: ./nudge.sh <agent-tmux-session> [message]}"
MSG="${2:-[NOTIFICATION] You have unread inbox message(s). FIRST ACTION: call check_inbox() now, then act on them per your role (issue-workflow skill). Do not wait.}"

if ! tmux has-session -t "$AGENT" 2>/dev/null; then
  echo "✗ no tmux session '$AGENT' (run start-demo.sh first?)"; exit 1
fi

# -l sends the literal text; then Enter / C-m to submit (Claude Code TUI needs a
# real submit — two forms for reliability across prompt states).
tmux send-keys -t "$AGENT" -l "$MSG"
sleep 1
tmux send-keys -t "$AGENT" Enter
sleep 1
tmux send-keys -t "$AGENT" C-m
echo "✓ nudged $AGENT — attach to watch:  tmux attach -t $AGENT"
