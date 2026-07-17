#!/bin/bash
#
# Upload the GitHub Actions secrets used by the Robrix release workflow.
#
# Fill in packaging/release-secrets.env (copy it from the .example) and run:
#   ./packaging/upload-release-secrets.sh [path-to-env-file]
#
# Blank entries are skipped, so you can set up desktop signing first and add
# iOS later by filling in more fields and re-running.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="${1:-$SCRIPT_DIR/release-secrets.env}"

if ! command -v gh >/dev/null 2>&1; then
    echo "Error: the GitHub CLI (gh) is not installed. See https://cli.github.com" >&2
    exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
    echo "Error: not logged in to gh. Run 'gh auth login' first." >&2
    exit 1
fi
if [[ ! -f "$ENV_FILE" ]]; then
    echo "Error: $ENV_FILE not found." >&2
    echo "Copy packaging/release-secrets.env.example there and fill it in." >&2
    exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

REPO="${RELEASE_REPO:-project-robius/robrix}"
echo "==> Uploading secrets to $REPO"
errors=0

# Secret from a literal string. No trailing newline so passwords stay exact.
set_str() {
    local name="$1" value="${2:-}"
    if [[ -z "$value" ]]; then echo "  -  skip $name (blank)"; return; fi
    printf '%s' "$value" | gh secret set "$name" -R "$REPO"
    echo "  ok  set  $name"
}

# Secret from a file's exact contents (preserves newlines, e.g. the .p8 key).
set_file() {
    local name="$1" file="${2:-}"
    if [[ -z "$file" ]]; then echo "  -  skip $name (blank)"; return; fi
    if [[ ! -f "$file" ]]; then
        echo "  X  FAIL $name: file not found: $file" >&2
        errors=$(( errors + 1 )); return
    fi
    gh secret set "$name" -R "$REPO" < "$file"
    echo "  ok  set  $name (from $file)"
}

# Secret from the single-line base64 of a file (for .p12 / .mobileprovision).
set_file_b64() {
    local name="$1" file="${2:-}"
    if [[ -z "$file" ]]; then echo "  -  skip $name (blank)"; return; fi
    if [[ ! -f "$file" ]]; then
        echo "  X  FAIL $name: file not found: $file" >&2
        errors=$(( errors + 1 )); return
    fi
    base64 < "$file" | tr -d '\n' | gh secret set "$name" -R "$REPO"
    echo "  ok  set  $name (base64 of $file)"
}

echo "-- Desktop updater signing (Linux, Windows) --"
set_file CARGO_PACKAGER_SIGNING_KEY      "${CARGO_PACKAGER_SIGNING_KEY_FILE:-}"
set_str  CARGO_PACKAGER_SIGNING_PASSWORD "${CARGO_PACKAGER_SIGNING_PASSWORD:-}"

echo "-- macOS signing + notarization --"
set_file_b64 APPLE_CERTIFICATE          "${APPLE_CERTIFICATE_FILE:-}"
set_str      APPLE_CERTIFICATE_PASSWORD "${APPLE_CERTIFICATE_PASSWORD:-}"
set_str      APPLE_ID                   "${APPLE_ID:-}"
set_str      APPLE_PASSWORD             "${APPLE_PASSWORD:-}"
set_str      APPLE_TEAM_ID              "${APPLE_TEAM_ID:-}"

echo "-- iOS (optional) --"
set_file     APP_STORE_CONNECT_API_KEY_CONTENT "${APP_STORE_CONNECT_API_KEY_FILE:-}"
set_str      APP_STORE_CONNECT_KEY_ID          "${APP_STORE_CONNECT_KEY_ID:-}"
set_str      APP_STORE_CONNECT_ISSUER_ID       "${APP_STORE_CONNECT_ISSUER_ID:-}"
set_file_b64 APPLE_PROVISIONING_PROFILE        "${APPLE_PROVISIONING_PROFILE_FILE:-}"
set_str      APPLE_KEYCHAIN_PASSWORD           "${APPLE_KEYCHAIN_PASSWORD:-}"

echo "-- Release token --"
set_str ROBRIX_RELEASE "${ROBRIX_RELEASE:-}"

echo ""
if (( errors > 0 )); then
    echo "==> Done with $errors error(s). Fix the file path(s) above and re-run." >&2
    exit 1
fi
echo "==> Done. Verify with: gh secret list -R $REPO"
