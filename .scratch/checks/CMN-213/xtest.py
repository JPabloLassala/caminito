"""Moves the pointer, clicks and scrolls on $DISPLAY through XTest, as xdotool would.

    xtest.py move <x> <y>
    xtest.py click <x> <y> <button>     1 left, 2 middle, 3 right, 4 wheel up, 5 wheel down
"""

import sys
import time

from Xlib import X, display
from Xlib.ext import xtest

d = display.Display()
x, y = int(sys.argv[2]), int(sys.argv[3])
xtest.fake_input(d, X.MotionNotify, x=x, y=y)
d.sync()
if sys.argv[1] == "click":
    button = int(sys.argv[4])
    time.sleep(0.2)
    xtest.fake_input(d, X.ButtonPress, button)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, button)
    d.sync()
