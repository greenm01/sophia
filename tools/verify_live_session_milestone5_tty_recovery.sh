#!/usr/bin/env bash
set -euo pipefail

evidence="${1:-${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/milestone5-gtk/recovery.log}"
expected_emergency="${2:-false}"

[[ -s "$evidence" ]] || {
    echo "Milestone 5 TTY recovery evidence is missing: $evidence" >&2
    exit 1
}
mapfile -t lines < <(grep '^sophia_x_tty_recovery ' "$evidence" || true)
(( ${#lines[@]} == 1 )) || {
    echo "Expected exactly one Sophia X TTY recovery record" >&2
    exit 1
}
read -r -a parts <<<"${lines[0]}"
declare -A fields=()
for item in "${parts[@]:1}"; do
    [[ "$item" == *=* ]] || { echo "Malformed recovery field: $item" >&2; exit 1; }
    key="${item%%=*}"
    [[ -z "${fields[$key]+x}" ]] || { echo "Duplicate recovery field: $key" >&2; exit 1; }
    fields["$key"]="${item#*=}"
done
expected=(schema status kd_mode_before kd_mode_after termios_restored keyd_restored processes emergency)
(( ${#fields[@]} == ${#expected[@]} )) || {
    echo "Unknown or missing recovery fields" >&2
    exit 1
}
for key in "${expected[@]}"; do
    [[ -n "${fields[$key]+x}" ]] || { echo "Missing recovery field: $key" >&2; exit 1; }
done
[[ "${fields[schema]}" == 1 ]]
[[ "${fields[status]}" == complete ]]
[[ "${fields[kd_mode_before]}" =~ ^[0-9]+$ ]]
[[ "${fields[kd_mode_after]}" == "${fields[kd_mode_before]}" ]]
[[ "${fields[termios_restored]}" == true ]]
[[ "${fields[keyd_restored]}" == true ]]
[[ "${fields[processes]}" == 0 ]]
[[ "${fields[emergency]}" == "$expected_emergency" ]]
echo "Milestone 5 Sophia X TTY recovery evidence passed: $evidence"
