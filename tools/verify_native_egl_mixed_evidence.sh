#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_M4_NATIVE_EGL_EVIDENCE:-/tmp/sophia-native-egl-mixed.log}}"

mapfile -t lines < <(grep -E '^sophia_native_egl_mixed ' "$EVIDENCE_FILE" 2>/dev/null || true)
if (( ${#lines[@]} != 1 )); then
    echo "Native EGL mixed evidence expected exactly one result line." >&2
    exit 1
fi

read -r -a parts <<<"${lines[0]}"
declare -A observed=()
for field in "${parts[@]:1}"; do
    [[ "$field" == *=* ]] || {
        echo "Native EGL mixed evidence has malformed field: $field" >&2
        exit 1
    }
    key="${field%%=*}"
    [[ -z "${observed[$key]+set}" ]] || {
        echo "Native EGL mixed evidence has duplicate field: $key" >&2
        exit 1
    }
    observed["$key"]="${field#*=}"
done

required=(schema case status stage cpu_layers dmabuf_layers child_outcome live_sources live_fences live_transactions)
for key in "${required[@]}"; do
    [[ -n "${observed[$key]+set}" ]] || {
        echo "Native EGL mixed evidence is missing field: $key" >&2
        exit 1
    }
done
[[ "${observed[schema]}" == "1" ]]
[[ "${observed[case]}" == "mixed" ]]
[[ "${observed[status]}" == "Exported" ]]
[[ "${observed[stage]}" == "Exported" ]]
[[ "${observed[child_outcome]}" == "completed" ]]
(( observed[cpu_layers] > 0 ))
(( observed[dmabuf_layers] > 0 ))
(( observed[live_sources] == 0 ))
(( observed[live_fences] == 0 ))
(( observed[live_transactions] == 0 ))

echo "Native EGL mixed evidence passed: $EVIDENCE_FILE"
