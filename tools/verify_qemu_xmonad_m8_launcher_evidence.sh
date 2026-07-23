#!/usr/bin/env bash
set -euo pipefail

evidence=${1:-/tmp/sophia-qemu-xmonad-m8-launcher.log}
[[ -r "$evidence" ]] || { echo "missing launcher evidence: $evidence" >&2; exit 1; }

required_chords=(
    meta_l+j meta_l+k meta_l+spc meta_l+2 meta_l+shift+1
    meta_l+ret meta_l+shift+c meta_l+shift+q
)
for chord in "${required_chords[@]}"; do
    grep -q "^sophia_qemu_xmonad_input schema=1 status=sent chord=$chord$" "$evidence" || {
        echo "missing launcher chord evidence: $chord" >&2
        exit 1
    }
done

for action in LaunchTerminal CloseFocused Logout; do
    grep -q "status=session_action_committed .*action=$action" "$evidence" || {
        echo "missing launcher session action: $action" >&2
        exit 1
    }
done

grep -q '^sophia_live_session_mode schema=1 mode=normal configured_apps=1 startup_apps=1$' "$evidence"
grep -q '^sophia_live_wm schema=1 status=ready adapter=external ' "$evidence"
grep -Eq '^sophia_live_wm schema=1 status=layout_committed .*surfaces=[2-9][0-9]* ' "$evidence"
grep -Eq '^sophia_live_session schema=14 status=bounded_complete .*wm_policy=external .*wm_restarts=1 .*wm_degraded=false ' "$evidence"
grep -q '^sophia_qemu_guest schema=1 status=complete scenario=xmonad-m8-launcher$' "$evidence"
grep -q '^sophia_qemu_xmonad schema=1 status=restart_injected target=compatibility_bridge$' "$evidence"
grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$evidence"
if grep -q ' status=failed ' "$evidence"; then
    echo "xmonad launcher evidence contains a failure marker" >&2
    exit 1
fi

echo "Milestone 8 normal xmonad launcher evidence passed: $evidence"
