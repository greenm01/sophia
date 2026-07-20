#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

tree="$(cargo tree --offline -p sophia-cli --features atomic-scanout-live -e normal)"
if grep -qiE 'sophia-wayland-authority|smithay|wayland-(server|backend|protocols)' <<<"$tree"; then
    echo "Production Sophia dependency graph still contains a Wayland frontend." >&2
    exit 1
fi

if rg -qi 'sophia-wayland-session|client-backend=wayland|sophia-wayland-authority' \
    Cargo.toml crates tools \
    --glob '!tools/audit_xcentric_runtime.sh'; then
    echo "The active workspace still exposes a Wayland runtime path." >&2
    exit 1
fi

echo "Sophia production is X-centric; retired Wayland sources are isolated under research/wayland."
