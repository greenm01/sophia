#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFLIGHT_FILE="${SOPHIA_ATOMIC_SCANOUT_PREFLIGHT:-/tmp/sophia-atomic-scanout-preflight.log}"
ATOMIC_EVIDENCE_FILE="${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}"
RUNTIME_EVIDENCE_FILE="${SOPHIA_RUNTIME_RENDERED_SCANOUT_EVIDENCE:-/tmp/sophia-runtime-rendered-scanout.log}"

status=0
MIN_RUST_MAJOR=1
MIN_RUST_MINOR=96

print_command_version() {
    local name="$1"
    if command -v "$name" >/dev/null 2>&1; then
        "$name" --version 2>/dev/null | head -n 1 || true
    else
        echo "$name: not found"
        status=1
    fi
}

check_rustc_minimum() {
    if ! command -v rustc >/dev/null 2>&1; then
        return
    fi

    local rustc_version
    rustc_version="$(rustc --version 2>/dev/null || true)"
    if [[ "$rustc_version" =~ ^rustc[[:space:]]+([0-9]+)\.([0-9]+)\. ]]; then
        local major="${BASH_REMATCH[1]}"
        local minor="${BASH_REMATCH[2]}"
        if (( major < MIN_RUST_MAJOR || (major == MIN_RUST_MAJOR && minor < MIN_RUST_MINOR) )); then
            echo "rustc minimum: need ${MIN_RUST_MAJOR}.${MIN_RUST_MINOR} or newer"
            status=1
            return
        fi
        echo "rustc minimum: ok"
    else
        echo "rustc minimum: unable to parse version"
        status=1
    fi
}

print_log_summary() {
    local label="$1"
    local file="$2"
    if [[ -s "$file" ]]; then
        local size
        size="$(wc -c < "$file" | tr -d '[:space:]')"
        echo "$label: present ($size bytes) $file"
    elif [[ -e "$file" ]]; then
        echo "$label: empty $file"
        status=1
    else
        echo "$label: missing $file"
        status=1
    fi
}

run_verifier() {
    local label="$1"
    local verifier="$2"
    local file="$3"
    if [[ ! -s "$file" ]]; then
        echo "$label verifier: skipped; log missing or empty"
        status=1
        return
    fi
    if "$ROOT_DIR/$verifier" "$file"; then
        echo "$label verifier: pass"
    else
        echo "$label verifier: fail"
        status=1
    fi
}

echo "Sophia atomic scanout hardware status"
echo

echo "Toolchain"
print_command_version rustc
print_command_version cargo
check_rustc_minimum
echo

echo "DRM device visibility"
if [[ -d /dev/dri ]]; then
    echo "/dev/dri: present"
    shopt -s nullglob
    card_nodes=(/dev/dri/card*)
    render_nodes=(/dev/dri/renderD*)
    shopt -u nullglob
    echo "primary card nodes: ${#card_nodes[@]}"
    echo "render nodes: ${#render_nodes[@]}"
    if [[ "${#card_nodes[@]}" -eq 0 ]]; then
        status=1
    fi
else
    echo "/dev/dri: missing"
    status=1
fi
echo

echo "Captured logs"
print_log_summary "preflight" "$PREFLIGHT_FILE"
print_log_summary "atomic evidence" "$ATOMIC_EVIDENCE_FILE"
print_log_summary "runtime evidence" "$RUNTIME_EVIDENCE_FILE"
echo

echo "Offline proof verification"
run_verifier "preflight" "tools/verify_atomic_scanout_preflight.sh" "$PREFLIGHT_FILE"
run_verifier "atomic evidence" "tools/verify_atomic_scanout_evidence.sh" "$ATOMIC_EVIDENCE_FILE"
run_verifier "runtime evidence" "tools/verify_runtime_rendered_scanout_evidence.sh" "$RUNTIME_EVIDENCE_FILE"
echo

if [[ "$status" -eq 0 ]]; then
    echo "Sophia atomic scanout hardware status: proof logs verify"
else
    echo "Sophia atomic scanout hardware status: proof incomplete"
fi

exit "$status"
