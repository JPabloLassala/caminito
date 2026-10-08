#!/usr/bin/env python3
"""A GTK3 window that asks to be maximised and says what it got.

  gtk-window.py TITLE

After 2 s it prints one JSON line: GDK's backend, the theme, the font, the cursor theme and
the window's size. It stays until it's closed. GDK_BACKEND picks Wayland or X11.
"""
import json
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402

window = Gtk.Window(title=sys.argv[1])
window.add(Gtk.Label(label=sys.argv[1]))
window.set_default_size(300, 200)
window.maximize()
window.connect("destroy", Gtk.main_quit)
window.show_all()


def report():
    settings = Gtk.Settings.get_default()
    width, height = window.get_size()
    print(json.dumps({
        "title": sys.argv[1],
        "backend": type(window.get_display()).__name__,
        "theme": settings.props.gtk_theme_name,
        "font": settings.props.gtk_font_name,
        "cursor_theme": settings.props.gtk_cursor_theme_name,
        "maximized": window.is_maximized(),
        "size": [width, height],
    }), flush=True)
    return False


GLib.timeout_add_seconds(2, report)
Gtk.main()
