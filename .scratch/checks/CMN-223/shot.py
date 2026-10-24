#!/usr/bin/env python3
"""Saves the X11 screen to a PNG: shot.py OUT.png. DISPLAY names the nested Caminito's host."""
import sys

import gi

gi.require_version("Gdk", "3.0")
from gi.repository import Gdk

display = Gdk.Display.get_default()
root = Gdk.get_default_root_window()
width, height = root.get_width(), root.get_height()
pixbuf = Gdk.pixbuf_get_from_window(root, 0, 0, width, height)
pixbuf.savev(sys.argv[1], "png", [], [])
print(f"{sys.argv[1]}: {width}x{height}")
