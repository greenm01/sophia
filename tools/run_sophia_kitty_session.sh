#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_TTY_PROFILE=kitty
exec "$ROOT_DIR/tools/run_sophia_xmonad_session.sh" "$@"
