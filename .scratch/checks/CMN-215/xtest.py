"""Moves the pointer, clicks, scrolls and types on $DISPLAY through XTest, as xdotool would.

    xtest.py move <x> <y>
    xtest.py click <x> <y> <button> [shift]  1 left, 2 middle, 3 right, 4 wheel up, 5 wheel
                                             down; with Shift held when shift is given
    xtest.py key <keysym> [<n>]              a key n times: Tab, space, Right, Escape…
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
    for _ in range(int(sys.argv[3]) if len(sys.argv) > 3 else 1):
        tap(keycode)
    sys.exit(0)

x, y = int(sys.argv[2]), int(sys.argv[3])
xtest.fake_input(d, X.MotionNotify, x=x, y=y)
d.sync()
if sys.argv[1] == "click":
    button = int(sys.argv[4])
    shift = d.keysym_to_keycode(XK.XK_Shift_L)
    held = len(sys.argv) > 5 and sys.argv[5] == "shift"
    time.sleep(0.2)
    if held:
        xtest.fake_input(d, X.KeyPress, shift)
        d.sync()
        time.sleep(0.05)
    xtest.fake_input(d, X.ButtonPress, button)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, button)
    d.sync()
    if held:
        time.sleep(0.05)
        xtest.fake_input(d, X.KeyRelease, shift)
        d.sync()
