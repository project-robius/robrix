#!/bin/bash
#
# Workaround for macOS Tahoe (26.x) bug where the Applications folder
# symlink icon is invisible in DMG files.
#
# This replaces the Unix symlink with a Finder alias, which embeds its
# own icon data and doesn't rely on Finder's broken overlay rendering.
# After replacing the symlink, we re-apply the Finder view settings
# (background image, icon positions, window size) because the
# convert-modify-convert cycle can lose them.
#
# Usage: ./fix-dmg-applications-icon.sh <path-to.dmg> <background-image>
#
# The background image is copied into the DMG at .background/background.png.
# Window size, icon positions, and icon size are configured in the
# AppleScript block below to match the Cargo.toml [package.metadata.packager.dmg]
# settings.

set -euo pipefail

DMG_PATH="${1:?Usage: $0 <path-to.dmg> <background-image>}"
BG_IMAGE="${2:?Usage: $0 <path-to.dmg> <background-image>}"

if [[ ! -f "$DMG_PATH" ]]; then
    echo "Error: DMG file not found: $DMG_PATH"
    exit 1
fi
if [[ ! -f "$BG_IMAGE" ]]; then
    echo "Error: Background image not found: $BG_IMAGE"
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

# Extract just the volume name (last path component of the mount point)
VOLUME_NAME="$(basename "$MOUNT_DIR")"

echo "Mounted at: $MOUNT_DIR (volume: $VOLUME_NAME)"

cleanup() {
    echo "Detaching DMG..."
    hdiutil detach "$DEV_NAME" -force 2>/dev/null || true
}
trap cleanup EXIT

# --- Step 1: Replace the Unix symlink with a Finder alias ---

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

if [[ ! -e "$APPS_LINK" ]]; then
    echo "Error: Failed to create Finder alias"
    exit 1
fi
echo "Finder alias created successfully."

# Copy the /Applications folder icon onto the alias so it's visible
# on macOS Tahoe, where Finder no longer renders overlay icons for aliases.
echo "Setting Applications folder icon on alias..."
osascript <<ICON_SCRIPT
use framework "AppKit"
set ws to current application's NSWorkspace's sharedWorkspace()
set theIcon to ws's iconForFile:"/Applications"
ws's setIcon:theIcon forFile:"$APPS_LINK" options:0
ICON_SCRIPT

# --- Step 2: Copy background image into the DMG ---

BG_DIR="$MOUNT_DIR/.background"
mkdir -p "$BG_DIR"
cp "$BG_IMAGE" "$BG_DIR/background.png"
echo "Background image copied to .background/background.png"

# --- Step 3: Re-apply Finder view settings via AppleScript ---
#
# These values must match the [package.metadata.packager.dmg] section
# in Cargo.toml:
#   window_size = { width = 960, height = 540 }
#   app_position = { x = 200, y = 250 }
#   application_folder_position = { x = 760, y = 250 }

echo "Applying Finder view settings..."

# Window bounds: {left, top, right, bottom}
# We place the window at (100, 100) so bounds are {100, 100, 1060, 640}
WIN_LEFT=100
WIN_TOP=100
WIN_RIGHT=$((WIN_LEFT + 960))
WIN_BOTTOM=$((WIN_TOP + 540))

osascript <<APPLESCRIPT
tell application "Finder"
    tell disk "$VOLUME_NAME"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set bounds of container window to {$WIN_LEFT, $WIN_TOP, $WIN_RIGHT, $WIN_BOTTOM}

        set theViewOptions to the icon view options of container window
        set arrangement of theViewOptions to not arranged
        set icon size of theViewOptions to 128
        set background picture of theViewOptions to file ".background:background.png"

        set position of item "Robrix.app" of container window to {200, 250}
        set position of item "Applications" of container window to {760, 250}

        close
        open
    end tell
end tell
APPLESCRIPT

echo "Finder view settings applied."

# Let Finder flush .DS_Store changes
sync
sleep 3

# --- Step 4: Detach and convert back to compressed DMG ---

trap - EXIT
echo "Detaching DMG..."
hdiutil detach "$DEV_NAME"
sleep 1

echo "Converting back to compressed DMG..."
rm -f "$DMG_PATH"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"
rm -f "$DMG_RW"

echo "Done! Fixed DMG: $DMG_PATH"
