#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
(( $# == 5 )) || { echo "usage: $0 M3_CLASSIC M3_CONFINED M4_GPU M5_CLASSIC M5_CONFINED" >&2; exit 2; }
"$root/tools/verify_live_session_milestone3_evidence.sh" "$1" "$2"
"$root/tools/verify_live_session_milestone4_evidence.sh" "$3"
"$root/tools/verify_live_session_milestone5_gtk_evidence.sh" "$4" "$5"
echo "Sophia X three-class session baseline passed"
