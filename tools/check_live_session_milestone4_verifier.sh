#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFY="$ROOT_DIR/tools/verify_live_session_milestone4_evidence.sh"
FIXTURES="$ROOT_DIR/tools/fixtures"

"$VERIFY" "$FIXTURES/live_session_milestone4_evidence_pass.log"
if "$VERIFY" "$FIXTURES/live_session_milestone4_evidence_no_mixed_export.log" >/dev/null 2>&1; then
    echo "Milestone 4 verifier accepted evidence without a mixed GPU export" >&2
    exit 1
fi

echo "Milestone 4 evidence verifier checks passed"
