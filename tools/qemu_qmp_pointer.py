#!/usr/bin/env python3
"""Perform bounded word selection through a QEMU virtio input device."""

import sys

sys.dont_write_bytecode = True

from qemu_qmp_type import QmpClient, fail


def relative(axis: str, value: int):
    return {"type": "rel", "data": {"axis": axis, "value": value}}


def button(down: bool):
    return {"type": "btn", "data": {"down": down, "button": "left"}}


def send(qmp, events):
    qmp.execute("input-send-event", {"events": events})


def main():
    if len(sys.argv) != 2:
        fail("usage: qemu_qmp_pointer.py QMP_SOCKET")
    with QmpClient(sys.argv[1]) as qmp:
        send(qmp, [relative("x", 40), relative("y", 18)])
        for _ in range(2):
            send(qmp, [button(True)])
            send(qmp, [button(False)])


if __name__ == "__main__":
    main()
