#!/usr/bin/env bash
# Test room settings panel via stdin commands.
# Usage:
#   Run the app: cargo run
#   In another terminal: ./test-room-settings.sh [room-name-keyword]
#
# Commands sent to stdin:
#   open-room-settings [keyword]   — opens modal for selected/first/keyword room
#   close-room-settings            — closes the modal

CMD=${1:-open-room-settings}

case "$1" in
    close)
        echo "close-room-settings"
        ;;
    open|"")
        KEYWORD=${2:-}
        echo "open-room-settings $KEYWORD"
        ;;
    *)
        echo "$*"
        ;;
esac
