#!/usr/bin/env python3
"""Type bounded lowercase text through a QEMU virtio input device over QMP."""

import json
import socket
import sys
import time


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


class QmpClient:
    def __init__(self, socket_path: str):
        self.socket_path = socket_path
        self.connection = None
        self.stream = None

    def __enter__(self):
        self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.connection.settimeout(5)
        self.connection.connect(self.socket_path)
        self.stream = self.connection.makefile("rwb", buffering=0)
        greeting = json.loads(self.stream.readline())
        if "QMP" not in greeting:
            fail("QMP greeting was missing")
        self.execute("qmp_capabilities")
        return self

    def __exit__(self, _kind, _value, _traceback):
        if self.stream is not None:
            self.stream.close()
        if self.connection is not None:
            self.connection.close()

    def execute(self, command: str, arguments=None):
        request = {"execute": command}
        if arguments is not None:
            request["arguments"] = arguments
        self.stream.write(
            (json.dumps(request, separators=(",", ":")) + "\n").encode()
        )
        self.stream.flush()
        while True:
            line = self.stream.readline()
            if not line:
                fail("QMP connection closed before a reply")
            message = json.loads(line)
            if "error" in message:
                fail(f"QMP command failed: {message['error']}")
            if "return" in message:
                return message["return"]


def key_event(qcode: str, down: bool):
    return {
        "type": "key",
        "data": {
            "down": down,
            "key": {"type": "qcode", "data": qcode},
        },
    }


def main():
    if len(sys.argv) not in (2, 3, 4):
        fail("usage: qemu_qmp_type.py QMP_SOCKET [--no-return] [LOWERCASE_TEXT]")
    socket_path = sys.argv[1]
    arguments = sys.argv[2:]
    append_return = True
    if arguments[:1] == ["--no-return"]:
        append_return = False
        arguments = arguments[1:]
    if len(arguments) > 1:
        fail("usage: qemu_qmp_type.py QMP_SOCKET [--no-return] [LOWERCASE_TEXT]")
    text = arguments[0] if arguments else ""
    if text and (not 1 <= len(text) <= 24 or not text.isascii() or not text.islower() or not text.isalpha()):
        fail("text must contain 1-24 lowercase ASCII letters")

    with QmpClient(socket_path) as qmp:
        qcodes = [*text]
        if append_return:
            qcodes.append("ret")
        for qcode in qcodes:
            qmp.execute(
                "input-send-event",
                {"events": [key_event(qcode, True), key_event(qcode, False)]},
            )
            # Pace transitions like an operator so the guest terminal and the
            # compositor observe each key pair instead of one QMP burst.
            time.sleep(0.2)


if __name__ == "__main__":
    main()
