#!/usr/bin/env bash
# Preflight checks for the robrix2 × agent-chat demo (local Palpo).
# Automates the "things to confirm on your machine" so you don't probe by hand.
#
# Usage:
#   ./preflight.sh                 # all checks (skips backend group test if backend down)
#   AC_DIR=/path ./preflight.sh    # custom agent-chat dir
#
# Exit code 0 = all REQUIRED checks passed. Non-zero = at least one failed.
# Checks marked [opt] never fail the run.

set -uo pipefail

AC_DIR="${AC_DIR:-/Users/zhangalex/Work/Projects/consult/agent-chat}"
ENV_FILE="${ENV_FILE:-$AC_DIR/.env}"
HS="${MATRIX_HOMESERVER_OVERRIDE:-http://127.0.0.1:8128}"   # Palpo CS-API on host
BACKEND="${BACKEND_OVERRIDE:-http://127.0.0.1:8090}"
SKILL_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/issue-workflow/SKILL.md"

pass=0; fail=0; warn=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; pass=$((pass+1)); }
no()   { printf '  \033[31m✗\033[0m %s\n' "$1"; fail=$((fail+1)); }
wn()   { printf '  \033[33m!\033[0m %s\n' "$1"; warn=$((warn+1)); }
hdr()  { printf '\n\033[1m%s\033[0m\n' "$1"; }

# Read a KEY=value from the env file (ignores comments). Empty if absent.
envval() { grep -E "^${1}=" "$ENV_FILE" 2>/dev/null | head -1 | cut -d= -f2- | sed 's/[[:space:]]*#.*$//' | xargs 2>/dev/null; }
is_placeholder() { case "$1" in ""|*"<"*">"*) return 0;; *) return 1;; esac; }

# ─────────────────────────────────────────────────────────────────────
hdr "1. Tooling"
command -v node  >/dev/null 2>&1 && ok "node $(node -v)"             || no "node not found"
command -v tmux  >/dev/null 2>&1 && ok "tmux $(tmux -V 2>/dev/null)" || no "tmux not found (agents run in tmux)"
command -v curl  >/dev/null 2>&1 && ok "curl present"                || no "curl not found"
if command -v agent-spec >/dev/null 2>&1; then
  ok "agent-spec $(agent-spec --version 2>/dev/null | head -1)"
  agent-spec parse --help            >/dev/null 2>&1 && ok "agent-spec parse available" || no "agent-spec parse missing"
  agent-spec lint  --help 2>&1 | grep -q -- "--min-score" && ok "agent-spec lint --min-score available" || no "agent-spec lint --min-score missing"
else
  no "agent-spec not on PATH (expected ~/.cargo/bin/agent-spec)"
fi

# ─────────────────────────────────────────────────────────────────────
hdr "2. Palpo CS-API @ $HS  (concern: :8128 login + MXID domain)"
VERS="$(curl -fsS --connect-timeout 3 --max-time 5 "$HS/_matrix/client/versions" 2>/dev/null)"
if [ -n "$VERS" ]; then
  ok "CS-API reachable (/_matrix/client/versions responds)"
else
  no "CS-API NOT reachable at $HS — is Palpo up? (compose maps 8128:8008)"
fi
WK="$(curl -fsS --connect-timeout 3 --max-time 5 "$HS/.well-known/matrix/client" 2>/dev/null)"
if [ -n "$WK" ]; then
  echo "$WK" | grep -q "127.0.0.1:8128" \
    && ok ".well-known/matrix/client points to 127.0.0.1:8128 (matches server_name)" \
    || wn ".well-known present but base_url != 127.0.0.1:8128 → $WK"
else
  wn "[opt] no /.well-known/matrix/client (fine if robrix2 is given the URL directly)"
fi

# ─────────────────────────────────────────────────────────────────────
hdr "3. agent-chat .env ($ENV_FILE)"
if [ -f "$ENV_FILE" ]; then
  ok ".env exists"
  # REQUIRED. API_TOKEN: backend-v2.js fails fast if empty (README:251) — pick any
  # non-empty string. BOT_PASSWORD / AGENT_PASSWORD_SECRET: you invent these; the
  # demo accounts are created from them. HOMESERVER / SERVER_NAME: the Palpo URL.
  for key in API_TOKEN MATRIX_BOT_PASSWORD MATRIX_AGENT_PASSWORD_SECRET MATRIX_HOMESERVER MATRIX_SERVER_NAME; do
    v="$(envval "$key")"
    if is_placeholder "$v"; then no "$key is unset/placeholder — fill it"; else ok "$key set"; fi
  done
  # OPTIONAL on localhost. MATRIX_BRIDGE_SECRET: only needed if you want the bridge
  # secret enforced; when blank the backend skips the check (createRequireBridgeSecret)
  # and 127.0.0.1 calls are already bearer-exempt (isLocalRequest). Blank = fine locally.
  v="$(envval MATRIX_BRIDGE_SECRET)"
  if is_placeholder "$v"; then wn "[opt] MATRIX_BRIDGE_SECRET blank — OK locally (check skipped when unset)"; else ok "MATRIX_BRIDGE_SECRET set"; fi
  hsv="$(envval MATRIX_HOMESERVER)"
  [ "$hsv" = "$HS" ] || wn "MATRIX_HOMESERVER='$hsv' (preflight assumes $HS)"
else
  no ".env not found — copy from agent-chat.env.demo into $ENV_FILE"
fi

# ─────────────────────────────────────────────────────────────────────
hdr "4. Bot login (concern: Palpo :8128 login actually works)"
BOTU="$(envval MATRIX_BOT_USERNAME)"; [ -n "$BOTU" ] || BOTU="agent-bridge"
BOTP="$(envval MATRIX_BOT_PASSWORD)"
if is_placeholder "$BOTP"; then
  wn "[opt] MATRIX_BOT_PASSWORD not set — skipping live login test"
else
  LOGIN="$(curl -fsS --connect-timeout 3 --max-time 8 -X POST "$HS/_matrix/client/v3/login" \
    -H 'Content-Type: application/json' \
    -d "{\"type\":\"m.login.password\",\"identifier\":{\"type\":\"m.id.user\",\"user\":\"$BOTU\"},\"password\":\"$BOTP\"}" 2>/dev/null)"
  if echo "$LOGIN" | grep -q '"access_token"'; then
    MXID="$(echo "$LOGIN" | grep -o '"user_id":"[^"]*"' | head -1 | cut -d'"' -f4)"
    ok "bot login OK → $MXID"
    echo "$MXID" | grep -q ":127.0.0.1:8128" && ok "MXID domain = 127.0.0.1:8128 (as expected)" || wn "MXID domain unexpected: $MXID"
  else
    wn "[opt] bot login failed (run ./register-accounts.mjs to pre-create accounts). Resp: $(echo "$LOGIN" | head -c 160)"
  fi
fi

# ─────────────────────────────────────────────────────────────────────
hdr "5. Skill linked into agent homes"
for t in "$HOME/.claude/skills/issue-workflow" "$HOME/.codex/skills/issue-workflow"; do
  if [ -e "$t/SKILL.md" ]; then ok "$t/SKILL.md present"; else wn "[opt] $t/SKILL.md missing — run ./link-skill.sh"; fi
done
[ -f "$SKILL_FILE" ] && ok "source skill present ($SKILL_FILE)" || no "source skill missing: $SKILL_FILE"

# ─────────────────────────────────────────────────────────────────────
hdr "6. Backend + MATRIX_BRIDGE_SECRET (concern: !mkgroup will succeed)"
if curl -fsS --connect-timeout 3 --max-time 5 "$BACKEND/health" >/dev/null 2>&1; then
  ok "backend /health responds at $BACKEND"
  SEC="$(envval MATRIX_BRIDGE_SECRET)"; TOK="$(envval API_TOKEN)"
  if is_placeholder "$SEC"; then
    wn "[opt] MATRIX_BRIDGE_SECRET unset — cannot test group creation"
  else
    G="_preflight_$$_$RANDOM"
    CODE="$(curl -s --connect-timeout 3 -o /tmp/pf_grp.$$ -w '%{http_code}' --max-time 8 -X POST "$BACKEND/api/groups" \
      -H 'Content-Type: application/json' -H "X-Bridge-Secret: $SEC" \
      ${TOK:+-H "Authorization: Bearer $TOK"} \
      -d "{\"name\":\"$G\",\"members\":[]}" 2>/dev/null)"
    if [ "$CODE" = "200" ] || [ "$CODE" = "201" ]; then
      ok "POST /api/groups accepted (MATRIX_BRIDGE_SECRET is correct) → !mkgroup will work"
      curl -s --connect-timeout 3 -o /dev/null --max-time 8 -X DELETE "$BACKEND/api/groups/$G" \
        -H "X-Bridge-Secret: $SEC" ${TOK:+-H "Authorization: Bearer $TOK"} 2>/dev/null
      ok "cleaned up test group"
    elif [ "$CODE" = "403" ]; then
      no "POST /api/groups → 403: MATRIX_BRIDGE_SECRET mismatch between this .env and the running backend"
    else
      no "POST /api/groups → HTTP $CODE ($(head -c 120 /tmp/pf_grp.$$ 2>/dev/null)) — check backend logs"
    fi
    rm -f /tmp/pf_grp.$$ 2>/dev/null
  fi
else
  wn "[opt] backend not running at $BACKEND — start it (or run start-demo.sh), then re-run preflight to test group creation"
fi

# ─────────────────────────────────────────────────────────────────────
hdr "Summary"
printf "  passed=%d  failed=%d  warnings=%d\n" "$pass" "$fail" "$warn"
if [ "$fail" -eq 0 ]; then
  printf "  \033[32mREQUIRED checks passed.\033[0m See CHECKLIST.md for the manual runtime checks (approve gating, full loop).\n"
  exit 0
else
  printf "  \033[31m%d required check(s) failed — fix before running the demo.\033[0m\n" "$fail"
  exit 1
fi
