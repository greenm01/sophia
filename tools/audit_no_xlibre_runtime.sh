#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

tree="$(cargo tree -p sophia-cli --features atomic-scanout-live -e normal)"
if grep -qiE 'sophia-x-bridge|xlibre' <<<"$tree"; then
    echo "Production Sophia dependency graph still contains the XLibre bridge." >&2
    exit 1
fi
if grep -qE '/usr/libexec/Xorg|xlibre-25|dummy_drv|--client-backend=xlibre-compat|configure_xlibre' \
    tools/run_sophia_kitty_session.sh tools/install_sophia_session.sh; then
    echo "Installed Sophia launcher still contains an XLibre/Xorg launch dependency." >&2
    exit 1
fi
echo "Sophia production runtime is XLibre-free; historical probes require xlibre-research."
