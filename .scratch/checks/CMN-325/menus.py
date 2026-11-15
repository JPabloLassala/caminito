#!/usr/bin/env python3
"""CMN-325: resolves the Control Center's, the Preferences' and the Applications' menus with
libmate-menu, as MATE does, and prints each entry's place. Run by check.sh, or with the session's
XDG_DATA_DIRS and XDG_CONFIG_DIRS, or without them for X11 MATE's:

    XDG_DATA_DIRS=/usr/share/caminito/xdg:/usr/local/share:/usr/share \
    XDG_CONFIG_DIRS=/usr/share/caminito/xdg-config:/etc/xdg python3 menus.py
"""
import sys
import gi

gi.require_version("MateMenu", "2.0")
from gi.repository import MateMenu

WATCH = {
    "mate-window-properties.desktop",
    "mate-at-properties.desktop",
    "mate-user-share-properties.desktop",
    "mate-notification-properties.desktop",
    "mate-screensaver-preferences.desktop",
}


def walk(directory, path, out):
    it = directory.iter()
    while True:
        kind = it.next()
        if kind == MateMenu.TreeItemType.INVALID:
            return
        if kind == MateMenu.TreeItemType.DIRECTORY:
            sub = it.get_directory()
            walk(sub, path + [sub.get_name()], out)
        elif kind == MateMenu.TreeItemType.ENTRY:
            entry = it.get_entry()
            out.append((" / ".join(path), entry.get_desktop_file_id()))


for menu in sys.argv[1:] or ["matecc.menu", "mate-settings.menu", "mate-applications.menu"]:
    tree = MateMenu.Tree.new(menu, MateMenu.TreeFlags.NONE)
    tree.load_sync()
    out = []
    walk(tree.get_root_directory(), [], out)
    print(f"== {menu}")
    categories = sorted({p for p, _ in out})
    print("   categories:", categories)
    for place, entry in out:
        if entry in WATCH:
            print(f"   {entry}: {place or '(top)'}")
