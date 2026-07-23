#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCH_LOG=/tmp/sophia-kitty-tty3-launch.log

if [[ ! -t 0 || "$(tty)" != /dev/tty3 ]]; then
    echo "Switch to tty3 with Ctrl+Alt+F3, log in, then run:" >&2
    echo "  $ROOT_DIR/tools/start_sophia_kitty_tty3.sh" >&2
    exit 1
fi

exec > >(tee "$LAUNCH_LOG") 2>&1
echo "Retaining complete launcher output in $LAUNCH_LOG"

display_manager=""
for candidate in lightdm greetd; do
    if [[ -e "/var/service/$candidate" ]] && sudo sv status "$candidate" 2>/dev/null | grep -q '^run:'; then
        display_manager="$candidate"
        break
    fi
done

restore_display_manager() {
    local status=$?
    if [[ -n "$display_manager" ]]; then
        echo "Restoring $display_manager..."
        sudo sv up "$display_manager" || status=1
    fi
    return "$status"
}
trap restore_display_manager EXIT

if [[ -n "$display_manager" ]]; then
    echo "Stopping $display_manager so Sophia can own DRM..."
    sudo sv down "$display_manager"
    for _ in {1..50}; do
        pgrep -x Xorg >/dev/null 2>&1 || break
        sleep 0.1
    done
    if pgrep -x Xorg >/dev/null 2>&1; then
        echo "Xorg remained active after $display_manager stopped; refusing takeover." >&2
        exit 1
    fi
fi

cd "$ROOT_DIR"
tools/run_sophia_kitty_session.sh "$@"
