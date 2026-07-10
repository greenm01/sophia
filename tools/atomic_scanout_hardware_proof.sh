#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFLIGHT_FILE="${SOPHIA_ATOMIC_SCANOUT_PREFLIGHT:-/tmp/sophia-atomic-scanout-preflight.log}"
ATOMIC_EVIDENCE_FILE="${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}"
RUNTIME_EVIDENCE_FILE="${SOPHIA_RUNTIME_RENDERED_SCANOUT_EVIDENCE:-/tmp/sophia-runtime-rendered-scanout.log}"

mkdir -p "$(dirname "$PREFLIGHT_FILE")"
mkdir -p "$(dirname "$ATOMIC_EVIDENCE_FILE")"
mkdir -p "$(dirname "$RUNTIME_EVIDENCE_FILE")"

echo "Sophia atomic scanout hardware proof"
echo "This proof may take DRM master on a primary /dev/dri/card* node."
echo "Preflight: $PREFLIGHT_FILE"
echo "Atomic evidence: $ATOMIC_EVIDENCE_FILE"
echo "Runtime evidence: $RUNTIME_EVIDENCE_FILE"

"$ROOT_DIR/tools/atomic_scanout_preflight.sh"

SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
SOPHIA_ATOMIC_SCANOUT_PREFLIGHT="$PREFLIGHT_FILE" \
SOPHIA_ATOMIC_SCANOUT_EVIDENCE="$ATOMIC_EVIDENCE_FILE" \
    "$ROOT_DIR/tools/atomic_scanout_smoke.sh" "$@"

SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
SOPHIA_ATOMIC_SCANOUT_PREFLIGHT="$PREFLIGHT_FILE" \
SOPHIA_RUNTIME_RENDERED_SCANOUT_EVIDENCE="$RUNTIME_EVIDENCE_FILE" \
    "$ROOT_DIR/tools/runtime_rendered_scanout_evidence.sh" "$@"

"$ROOT_DIR/tools/verify_atomic_scanout_preflight.sh" "$PREFLIGHT_FILE"
"$ROOT_DIR/tools/verify_atomic_scanout_evidence.sh" "$ATOMIC_EVIDENCE_FILE"
"$ROOT_DIR/tools/verify_runtime_rendered_scanout_evidence.sh" "$RUNTIME_EVIDENCE_FILE"

echo "Sophia atomic scanout hardware proof passed"
