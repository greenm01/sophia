#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SKIP_INSTALL=false
DRY_RUN=false

usage() {
    echo "usage: $0 [--skip-install] [--dry-run]" >&2
}

while (( $# > 0 )); do
    case "$1" in
        --skip-install)
            SKIP_INSTALL=true
            ;;
        --dry-run)
            DRY_RUN=true
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

if [[ "$SKIP_INSTALL" != true ]]; then
    command -v sudo >/dev/null || {
        echo "Missing required command: sudo" >&2
        exit 1
    }
    command -v xbps-install >/dev/null || {
        echo "Missing required command: xbps-install" >&2
        exit 1
    }
    echo "Installing Void Linux DMA-BUF proof dependencies..."
    sudo xbps-install -S \
        gcc \
        pkg-config \
        wayland-devel \
        wayland-protocols \
        MesaLib-devel \
        libdrm-devel
fi

cd "$ROOT_DIR"
if [[ "$DRY_RUN" == true ]]; then
    exec tools/finish_wayland_kitty_milestones.sh --dry-run
fi

if [[ ! -t 0 ]] || [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "After dependency installation, rerun this from a dedicated local text TTY." >&2
    exit 1
fi

exec tools/finish_wayland_kitty_milestones.sh
