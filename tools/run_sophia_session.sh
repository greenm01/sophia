#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
wm=xmonad
if [[ "${1:-}" == --wm=* ]]; then
    wm="${1#--wm=}"
    shift
fi
case "$wm" in
    xmonad|native|none) ;;
    *) echo "--wm expects xmonad, native, or none" >&2; exit 1 ;;
esac

resolve_program() {
    local configured=$1
    local fallback=$2
    if [[ -n "$configured" && -x "$configured" ]]; then
        printf '%s\n' "$configured"
        return
    fi
    command -v "$fallback" || true
}

sophia_bin="$(resolve_program "${SOPHIA_BIN:-$ROOT_DIR/target/debug/sophia}" sophia)"
terminal_bin="$(resolve_program "${SOPHIA_TERMINAL_BIN:-}" xterm)"
if [[ -z "$sophia_bin" || -z "$terminal_bin" ]]; then
    echo "Sophia and xterm must be built or installed before starting a normal session." >&2
    exit 1
fi

wm_args=()
case "$wm" in
    xmonad)
        bridge="$(resolve_program "${SOPHIA_X11_WM_BRIDGE_BIN:-$ROOT_DIR/target/debug/sophia-x11-wm-bridge}" sophia-x11-wm-bridge)"
        xmonad="$(resolve_program "${SOPHIA_XMONAD_BIN:-}" xmonad)"
        if [[ -z "$bridge" || -z "$xmonad" ]]; then
            echo "The xmonad selection requires sophia-x11-wm-bridge and xmonad." >&2
            exit 1
        fi
        wm_args=(
            "--wm-process=$bridge"
            "--wm-process-arg=--wm=$xmonad"
            "--wm-process-arg=--profile=xmonad"
            "--wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux"
        )
        ;;
    native)
        native_wm="$(resolve_program "${SOPHIA_NATIVE_WM_BIN:-$ROOT_DIR/target/debug/sophia-wm-demo}" sophia-wm-demo)"
        if [[ -z "$native_wm" ]]; then
            echo "The native selection requires sophia-wm-demo." >&2
            exit 1
        fi
        wm_args=("--wm-process=$native_wm")
        ;;
esac

session_args=(
    sophia-live-session
    --session-mode=normal
    "--session-app=terminal=$terminal_bin"
    --session-app-arg=terminal=-cm
    --session-app-arg=terminal=-dc
    --session-app-arg=terminal=-title
    "--session-app-arg=terminal=Sophia Terminal"
    --session-start=terminal
    --session-action-app=terminal=terminal
)
firefox_bin="$(resolve_program "${SOPHIA_FIREFOX_BIN:-}" firefox)"
if [[ -n "$firefox_bin" ]]; then
    session_args+=(
        "--session-app=firefox=$firefox_bin"
        --session-action-app=firefox=firefox
    )
fi
session_args+=("${wm_args[@]}")
session_args+=("$@")
exec "$sophia_bin" "${session_args[@]}"
