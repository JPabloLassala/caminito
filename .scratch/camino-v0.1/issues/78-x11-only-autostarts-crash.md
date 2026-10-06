# X11-only programs that mate-session autostarts crash in the session

Type: implementation
Status: resolved
Milestone: M5
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

- [x] The decision is recorded, in an ADR if it adds a carried patch.
- [x] A login as `caminotest` on a tty, on a machine with `mate-screensaver`, `mate-power-manager` and `mate-utils` installed, leaves no core dump (`coredumpctl list --since` the login).
- [x] `mate-screensaver` isn't running in the session, and `org.mate.ScreenSaver` is Camino's.

## Notes

- The stacks are in `coredumpctl info` for pids 452735, 452729 and 452756 on the developer's machine, while its journal keeps them.
- **M5, and it blocks [73](73-packaging-and-closing-m5.md):** the developer's decision on 2026-10-05.
- **Seen again the same day** in the developer's own login from sddm: the same three, with core dumps at 11:59:23 (pids 463867, 463861 and 463884).
- **The decision ([ADR 0014](../../../docs/adr/0014-mate-session-skips-or-moves-x11-only-autostarts-under-camino.md)):** two more Carried patches in `mate-session-manager-camino`, which is now `pkgrel` 2, both gated on `MATE_WM_NAME=Camino`:
  - `0003`: the autostart entries `mate-screensaver.desktop` and `mate-maximus-autostart.desktop` are treated as disabled;
  - `0004`: `mate-power-manager.desktop` is started with `GDK_BACKEND=x11`.
- **mate-maximus is mate-netbook's,** not mate-utils's, so the second criterion's machine needs `mate-netbook`. The developer's has all three.
- **Autostart overrides in `/usr/share/mate/autostart`** would have needed no patch: mate-session reads that directory before `/etc/xdg/autostart` (`gsm-util.c:247`), and the first entry with an id wins. The ADR says why it's the patch.
- **Checked on the developer's machine (2026-10-05), with no login:**
  - The package builds from the PKGBUILD (`makepkg -f`), and the four patches apply with no offset. `namcap PKGBUILD` prints nothing.
  - **The patches, with a mate-session built from the series,** on Xvfb and a private bus, with `--autostart` on a directory of four stub entries that write their environment to a file: the three ids and one other. With `MATE_WM_NAME=Camino`: the log has "is disabled under Camino" for the screensaver's and maximus's, neither runs, the power manager's has `GDK_BACKEND=x11`, and the other has no `GDK_BACKEND`. With `MATE_WM_NAME=Marco`: all four run, with the `GDK_BACKEND` mate-session was started with.
  - **mate-power-manager on XWayland under Camino:** in the developer's running session, `GDK_BACKEND=x11 mate-power-manager --verbose` ran for 8 s until it was ended, and left no core dump. What it does there wasn't looked at.
- **The second and third criteria are unticked, and the ticket stays open for them.** They need the new package installed and a login, which are the developer's: `makepkg -si` in `packaging/arch/mate-session-manager-camino`, a login as `caminotest`, then `coredumpctl list --since` the login, `pgrep mate-screensave` (15 characters: `pgrep mate-screensaver` matches nothing, running or not), and `busctl --user status org.mate.ScreenSaver` for the owner.
- **Seen in a login on Xvfb (2026-10-05), in the OrbStack machine,** with `mate-screensaver`, `mate-power-manager`, `mate-netbook` and `mate-utils` installed there: [`78-checks/login-check.sh`](78-checks/login-check.sh), which runs 72's `xvfb-login.sh` as `caminotest`. It isn't the login on a tty, so the two criteria stay unticked.
  - **With `pkgrel` 1:** the kernel logged a core dump for each of the three (`coredump: … (mate-maximus)`, `(mate-screensave)`, `(mate-power-mana)`).
  - **With `pkgrel` 2:** the kernel logged no core dump, of any program, from the start of the login to after the logout. 25 s after the start, mate-screensaver and mate-maximus aren't among `caminotest`'s processes and mate-power-manager is, with `GDK_BACKEND=x11` and `DISPLAY=:0` in its environment. `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` are Camino's pid.
  - **`coredumpctl` shows nothing in that machine,** crash or not: the kernel says "core_pattern handlers not supported in namespaces". The script reads the kernel's `coredump:` lines from the journal.
  - **mate-power-manager aborts there with no polkit rule,** as the runs before the script had one showed, with `pkgrel` 1 (on Wayland) and with `pkgrel` 2 (on X11): logind refuses `caminotest` the inhibitor locks it takes as it starts (`Error in dbus - … AccessDenied`, and `systemd-inhibit` as `caminotest` gets the same refusal). The script adds a polkit rule that allows them for the run, and removes it. The login on a tty of 76 had no such abort: there mate-power-manager got as far as its keys.
  - **72's `xvfb-login.sh` said "mate-screensaver running: no" always:** `pgrep -x mate-screensaver` matches nothing, the name being 16 characters. It asks for `mate-screensave` now. 72's own result stands: mate-screensaver wasn't installed in that machine then.
- **The login on a tty (2026-10-06), on the developer's machine,** as `caminotest` on tty4, with `camino --session` started at 15:36:43, on `camino-git r195.9e4baac-1` and `mate-session-manager-camino 1.28.0.r23.g57a9178-2`, with `mate-screensaver`, `mate-power-manager`, `mate-netbook` and `mate-utils` installed. Read four minutes in, from the developer's own session. Both criteria pass, and the ticket is resolved:
  - `coredumpctl list --since 15:34` says "No coredumps found", and the kernel's journal has no `coredump` or `segfault` line since the login.
  - mate-screensaver and mate-maximus aren't among `caminotest`'s processes. mate-power-manager is, with `GDK_BACKEND=x11` and `DISPLAY=:2` in its environment.
  - `busctl --user status org.mate.ScreenSaver`, as `caminotest`, gives Camino's pid (`camino --session`).
- **Not looked at still:** which other autostart entries of a full MATE install are X11-only, D-Bus activation of `org.mate.PowerManager`, which starts it on Wayland, and entries of a saved session, which the lists don't match (both in ADR 0014).
- **Review left as is:** the two patches each spell out the same test of an id against a list, and `is_under_camino` is the third spelling of the gate beside `main.c`'s two; each patch stays readable alone.
