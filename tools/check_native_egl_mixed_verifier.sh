#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_native_egl_mixed_evidence.sh"
FIXTURES="$ROOT_DIR/tools/fixtures"

"$VERIFY" "$FIXTURES/native_egl_mixed_evidence_pass.log"
if "$VERIFY" "$FIXTURES/native_egl_mixed_evidence_no_cpu.log" >/dev/null 2>&1; then
    echo "Native EGL mixed verifier accepted evidence without a CPU layer." >&2
    exit 1
fi

echo "Native EGL mixed evidence verifier checks passed"
