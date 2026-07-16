#!/usr/bin/env bash
set -euo pipefail

classic="${1:-/tmp/sophia-milestone5-gtk-classic.log}"
confined="${2:-/tmp/sophia-milestone5-gtk-confined.log}"

verify_one() {
    local evidence="$1" profile="$2"
    [[ -s "$evidence" ]] || { echo "Milestone 5 GTK evidence is missing: $evidence" >&2; exit 1; }
    ! grep -q '^Error:' "$evidence" || { echo "Milestone 5 GTK evidence contains an error: $evidence" >&2; exit 1; }
    mapfile -t records < <(grep '^sophia_x_application_session ' "$evidence" || true)
    (( ${#records[@]} == 1 )) || { echo "Expected exactly one GTK application record in $evidence" >&2; exit 1; }
    read -r -a parts <<<"${records[0]}"
    declare -A fields=()
    for item in "${parts[@]:1}"; do
        [[ "$item" == *=* ]] || { echo "Malformed application field: $item" >&2; exit 1; }
        key="${item%%=*}"
        [[ -z "${fields[$key]+x}" ]] || { echo "Duplicate application field: $key" >&2; exit 1; }
        fields[$key]="${item#*=}"
    done
    expected=(schema status class client profile child_outcome exit_code stdout_match protocol_errors first_error physical_text pointer_button surface_resize buffer_path native_presentation cleanup)
    (( ${#fields[@]} == ${#expected[@]} )) || { echo "Unknown or missing application fields in $evidence" >&2; exit 1; }
    for key in "${expected[@]}"; do [[ -n "${fields[$key]+x}" ]] || { echo "Missing application field: $key" >&2; exit 1; }; done
    [[ "${fields[schema]}" == 1 && "${fields[status]}" == passed && "${fields[class]}" == gtk3_software ]]
    [[ "${fields[client]}" == zenity && "${fields[profile]}" == "$profile" ]]
    [[ "${fields[child_outcome]}" == normal && "${fields[exit_code]}" == 0 && "${fields[stdout_match]}" == true ]]
    [[ "${fields[protocol_errors]}" == 0 && "${fields[first_error]}" == none ]]
    [[ "${fields[physical_text]}" == true && "${fields[pointer_button]}" == true ]]
    [[ "${fields[surface_resize]}" == committed && "${fields[buffer_path]}" == cpu_shm ]]
    [[ "${fields[native_presentation]}" == enabled && "${fields[cleanup]}" == clean ]]
}

verify_one "$classic" classic_shared
verify_one "$confined" confined
echo "Milestone 5 GTK paired evidence passed"
