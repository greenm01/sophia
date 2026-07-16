#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verify="$root/tools/verify_live_session_milestone5_gtk_evidence.sh"
classic="$root/tools/fixtures/live_session_milestone5_gtk_classic_pass.log"
confined="$root/tools/fixtures/live_session_milestone5_gtk_confined_pass.log"
bad="$root/tools/fixtures/live_session_milestone5_gtk_protocol_error.log"
"$verify" "$classic" "$confined"
if "$verify" "$classic" "$bad" >/dev/null 2>&1; then
    echo "Milestone 5 verifier accepted protocol-error evidence" >&2
    exit 1
fi
echo "Milestone 5 GTK verifier fixtures passed"
