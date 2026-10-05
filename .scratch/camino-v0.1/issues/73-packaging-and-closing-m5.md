# Packaging and closing M5

Type: implementation
Status: open
Milestone: M5
Blocked by: 72, 74, 76, 77
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
- [ ] The five checklist items are written.
- [ ] Closing M5:
  - from the display manager, and as `caminotest` on a tty, "MATE on Camino" starts mate-session; mate-panel shows the window list and the workspace switcher; caja draws the desktop icons; msd sets the wallpaper and applies the saved display layout over wlr-output-management;
  - logging out through the panel ends the session, and Camino exits with mate-session's status;
  - X11 MATE on the same machine still works with the Patched packages installed;
  - the checklist results are in a `Close M5 MATE session` commit.

## Notes

- **The nested and tty items of M1 to M4** still apply. 43's, 54's and 59's wizards take the ticket to write to (`TICKET=…`).
- **[60](60-tty-checks-left-over.md)'s hot-plug** is still open and gates no milestone; the same sitting can run it.
- **The session can't lock until M7** ([62](62-screensaver-names.md)), so the developer doesn't use it daily yet (17).
- **Handed on by [70](70-mate-panel-camino.md):** `mate-panel-camino` has master's `libmate-panel-applet-4.so.2`, and Arch's `mate-applets`, `mate-sensors-applet` and `mate-notification-daemon`'s applet are linked to `.so.1`, so they don't start under the patched panel, on X11 too. That is against "X11 MATE on the same machine still works" for anyone who uses those applets. Decide here: build those packages again against the patched panel, or keep the stock library beside the new one.
