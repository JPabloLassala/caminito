"""Moves the pointer, clicks and types on $DISPLAY through XTest, as xdotool would.

    xtest.py move <x> <y>
    xtest.py click <x> <y> <button>          1 left, 2 middle, 3 right
    xtest.py key <keysym> [<n>] [shift]      a key n times, with Shift held when shift is given
"""

import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

d = display.Display()


def tap(keycode):
    xtest.fake_input(d, X.KeyPress, keycode)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.KeyRelease, keycode)
    d.sync()
    time.sleep(0.15)


if sys.argv[1] == "key":
    keycode = d.keysym_to_keycode(XK.string_to_keysym(sys.argv[2]))
    shift = d.keysym_to_keycode(XK.XK_Shift_L)
    held = len(sys.argv) > 4 and sys.argv[4] == "shift"
    if held:
        xtest.fake_input(d, X.KeyPress, shift)
        d.sync()
    for _ in range(int(sys.argv[3]) if len(sys.argv) > 3 else 1):
        tap(keycode)
    if held:
        xtest.fake_input(d, X.KeyRelease, shift)
        d.sync()
    sys.exit(0)

x, y = int(sys.argv[2]), int(sys.argv[3])
xtest.fake_input(d, X.MotionNotify, x=x, y=y)
d.sync()
if sys.argv[1] == "click":
    time.sleep(0.2)
    xtest.fake_input(d, X.ButtonPress, int(sys.argv[4]))
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, int(sys.argv[4]))
    d.sync()
