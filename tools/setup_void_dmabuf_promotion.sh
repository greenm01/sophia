#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
INSTALL_LOG="$STATE_DIR/void-dmabuf-install.log"
REQUIRED_PACKAGES=(
    gcc
    pkg-config
    wayland-devel
    wayland-protocols
    MesaLib-devel
    libdrm-devel
)
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
    command -v xbps-query >/dev/null || {
        echo "Missing required command: xbps-query" >&2
        exit 1
    }
    missing_packages=()
    for package in "${REQUIRED_PACKAGES[@]}"; do
        if ! xbps-query -p pkgver "$package" >/dev/null 2>&1; then
            missing_packages+=("$package")
        fi
    done
    if (( ${#missing_packages[@]} == 0 )); then
        echo "All Void Linux DMA-BUF proof dependencies are already installed."
    else
        echo "Installing missing Void Linux DMA-BUF proof dependencies..."
        printf '  %s\n' "${missing_packages[@]}"
        mkdir -p "$STATE_DIR"
        chmod 700 "$STATE_DIR"
        if ! sudo xbps-install -S "${missing_packages[@]}" 2>&1 | tee "$INSTALL_LOG"; then
            echo "Dependency installation failed; full log: $INSTALL_LOG" >&2
            echo "Void does not support partial upgrades. Run 'sudo xbps-install -Syu', then rerun this helper." >&2
            exit 1
        fi
    fi
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
