#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Host: $(hostname)"
echo "Repository: $ROOT_DIR"
echo
echo "Graphical session owners:"
found_graphical=false
for process in river niri sway Hyprland kwin_wayland Xorg; do
    if pgrep -x "$process" >/dev/null 2>&1; then
        echo "  running: $process"
        found_graphical=true
    fi
done
if [[ "$found_graphical" == false ]]; then
    echo "  none detected"
fi

echo
echo "Sophia processes:"
if ! pgrep -a -f '(^|/)sophia([[:space:]]|$)' 2>/dev/null; then
    echo "  none detected"
fi

echo
echo "DRM nodes:"
drm_nodes=(/dev/dri/card*)
if [[ -e "${drm_nodes[0]}" ]]; then
    for node in "${drm_nodes[@]}"; do
        if [[ -r "$node" && -w "$node" ]]; then
            access=read-write
        elif [[ -r "$node" ]]; then
            access=read-only
        else
            access=unavailable
        fi
        echo "  $node: $access"
    done
else
    echo "  none detected"
fi

echo
echo "Repository changes:"
if [[ -d "$ROOT_DIR/.git" ]]; then
    changes="$(git -C "$ROOT_DIR" status --short)"
    if [[ -n "$changes" ]]; then
        printf '%s\n' "$changes"
    else
        echo "  clean"
    fi
else
    echo "  no Git metadata (synchronized source tree)"
fi
