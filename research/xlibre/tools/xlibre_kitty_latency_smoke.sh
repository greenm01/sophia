#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_XLIBRE_LATENCY_CLIENT=kitty
export SOPHIA_XLIBRE_LATENCY_VERIFIER=tools/verify_xlibre_kitty_latency_evidence.sh
export SOPHIA_XLIBRE_LATENCY_EVIDENCE="${SOPHIA_XLIBRE_KITTY_EVIDENCE:-/tmp/sophia-xlibre-kitty-latency.log}"
exec "$ROOT_DIR/tools/xlibre_compat_latency_smoke.sh"
