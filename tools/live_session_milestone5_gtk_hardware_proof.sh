#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
classic="${SOPHIA_M5_GTK_CLASSIC_EVIDENCE:-/tmp/sophia-milestone5-gtk-classic.log}"
confined="${SOPHIA_M5_GTK_CONFINED_EVIDENCE:-/tmp/sophia-milestone5-gtk-confined.log}"
devices="${SOPHIA_M5_GTK_INPUT_DEVICES:-}"
[[ -n "$devices" ]] || { echo "Set SOPHIA_M5_GTK_INPUT_DEVICES to comma-separated keyboard and pointer event paths" >&2; exit 1; }
echo "For each dialog, type sophia without Return, move the pointer, and click OK."
run_profile() {
    local profile="$1" evidence="$2"
    shift 2
    SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
      "$root/tools/live_session_persistent_hardware_proof.sh" \
      --namespace-profile="$profile" --client=zenity --client-arg=--entry \
      --client-arg=--title --client-arg='Sophia GTK proof' \
      --client-arg=--text --client-arg='Type sophia, then click OK' \
      --expect-client-stdout=$'sophia\n' --require-client-normal-exit \
      --expect-physical-text=sophia --expect-physical-pointer --exit-after-input-proof \
      --inject-surface-resize=640x360 --input-devices="$devices" "$@"
}
run_profile classic-shared "$classic" "$@"
SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 run_profile confined "$confined" "$@"
"$root/tools/verify_live_session_milestone5_gtk_evidence.sh" "$classic" "$confined"
