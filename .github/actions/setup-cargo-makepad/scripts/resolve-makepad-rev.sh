#!/usr/bin/env bash
set -euo pipefail

lockfile="${1:-Cargo.lock}"

source_line="$(
  awk '
    $1 == "name" && $3 == "\"makepad-widgets\"" { in_pkg = 1; next }
    in_pkg && $1 == "source" { print $3; exit }
    in_pkg && $1 == "[[package]]" { in_pkg = 0 }
  ' "$lockfile"
)"

if [[ -z "$source_line" ]]; then
  echo "error=makepad-widgets source not found in $lockfile" >&2
  exit 1
fi

source_line="${source_line%\"}"
source_line="${source_line#\"}"
source_line="${source_line#git+}"

rev="${source_line##*#}"
repo_with_query="${source_line%#*}"
repo="${repo_with_query%%\?*}"

printf 'repo=%s\n' "$repo"
printf 'rev=%s\n' "$rev"
