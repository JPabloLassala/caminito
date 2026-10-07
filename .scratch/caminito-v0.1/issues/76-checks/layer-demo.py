#!/usr/bin/env python3
"""A gtk-layer-shell surface with a text entry, for 63's keyboard modes.

  layer-demo.py exclusive|on_demand|none [LAYER]

It maps in the middle of the output, on the overlay layer unless LAYER names another, with
the namespace caminito-76-demo. Escape, when it has the keyboard, and its button close it.
"""
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
from gi.repository import Gdk, Gtk, GtkLayerShell  # noqa: E402

mode = sys.argv[1]
layer = sys.argv[2] if len(sys.argv) > 2 else "overlay"

window = Gtk.Window()
GtkLayerShell.init_for_window(window)
GtkLayerShell.set_namespace(window, "caminito-76-demo")
GtkLayerShell.set_layer(window, getattr(GtkLayerShell.Layer, layer.upper()))
GtkLayerShell.set_keyboard_mode(window, getattr(GtkLayerShell.KeyboardMode, mode.upper()))

box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin=16)
box.add(Gtk.Label(label=f"layer-shell demo: keyboard {mode}, on {layer}"))
entry = Gtk.Entry(placeholder_text="what's typed here went to the demo")
box.add(entry)
close = Gtk.Button(label="Close")
close.connect("clicked", Gtk.main_quit)
box.add(close)
window.add(box)


def on_key(_window, event):
    if event.keyval == Gdk.KEY_Escape:
        Gtk.main_quit()
    return False


window.connect("key-press-event", on_key)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
