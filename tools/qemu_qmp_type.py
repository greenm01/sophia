#!/usr/bin/env python3
"""Type bounded lowercase text through a QEMU virtio input device over QMP."""

import json
import socket
import sys


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def read_reply(stream):
    while True:
        line = stream.readline()
        if not line:
            fail("QMP connection closed before a reply")
        message = json.loads(line)
        if "error" in message:
            fail(f"QMP command failed: {message['error']}")
        if "return" in message:
            return message["return"]


def execute(stream, command: str, arguments=None):
    request = {"execute": command}
    if arguments is not None:
        request["arguments"] = arguments
    stream.write((json.dumps(request, separators=(",", ":")) + "\n").encode())
    stream.flush()
    return read_reply(stream)


def key_event(qcode: str, down: bool):
    return {
        "type": "key",
        "data": {
            "down": down,
            "key": {"type": "qcode", "data": qcode},
        },
    }


def main():
    if len(sys.argv) != 3:
        fail("usage: qemu_qmp_type.py QMP_SOCKET LOWERCASE_TEXT")
    socket_path, text = sys.argv[1:]
    if not 1 <= len(text) <= 24 or not text.isascii() or not text.islower() or not text.isalpha():
        fail("text must contain 1-24 lowercase ASCII letters")

    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(5)
        connection.connect(socket_path)
        stream = connection.makefile("rwb", buffering=0)
        greeting = json.loads(stream.readline())
        if "QMP" not in greeting:
            fail("QMP greeting was missing")
        execute(stream, "qmp_capabilities")
        for qcode in [*text, "ret"]:
            execute(
                stream,
                "input-send-event",
                {"events": [key_event(qcode, True), key_event(qcode, False)]},
            )


if __name__ == "__main__":
    main()
