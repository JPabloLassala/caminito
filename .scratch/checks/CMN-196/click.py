"""Clicks the first button at x, y on $DISPLAY through XTest, as xdotool would.

    click.py <x> <y>
"""

import sys
import time

from Xlib import X, display
from Xlib.ext import xtest

d = display.Display()
x, y = int(sys.argv[1]), int(sys.argv[2])
xtest.fake_input(d, X.MotionNotify, x=x, y=y)
d.sync()
time.sleep(0.2)
xtest.fake_input(d, X.ButtonPress, 1)
d.sync()
time.sleep(0.05)
xtest.fake_input(d, X.ButtonRelease, 1)
d.sync()
