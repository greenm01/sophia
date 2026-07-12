#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ ! -t 0 ]]; then
    echo "Run this interactively from a dedicated local TTY." >&2
    exit 1
fi

active_sessions=()
for process in river niri sway Hyprland kwin_wayland Xorg; do
    if pgrep -x "$process" >/dev/null 2>&1; then
        active_sessions+=("$process")
    fi
done
if (( ${#active_sessions[@]} > 0 )); then
    echo "Refusing to disturb the live graphical session." >&2
    echo "Still active: ${active_sessions[*]}" >&2
    echo >&2
    echo "Log out of the graphical session, switch to a dedicated TTY, then run:" >&2
    echo "  cd $ROOT_DIR && tools/finish_milestones_1_2.sh" >&2
    exit 1
fi

keyboard="${SOPHIA_OPERATOR_KEYBOARD:-}"
if [[ -z "$keyboard" ]]; then
    mapfile -t keyboards < <(
        find /dev/input/by-id /dev/input/by-path \
            -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null \
            | sort -u
    )
    if (( ${#keyboards[@]} != 1 )); then
        echo "Expected exactly one keyboard path, found ${#keyboards[@]}." >&2
        printf '  %s\n' "${keyboards[@]}" >&2
        echo "Set SOPHIA_OPERATOR_KEYBOARD to the keyboard you will type on." >&2
        exit 1
    fi
    keyboard="${keyboards[0]}"
fi

if [[ ! -r "$keyboard" ]]; then
    echo "Keyboard is not readable: $keyboard" >&2
    echo "Your login must have input-device access before running the proof." >&2
    exit 1
fi
shopt -s nullglob
drm_cards=(/dev/dri/card*)
shopt -u nullglob
drm_access=false
for card in "${drm_cards[@]}"; do
    if [[ -r "$card" && -w "$card" ]]; then
        drm_access=true
        break
    fi
done
if [[ "$drm_access" != true ]]; then
    echo "The primary DRM card is not readable and writable by this login." >&2
    echo "Your login must have video-device access before running the proof." >&2
    exit 1
fi

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

if pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd for direct physical-keyboard evidence..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
    trap restore_keyd EXIT
fi

echo "Sophia milestone 1 + 2 final proof"
echo "Repository: $ROOT_DIR"
echo "Keyboard: $keyboard"
echo
echo "The script will:"
echo "  1. rerun the real xmonad two-window bridge smoke"
echo "  2. verify non-destructive atomic KMS preflight"
echo "  3. wait for you to type the exact 'sophia' + Return sequence"
echo "  4. prove VRR activation and fixed-refresh fallback"
echo

cd "$ROOT_DIR"

echo
echo "[1/4] Real xmonad bridge smoke"
tools/xmonad_wm_bridge_smoke.sh

echo
echo "[2/4] Atomic KMS preflight"
tools/atomic_scanout_preflight.sh

echo
echo "[3/4] Operator keyboard pixel proof"
echo "Type sophia and press Return only when the scanned-out xterm prompts you."
SOPHIA_OPERATOR_KEYBOARD="$keyboard" \
SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
    tools/operator_keyboard_hardware_proof.sh

echo
echo "[4/4] AMD VRR activation and fixed-refresh fallback"
SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 tools/vrr_hardware_proof.sh

echo
echo "All milestone 1 + 2 proof gates passed."
echo "Persistent session evidence: ${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-persistent.log}"
echo "VRR evidence: ${SOPHIA_VRR_HARDWARE_EVIDENCE:-/tmp/sophia-vrr-hardware.log}"
