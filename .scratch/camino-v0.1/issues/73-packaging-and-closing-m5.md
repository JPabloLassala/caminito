# Packaging and closing M5

Type: implementation
Status: open
Milestone: M5
Blocked by: 72, 74, 76, 77, 78, 79
Map: [Camino v0.1](../map.md)

## What to build

M5's checklist items and the closing commit ([17](17-milestone-slicing.md)), on the installed packages.

- **Packaging:** `namcap` on `camino-git` reports nothing unexplained with M5's code, and the four packages are installed on the developer's machine with `makepkg -si`.
- **Checklist items tagged `M5`** in `docs/testing/checklist.md`, in a new *Session* section, each with steps, the expected result and the debug tool:
  - logging into *MATE on Camino* from the display manager, and as `caminotest` on a tty;
  - logout and Shut Down from the panel: Log Out ends the session and Camino exits with mate-session's status; Shut Down shows mate-session's dialog and powers off;
  - X11 clipboard contents survive the app quitting (11), through msd's data-control clipboard manager;
  - X11 MATE and the Wayfire session unchanged on the developer's own user, with the Patched packages installed (the fall-back rule, ADR 0011);
  - msd applying the saved display layout at login.

  The existing *`DISPLAY` only exported after the root properties exist* item is already tagged `M5` (43) and is run now for the first time.
- **A closing wizard,** `73-closing-checks.sh`, as 43's and 59's: it walks through M5's exit and the new items, saves each result as it's given, and resumes a stopped run.
- **Closing:** M5's exit from 17, with the results of every `M1` to `M5` checklist item that still applies recorded in a `Close M5 MATE session` commit. The map's M5 line becomes *closed*, and the README's milestone table and `--session` note are brought up to date.

## Acceptance criteria

- [ ] The Packaging job and CI are green on the closing commit's parent.
- [x] The five checklist items are written.
- [ ] Closing M5:
  - from the display manager, and as `caminotest` on a tty, "MATE on Camino" starts mate-session; mate-panel shows the window list and the workspace switcher; caja draws the desktop icons; msd sets the wallpaper and applies the saved display layout over wlr-output-management;
  - logging out through the panel ends the session, and Camino exits with mate-session's status;
  - X11 MATE on the same machine still works with the Patched packages installed;
  - the checklist results are in a `Close M5 MATE session` commit.

## Notes

- **The nested and tty items of M1 to M4** still apply. 43's, 54's and 59's wizards take the ticket to write to (`TICKET=…`).
- **[60](60-tty-checks-left-over.md)'s hot-plug** gates no milestone. It was run on 2026-10-06, and 60 is resolved.
- **The session can't lock until M7** ([62](62-screensaver-names.md)), so the developer doesn't use it daily yet (17).
- **Handed on by [70](70-mate-panel-camino.md):** `mate-panel-camino` has master's `libmate-panel-applet-4.so.2`, and Arch's `mate-applets`, `mate-sensors-applet` and `mate-notification-daemon`'s applet are linked to `.so.1`, so they don't start under the patched panel, on X11 too. That is against "X11 MATE on the same machine still works" for anyone who uses those applets. Decide here: build those packages again against the patched panel, or keep the stock library beside the new one.
- **Not closed (2026-10-05).** The checklist items, the wizard and `namcap` are done. The rest waits: [76](76-tty-login-left-over.md) and [77](77-session-test-times-out-in-ci.md) are open, the Patched packages aren't installed on the developer's machine, and every check of the closing needs a session and the developer. This was an agent's session, which didn't install packages (installing takes the stock MATE packages out) and can't sit at a tty or a greeter. The map's M5 line, the README's status and milestone table, and its `--session` note are the closing commit's, and unchanged.
- **CI on the closing commit's parent:** not green yet. The last push is `6c21223`, where the Packaging job passed and CI failed with 77's test. `master` is nine commits ahead of it, with 74's, 75's and 76's.
- **`namcap`, on `camino-git r162.678d85a-1`,** built here from `678d85a` with `makepkg -f -d --nocheck` (x86_64; `-d` since the Patched packages aren't installed): `namcap PKGBUILD` prints nothing. On the package, only warnings, each explained in the PKGBUILD's comment: ld-linux, and `librsvg`, `marco`, `mate-themes`, `xorg-xwayland` and `libglvnd` as in 59, and the six of the session that 72 added, which camino starts and doesn't link (`mate-session-manager-camino`, `mate-panel-camino`, `mate-settings-daemon-camino`, `caja`, `mate-notification-daemon`, `mate-polkit`). `readelf -d` on the packaged `camino` has the same libraries as in 59. 74 and 75 added no dependency. `makepkg -si` for the four is [76-checks/prepare.sh](76-checks/prepare.sh)'s, and not done.
- **The checklist's *Session* section** has the five items, before *Privileged clients*, and a paragraph at the top on the items in the session. Beyond the ticket's list:
  - The saved layout is `~/.config/monitors-wayland.xml`. Stock mate-control-center may have no Wayland Display capplet (the patched one is M6's), so the item also gives the file to write by hand, in the form msd saves (71).
  - The clipboard item copies in pluma on Xwayland, since X gets and gives a selection only with an XWayland window focused (37).
  - The fall-back item looks for applets linked to an applet library that's gone, which is 70's soname.
- **The wizard,** [73-closing-checks.sh](73-closing-checks.sh), has four parts, since the checks are run by two users in four places, and each part keeps its own results and resumes:
  - `tty`, as `caminotest` beside `camino --session` on a tty: the exit's login, the `DISPLAY` item (its first run), the clipboard, Shut Down's dialog cancelled, a layout saved, Log Out with the status the tty prints, and the layout at the next login. It writes a `monitors-wayland.xml` with the first output at 1.25, compares the next login with it, and puts back what was there. From a copy its results go to `~/.local/state/camino/73-closing-results.md`.
  - `dm`, as the developer in *MATE on Camino* from the greeter: the exit's login, and Shut Down for real. It keeps the boot's id, and after the machine is back reads the last boot's journal.
  - `x11`, as the developer in X11 MATE and then in *MATE on Wayfire*: the fall-back item.
  - `nested`, beside `tools/dev-nested`: 68's two *Privileged clients* items, which are tagged `M5` and had no wizard.
  - Each part's last stage lists what else the closing needs: the other parts, 43's and 54's wizards with `TICKET=`, 59's for M4's tty items, 76's, 60's hot-plug, and the push.
- **The wizard's dry run:** only the `x11` part, here on the host desktop (KDE on Wayland, stock MATE packages) on a private bus with a scratch dconf, with every check answered as skipped: its five stages ran with no script error, across the stop it makes for the Wayfire login, and wrote its results to a scratch ticket. The two scripts that write and compare the layout were run against a made-up `wlr-randr --json`. The `tty`, `dm` and `nested` parts have never run: they need a session, and `wl-clipboard` isn't installed here.
- **70's soname, not decided: it needs the developer.** What was found here, on the developer's machine with the stock packages:
  - It's seven packages, not three. `mate-applets` (13 applets), `mate-media` (the volume applet), `mate-power-manager` (2), `mate-netbook`, `mate-notification-daemon`, `mate-sensors-applet` and `mate-utils` (the dictionary) each have an applet linked to `libmate-panel-applet-4.so.1`.
  - Every one of them is a program of its own (`InProcess=false`, or unset), so it loads its own copy of the library and talks to the panel over D-Bus. The old library beside the new one would never put both in one process.
  - The developer's panels have only mate-panel's own applets (the clock, the notification area, and the window list, switcher and Show Desktop), so nothing is lost on this machine either way.
  - `mate-charpick-applet` already can't start with the stock panel: `libgucharmap_2_90.so.7` isn't installed. The wizard names the library each broken applet misses, so this one isn't taken for the soname.
  - **Proposed:** keep the stock library beside the new one, by having `mate-panel-camino` also build Arch's 1.28.7 tag and install only its `libmate-panel-applet-4.so.1*`. The other way is seven more PKGBUILDs to build again at each of Arch's updates, for applets that can't show on Wayland anyway. Not checked, and it needs the patched panel installed: that an applet on `.so.1` and master's panel still understand each other over D-Bus.
- **The full test suite** passed here on `678d85a` (610 tests in `camino`, with 77's among them). This ticket adds no Rust.
- **The review,** applied: the checklist's login item no longer expects logind's type to be `wayland` on a tty, and looks for time-outs where mate-session writes; the Shut Down item allows the `didn't unset` line (61); the fall-back item puts the developer's own theme back. In the wizard: a saved pid is checked to be Camino's, `dm` reads the journal of the boot the Shut Down was asked in and shows the developer's own saved layout, and `x11` puts the theme back when it's stopped. **Left as is:** the helpers copied from 76's wizard, as the wizards before; the last step of the layout item (the defaults back once the file is gone), which no stage walks through; and a Resume that Enter answers with no, which is the library's.
