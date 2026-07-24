#!/usr/bin/env python3
"""Get or set Linux virtual-terminal display and keyboard modes."""

from __future__ import annotations

import array
import fcntl
import sys

KDSETMODE = 0x4B3A
KDGETMODE = 0x4B3B
KD_TEXT = 0
KD_GRAPHICS = 1
KDGKBMODE = 0x4B44
KDSKBMODE = 0x4B45
K_OFF = 4


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit(
            "usage: sophia_tty_mode.py get|text|graphics|MODE|"
            "get-keyboard|keyboard-off|keyboard-MODE"
        )
    action = sys.argv[1]
    with open("/dev/tty", "rb+", buffering=0) as tty:
        if action == "get-keyboard":
            value = array.array("i", [0])
            fcntl.ioctl(tty.fileno(), KDGKBMODE, value, True)
            print(value[0])
            return 0
        if action == "keyboard-off":
            fcntl.ioctl(tty.fileno(), KDSKBMODE, K_OFF)
            return 0
        if action.startswith("keyboard-"):
            fcntl.ioctl(tty.fileno(), KDSKBMODE, int(action.removeprefix("keyboard-"), 10))
            return 0
        if action == "get":
            value = array.array("i", [0])
            fcntl.ioctl(tty.fileno(), KDGETMODE, value, True)
            print(value[0])
            return 0
        if action == "text":
            mode = KD_TEXT
        elif action == "graphics":
            mode = KD_GRAPHICS
        else:
            mode = int(action, 10)
        fcntl.ioctl(tty.fileno(), KDSETMODE, mode)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
