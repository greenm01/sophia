#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
classic="${SOPHIA_M5_GTK_CLASSIC_EVIDENCE:-/tmp/sophia-milestone5-gtk-classic.log}"
confined="${SOPHIA_M5_GTK_CONFINED_EVIDENCE:-/tmp/sophia-milestone5-gtk-confined.log}"
devices="${SOPHIA_M5_GTK_INPUT_DEVICES:-}"
runtime_msec="${SOPHIA_M5_GTK_RUNTIME_MSEC:-30000}"
keyd_was_running=false

restore_keyd() {
    local status=$?
    if [[ "$keyd_was_running" == true ]]; then
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    return "$status"
}
trap restore_keyd EXIT

command -v zenity >/dev/null || {
    echo "zenity is required on the X13 target" >&2
    exit 1
}
[[ "$runtime_msec" =~ ^[0-9]+$ ]] && (( runtime_msec >= 15000 && runtime_msec <= 120000 )) || {
    echo "SOPHIA_M5_GTK_RUNTIME_MSEC must be 15000-120000" >&2
    exit 1
}

if [[ -z "$devices" ]]; then
    mapfile -t keyboards < <(
        find /dev/input/by-path -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null | sort -u
    )
    mapfile -t pointers < <(
        find /dev/input/by-path -maxdepth 1 -type l \( -name '*-event-mouse' -o -name '*-event-touchpad' \) -print 2>/dev/null | sort -u
    )
    (( ${#keyboards[@]} == 1 )) || {
        echo "Expected exactly one stable keyboard path; found ${#keyboards[@]}." >&2
        echo "Set SOPHIA_M5_GTK_INPUT_DEVICES explicitly if X13 exposes more than one." >&2
        exit 1
    }
    (( ${#pointers[@]} >= 1 )) || {
        echo "No stable mouse or touchpad event path was found." >&2
        echo "Set SOPHIA_M5_GTK_INPUT_DEVICES explicitly." >&2
        exit 1
    }
    selected=("${keyboards[0]}" "${pointers[@]}")
    devices="$(IFS=,; echo "${selected[*]}")"
fi

IFS=',' read -r -a input_paths <<<"$devices"
for path in "${input_paths[@]}"; do
    [[ "$path" == /* && -r "$path" ]] || {
        echo "Input device is not an absolute readable path: $path" >&2
        exit 1
    }
done

if pgrep -x keyd >/dev/null 2>&1; then
    command -v sudo >/dev/null && command -v sv >/dev/null || {
        echo "keyd owns the keyboard, but sudo/sv is unavailable to release it" >&2
        exit 1
    }
    echo "Temporarily stopping keyd so Sophia can receive physical input..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

echo "Sophia Milestone 5 GTK3 paired proof on X13"
echo "Input devices: $devices"
echo "Classic evidence: $classic"
echo "Confined evidence: $confined"
echo "For each dialog: type sophia without Return, then move the pointer and click OK."
echo "Do not press Enter; the physical pointer click is part of the acceptance gate."
run_profile() {
    local profile="$1" evidence="$2"
    shift 2
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
      SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$runtime_msec" \
      "$root/tools/live_session_persistent_hardware_proof.sh" \
      --namespace-profile="$profile" --client=zenity --client-arg=--entry \
      --client-arg=--title --client-arg='Sophia GTK proof' \
      --client-arg=--text --client-arg='Type sophia, then click OK' \
      --expect-client-stdout=$'sophia\n' --require-client-normal-exit \
      --expect-physical-text=sophia --expect-physical-pointer --exit-after-input-proof \
      --inject-surface-resize=640x360 --input-devices="$devices" "$@"
}
run_profile classic-shared "$classic" "$@"
SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 run_profile confined "$confined" "$@"
"$root/tools/verify_live_session_milestone5_gtk_evidence.sh" "$classic" "$confined"
