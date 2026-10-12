# 14. mate-session skips X11-only autostart entries under Caminito, or starts them on XWayland

Date: 2026-10-05
Status: Accepted. Qualified on 2026-10-08 by ticket 107: mate-power-manager's keys are grabbed through Caminito, by a carried patch (ADR 0011), so they work whatever has the focus. Qualified on 2026-10-12 by ticket 206: an audit of Ubuntu 26.04's `ubuntu-mate-desktop` adds `mate-hud`, `plank`, `dock-replace`, `picom` and `magnus-autostart` to the skipped list and `tilda` to the XWayland one. `plank` is the `dock` required component, which mate-session starts as an autostart entry of that id.

## Context

mate-session unsets `GDK_BACKEND` under Caminito (ADR 0004), so what it autostarts opens GDK on Wayland. Three programs of a full MATE install call Xlib without checking for an X display, and died with SIGSEGV at every login (76, 78):

- `mate-screensaver`, in libmate-desktop's `XInternAtom`, from a `g_initable_new`, before it asks for `org.mate.ScreenSaver`;
- `mate-maximus` (mate-netbook), as it sets up libfakekey;
- `mate-power-manager`, in `XKeysymToKeycode`, as it grabs its keys.

`caminito-git` depends on none of them. Their autostart entries are `/etc/xdg/autostart/*.desktop` files that their own packages own.

mate-session reads the autostart directories in order, and the first entry with an id wins: the user's `~/.config/autostart`, then `mate/autostart` under each of `XDG_DATA_DIRS`, then `autostart` under each of `XDG_CONFIG_DIRS`. Its autostart conditions (`if-exists`, `unless-exists`, `MATE`, `GSettings`) can't ask which compositor runs.

## Decision

Two carried patches in `mate-session-manager-caminito`, both active only when `MATE_WM_NAME=Caminito` (ADR 0011). They name the entries by their id, the file's name, in two lists in `gsm-autostart-app.c`:

- **Skipped:** `mate-screensaver.desktop` and `mate-maximus-autostart.desktop` are treated as disabled, as an entry with `Hidden=true` is.
  - Caminito owns `org.mate.ScreenSaver` before mate-session starts (62), so mate-screensaver would exit anyway.
  - mate-maximus takes the decorations off maximised windows through libwnck. Under Caminito that's the compositor's decision, and it would reach the XWayland windows only.
- **Started on XWayland:** `mate-power-manager.desktop` gets `GDK_BACKEND=x11` in its environment. Caminito has no stand-in for its keys and idle dimming (78), nor for its battery and lid policy, its notifications or its tray icon.

An entry goes in the first list when Caminito does its work or the work is the compositor's, and in the second when it's X11-only and nothing stands in for it.

## Consequences

- The lists are by id, so they hold whichever directory an entry is read from, a user's copy in `~/.config/autostart` included. Startup Applications writes such a copy when an entry is switched off and on again.
- A new X11-only autostart entry needs a change to the patch and a new `pkgrel`. The lists cover what crashed on the developer's machine; the other entries of a full MATE install weren't audited.
- Only the autostart path is covered. `org.mate.PowerManager` is also D-Bus activatable, and an activation starts mate-power-manager on Wayland, where it crashes. That needs a caller of the name while it isn't running: after the user has ended it, or before its autostart entry has the name. The second wasn't seen, or looked for.
- A session saved by mate-session isn't covered either: a saved entry's file is named after its startup id, so it matches neither list. That needs session saving on and the program saved as a client, which wasn't checked.
- mate-power-manager on XWayland: its key grabs see the keys only while an XWayland window has the focus (107 grabs them through Caminito instead), and its idle time is XWayland's, which is inaccurate (ADR 0004). Its idle dimming and sleep wait for M7's idle.
- Rejected:
  - Autostart overrides shipped by `caminito-git` in `/usr/share/mate/autostart`, which mate-session reads before `/etc/xdg/autostart`. No patch, but the override is read in X11 MATE too, so each `Exec` would be a shell test of `MATE_WM_NAME`; it copies the stock entry's keys; and a user's copy in `~/.config/autostart` wins over it.
  - `GDK_BACKEND=x11` for all three. mate-screensaver would find its name taken and exit at every login, and mate-maximus would act on the XWayland windows alone.
  - Patches to the three programs. That's three more patched packages, for programs Caminito doesn't depend on.
