"""Looks at ticket 119's nested session: the menu the extension serves, and the panel's.

  look.py served                      the served menu's toggles, as JSON by label path
  look.py revision                    the served layout's revision
  look.py labels [<menu> ...]         the served labels under a path
  look.py panel [<menu> ...]          the panel's items under a path, with role and states
  look.py click <menu> ...            clicks the panel's item at a path, which opens a menu
  look.py showing                     the panel's menus that are on screen
"""
import json
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, Gio, GLib  # noqa: E402

MENU_PATH = "/org/mate/Caminito/GlobalMenuExtension/MenuBar"


def plain(label):
    """A label without its mnemonic's mark, dbusmenu's `_` or VS Code's `&&`."""
    return label.replace("&&", "").replace("__", "\0").replace("_", "").replace("\0", "_")


def servers(bus):
    """The connections whose claims Caminito holds, each with the layout it serves."""
    dump = bus.call_sync("org.mate.Caminito.Debug", "/org/mate/Caminito/Debug",
                         "org.mate.Caminito.Debug", "DumpState", None, None, 0, 2000, None)
    for claim in json.loads(dump.unpack()[0])["menu_claims"]:
        reply = bus.call_sync(claim["caller"], claim["path"], "com.canonical.dbusmenu", "GetLayout",
                              GLib.Variant("(iias)", (0, -1, [])), None, 0, 2000, None)
        yield claim["caller"], reply.unpack()


def labels(node, path):
    """The labels under the served item at a path, a separator as an empty one."""
    _id, props, children = node
    if not path:
        return [plain(child[1].get("label", "")) for child in children]
    for child in children:
        if plain(child[1].get("label", "")) == path[0]:
            return labels(child, path[1:])
    return None


def toggles(node, path, into):
    _id, props, children = node
    here = path + [plain(props["label"])] if "label" in props else path
    if "toggle-type" in props:
        into[" > ".join(here)] = [props["toggle-type"], props.get("toggle-state")]
    for child in children:
        toggles(child, here, into)
    return into


def panel():
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        child = desktop.get_child_at_index(index)
        if child is not None and child.get_name() == "mate-panel":
            return child
    sys.exit("mate-panel isn't on the accessibility bus")


def bars(node, depth=0):
    if node is None or depth > 14:
        return
    if node.get_role() == Atspi.Role.MENU_BAR:
        yield node
        return
    for index in range(min(node.get_child_count(), 40)):
        yield from bars(node.get_child_at_index(index), depth + 1)


def children(node):
    return [c for c in (node.get_child_at_index(i) for i in range(node.get_child_count())) if c]


def states(node):
    held = node.get_state_set()
    names = {"checked": Atspi.StateType.CHECKED, "showing": Atspi.StateType.SHOWING,
             "selected": Atspi.StateType.SELECTED}
    return [name for name, state in names.items() if held.contains(state)]


def at(path):
    """The panel's item at a path of labels, in the menubar that has it."""
    for bar in bars(panel()):
        node = bar
        for label in path:
            node = next((c for c in children(node) if plain(c.get_name()) == label), None)
            if node is None:
                break
        if node is not None:
            return node
    sys.exit(f"the panel has no {' > '.join(path)}")


def describe(node):
    return {"name": node.get_name(), "role": node.get_role_name(), "states": states(node)}


def showing(node, path, into):
    for child in children(node):
        here = path + [plain(child.get_name())]
        if child.get_role() == Atspi.Role.MENU and child.get_child_count() > 0:
            first = children(child)
            if first and Atspi.StateType.SHOWING in [s for c in first[:3] for s in
                                                     [Atspi.StateType.SHOWING] if
                                                     c.get_state_set().contains(s)]:
                into.append(" > ".join(here))
            showing(child, here, into)
    return into


command, args = sys.argv[1], sys.argv[2:]
if command in ("served", "revision", "labels"):
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    found = {name: layout for name, layout in servers(bus)}
    if command == "labels":
        print(json.dumps({name: labels(layout[1], args) for name, layout in found.items()}))
    elif command == "revision":
        print(json.dumps({name: layout[0] for name, layout in found.items()}))
    else:
        print(json.dumps({name: toggles(layout[1], [], {}) for name, layout in found.items()}))
elif command == "panel":
    nodes = [at(args)] if args else list(bars(panel()))
    print(json.dumps([[describe(child) for child in children(node)] for node in nodes]))
elif command == "click":
    node = at(args)
    print(json.dumps({"actions": [node.get_action_name(i) for i in range(node.get_n_actions())],
                      "done": node.do_action(0)}))
elif command == "showing":
    print(json.dumps([path for bar in bars(panel()) for path in showing(bar, [], [])]))
