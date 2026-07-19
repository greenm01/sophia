#!/usr/bin/env bash
set -euo pipefail

configured="${SOPHIA_XMONAD_BIN:-}"
if [[ -n "$configured" ]]; then
    [[ -x "$configured" ]] || { echo "configured xmonad binary is not executable: $configured" >&2; exit 1; }
    printf '%s\n' "$configured"
    exit 0
fi

if command -v xmonad >/dev/null 2>&1; then
    command -v xmonad
    exit 0
fi

source_dir="${SOPHIA_XMONAD_SOURCE:-${HOME}/src/xmonad}"
if [[ -d "$source_dir" ]]; then
    candidate="$(find "$source_dir/dist-newstyle" -type f \
        -path '*/x/xmonad/build/xmonad/xmonad' -perm -111 \
        -printf '%T@ %p\n' 2>/dev/null | sort -nr | sed -n '1s/^[^ ]* //p')"
    if [[ -n "$candidate" && -x "$candidate" ]]; then
        printf '%s\n' "$candidate"
        exit 0
    fi
fi

echo "xmonad is required; set SOPHIA_XMONAD_BIN or build ${source_dir}" >&2
exit 1
