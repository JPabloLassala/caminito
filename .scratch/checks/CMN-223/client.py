#!/usr/bin/env python3
"""A plain window for the work area to be measured against: client.py [maximized|plain].

Run it with GDK_BACKEND=wayland. A maximised one fills the work area, which is what a
panel's exclusive zone leaves of the output.
"""
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk

# The app id, which the Dock resolves to run.sh's cmn-223-window.desktop.
GLib.set_prgname("cmn-223-window")
window = Gtk.Window(title="CMN-223 test window")
window.set_default_size(560, 320)
if sys.argv[1:] == ["maximized"]:
    window.maximize()
window.add(Gtk.Label(label="a maximised window"))
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
