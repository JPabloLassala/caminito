"""Owns a bus name for a number of seconds, as a stand-in for the applet."""
import sys
from gi.repository import Gio, GLib

loop = GLib.MainLoop()
Gio.bus_own_name(Gio.BusType.SESSION, sys.argv[1], Gio.BusNameOwnerFlags.NONE, None, None, None)
GLib.timeout_add_seconds(int(sys.argv[2]), loop.quit)
loop.run()
