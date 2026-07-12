#!/usr/bin/env python3
"""Send one Ctrl-Alt-Backspace chord through QEMU's virtual keyboard."""

import sys

sys.dont_write_bytecode = True

from qemu_qmp_type import QmpClient, fail, key_event


def main():
    if len(sys.argv) != 2:
        fail("usage: qemu_qmp_emergency_chord.py QMP_SOCKET")

    events = [
        key_event("ctrl", True),
        key_event("alt", True),
        key_event("backspace", True),
        key_event("backspace", False),
        key_event("alt", False),
        key_event("ctrl", False),
    ]
    with QmpClient(sys.argv[1]) as qmp:
        qmp.execute("input-send-event", {"events": events})


if __name__ == "__main__":
    main()
