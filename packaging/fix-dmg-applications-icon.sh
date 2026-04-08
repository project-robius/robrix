#!/bin/bash
#
# Workaround for macOS Tahoe (26.x) bug where the Applications folder
# symlink icon is invisible in DMG files.
#
# This replaces the Unix symlink with a Finder alias, which embeds its
# own icon data and doesn't rely on Finder's broken overlay rendering.
# No extra dependencies required (just osascript, which ships with macOS).
#
# Usage: ./fix-dmg-applications-icon.sh <path-to.dmg>

set -euo pipefail

DMG_PATH="${1:?Usage: $0 <path-to.dmg>}"

if [[ ! -f "$DMG_PATH" ]]; then
    echo "Error: DMG file not found: $DMG_PATH"
    exit 1
fi

DMG_DIR="$(dirname "$DMG_PATH")"
DMG_BASE="$(basename "$DMG_PATH" .dmg)"
DMG_RW="${DMG_DIR}/${DMG_BASE}_rw.dmg"

echo "Converting DMG to read-write..."
hdiutil convert "$DMG_PATH" -format UDRW -o "$DMG_RW"

echo "Mounting read-write DMG..."
MOUNT_OUTPUT=$(hdiutil attach "$DMG_RW" -readwrite -noverify -noautoopen)
MOUNT_DIR=$(echo "$MOUNT_OUTPUT" | grep -oE '/Volumes/.*' | head -1)
DEV_NAME=$(echo "$MOUNT_OUTPUT" | head -1 | awk '{print $1}')

if [[ -z "$MOUNT_DIR" ]]; then
    echo "Error: Failed to determine mount point"
    exit 1
fi

echo "Mounted at: $MOUNT_DIR"

cleanup() {
    echo "Detaching DMG..."
    hdiutil detach "$DEV_NAME" -force 2>/dev/null || true
}
trap cleanup EXIT

# Replace the Unix symlink with a Finder alias.
# Finder aliases embed their own icon, so they don't depend on
# Finder's broken overlay icon rendering on macOS Tahoe.
APPS_LINK="$MOUNT_DIR/Applications"

if [[ -L "$APPS_LINK" ]]; then
    echo "Removing existing symlink..."
    rm "$APPS_LINK"
elif [[ -e "$APPS_LINK" ]]; then
    echo "Removing existing Applications entry..."
    rm -rf "$APPS_LINK"
fi

echo "Creating Finder alias to /Applications..."
osascript -e "
    tell application \"Finder\"
        make new alias file at POSIX file \"$MOUNT_DIR\" to POSIX file \"/Applications\" with properties {name:\"Applications\"}
    end tell
"

# Verify the alias was created
if [[ -e "$APPS_LINK" ]]; then
    echo "Finder alias created successfully."
else
    echo "Error: Failed to create Finder alias"
    exit 1
fi

# Let Finder flush changes
sleep 2

# Detach explicitly for the conversion step
trap - EXIT
echo "Detaching DMG..."
hdiutil detach "$DEV_NAME"
sleep 1

echo "Converting back to compressed DMG..."
rm -f "$DMG_PATH"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"
rm -f "$DMG_RW"

echo "Done! Fixed DMG: $DMG_PATH"
