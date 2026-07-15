#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLASSIC_EVIDENCE="${SOPHIA_MILESTONE3_CLASSIC_EVIDENCE:-/tmp/sophia-milestone3-classic.log}"
CONFINED_EVIDENCE="${SOPHIA_MILESTONE3_CONFINED_EVIDENCE:-/tmp/sophia-milestone3-confined.log}"
UPDATE_SIZE="${SOPHIA_MILESTONE3_OUTPUT_SIZE:-1024x768}"
SURFACE_SIZE="${SOPHIA_MILESTONE3_SURFACE_SIZE:-960x640}"
INPUT_DEVICES="${SOPHIA_MILESTONE3_INPUT_DEVICES:-}"

keyd_was_running=false
restore_keyd() {
    local status=$?
    if [[ "$keyd_was_running" == true ]]; then
        echo
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    return "$status"
}

trap restore_keyd EXIT

echo "Sophia Milestone 3 paired hardware proof"
echo "This runs two exclusive-DRM sessions: classic shared-X, then fresh confined."
echo "For each session, type sophia and Return once in the prompted terminal."
echo "When the prompt disappears, move/click either the touchpad or TrackPoint."

if [[ -z "$INPUT_DEVICES" ]]; then
    mapfile -t keyboards < <(
        find /dev/input/by-path -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null \
            | sort -u
    )
    mapfile -t pointers < <(
        find /dev/input/by-path -maxdepth 1 -type l -name '*-event-mouse' -print 2>/dev/null \
            | sort -u
    )
    if (( ${#keyboards[@]} != 1 )); then
        echo "Expected exactly one stable keyboard event path, found ${#keyboards[@]}." >&2
        echo "Set SOPHIA_MILESTONE3_INPUT_DEVICES explicitly." >&2
        exit 1
    fi
    if (( ${#pointers[@]} == 0 )); then
        echo "No stable pointer event path was found." >&2
        echo "Set SOPHIA_MILESTONE3_INPUT_DEVICES explicitly." >&2
        exit 1
    fi
    devices=("${keyboards[0]}" "${pointers[@]}")
    INPUT_DEVICES="$(IFS=,; echo "${devices[*]}")"
fi

echo "Physical input devices: $INPUT_DEVICES"

IFS=',' read -r -a input_paths <<< "$INPUT_DEVICES"
for input_path in "${input_paths[@]}"; do
    if [[ ! -r "$input_path" ]]; then
        echo "Milestone 3 physical input device is not readable: $input_path" >&2
        exit 1
    fi
done

# keyd takes an exclusive evdev grab. The device remains openable and readable,
# but Sophia receives no events while that grab is active. Release it for the
# paired proof and restore the service on every exit path.
if pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd so Sophia can own the physical keyboard..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$CLASSIC_EVIDENCE" \
    "$ROOT_DIR/tools/live_session_two_xterm_hardware_proof.sh" \
    --namespace-profile=classic-shared --inject-output-size="$UPDATE_SIZE" \
    --inject-surface-resize="$SURFACE_SIZE" --input-devices="$INPUT_DEVICES" \
    --expect-physical-text=sophia --expect-physical-pointer "$@"

SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$CONFINED_EVIDENCE" \
    SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
    "$ROOT_DIR/tools/live_session_two_xterm_hardware_proof.sh" \
    --namespace-profile=confined --inject-output-size="$UPDATE_SIZE" \
    --inject-surface-resize="$SURFACE_SIZE" --input-devices="$INPUT_DEVICES" \
    --expect-physical-text=sophia --expect-physical-pointer "$@"

"$ROOT_DIR/tools/verify_live_session_milestone3_evidence.sh" \
    "$CLASSIC_EVIDENCE" "$CONFINED_EVIDENCE"
