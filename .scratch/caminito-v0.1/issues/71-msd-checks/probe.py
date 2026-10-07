#!/usr/bin/env python3
"""What the msd checks read that no command-line tool gives.

  probe.py owner NAME      the X11 window that owns the selection NAME, on DISPLAY
  probe.py png FILE COLOUR a 64x64 PNG of one colour (rrggbb)
  probe.py watch           a GTK3 window that prints its theme, font and cursor theme, at
                           start and at each change
  probe.py layout FILE SCALE  a monitors-wayland.xml for the outputs wlr-randr shows
"""
import ctypes
import json
import subprocess
import sys


def owner(name):
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    x11.XInternAtom.restype = ctypes.c_ulong
    x11.XGetSelectionOwner.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XGetSelectionOwner.restype = ctypes.c_ulong
    display = x11.XOpenDisplay(None)
    print(hex(x11.XGetSelectionOwner(display, x11.XInternAtom(display, name.encode(), 0))))


def png(file, colour):
    import gi
    gi.require_version("GdkPixbuf", "2.0")
    from gi.repository import GdkPixbuf
    pixbuf = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, 64, 64)
    pixbuf.fill(int(colour, 16) << 8 | 0xFF)
    pixbuf.savev(file, "png", [], [])


def watch():
    import gi
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gtk
    names = ["gtk-theme-name", "gtk-font-name", "gtk-cursor-theme-name"]
    settings = Gtk.Settings.get_default()

    def show(*_):
        print(" ".join("%s=%s" % (n, settings.get_property(n)) for n in names), flush=True)

    for name in names:
        settings.connect("notify::" + name, show)
    window = Gtk.Window(title="T71WATCH")
    window.add(Gtk.Label(label="ticket 71"))
    window.show_all()
    show()
    Gtk.main()


def layout(file, scale):
    heads = json.loads(subprocess.check_output(["wlr-randr", "--json"]))
    with open(file, "w") as out:
        out.write('<monitors version="1">\n  <configuration>\n')
        for head in heads:
            mode = next(m for m in head["modes"] if m["current"])
            out.write('      <head name="%s">\n' % head["name"])
            out.write("          <enabled>yes</enabled>\n")
            out.write("          <width>%d</width>\n" % mode["width"])
            out.write("          <height>%d</height>\n" % mode["height"])
            out.write("          <rate>%d</rate>\n" % round(mode["refresh"] * 1000))
            out.write("          <x>0</x>\n          <y>0</y>\n")
            out.write("          <scale>%s</scale>\n" % scale)
            # As msd saves it. Nested, the winit output is flipped-180, and Caminito fails a
            # configuration that asks for another transform (58).
            out.write("          <transform>%s</transform>\n" % head["transform"])
            out.write("      </head>\n")
        out.write("  </configuration>\n</monitors>\n")


{"owner": owner, "png": png, "watch": watch,
 "layout": layout}[sys.argv[1]](*sys.argv[2:])
