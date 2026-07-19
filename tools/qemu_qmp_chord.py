#!/usr/bin/env python3
"""Send one bounded key chord through QEMU's virtio keyboard over QMP."""

import json
import socket
import sys
import time

ALLOWED = {
    "shift", "ctrl", "alt", "meta_l", "a", "c", "f", "j", "k", "p", "q", "v",
    "spc", "tab", "insert", "1", "2", "ret",
}


def fail(message):
    raise SystemExit(message)


def event(qcode, down):
    return {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": qcode}}}


def execute(stream, command, arguments=None):
    request = {"execute": command}
    if arguments is not None:
        request["arguments"] = arguments
    stream.write((json.dumps(request, separators=(",", ":")) + "\n").encode())
    stream.flush()
    while True:
        line = stream.readline()
        if not line:
            fail("QMP connection closed before a reply")
        message = json.loads(line)
        if "error" in message:
            fail(f"QMP command failed: {message['error']}")
        if "return" in message:
            return


def main():
    if len(sys.argv) != 3:
        fail("usage: qemu_qmp_chord.py QMP_SOCKET KEY+KEY")
    keys = sys.argv[2].split("+")
    if not keys or len(keys) > 4 or len(set(keys)) != len(keys) or any(key not in ALLOWED for key in keys):
        fail("chord contains unsupported, duplicate, or excessive keys")
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(5)
    connection.connect(sys.argv[1])
    try:
        stream = connection.makefile("rwb", buffering=0)
        greeting = json.loads(stream.readline())
        if "QMP" not in greeting:
            fail("QMP greeting was missing")
        execute(stream, "qmp_capabilities")
        execute(stream, "input-send-event", {"events": [event(key, True) for key in keys]})
        time.sleep(0.08)
        execute(stream, "input-send-event", {"events": [event(key, False) for key in reversed(keys)]})
        time.sleep(0.2)
    finally:
        connection.close()


if __name__ == "__main__":
    main()
