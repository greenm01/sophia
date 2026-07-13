#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${SOPHIA_DMABUF_PROMOTION_EVIDENCE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/dmabuf-promotion}"
DRY_RUN=false

if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    shift
fi
if (( $# != 0 )); then
    echo "usage: $0 [--dry-run]" >&2
    exit 2
fi

if [[ "$DRY_RUN" != true ]]; then
    if [[ ! -t 0 ]]; then
        echo "Run this script interactively from a dedicated local text TTY." >&2
        exit 1
    fi
    if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
        echo "A graphical display is active in this shell; use a dedicated text TTY." >&2
        exit 1
    fi
fi

keyboard="${SOPHIA_OPERATOR_KEYBOARD:-}"
if [[ -z "$keyboard" ]]; then
    keyboards=()
    for directory in /dev/input/by-id /dev/input/by-path; do
        [[ -d "$directory" ]] || continue
        mapfile -t keyboards < <(
            find "$directory" -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null \
                | sort -u
        )
        (( ${#keyboards[@]} > 0 )) && break
    done
    if (( ${#keyboards[@]} != 1 )); then
        echo "Expected exactly one stable keyboard event path, found ${#keyboards[@]}." >&2
        printf '  %s\n' "${keyboards[@]}" >&2
        echo "Set SOPHIA_OPERATOR_KEYBOARD explicitly and rerun." >&2
        exit 1
    fi
    keyboard="${keyboards[0]}"
fi

input_devices="${SOPHIA_INPUT_DEVICES:-}"
if [[ -z "$input_devices" ]]; then
    pointers=()
    for directory in /dev/input/by-id /dev/input/by-path; do
        [[ -d "$directory" ]] || continue
        mapfile -t pointers < <(
            find "$directory" -maxdepth 1 -type l -name '*-event-mouse' -print 2>/dev/null \
                | sort -u
        )
        (( ${#pointers[@]} > 0 )) && break
    done
    if (( ${#pointers[@]} == 0 )); then
        echo "No stable pointer event path was found." >&2
        echo "Set SOPHIA_INPUT_DEVICES to keyboard,pointer paths and rerun." >&2
        exit 1
    fi
    devices=("$keyboard" "${pointers[@]}")
    input_devices="$(IFS=,; echo "${devices[*]}")"
fi

IFS=',' read -r -a devices <<<"$input_devices"
if (( ${#devices[@]} < 2 )); then
    echo "The proof needs at least one keyboard and one pointer event path." >&2
    exit 1
fi
for device in "$keyboard" "${devices[@]}"; do
    if [[ "$device" != /dev/input/* || ! -e "$device" ]]; then
        echo "Invalid input event path: $device" >&2
        exit 1
    fi
    if [[ "$DRY_RUN" != true && ! -r "$device" ]]; then
        echo "Input event path is not readable: $device" >&2
        exit 1
    fi
done

echo "Sophia native Wayland Kitty milestone proof"
echo "  keyboard: $keyboard"
echo "  devices:  $input_devices"
echo "  evidence: $EVIDENCE_DIR"

if [[ "$DRY_RUN" == true ]]; then
    echo "Discovery passed; rerun without --dry-run from a dedicated text TTY."
    exit 0
fi

echo
echo "The promotion gate runs controlled three-frame and 300-frame DMA-BUF proofs."
echo "It then asks you to complete the guarded Kitty input sequence three times."
echo "Exit Kitty normally; do not use the emergency chord for a passing proof."
echo

cd "$ROOT_DIR"
SOPHIA_OPERATOR_KEYBOARD="$keyboard" \
SOPHIA_INPUT_DEVICES="$input_devices" \
SOPHIA_DMABUF_PROMOTION_EVIDENCE_DIR="$EVIDENCE_DIR" \
    tools/wayland_kitty_dmabuf_promotion_gate.sh

echo
echo "All native Wayland Kitty hardware gates passed."
echo "Evidence: $EVIDENCE_DIR"
