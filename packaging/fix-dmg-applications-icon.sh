#!/bin/bash
#
# Workaround for macOS Tahoe (26.x) bug where the Applications folder
# symlink icon is invisible in DMG files.
#
# This replaces the Unix symlink with a Finder alias (using NSURL bookmark
# APIs, no Finder.app needed) and explicitly sets the /Applications folder
# icon on it via NSWorkspace. Both APIs are headless-safe, so this works
# in CI environments (e.g., GitHub Actions macOS runners) as well as locally.
#
# If Finder.app is available, the script also re-applies DMG view settings
# (background, icon positions, window size) as a safety net. On headless CI,
# this step is skipped gracefully and the original .DS_Store from
# cargo-packager is preserved.
#
# Usage: ./fix-dmg-applications-icon.sh <path-to.dmg> <background-image>

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

VOLUME_NAME="$(basename "$MOUNT_DIR")"
echo "Mounted at: $MOUNT_DIR (volume: $VOLUME_NAME)"

cleanup() {
    echo "Detaching DMG..."
    hdiutil detach "$DEV_NAME" -force 2>/dev/null || true
}
trap cleanup EXIT

# --- Step 1: Replace the Unix symlink with a Finder alias ---
# Uses NSURL bookmark APIs (headless-safe, no Finder.app needed).

APPS_LINK="$MOUNT_DIR/Applications"

if [[ -L "$APPS_LINK" ]]; then
    echo "Removing existing symlink..."
    rm "$APPS_LINK"
elif [[ -e "$APPS_LINK" ]]; then
    echo "Removing existing Applications entry..."
    rm -rf "$APPS_LINK"
fi

echo "Creating Finder alias to /Applications (headless)..."
osascript <<ALIAS_SCRIPT
use framework "Foundation"
set sourceURL to current application's |NSURL|'s fileURLWithPath:"/Applications"
set aliasURL to current application's |NSURL|'s fileURLWithPath:"$MOUNT_DIR/Applications"
set bkOpts to current application's NSURLBookmarkCreationSuitableForBookmarkFile
set bookmarkData to sourceURL's bookmarkDataWithOptions:bkOpts includingResourceValuesForKeys:(missing value) relativeToURL:(missing value) |error|:(missing value)
current application's |NSURL|'s writeBookmarkData:bookmarkData toURL:aliasURL options:bkOpts |error|:(missing value)
ALIAS_SCRIPT

if [[ ! -e "$APPS_LINK" ]]; then
    echo "Error: Failed to create Finder alias"
    exit 1
fi
echo "Finder alias created successfully."

# --- Step 2: Set the /Applications folder icon on the alias ---
# Uses NSWorkspace (headless-safe, needs WindowServer but not Finder.app).

echo "Setting Applications folder icon on alias..."
osascript <<ICON_SCRIPT
use framework "AppKit"
set ws to current application's NSWorkspace's sharedWorkspace()
set theIcon to ws's iconForFile:"/Applications"
ws's setIcon:theIcon forFile:"$APPS_LINK" options:0
ICON_SCRIPT

# --- Step 3: Ensure background image is in the DMG ---

BG_DIR="$MOUNT_DIR/.background"
mkdir -p "$BG_DIR"
cp "$BG_IMAGE" "$BG_DIR/background.png"
echo "Background image copied to .background/background.png"

# --- Step 4: Re-apply Finder view settings (non-fatal) ---
# This is a safety net in case the .DS_Store was corrupted. It requires
# Finder.app, so it will be skipped gracefully on headless CI runners
# (where the original .DS_Store from cargo-packager is preserved anyway).

WIN_LEFT=100
WIN_TOP=100
WIN_RIGHT=$((WIN_LEFT + 960))
WIN_BOTTOM=$((WIN_TOP + 540))

if osascript <<APPLESCRIPT 2>/dev/null; then
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
else
    echo "Skipping Finder view settings (headless environment)."
fi

# Let any Finder/filesystem changes flush
sync
sleep 3

# --- Step 5: Detach and convert back to compressed DMG ---

trap - EXIT
echo "Detaching DMG..."
hdiutil detach "$DEV_NAME"
sleep 1

echo "Converting back to compressed DMG..."
rm -f "$DMG_PATH"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"
rm -f "$DMG_RW"

echo "Done! Fixed DMG: $DMG_PATH"
