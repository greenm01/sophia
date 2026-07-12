#!/usr/bin/env bash
set -euo pipefail

DISPLAY_NAME="${1:?usage: configure_xlibre_evdev_keymap.sh :DISPLAY}"
if [[ ! "$DISPLAY_NAME" =~ ^:[0-9]+$ ]]; then
    echo "XLibre display must have the form :NUMBER" >&2
    exit 1
fi
for command in setxkbmap xmodmap; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "missing required command: $command" >&2
        exit 1
    fi
done

setxkbmap -display "$DISPLAY_NAME" -rules evdev -model pc105 -layout us
keymap="$(xmodmap -display "$DISPLAY_NAME" -pk)"

require_keysym() {
    local keycode="$1"
    local keysym="$2"
    local line
    line="$(awk -v keycode="$keycode" '$1 == keycode { print; exit }' <<< "$keymap")"
    if [[ "$line" != *"($keysym)"* ]]; then
        echo "XLibre evdev keymap is missing $keysym at keycode $keycode" >&2
        exit 1
    fi
}

require_keysym 111 Up
require_keysym 113 Left
require_keysym 114 Right
require_keysym 116 Down

echo "sophia_xlibre_keymap schema=1 status=ready rules=evdev up=111 left=113 right=114 down=116"
