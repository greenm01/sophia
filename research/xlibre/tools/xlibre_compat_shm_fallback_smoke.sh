#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export SOPHIA_XLIBRE_LATENCY_CLIENT=kitty
export SOPHIA_XLIBRE_LATENCY_VERIFIER=tools/verify_xlibre_kitty_latency_evidence.sh
export SOPHIA_XLIBRE_LATENCY_EVIDENCE="${SOPHIA_XLIBRE_FALLBACK_EVIDENCE:-/tmp/sophia-xlibre-shm-fallback.log}"
export SOPHIA_XLIBRE_DISABLE_SHM=1
export SOPHIA_XLIBRE_EXPECT_VERIFIER_REJECTION=1
exec "$ROOT_DIR/tools/xlibre_compat_latency_smoke.sh"
