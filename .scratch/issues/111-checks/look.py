"""Looks at an app's menubars through AT-SPI, or activates an item of one.

  look.py bars <app>                 each menubar's height and the names of its menus
  look.py activate <app> <menu> <item>
"""
import sys
import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def app(name):
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        child = desktop.get_child_at_index(index)
        if child is not None and child.get_name() == name:
            return child
    print(f"{name} isn't on the accessibility bus")
    sys.exit(1)


def bars(node, depth=0):
    """The menubars under a node. A menubar's own menus aren't looked into."""
    if node is None or depth > 14:
        return
    if node.get_role() == Atspi.Role.MENU_BAR:
        yield node
        return
    for index in range(min(node.get_child_count(), 40)):
        yield from bars(node.get_child_at_index(index), depth + 1)


def children(node):
    return [node.get_child_at_index(index) for index in range(node.get_child_count())]


def named(nodes, name):
    return next((node for node in nodes if node is not None and node.get_name() == name), None)


command, name = sys.argv[1], sys.argv[2]
found = list(bars(app(name)))
if command == "bars":
    for bar in found:
        height = bar.get_extents(Atspi.CoordType.WINDOW).height
        print(f"height {height}: {[menu.get_name() for menu in children(bar)]}")
    if not found:
        print("no menubar")
else:
    for bar in found:
        menu = named(children(bar), sys.argv[3])
        item = menu and named(children(menu), sys.argv[4])
        if item is not None:
            print(f"activated: {item.do_action(0)}")
            break
    else:
        print(f"no {sys.argv[3]} > {sys.argv[4]}")
