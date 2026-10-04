#!/usr/bin/env python3
"""Virtual input devices for ticket 56 checks. Reads commands from a FIFO."""
import os, sys, time
from evdev import UInput, AbsInfo, ecodes as e

FIFO = "/tmp/t56.fifo"
devs = {}

def mk(kind, name):
    if kind == "mouse":
        caps = {e.EV_KEY: [e.BTN_LEFT, e.BTN_RIGHT, e.BTN_MIDDLE],
                e.EV_REL: [e.REL_X, e.REL_Y, e.REL_WHEEL, e.REL_HWHEEL]}
        devs[name] = UInput(caps, name=name, vendor=0x1234, product=0x0001)
    elif kind == "kbd":
        caps = {e.EV_KEY: list(range(1, 128))}
        devs[name] = UInput(caps, name=name, vendor=0x1234, product=0x0002)
    elif kind in ("touchpad", "padbuttons"):
        ax = AbsInfo(0, 0, 4000, 0, 0, 40)
        ay = AbsInfo(0, 0, 2400, 0, 0, 40)
        caps = {e.EV_KEY: [e.BTN_LEFT] + ([e.BTN_RIGHT] if kind == "padbuttons" else []) + [e.BTN_TOUCH, e.BTN_TOOL_FINGER,
                           e.BTN_TOOL_DOUBLETAP, e.BTN_TOOL_TRIPLETAP],
                e.EV_ABS: [(e.ABS_X, ax), (e.ABS_Y, ay),
                           (e.ABS_MT_SLOT, AbsInfo(0, 0, 4, 0, 0, 0)),
                           (e.ABS_MT_TRACKING_ID, AbsInfo(0, 0, 65535, 0, 0, 0)),
                           (e.ABS_MT_POSITION_X, ax), (e.ABS_MT_POSITION_Y, ay)]}
        devs[name] = UInput(caps, name=name, vendor=0x1234, product=0x0003,
                            input_props=[e.INPUT_PROP_POINTER] + ([] if kind == "padbuttons" else [e.INPUT_PROP_BUTTONPAD]))
    time.sleep(1.5)

tid = [100]
def fingers_down(u, pts):
    for slot, (x, y) in enumerate(pts):
        tid[0] += 1
        u.write(e.EV_ABS, e.ABS_MT_SLOT, slot)
        u.write(e.EV_ABS, e.ABS_MT_TRACKING_ID, tid[0])
        u.write(e.EV_ABS, e.ABS_MT_POSITION_X, x)
        u.write(e.EV_ABS, e.ABS_MT_POSITION_Y, y)
    u.write(e.EV_ABS, e.ABS_X, pts[0][0]); u.write(e.EV_ABS, e.ABS_Y, pts[0][1])
    u.write(e.EV_KEY, e.BTN_TOUCH, 1)
    u.write(e.EV_KEY, e.BTN_TOOL_FINGER if len(pts) == 1 else e.BTN_TOOL_DOUBLETAP, 1)
    u.syn()

def fingers_move(u, pts):
    for slot, (x, y) in enumerate(pts):
        u.write(e.EV_ABS, e.ABS_MT_SLOT, slot)
        u.write(e.EV_ABS, e.ABS_MT_POSITION_X, x)
        u.write(e.EV_ABS, e.ABS_MT_POSITION_Y, y)
    u.write(e.EV_ABS, e.ABS_X, pts[0][0]); u.write(e.EV_ABS, e.ABS_Y, pts[0][1])
    u.syn()

def fingers_up(u, n):
    for slot in range(n):
        u.write(e.EV_ABS, e.ABS_MT_SLOT, slot)
        u.write(e.EV_ABS, e.ABS_MT_TRACKING_ID, -1)
    u.write(e.EV_KEY, e.BTN_TOUCH, 0)
    u.write(e.EV_KEY, e.BTN_TOOL_FINGER if n == 1 else e.BTN_TOOL_DOUBLETAP, 0)
    u.syn()

def run(cmd, name, *a):
    if cmd == "mk":
        return mk(name, a[0])
    u = devs[name]
    if cmd == "rm":
        u.close(); del devs[name]; time.sleep(1)
    elif cmd == "rel":
        n = int(a[2]) if len(a) > 2 else 1
        for _ in range(n):
            u.write(e.EV_REL, e.REL_X, int(a[0])); u.write(e.EV_REL, e.REL_Y, int(a[1])); u.syn()
            time.sleep(0.008)
    elif cmd == "btn":
        c = getattr(e, a[0])
        u.write(e.EV_KEY, c, 1); u.syn(); time.sleep(0.05)
        u.write(e.EV_KEY, c, 0); u.syn()
    elif cmd == "hold":  # press without release, to hold a button across a VT switch
        u.write(e.EV_KEY, getattr(e, a[0]), 1); u.syn()
    elif cmd == "wheel":
        u.write(e.EV_REL, e.REL_WHEEL, int(a[0])); u.syn()
    elif cmd == "hwheel":
        u.write(e.EV_REL, e.REL_HWHEEL, int(a[0])); u.syn()
    elif cmd == "key":
        mods = [getattr(e, k) for k in a[:-1]]
        c = getattr(e, a[-1])
        for m in mods: u.write(e.EV_KEY, m, 1); u.syn(); time.sleep(0.02)
        u.write(e.EV_KEY, c, 1); u.syn(); time.sleep(0.03)
        u.write(e.EV_KEY, c, 0); u.syn()
        for m in mods: u.write(e.EV_KEY, m, 0); u.syn()
    elif cmd == "chord":
        u.write(e.EV_KEY, e.BTN_LEFT, 1); u.write(e.EV_KEY, e.BTN_RIGHT, 1); u.syn(); time.sleep(0.2)
        u.write(e.EV_KEY, e.BTN_LEFT, 0); u.write(e.EV_KEY, e.BTN_RIGHT, 0); u.syn()
    elif cmd == "click":  # n fingers resting high on the pad, then the pad pressed
        n = int(a[0]); pts = [(1500 + 600 * i, 600) for i in range(n)]
        fingers_down(u, pts); time.sleep(0.25)
        u.write(e.EV_KEY, e.BTN_LEFT, 1); u.syn(); time.sleep(0.08)
        u.write(e.EV_KEY, e.BTN_LEFT, 0); u.syn(); time.sleep(0.05)
        fingers_up(u, n)
    elif cmd == "tap":
        fingers_down(u, [(2000, 1200)]); time.sleep(0.04); fingers_up(u, 1)
    elif cmd == "swipe":  # one finger, dx dy in device units
        x, y = 1500, 1000
        fingers_down(u, [(x, y)]); time.sleep(0.02)
        for i in range(1, 21):
            fingers_move(u, [(x + int(a[0]) * i // 20, y + int(a[1]) * i // 20)]); time.sleep(0.012)
        fingers_up(u, 1)
    elif cmd == "scroll2":  # two fingers, dy in device units
        p = [(1500, 1000), (2100, 1000)]
        fingers_down(u, p); time.sleep(0.02)
        for i in range(1, 21):
            dx = int(a[0]) * i // 20; d = int(a[1]) * i // 20
            fingers_move(u, [(p[0][0] + dx, p[0][1] + d), (p[1][0] + dx, p[1][1] + d)]); time.sleep(0.012)
        fingers_up(u, 2)
    time.sleep(0.3)

if os.path.exists(FIFO): os.remove(FIFO)
os.mkfifo(FIFO)
while True:
    with open(FIFO) as f:
        for line in f:
            p = line.split()
            if not p: continue
            if p[0] == "quit": sys.exit(0)
            try:
                run(*p)
                print("ok", line.strip(), flush=True)
            except Exception as ex:
                print("ERR", line.strip(), repr(ex), flush=True)
