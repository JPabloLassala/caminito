#!/usr/bin/env python3
"""Sends LauncherEntry Update signals and stays on the session bus until it is interrupted.

`gdbus emit` leaves the bus as soon as it has sent, and the Dock applet clears a Launcher
entry when its sender leaves (CMN-227), so a badge from `gdbus emit` is gone at once. This
keeps the connection, and so the badge, until Ctrl+C:

    hold-emit.py "application://org.mate.Caja.desktop" "{'count': <int64 5>, 'count-visible': <true>}"

It takes the app_uri and the properties as `gdbus emit` does, and any number of such pairs.
"""
import sys
import time

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

if len(sys.argv) < 3 or len(sys.argv) % 2 == 0:
    sys.exit(f"usage: {sys.argv[0]} APP_URI PROPERTIES [APP_URI PROPERTIES]...")
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
for uri, properties in zip(sys.argv[1::2], sys.argv[2::2]):
    parameters = GLib.Variant.parse(
        GLib.VariantType("(sa{sv})"), f"({GLib.Variant('s', uri).print_(False)}, {properties})"
    )
    bus.emit_signal(None, "/test", "com.canonical.Unity.LauncherEntry", "Update", parameters)
bus.flush_sync(None)
print(f"sent from {bus.get_unique_name()}, holding the bus: Ctrl+C to leave", flush=True)
try:
    time.sleep(3600)
except KeyboardInterrupt:
    pass
