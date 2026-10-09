"""Gives the keyboard to windows of ticket 122's nested session, in turn.

  focus.py list                       each window's title, and whether it's the active one
  focus.py <gap ms> <title> ...       activates the window whose title has each text, in
                                      order, with the gap between one and the next

It speaks zwlr_foreign_toplevel_manager_v1 on the socket in WAYLAND_DISPLAY, by hand: no
Wayland binding for Python is installed here.
"""
import os
import socket
import struct
import sys
import time

ACTIVATED = 2


class Wire:
    def __init__(self):
        name = os.environ["WAYLAND_DISPLAY"]
        if not name.startswith("/"):
            name = os.path.join(os.environ["XDG_RUNTIME_DIR"], name)
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(name)
        self.buffer = b""
        self.last = 1

    def new_id(self):
        self.last += 1
        return self.last

    def send(self, target, opcode, payload=b""):
        size = 8 + len(payload)
        self.socket.sendall(struct.pack("<II", target, size << 16 | opcode) + payload)

    def events(self):
        """The events that have come, until the callback of a sync made now."""
        callback = self.new_id()
        self.send(1, 0, struct.pack("<I", callback))
        while True:
            while len(self.buffer) < 8:
                self.buffer += self.socket.recv(65536)
            target, word = struct.unpack_from("<II", self.buffer)
            size = word >> 16
            while len(self.buffer) < size:
                self.buffer += self.socket.recv(65536)
            payload, self.buffer = self.buffer[8:size], self.buffer[size:]
            if target == callback:
                return
            yield target, word & 0xFFFF, payload


def string(text):
    data = text.encode() + b"\0"
    return struct.pack("<I", len(data)) + data + b"\0" * (-len(data) % 4)


def read_string(payload, at=0):
    (length,) = struct.unpack_from("<I", payload, at)
    return payload[at + 4 : at + 4 + length - 1].decode(), at + 4 + length + (-length % 4)


def read(wire, manager, windows):
    for target, opcode, payload in wire.events():
        if target == manager and opcode == 0:
            windows[struct.unpack("<I", payload)[0]] = {"title": "", "active": False}
        elif target in windows and opcode == 0:
            windows[target]["title"] = read_string(payload)[0]
        elif target in windows and opcode == 4:
            (length,) = struct.unpack_from("<I", payload)
            states = struct.unpack_from(f"<{length // 4}I", payload, 4)
            windows[target]["active"] = ACTIVATED in states
        elif target in windows and opcode == 6:
            del windows[target]


wire = Wire()
registry = wire.new_id()
wire.send(1, 1, struct.pack("<I", registry))
seat = manager = None
for target, opcode, payload in list(wire.events()):
    if target == registry and opcode == 0:
        (name,) = struct.unpack_from("<I", payload)
        interface, at = read_string(payload, 4)
        if interface == "wl_seat" and seat is None:
            seat = wire.new_id()
            wire.send(registry, 0, struct.pack("<I", name) + string(interface) + struct.pack("<II", 1, seat))
        elif interface == "zwlr_foreign_toplevel_manager_v1":
            manager = wire.new_id()
            wire.send(registry, 0, struct.pack("<I", name) + string(interface) + struct.pack("<II", 1, manager))
if seat is None or manager is None:
    sys.exit("the compositor has no seat or no zwlr_foreign_toplevel_manager_v1")
windows = {}
read(wire, manager, windows)
read(wire, manager, windows)

if sys.argv[1] == "list":
    for window in windows.values():
        print(("* " if window["active"] else "  ") + window["title"])
    sys.exit()

gap = float(sys.argv[1]) / 1000
handles = []
for text in sys.argv[2:]:
    found = [handle for handle, window in windows.items() if text in window["title"]]
    if len(found) != 1:
        sys.exit(f"{len(found)} windows have {text!r} in their title")
    handles.append(found[0])
for index, handle in enumerate(handles):
    if index:
        time.sleep(gap)
    wire.send(handle, 4, struct.pack("<I", seat))
    print(f"{time.time() * 1000:.0f} {sys.argv[2 + index]}")
read(wire, manager, windows)
