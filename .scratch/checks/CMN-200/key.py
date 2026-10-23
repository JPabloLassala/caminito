"""Types on $DISPLAY through XTest, as xdotool would: text, or a key by its keysym name.

    key.py type <text>
    key.py key <keysym name>, as Return or BackSpace
"""

import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

d = display.Display()


def press(keysym):
    code = d.keysym_to_keycode(keysym)
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    time.sleep(0.03)
    xtest.fake_input(d, X.KeyRelease, code)
    d.sync()
    time.sleep(0.03)


if sys.argv[1] == "type":
    for char in sys.argv[2]:
        press(XK.string_to_keysym(char))
else:
    press(XK.string_to_keysym(sys.argv[2]))
