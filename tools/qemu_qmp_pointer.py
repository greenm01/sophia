#!/usr/bin/env python3
"""Perform bounded word selection through a QEMU virtio input device."""

import sys

sys.dont_write_bytecode = True

from qemu_qmp_type import QmpClient, fail


def relative(axis: str, value: int):
    return {"type": "rel", "data": {"axis": axis, "value": value}}


def button(down: bool, name: str):
    return {"type": "btn", "data": {"down": down, "button": name}}


def send(qmp, events):
    qmp.execute("input-send-event", {"events": events})


def main():
    if len(sys.argv) not in (2, 5, 6):
        fail("usage: qemu_qmp_pointer.py QMP_SOCKET [DX DY CLICKS [BUTTON]]")
    socket_path = sys.argv[1]
    dx, dy, clicks = (40, 18, 2)
    button_name = "left"
    if len(sys.argv) >= 5:
        try:
            dx, dy, clicks = map(int, sys.argv[2:5])
        except ValueError:
            fail("DX, DY, and CLICKS must be integers")
        if not -4096 <= dx <= 4096 or not -4096 <= dy <= 4096 or not 1 <= clicks <= 4:
            fail("pointer movement must be within +/-4096 and clicks within 1-4")
    if len(sys.argv) == 6:
        button_name = sys.argv[5]
        if button_name not in ("left", "middle", "right"):
            fail("BUTTON must be left, middle, or right")
    with QmpClient(socket_path) as qmp:
        send(qmp, [relative("x", dx), relative("y", dy)])
        for _ in range(clicks):
            send(qmp, [button(True, button_name)])
            send(qmp, [button(False, button_name)])


if __name__ == "__main__":
    main()
