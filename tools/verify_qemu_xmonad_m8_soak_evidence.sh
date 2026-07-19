#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-/tmp/sophia-qemu-xmonad-m8-soak.log}
[[ -s "$evidence" ]] || { echo "missing M8 soak evidence: $evidence" >&2; exit 1; }
cycles=$(grep -c '^sophia_qemu_m8_soak schema=1 status=cycle_complete ' "$evidence" || true)
(( cycles >= 20 )) || { echo "M8 soak completed only $cycles cycles" >&2; exit 1; }
restarts=$(grep -c '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$evidence" || true)
(( restarts >= 2 )) || { echo "M8 soak observed only $restarts bridge recoveries" >&2; exit 1; }
completion=$(grep '^sophia_live_session schema=14 status=bounded_complete ' "$evidence")
elapsed=$(tr ' ' '\n' <<<"$completion" | sed -n 's/^elapsed_msec=//p')
[[ "$elapsed" =~ ^[0-9]+$ ]] && (( elapsed >= 1800000 )) || {
    echo "M8 soak ended before 30 minutes: ${elapsed:-missing}" >&2
    exit 1
}
summary=$(grep '^sophia_qemu_m8_soak schema=1 status=cycle_complete ' "$evidence" | tail -n 1)
for field in terminal_restarts firefox_restarts launcher_restarts; do
    value=$(tr ' ' '\n' <<<"$summary" | sed -n "s/^$field=//p")
    [[ "$value" =~ ^[0-9]+$ ]] && (( value >= 20 )) || {
        echo "M8 soak did not complete 20 $field: ${value:-missing}" >&2
        exit 1
    }
done
close_actions=$(tr ' ' '\n' <<<"$summary" | sed -n 's/^close_actions=//p')
[[ "$close_actions" =~ ^[0-9]+$ ]] && (( close_actions >= 60 )) || {
    echo "M8 soak did not commit 60 close actions: ${close_actions:-missing}" >&2
    exit 1
}
grep -q '^sophia_firefox_m8 schema=1 status=complete stages=6 ' "$evidence"
grep -Eq '^sophia_live_session_protocol_errors schema=1 expected=[0-9]+ unexpected=0$' "$evidence"
grep -q '^sophia_live_session_health schema=1 status=clean protocol_errors=0 pending_wm=0 pending_actions=0 pending_input=0 wm_degraded=false$' "$evidence"
grep -q '^sophia_live_session_cleanup schema=1 status=clean app_groups=0 frontend_workers=0 namespace=revoked xauthority=removed$' "$evidence"
grep -q '^sophia_qemu_guest schema=1 status=complete scenario=xmonad-m8-soak$' "$evidence"
if grep -q ' status=failed ' "$evidence"; then
    echo "M8 soak evidence contains a failure marker" >&2
    exit 1
fi
echo "Milestone 8 30-minute soak evidence passed: $evidence"
