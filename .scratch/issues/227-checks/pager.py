"""Looks at mate-panel's workspace switcher through AT-SPI, or clicks one of its buttons.

  pager.py buttons          each button's label, with a star on the pressed one
  pager.py click <label>
"""
import sys
import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def panel():
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        child = desktop.get_child_at_index(index)
        if child is not None and child.get_name() == "mate-panel":
            return child
    print("mate-panel isn't on the accessibility bus")
    sys.exit(1)


def toggles(node, depth=0):
    """The toggle buttons under a node. The window list's buttons are toggles too, but none
    is there: the check opens no window."""
    if node is None or depth > 14:
        return
    if node.get_role() == Atspi.Role.TOGGLE_BUTTON:
        yield node
        return
    for index in range(min(node.get_child_count(), 40)):
        yield from toggles(node.get_child_at_index(index), depth + 1)


found = list(toggles(panel()))
if sys.argv[1] == "buttons":
    pressed = Atspi.StateType.CHECKED
    print(", ".join(b.get_name() + ("*" if b.get_state_set().contains(pressed) else "") for b in found))
else:
    button = next((b for b in found if b.get_name() == sys.argv[2]), None)
    print(f"clicked {sys.argv[2]}: {button.do_action(0)}" if button else f"no button {sys.argv[2]}")
