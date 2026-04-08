#!/bin/bash
#
# Build a macOS DMG for Robrix with the Applications folder icon fix.
#
# This wraps cargo-packager to work around a macOS Tahoe bug where
# the Applications folder icon is invisible in DMGs.
#
# Strategy:
#   1. Temporarily strip signing_identity from Cargo.toml
#   2. Run cargo packager to build an unsigned DMG
#   3. Apply the Applications folder icon fix
#   4. Codesign the DMG manually
#   5. Restore Cargo.toml
#
# Usage: ./packaging/build-macos-dmg.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CARGO_TOML="$PROJECT_DIR/Cargo.toml"

cd "$PROJECT_DIR"

# Extract the signing identity from Cargo.toml before we remove it.
SIGNING_IDENTITY=$(grep -oP 'signing_identity\s*=\s*"\K[^"]+' "$CARGO_TOML" || true)

if [[ -z "$SIGNING_IDENTITY" ]]; then
    echo "Warning: No signing_identity found in Cargo.toml. DMG will not be codesigned."
fi

# Step 1: Temporarily comment out signing_identity so cargo-packager
#         produces an unsigned DMG.
sed -i.bak 's/^signing_identity\s*=/#&/' "$CARGO_TOML"
trap 'mv "$CARGO_TOML.bak" "$CARGO_TOML"; echo "Restored Cargo.toml"' EXIT

# Step 2: Run cargo packager for DMG only.
echo "==> Building unsigned DMG with cargo packager..."
cargo packager --release --formats dmg

# Step 3: Find the generated DMG and apply the icon fix.
DMG_FILE=$(find "$PROJECT_DIR/dist" -name '*.dmg' -newer "$CARGO_TOML.bak" -print -quit)

if [[ -z "$DMG_FILE" ]]; then
    echo "Error: No DMG file found in dist/"
    exit 1
fi

echo "==> Applying Applications folder icon fix to: $DMG_FILE"
"$SCRIPT_DIR/fix-dmg-applications-icon.sh" "$DMG_FILE"

# Step 4: Codesign the DMG if we have a signing identity.
if [[ -n "$SIGNING_IDENTITY" ]]; then
    echo "==> Codesigning DMG with: $SIGNING_IDENTITY"
    codesign --force --sign "$SIGNING_IDENTITY" "$DMG_FILE"
    echo "==> Codesigned successfully."
else
    echo "==> Skipping codesign (no signing identity)."
fi

echo "==> Done! DMG: $DMG_FILE"
