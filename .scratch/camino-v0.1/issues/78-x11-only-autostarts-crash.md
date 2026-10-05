# X11-only programs that mate-session autostarts crash in the session

Type: implementation
Status: open
Milestone: M5 (proposed)
Blocked by: 69
Map: [Camino v0.1](../map.md)

## What to build

In the first login on a tty ([76](76-tty-login-left-over.md), 2026-10-05, on the developer's machine), three programs that mate-session autostarts died with SIGSEGV as they started, each leaving a core dump: `mate-screensaver`, `mate-power-manager` and `mate-maximus`. mate-session unsets `GDK_BACKEND` under Camino ([69](69-mate-session-manager-camino.md)), so what it starts opens GDK on Wayland, and these three call Xlib with no X display:

- `mate-maximus`: `XTestQueryExtension`, from libfakekey's `fakekey_init`.
- `mate-power-manager`: `XKeysymToKeycode`, as it grabs its keys.
- `mate-screensaver`: in libmate-desktop, from a `g_initable_new`, before it asks for its name.

72's run on Xvfb didn't see it: none of the three was installed in that machine. `camino-git` depends on none of them, but a machine with MATE has them.

- **mate-screensaver matters most.** [62](62-screensaver-names.md) has it find `org.mate.ScreenSaver` taken and exit. It never gets that far: it crashes. The outcome is the same, it isn't running, but with a core dump at every login.
- **What to decide:** how the session keeps X11-only autostarts from starting on Wayland. Some ways: mate-session under Camino skips the autostart entries Camino stands in for (`mate-screensaver.desktop`), as the carried patch already skips the window manager; or it starts the entries known to be X11-only with `GDK_BACKEND=x11`; or Camino's package ships autostart overrides. mate-power-manager's keys and idle dimming have no stand-in yet, so on XWayland it would at least run.
- **Not looked at:** whether each of the three works on XWayland under Camino once it starts there, and which other autostart entries of a full MATE install are X11-only.

## Acceptance criteria

- [ ] The decision is recorded, in an ADR if it adds a carried patch.
- [ ] A login as `caminotest` on a tty, on a machine with `mate-screensaver`, `mate-power-manager` and `mate-utils` installed, leaves no core dump (`coredumpctl list --since` the login).
- [ ] `mate-screensaver` isn't running in the session, and `org.mate.ScreenSaver` is Camino's.

## Notes

- The stacks are in `coredumpctl info` for pids 452735, 452729 and 452756 on the developer's machine, while its journal keeps them.
- The milestone needs the developer: the session logs in and works without the three, so M5's exit doesn't ask for this, and it doesn't block [73](73-packaging-and-closing-m5.md) as filed.
- **Seen again the same day** in the developer's own login from sddm: the same three, with core dumps at 11:59:23 (pids 463867, 463861 and 463884).
