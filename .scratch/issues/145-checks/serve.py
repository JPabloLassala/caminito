"""Serves a small dbusmenu on the session bus, for look.vala to import (145).

  serve.py <path-of-com.canonical.dbusmenu.xml>

The menu is File with Open File... (Ctrl+O), Open Folder... (Ctrl+K Ctrl+O), Close Folder
(Ctrl+K F) and Save (no shortcut). It owns org.caminito.Check145 and serves at /MenuBar.
Calling org.caminito.Check145.Shortcut(i id, s json) on the same path sets an item's shortcut
and sends one ItemsPropertiesUpdated, as the extension does.
"""
import json
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

NAME = "org.caminito.Check145"
PATH = "/MenuBar"

CONTROL = """<node><interface name="org.caminito.Check145">
  <method name="Shortcut"><arg type="i" direction="in"/><arg type="s" direction="in"/></method>
</interface></node>"""

ITEMS = {
    1: {"label": "_File", "children-display": "submenu"},
    2: {"label": "Open _File...", "shortcut": [["Control", "o"]]},
    3: {"label": "Open _Folder...", "shortcut": [["Control", "k"], ["Control", "o"]]},
    4: {"label": "Close _Folder", "shortcut": [["Control", "k"], ["f"]]},
    5: {"label": "_Save"},
}
CHILDREN = {0: [1], 1: [2, 3, 4, 5]}
revision = 1


def props(item_id):
    out = {}
    for key, value in ITEMS.get(item_id, {}).items():
        if key == "shortcut":
            out[key] = GLib.Variant("aas", value)
        else:
            out[key] = GLib.Variant("s", value)
    return out


def layout(item_id, depth):
    children = []
    if depth != 0:
        children = [GLib.Variant("(ia{sv}av)", layout(child, depth - 1))
                    for child in CHILDREN.get(item_id, [])]
    return (item_id, props(item_id), children)


def on_call(conn, sender, path, interface, method, params, invocation):
    global revision
    args = params.unpack()
    if interface == "org.caminito.Check145":
        item_id, shortcut = args
        shortcut = json.loads(shortcut)
        if shortcut:
            ITEMS[item_id]["shortcut"] = shortcut
            updated = [(item_id, {"shortcut": GLib.Variant("aas", shortcut)})]
            removed = []
        else:
            ITEMS[item_id].pop("shortcut", None)
            updated = []
            removed = [(item_id, ["shortcut"])]
        conn.emit_signal(None, PATH, "com.canonical.dbusmenu", "ItemsPropertiesUpdated",
                         GLib.Variant("(a(ia{sv})a(ias))", (updated, removed)))
        invocation.return_value(None)
    elif method == "GetLayout":
        parent, depth, _names = args
        invocation.return_value(GLib.Variant("(u(ia{sv}av))", (revision, layout(parent, depth))))
    elif method == "GetGroupProperties":
        ids, _names = args
        invocation.return_value(GLib.Variant("(a(ia{sv}))", ([(i, props(i)) for i in ids],)))
    elif method == "GetProperty":
        item_id, name = args
        invocation.return_value(GLib.Variant("(v)", (props(item_id).get(name, GLib.Variant("s", "")),)))
    elif method == "AboutToShow":
        invocation.return_value(GLib.Variant("(b)", (False,)))
    elif method == "AboutToShowGroup":
        invocation.return_value(GLib.Variant("(aiai)", ([], [])))
    elif method == "EventGroup":
        invocation.return_value(GLib.Variant("(ai)", ([],)))
    else:
        invocation.return_value(None)


def on_get(conn, sender, path, interface, name):
    return {
        "Version": GLib.Variant("u", 3),
        "TextDirection": GLib.Variant("s", "ltr"),
        "Status": GLib.Variant("s", "normal"),
        "IconThemePath": GLib.Variant("as", []),
    }[name]


def main():
    with open(sys.argv[1]) as xml:
        menu = Gio.DBusNodeInfo.new_for_xml(xml.read()).interfaces[0]
    control = Gio.DBusNodeInfo.new_for_xml(CONTROL).interfaces[0]
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    bus.register_object(PATH, menu, on_call, on_get, None)
    bus.register_object(PATH, control, on_call, None, None)
    loop = GLib.MainLoop()
    Gio.bus_own_name_on_connection(bus, NAME, Gio.BusNameOwnerFlags.NONE,
                                   lambda *_: print("serving", flush=True), lambda *_: loop.quit())
    loop.run()


if __name__ == "__main__":
    main()
