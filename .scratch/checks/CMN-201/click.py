"""Clicks a button at x, y on $DISPLAY through XTest, as xdotool would: the first, or 5 to
scroll down, as many times as asked.

    click.py <x> <y> [<button> [<times>]]
"""

import sys
import time

from Xlib import X, display
from Xlib.ext import xtest

d = display.Display()
x, y = int(sys.argv[1]), int(sys.argv[2])
button = int(sys.argv[3]) if len(sys.argv) > 3 else 1
times = int(sys.argv[4]) if len(sys.argv) > 4 else 1
xtest.fake_input(d, X.MotionNotify, x=x, y=y)
d.sync()
time.sleep(0.2)
for _ in range(times):
    xtest.fake_input(d, X.ButtonPress, button)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, button)
    d.sync()
