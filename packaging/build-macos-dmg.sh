#!/bin/bash
#
# Build a fully codesigned, notarized, and stapled macOS DMG for Robrix,
# with the Applications-folder icon fix applied.
#
# Why this script exists:
#   1. cargo-packager's built-in DMG output triggers a macOS Tahoe bug
#      where the Applications folder icon is invisible. We have to fix
#      the DMG layout post-build, which invalidates any DMG signature.
#   2. cargo-packager 0.10.1 hard-codes `--timestamp` with no retry, and
#      Apple's timestamp service occasionally returns "A timestamp was
#      expected but was not found." When that happens, cargo-packager
#      dies and a fresh build re-hits the same flaky service.
#   3. cargo-packager's error reporting (shell.rs:86) reads `errno`
#      after a failed subprocess, which is garbage data -- it surfaces
#      as the misleading "File exists (os error 17)".
#
# Strategy: do all codesign and notarization ourselves so we can retry
# on transient timestamp failures. cargo-packager is reduced to building
# the unsigned .app and DMG layout.
#
# Flow:
#   1. Comment out signing_identity in Cargo.toml so cargo-packager
#      skips both codesign and notarize entirely.
#   2. Run `cargo packager --release` with APPLE_* unset. Produces an
#      unsigned .app and unsigned DMG.
#   3. Codesign the standalone .app (binary first, then bundle) with
#      hardened runtime + entitlements + timestamp, retrying on
#      timestamp-service transient failures.
#   4. Apply the Applications-folder icon fix to the DMG.
#   5. Mount the fixed DMG read-write, replace the unsigned .app inside
#      with our signed copy, recompress.
#   6. Codesign the DMG itself (with retry on timestamp failures).
#   7. Submit the DMG to Apple's notary service via xcrun notarytool.
#   8. Staple the notarization ticket and verify with spctl.
#
# Required environment variables (none are written into this script):
#   APPLE_ID        Apple ID email used for notarization
#   APPLE_PASSWORD  App-specific password for that Apple ID
#   APPLE_TEAM_ID   Apple Developer Team ID
#
# The Developer ID signing certificate name is read from
# package.metadata.packager.macos.signing_identity in Cargo.toml.
#
# Usage:
#   APPLE_ID=… APPLE_PASSWORD=… APPLE_TEAM_ID=… ./packaging/build-macos-dmg.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CARGO_TOML="$PROJECT_DIR/Cargo.toml"
ENTITLEMENTS="$PROJECT_DIR/packaging/Entitlements.plist"
BG_IMAGE="$PROJECT_DIR/packaging/Robrix macOS dmg background.png"

cd "$PROJECT_DIR"

# --- Validate required env vars and config files ------------------------------

for var in APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do
    if [[ -z "${!var:-}" ]]; then
        echo "Error: $var is not set." >&2
        echo "Required env vars: APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID" >&2
        exit 1
    fi
done

if [[ ! -f "$ENTITLEMENTS" ]]; then
    echo "Error: Entitlements file not found at $ENTITLEMENTS" >&2
    exit 1
fi
if [[ ! -f "$BG_IMAGE" ]]; then
    echo "Error: DMG background image not found at $BG_IMAGE" >&2
    exit 1
fi

# Read signing_identity from Cargo.toml. Use [[:space:]] -- BSD sed on macOS
# does not understand \s.
SIGNING_IDENTITY=$(sed -n 's/^signing_identity[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$CARGO_TOML")
if [[ -z "$SIGNING_IDENTITY" ]]; then
    echo "Error: signing_identity not found in Cargo.toml -- required for notarization." >&2
    exit 1
fi

# --- Codesign helper with timestamp-failure retry -----------------------------
#
# Apple's timestamp service occasionally returns "A timestamp was expected
# but was not found." That's transient -- the next attempt, possibly
# minutes later, usually succeeds. We retry with exponential backoff.
#
# kind=app : adds --entitlements + --options runtime (for .app bundles
#            and Mach-O binaries). Required for hardened runtime.
# kind=dmg : timestamp-only (codesigning a DMG file).

codesign_with_retry() {
    local target="$1"
    local kind="$2"
    local cs_args=(--force --sign "$SIGNING_IDENTITY" --timestamp)
    if [[ "$kind" == "app" ]]; then
        cs_args+=(--entitlements "$ENTITLEMENTS" --options runtime)
    fi

    local max_attempts=5
    local attempt=1
    local delay=15
    local logfile
    logfile=$(mktemp)

    while (( attempt <= max_attempts )); do
        if codesign "${cs_args[@]}" "$target" >"$logfile" 2>&1; then
            # Show codesign's stderr lines (e.g. "replacing existing signature")
            # so the user can see what happened.
            cat "$logfile" >&2
            rm -f "$logfile"
            return 0
        fi

        # Codesign exited non-zero. Print what it said.
        cat "$logfile" >&2

        # Anything mentioning "timestamp" in a failure is the Apple
        # timestamp service flaking -- retry. Other failures are real
        # codesign errors and should not retry.
        if grep -qi 'timestamp' "$logfile"; then
            if (( attempt < max_attempts )); then
                echo "  -> Apple timestamp service transient failure; sleeping ${delay}s before retry $((attempt+1))/${max_attempts}..." >&2
                sleep "$delay"
                delay=$(( delay * 2 ))
            fi
            attempt=$(( attempt + 1 ))
        else
            echo "  -> codesign failed with a non-transient error; giving up." >&2
            rm -f "$logfile"
            return 1
        fi
    done

    echo "  -> codesign still failing after ${max_attempts} attempts; Apple's timestamp service is down. Try again later." >&2
    rm -f "$logfile"
    return 1
}

# --- Step 1: Clean prior build artifacts in dist/ -----------------------------

PRODUCT_VERSION=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$CARGO_TOML" | head -1)
case "$(uname -m)" in
    arm64)  PACKAGER_ARCH=aarch64 ;;
    x86_64) PACKAGER_ARCH=x86_64 ;;
    *)      PACKAGER_ARCH=$(uname -m) ;;
esac
CANONICAL_DMG="$PROJECT_DIR/dist/Robrix_${PRODUCT_VERSION}_${PACKAGER_ARCH}.dmg"

echo "==> Cleaning prior build artifacts in dist/..."
rm -rf "$PROJECT_DIR/dist/Robrix.app" \
       "$PROJECT_DIR/dist/.cargo-packager" \
       "$CANONICAL_DMG"

# --- Step 2: Run cargo-packager with signing disabled -------------------------
#
# We comment out signing_identity so cargo-packager's codesign + notarize
# block (gated on `signing_identity.as_ref()` in app/mod.rs:134) is
# completely skipped. We also unset APPLE_* so cargo-packager has no
# way to find notarization credentials. Result: unsigned .app + DMG.

sed -i.bak 's/^signing_identity[[:space:]]*=/#&/' "$CARGO_TOML"
trap 'mv "$CARGO_TOML.bak" "$CARGO_TOML" 2>/dev/null && echo "Restored Cargo.toml"' EXIT

TS_MARKER=$(mktemp)

echo "==> Running cargo packager (unsigned: we sign + notarize ourselves with retries)..."
env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID cargo packager --release

APP_PATH="$PROJECT_DIR/dist/Robrix.app"
DMG_FILE=$(find "$PROJECT_DIR/dist" -maxdepth 1 -name '*.dmg' -newer "$TS_MARKER" -print -quit)
rm -f "$TS_MARKER"

if [[ -z "$DMG_FILE" || ! -f "$DMG_FILE" ]]; then
    echo "Error: cargo packager did not produce a DMG in dist/" >&2
    exit 1
fi
if [[ ! -d "$APP_PATH" ]]; then
    echo "Error: $APP_PATH not found after cargo packager run." >&2
    exit 1
fi
echo "==> Found unsigned DMG: $DMG_FILE"

# --- Step 3: Codesign the standalone .app -------------------------------------

echo "==> Codesigning $APP_PATH..."
xattr -cr "$APP_PATH"
codesign_with_retry "$APP_PATH/Contents/MacOS/robrix" app
codesign_with_retry "$APP_PATH" app
codesign --verify --verbose=2 "$APP_PATH"

# --- Step 4: Apply Applications-folder icon fix to DMG ------------------------

echo "==> Applying Applications folder icon fix to DMG..."
"$SCRIPT_DIR/fix-dmg-applications-icon.sh" "$DMG_FILE" "$BG_IMAGE"

# --- Step 5: Embed signed .app into DMG ---------------------------------------
#
# The DMG produced by cargo-packager contains the unsigned .app from
# step 2. The icon fix didn't touch the .app inside (only the
# Applications symlink and DMG-level metadata). We mount the fixed DMG
# read-write, ditto the signed .app over the unsigned one (same name,
# so the .DS_Store icon position survives), unmount, recompress.

echo "==> Embedding signed .app into DMG..."
DMG_DIR="$(dirname "$DMG_FILE")"
DMG_BASE="$(basename "$DMG_FILE" .dmg)"
DMG_RW="$DMG_DIR/${DMG_BASE}_signing.dmg"

hdiutil convert "$DMG_FILE" -format UDRW -o "$DMG_RW" >/dev/null
MOUNT_OUTPUT=$(hdiutil attach "$DMG_RW" -readwrite -noverify -noautoopen)
MOUNT_DIR=$(echo "$MOUNT_OUTPUT" | grep -oE '/Volumes/.*' | head -1)
DEV_NAME=$(echo "$MOUNT_OUTPUT" | head -1 | awk '{print $1}')

if [[ -z "$MOUNT_DIR" || -z "$DEV_NAME" ]]; then
    echo "Error: failed to mount $DMG_RW" >&2
    rm -f "$DMG_RW"
    exit 1
fi

cleanup_rw() {
    hdiutil detach "$DEV_NAME" -force >/dev/null 2>&1 || true
    rm -f "$DMG_RW"
}
trap 'cleanup_rw; mv "$CARGO_TOML.bak" "$CARGO_TOML" 2>/dev/null && echo "Restored Cargo.toml"' EXIT

if [[ ! -d "$MOUNT_DIR/Robrix.app" ]]; then
    echo "Error: Robrix.app not found inside mounted DMG at $MOUNT_DIR" >&2
    exit 1
fi
rm -rf "$MOUNT_DIR/Robrix.app"
ditto "$APP_PATH" "$MOUNT_DIR/Robrix.app"

sync
sleep 2
hdiutil detach "$DEV_NAME" >/dev/null
sleep 1

rm -f "$DMG_FILE"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_FILE" >/dev/null
rm -f "$DMG_RW"

# RW DMG is gone; drop that part of the trap.
trap 'mv "$CARGO_TOML.bak" "$CARGO_TOML" 2>/dev/null && echo "Restored Cargo.toml"' EXIT

# --- Step 6: Codesign the DMG itself ------------------------------------------

echo "==> Codesigning DMG..."
codesign_with_retry "$DMG_FILE" dmg

# --- Step 7: Notarize ---------------------------------------------------------
#
# notarytool exits non-zero if the submission ends in any state other
# than "Accepted", so set -e catches a rejection.

echo "==> Submitting DMG for notarization (this can take several minutes)..."
xcrun notarytool submit "$DMG_FILE" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait

# --- Step 8: Staple and verify ------------------------------------------------

echo "==> Stapling notarization ticket to DMG..."
xcrun stapler staple "$DMG_FILE"
xcrun stapler validate "$DMG_FILE"

echo "==> Verifying DMG with spctl..."
spctl --assess --type open --context context:primary-signature --verbose "$DMG_FILE" || true

echo ""
echo "==> Done!"
echo "    DMG:      $DMG_FILE"
echo "    Identity: $SIGNING_IDENTITY"
