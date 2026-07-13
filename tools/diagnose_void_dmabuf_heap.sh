#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
EVIDENCE_DIR="$STATE_DIR/dmabuf-promotion"
INSTALL_LOG="$STATE_DIR/void-dmabuf-diagnostic-install.log"
FRAME_COUNT="${SOPHIA_DMABUF_DIAGNOSTIC_FRAMES:-3}"
REQUIRED_PACKAGES=(
    gcc
    pkg-config
    wayland-devel
    wayland-protocols
    MesaLib-devel
    libdrm-devel
    gdb
)
SKIP_INSTALL=false

usage() {
    echo "usage: $0 [--skip-install]" >&2
}

while (( $# > 0 )); do
    case "$1" in
        --skip-install)
            SKIP_INSTALL=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
    shift
done

if [[ ! -r /etc/os-release ]] || ! grep -Eq '^ID="?void"?$' /etc/os-release; then
    echo "This helper supports Void Linux only." >&2
    exit 1
fi
if [[ ! -t 0 ]] || [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run this diagnostic from a dedicated local text TTY." >&2
    exit 1
fi
if [[ ! "$FRAME_COUNT" =~ ^[0-9]+$ ]] || (( FRAME_COUNT < 2 || FRAME_COUNT > 1000 )); then
    echo "SOPHIA_DMABUF_DIAGNOSTIC_FRAMES must be an integer from 2 to 1000." >&2
    exit 1
fi

mkdir -p "$STATE_DIR" "$EVIDENCE_DIR"
chmod 700 "$STATE_DIR" "$EVIDENCE_DIR"

if [[ "$SKIP_INSTALL" != true ]]; then
    for command in sudo xbps-install xbps-query; do
        command -v "$command" >/dev/null || {
            echo "Missing required command: $command" >&2
            exit 1
        }
    done
    missing_packages=()
    for package in "${REQUIRED_PACKAGES[@]}"; do
        if ! xbps-query -p pkgver "$package" >/dev/null 2>&1; then
            missing_packages+=("$package")
        fi
    done
    if (( ${#missing_packages[@]} > 0 )); then
        echo "Installing DMA-BUF diagnostic dependencies..."
        printf '  %s\n' "${missing_packages[@]}"
        if ! sudo xbps-install -S "${missing_packages[@]}" 2>&1 | tee "$INSTALL_LOG"; then
            echo "Dependency installation failed; full log: $INSTALL_LOG" >&2
            echo "Void does not support partial upgrades. Run 'sudo xbps-install -Syu', then rerun this helper." >&2
            exit 1
        fi
    fi
fi

if ! command -v gdb >/dev/null; then
    echo "gdb is required; rerun without --skip-install after installing it." >&2
    exit 1
fi

if (( FRAME_COUNT == 3 )); then
    EVIDENCE_FILE="$EVIDENCE_DIR/controlled-first-frame-diagnostic.log"
else
    EVIDENCE_FILE="$EVIDENCE_DIR/controlled-${FRAME_COUNT}-frame-diagnostic.log"
fi

echo "Sophia DMA-BUF heap diagnostic (${FRAME_COUNT} frames)"
echo "  evidence: $EVIDENCE_FILE"
echo "  GDB log:  $EVIDENCE_FILE.gdb.log"
echo "  scope:    one controlled DMA-BUF run only"

cd "$ROOT_DIR"
exec env \
    SOPHIA_DMABUF_DIAGNOSTIC=1 \
    SOPHIA_DMABUF_PRODUCER_FRAMES="$FRAME_COUNT" \
    SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$EVIDENCE_FILE" \
    tools/wayland_dmabuf_first_frame_hardware_proof.sh
