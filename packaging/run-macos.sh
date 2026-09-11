#!/bin/bash
# Cargo's macOS runner gives development builds their own native app identity.
set -euo pipefail

binary=${1:?Expected an executable from Cargo}
shift

# Run the executable as-is unless bundling it would actually help. Cargo routes
# test binaries through here too, and a bundle is useless (or impossible) when
# there is no GUI session to open it in. Set ROBRIX_DEV_NO_BUNDLE=1 to opt out,
# for example to attach a debugger to the process cargo itself started.
# Without the bundle the app still runs; macOS just attributes microphone,
# speech and location prompts to the terminal instead of to Robrix.
if [[ ${binary##*/} != robrix || -n ${ROBRIX_DEV_NO_BUNDLE:-} ]]; then
    exec "$binary" "$@"
fi
# A missing or broken codesign is handled by the signing step below, which falls
# back to running unbundled rather than failing the run.
if [[ $(launchctl managername 2>/dev/null) != Aqua ]]; then
    echo 'robrix: no macOS window session, so running without an app bundle. Native permission prompts will be attributed to the terminal.' >&2
    exec "$binary" "$@"
fi

working_dir=$PWD
binary_dir=$(cd "$(dirname "$binary")" && pwd -P)
binary="$binary_dir/robrix"
packaging_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bundle_dir="$binary_dir/.robrix-dev"
bundle="$bundle_dir/Robrix.app"
mkdir -p "$bundle_dir"
staging=$(mktemp -d "$bundle_dir/.stage.XXXXXX")
launch_dir=$(mktemp -d "${TMPDIR:-/tmp}/robrix-launch.XXXXXX")
open_pid=
app_pid=
stdout_pid=
stderr_pid=
interrupted=0
pending_signal=
completed=0

cleanup() {
    trap - EXIT INT TERM HUP
    if [[ $completed == 0 && -n $app_pid ]]; then
        kill -TERM "$app_pid" 2>/dev/null || true
    fi
    for child in "$open_pid" "$stdout_pid" "$stderr_pid"; do
        if [[ -n $child ]]; then
            kill "$child" 2>/dev/null || true
            wait "$child" 2>/dev/null || true
        fi
    done
    rm -rf "$staging" "$launch_dir"
}
forward_signal() {
    pending_signal=$1
    interrupted=$2
    if [[ -n $app_pid ]]; then
        kill -"$pending_signal" "$app_pid" 2>/dev/null || true
        pending_signal=
    fi
}
trap cleanup EXIT
trap 'forward_signal INT 130' INT
trap 'forward_signal TERM 143' TERM
trap 'forward_signal HUP 129' HUP

inputs=$(/usr/bin/shasum -a 256 "$binary" "$packaging_dir/macos/Info.plist" "${BASH_SOURCE[0]}")
previous_inputs=
if [[ -f $bundle_dir/inputs ]]; then previous_inputs=$(< "$bundle_dir/inputs"); fi
if [[ $inputs != "$previous_inputs" || ! -f $bundle/Contents/MacOS/robrix ]]; then
    # TCC may inspect the bundle again while it is running. Keep its executable
    # and signature in place until that instance has exited.
    while IFS= read -r running_binary; do
        if [[ $running_binary == "$bundle/Contents/MacOS/robrix" ]]; then
            echo 'Quit the running Robrix development app before launching a rebuilt version.' >&2
            exit 1
        fi
    done < <(/bin/ps -axo comm=)
    # Copy before signing: a symlink or hard link would mutate Cargo's executable.
    mkdir -p "$staging/Robrix.app/Contents/MacOS"
    cp "$binary" "$staging/Robrix.app/Contents/MacOS/robrix"
    cp "$packaging_dir/macos/Info.plist" "$staging/Robrix.app/Contents/Info.plist"
    plist="$staging/Robrix.app/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier rs.robius.robrix.development' "$plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName Robrix Development' "$plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleName Robrix Development' "$plist"
    # Development builds must not register URL handlers over an installed Robrix.
    /usr/libexec/PlistBuddy -c 'Delete :CFBundleURLTypes' "$plist"
    if ! /usr/bin/codesign --force --sign - "$staging/Robrix.app" >"$staging/signing.log" 2>&1; then
        cat "$staging/signing.log" >&2
        echo 'robrix: could not sign the development app bundle, so running without it.' >&2
        cleanup
        exec "$binary" "$@"
    fi
    if [[ -e $bundle || -L $bundle ]]; then
        mv "$bundle" "$staging/previous.app"
    fi
    mv "$staging/Robrix.app" "$bundle"
    printf '%s\n' "$inputs" > "$bundle_dir/inputs"
fi

# LaunchServices opens these paths in launchd, so /dev/stdout does not refer to
# Cargo's output. Private FIFOs preserve logs for terminals and redirected runs.
mkfifo "$launch_dir/stdout" "$launch_dir/stderr"
(trap '' INT; exec cat "$launch_dir/stdout") &
stdout_pid=$!
(trap '' INT; exec cat "$launch_dir/stderr" >&2) &
stderr_pid=$!
# Makepad's macOS event loop completes this handshake: it adopts the working
# directory, writes its pid here on the way in, and its exit status on the way out.
export MAKEPAD_DEV_LAUNCH_DIR="$launch_dir"
export MAKEPAD_DEV_WORKING_DIR="$working_dir"
(trap '' INT; exec /usr/bin/open -n -W --stdout "$launch_dir/stdout" --stderr "$launch_dir/stderr" \
    "$bundle" --args "$@") &
open_pid=$!

# Makepad reports the pid before initializing the app. Never use a bundle-wide
# process lookup: another development session may already be open.
startup_seconds=$SECONDS
open_exited_seconds=
while [[ ! -s $launch_dir/pid ]]; do
    if (( SECONDS - startup_seconds >= 30 )); then
        echo 'Robrix did not complete its development launcher handshake.' >&2
        exit 1
    fi
    # `open -W` sometimes returns before the app has finished launching. Giving up
    # the moment it exits would lose the app's PID, leaving nothing to forward a
    # Ctrl-C to and no way to stop the app, so allow a short grace period first.
    if ! kill -0 "$open_pid" 2>/dev/null; then
        if [[ -z $open_exited_seconds ]]; then
            open_exited_seconds=$SECONDS
        elif (( SECONDS - open_exited_seconds >= 5 )); then
            break
        fi
    fi
    sleep 0.05 || true
done
if [[ -s $launch_dir/pid ]]; then
    app_pid=$(< "$launch_dir/pid")
    if [[ ! $app_pid =~ ^[0-9]+$ || $app_pid -le 1 ]]; then
        app_pid=
        echo 'Robrix returned an invalid development process ID.' >&2
        exit 1
    fi
    if [[ -n $pending_signal ]]; then
        forward_signal "$pending_signal" "$interrupted"
    fi
fi

# A trapped signal interrupts wait even when open is still waiting for the app.
while true; do
    if wait "$open_pid"; then open_status=0; else open_status=$?; fi
    if ! kill -0 "$open_pid" 2>/dev/null; then break; fi
done
open_pid=
# Terminal Ctrl-C can also terminate open. The native app belongs to launchd,
# so explicitly wait for the PID to exit before tearing down its log relays.
if [[ -n $app_pid ]]; then
    if (( open_status != 0 && interrupted == 0 )); then
        kill -TERM "$app_pid" 2>/dev/null || true
    fi
    while kill -0 "$app_pid" 2>/dev/null; do sleep 0.05 || true; done
    app_pid=
fi
if (( interrupted != 0 )); then exit "$interrupted"; fi
if (( open_status != 0 )); then exit "$open_status"; fi
if [[ ! -s $launch_dir/status ]]; then
    echo 'Robrix exited before reporting a normal application shutdown.' >&2
    exit 1
fi
app_status=$(< "$launch_dir/status")
if [[ ! $app_status =~ ^[0-9]+$ || $app_status -gt 255 ]]; then
    echo 'Robrix returned an invalid development exit status.' >&2
    exit 1
fi
completed=1
app_pid=
wait "$stdout_pid" || true
stdout_pid=
wait "$stderr_pid" || true
stderr_pid=
exit "$app_status"
