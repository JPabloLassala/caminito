"""The Night Light tab's check (CMN-115, ticket 142), run by nested.sh inside its session:
caminito-displays on the nested Caminito, driven through AT-SPI as a screen reader would, and
read back through gsettings and Caminito's NightLight property. Prints one line per check,
"ok" or "FAIL", and exits 1 if any failed.

    python3 drive.py <caminito-displays>
"""

import os
import subprocess
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

BIN = sys.argv[1]
SCHEMA = "org.mate.Caminito"
APP = "org.mate.Caminito.Displays"
failed = False


def check(what, holds, detail=""):
    global failed
    failed |= not holds
    print(f"{'ok  ' if holds else 'FAIL'} {what}" + (f" ({detail})" if detail else ""), flush=True)


def key(name):
    return subprocess.run(["gsettings", "get", SCHEMA, name], capture_output=True, text=True).stdout.strip()


def wait(condition, timeout=10.0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = condition()
        if value:
            return value
        time.sleep(0.1)
    return condition()


def owner():
    """The application's name's owner on the session bus, if it has one."""
    out = subprocess.run(
        ["busctl", "--user", "status", APP], capture_output=True, text=True
    )
    return out.returncode == 0


def apps():
    desktop = Atspi.get_desktop(0)
    return [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]


def displays_app():
    for app in apps():
        if app is not None and app.get_name() == "caminito-displays":
            return app
    return None


def walk(node):
    yield node
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child is not None:
            yield from walk(child)


def find(role, name):
    app = displays_app()
    if app is None:
        return None
    for node in walk(app):
        if node.get_role_name() == role and node.get_name() == name:
            return node
    return None


def frames():
    app = displays_app()
    if app is None:
        return 0
    return sum(1 for i in range(app.get_child_count()) if app.get_child_at_index(i).get_role_name() == "frame")


def has(node, state):
    return node.get_state_set().contains(state)


def describe(node):
    actions = [Atspi.Action.get_action_name(node, k) for k in range(Atspi.Action.get_n_actions(node))]
    return f"{node.get_role_name()} {node.get_name()!r} actions {actions} {node.get_interfaces()}"


def act(node):
    try:
        Atspi.Action.do_action(node, 0)
    except gi.repository.GLib.Error:
        print("info", describe(node), "under", describe(node.get_parent()))
        raise


def selected_tab():
    for name in ["Displays", "Night Light"]:
        tab = find("page tab", name)
        if tab is not None and has(tab, Atspi.StateType.SELECTED):
            return name
    return None


def run(*arguments):
    started = time.monotonic()
    done = subprocess.run([BIN, *arguments], capture_output=True, text=True, timeout=20)
    return done, time.monotonic() - started


# The window, opened as the Displays entry opens it.
window = subprocess.Popen([BIN], stderr=open("displays.log", "w"))
check("the window opens", wait(lambda: frames() == 1, 20), f"{frames()} frames")
check("it opens on Displays", wait(lambda: selected_tab() == "Displays"), str(selected_tab()))

# --night-light from a second start: the running window switches, and the start exits.
done, took = run("--night-light")
check("--night-light from a second start exits", done.returncode == 0, f"{took:.1f} s, {done.returncode}")
check("--night-light switches the running window to Night Light",
      wait(lambda: selected_tab() == "Night Light"), str(selected_tab()))
check("the window is still the only one", frames() == 1, f"{frames()} frames")

# Each control writes its key.
switch = find("switch", "Night Light")
check("the tab has its switch", switch is not None)
before = key("night-light-enabled")
act(switch)
check("the switch writes night-light-enabled", wait(lambda: key("night-light-enabled") == "true"), f"{before} -> {key('night-light-enabled')}")

constant = find("toggle button", "Constant")
check("the tab has Constant", constant is not None)
act(constant)
check("Constant writes night-light-schedule", wait(lambda: key("night-light-schedule") == "'constant'"), key("night-light-schedule"))

slider = wait(lambda: find("slider", "Temperature"))
check("the constant schedule shows its slider", slider is not None)
noon = find("slider", "At noon")
check("and not the sun's", noon is None or not has(noon, Atspi.StateType.SHOWING))
Atspi.Value.set_current_value(slider, 3000.0)
check("the slider writes night-light-constant-temperature",
      wait(lambda: key("night-light-constant-temperature") == "uint32 3000"),
      key("night-light-constant-temperature"))

# The tab follows a key changed from elsewhere.
subprocess.run(["gsettings", "set", SCHEMA, "night-light-schedule", "sun"])
check("a schedule changed elsewhere shows the sun's sliders",
      wait(lambda: (s := find("slider", "At noon")) is not None and has(s, Atspi.StateType.SHOWING)))

# --toggle-night-light while the window is open: the key flips, the switch follows, and no
# window opens.
done, took = run("--toggle-night-light")
check("--toggle-night-light exits at once", done.returncode == 0 and took < 5, f"{took:.1f} s, {done.returncode}, {done.stderr.strip()}")
check("--toggle-night-light flips night-light-enabled", wait(lambda: key("night-light-enabled") == "false"), key("night-light-enabled"))
check("the switch follows", wait(lambda: not has(find("switch", "Night Light"), Atspi.StateType.CHECKED)))
check("still one window", frames() == 1, f"{frames()} frames")

# The location note against Caminito's property.
out = subprocess.run(
    ["busctl", "--user", "get-property", "org.mate.Caminito", "/org/mate/Caminito", "org.mate.Caminito", "NightLight"],
    capture_output=True, text=True,
).stdout
source = "none" if '"location-source" s "none"' in out else "other"
note = find("label", "Location unknown, using fixed times")
shown = note is not None and has(note, Atspi.StateType.SHOWING)
check(f"the note follows the location source ({source})", shown == (source == "none"), out.strip())

# The pause button, with Night Light on.
subprocess.run(["gsettings", "set", SCHEMA, "night-light-enabled", "true"])
pause = wait(lambda: find("button", "Pause until sunrise"))
check("the pause button reads Pause until sunrise", pause is not None)
if pause is not None:
    act(pause)
    paused = wait(lambda: key("night-light-paused-until") not in ("int64 0", ""), 3)
    message = [n.get_name() for n in walk(displays_app()) if n.get_role_name() == "label" and "paused" in n.get_name().lower()]
    print(f"info Pause: night-light-paused-until is {key('night-light-paused-until')}; labels: {message}")
    if paused:
        check("Pause: the button reads Resume", wait(lambda: find("button", "Resume")) is not None)

if os.environ.get("SHOT"):
    subprocess.run(["import", "-window", "root", os.environ["SHOT"]], env={**os.environ, "DISPLAY": ":142"})

# Closing, then --toggle-night-light with no window: the key flips and no window opens.
window.terminate()
window.wait(10)
check("the window has closed", wait(lambda: displays_app() is None and not owner()))
before = key("night-light-enabled")
done, took = run("--toggle-night-light")
check("--toggle-night-light with no window flips the key",
      wait(lambda: key("night-light-enabled") != before), f"{before} -> {key('night-light-enabled')}")
time.sleep(2)  # Long enough for a window to have opened, which is what's checked not to.
check("and opens no window, and leaves no process", displays_app() is None and not owner(), f"exit {done.returncode}")

sys.exit(1 if failed else 0)
