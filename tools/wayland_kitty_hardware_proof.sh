#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_WAYLAND_KITTY_EVIDENCE:-/tmp/sophia-wayland-kitty-hardware.log}"
SESSION_LOG="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/kitty-session/session.log"
INPUT_DEVICES="${SOPHIA_INPUT_DEVICES:-}"
KEYBOARD="${SOPHIA_OPERATOR_KEYBOARD:-}"
EXPECTED_KEYCODES="31,24,25,35,23,30,28,103,105,106,108"

if [[ ! -t 0 ]]; then
    echo "Run this proof interactively from a dedicated local text TTY." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run the native Wayland Kitty proof from a dedicated text TTY." >&2
    exit 1
fi
if [[ -z "$INPUT_DEVICES" ]]; then
    echo "Set SOPHIA_INPUT_DEVICES to comma-separated keyboard and pointer event paths." >&2
    exit 1
fi
if [[ -z "$KEYBOARD" ]]; then
    IFS=',' read -r KEYBOARD _ <<<"$INPUT_DEVICES"
fi
if [[ ! -r "$KEYBOARD" ]]; then
    echo "Keyboard is not readable: $KEYBOARD" >&2
    exit 1
fi

cd "$ROOT_DIR"
echo "[1/2] Proving real software-rendered Kitty resize without taking DRM ownership."
tools/wayland_kitty_smoke.sh

initial_kd_mode="$(python3 tools/sophia_tty_mode.py get)"
initial_termios="$(stty -g)"
keyd_was_running=0
if pgrep -x keyd >/dev/null 2>&1; then
    keyd_was_running=1
fi

echo "[2/2] Starting guarded native Kitty DMA-BUF proof."
echo "In Kitty: type 'sophia' and Enter, press all four arrow keys, move/click the pointer,"
echo "then type 'exit' and Enter. Do not use the emergency chord for a passing proof."
SOPHIA_OPERATOR_KEYBOARD="$KEYBOARD" \
SOPHIA_INPUT_DEVICES="$INPUT_DEVICES" \
    tools/run_sophia_kitty_session.sh \
        --expect-keycodes="$EXPECTED_KEYCODES" \
        --expect-pointer-input \
        --expect-input-presentation \
        --max-input-latency-ms=100

if [[ ! -s "$SESSION_LOG" ]]; then
    echo "Native Kitty session evidence is missing: $SESSION_LOG" >&2
    exit 1
fi
mkdir -p "$(dirname "$EVIDENCE_FILE")"
install -m 600 "$SESSION_LOG" "$EVIDENCE_FILE"

restored_kd_mode="$(python3 tools/sophia_tty_mode.py get)"
if [[ "$restored_kd_mode" != "$initial_kd_mode" ]]; then
    echo "TTY KD mode was not restored: before=$initial_kd_mode after=$restored_kd_mode" >&2
    exit 1
fi
restored_termios="$(stty -g)"
if [[ "$restored_termios" != "$initial_termios" ]]; then
    echo "TTY termios state was not restored" >&2
    exit 1
fi
keyd_restored=1
if [[ "$keyd_was_running" == 1 ]] && ! pgrep -x keyd >/dev/null 2>&1; then
    keyd_restored=0
fi
if pgrep -af 'target/release/sophia (sophia-wayland-session|sophia-session-input-guard)' \
    >/dev/null 2>&1; then
    echo "Sophia Wayland session or input guard survived wrapper cleanup" >&2
    exit 1
fi
printf 'sophia_wayland_recovery schema=1 status=complete kd_mode=%s termios_restored=1 keyd_restored=%s processes=0\n' \
    "$restored_kd_mode" "$keyd_restored" >>"$EVIDENCE_FILE"

SOPHIA_WAYLAND_REQUIRE_DMABUF=1 \
SOPHIA_WAYLAND_REQUIRE_INPUT=1 \
SOPHIA_WAYLAND_REQUIRE_RECOVERY=1 \
    tools/verify_wayland_kitty_evidence.sh "$EVIDENCE_FILE"
