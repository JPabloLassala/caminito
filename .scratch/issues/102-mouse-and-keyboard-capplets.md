# The Mouse and Keyboard capplets

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: 101
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The control center's Mouse and Keyboard dialogs work in a Session, and what they change applies live ([06](06-v01-protocol-and-integration-set.md)'s *Mouse and Keyboard capplets* row, [16](16-packaging.md), [17](17-milestone-slicing.md)).

- **The carried patches,** in `mate-control-center-caminito`, gated on `MATE_WM_NAME=Caminito` (ADR 0011):
  - both capplets call `gdk_set_allowed_backends("x11")` before `gtk_init`, so they run on XWayland, where the Keyboard capplet's layout pages work. 16 chose this over `.desktop` overrides;
  - `touchpad_is_present` answers TRUE under the same gate, so Mouse shows its touchpad page.
- **Caminito's side is done:** it applies the `org.mate.peripherals-*` keys and the xkb keys live since M4 ([53](53-input-settings-translation.md), [54](54-keyboard-layouts.md)). Fix here any key the dialogs write that Caminito doesn't follow.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] `mate-control-center-caminito` builds with the patches in its series, and its `makepkg` and `namcap` job passes. (The series applies to the 1.29.1 tag, and `capplets/common`, `mouse`, `keyboard`, `windows` and `keybindings` build with it without a warning from the patched lines, on Ubuntu 24.04's MATE 1.26 libraries with configure's floors lowered for the check. `makepkg` and `namcap` weren't run: there's no Arch here. The CI job has the package, and its run is the push's.) (Ticked on 2026-10-08: the Packaging job passed on `master` on 2026-10-08 (run 37823877022, at `88710e2`), with `mate-control-center-caminito` 1.29.1-3 and its series, and `namcap` had no error.)
- [ ] The checklist has the item (*Mouse and Keyboard open under Caminito, and apply live*). It wasn't run in a Session, which is left to [106](106-packaging-and-closing-m6.md); what was run instead is in the resolution. The item: Mouse opens with its touchpad page, and the pointer speed and tap-to-click change as they're set; Keyboard opens, a layout added there can be switched to, and the repeat rate changes. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [x] On X11 MATE and on MATE on Wayfire both dialogs work as before (the gate). (Seen on Xvfb, and on a headless Weston standing in for Wayfire, against the series without 0004 and 0005. The sessions themselves are in the checklist item. The developer saw both dialogs work in the X11 session on 2026-10-08; and in the Wayfire session that day both crashed at start with a segmentation fault, as the stock ones do on any Wayland display (the table below): `mate-mouse-properties` in `XQueryExtension` from `supports_xinput_devices`, and `mate-keyboard-properties` in `XInternAtom` from libxklavier's `xkl_engine_get_instance`, by `coredumpctl`.)

## Notes

- **A machine with no touchpad** still shows the page under Caminito. That's 06's choice: the capplet can't ask libinput.

## Resolution

Two patches end `mate-control-center-caminito`'s series, `0004` and `0005`, and `pkgrel` is 3. Caminito's side needed nothing: every key the two dialogs write in `org.mate.peripherals-mouse`, `-touchpad`, `-keyboard` and `-keyboard-xkb.kbd` is one Caminito follows (53, 54) or one 09 put in *never* (`motion-threshold`). The rest are `org.mate.interface`'s, which Caminito serves as XSETTINGS (ADR 0006); `org.mate.Marco.general`'s `visual-bell` and `visual-bell-type`, which it follows; `org.mate.peripherals-keyboard-xkb.general`'s `group-per-window` and `default-group`, unsupported (09, 54), and `load-extra-items`, the dialog's own; `org.mate.typing-break`'s (13); and `org.mate.accessibility-keyboard`'s, which are M7's AccessX (17).

Decided here, on 2026-10-08:

- **The gate is `wm-common`'s.** `0004` makes 101's `wm_common_is_caminito` public, and both capplets ask it before `gtk_init`. It reads only `MATE_WM_NAME`, so it can be asked there, and both capplets link `libcommon` already.
- **The backend is set before the option context is parsed,** not just before `gtk_init`: `capplet_init` adds GTK's option group, whose parse opens the display.
- **Under Caminito `GDK_BACKEND` can only choose X11.** With `GDK_BACKEND=wayland` there the capplets say *Cannot open display*, where the stock ones crashed. Caminito's session doesn't set it (61, mate-session's patch), so nothing is lost; the commit message says so.
- **The touchpad page's gate is `MATE_WM_NAME` alone,** as 16 has it, though the capplet is on X11 there. So under X11 MATE with the variable somehow left set, the page is shown with no touchpad. Telling XWayland from X11 by `WAYLAND_DISPLAY` would be a second gate that 16 and ADR 0011 don't have.

How it was tested, with the full series applied to 1.29.1 and built as above, on a private bus with a stub `org.mate.SettingsDaemon` (the capplets call its `Awake`), the memory GSettings backend, and a GTK module that prints the backend, the dialog and its notebook's pages:

| Where | Mouse | Keyboard |
|---|---|---|
| Headless Weston and Xvfb, `MATE_WM_NAME=Caminito` | X11, *Mouse* and *Touchpad* | X11, *General*, *Layouts*, *Accessibility*, *Mouse Keys* |
| The same with `GDK_BACKEND=wayland` | *Cannot open display* | *Cannot open display* |
| Headless Weston and Xvfb, no `MATE_WM_NAME`, or `Metacity` | segfault | segfault |
| Xvfb only (X11 MATE) | X11, *Mouse* | X11, the four pages |

The same runs of the series without `0004` and `0005` give the same rows on Xvfb and with no `MATE_WM_NAME` or `Metacity`, and a segfault under `MATE_WM_NAME=Caminito` too: the stock capplets crash on any Wayland display. So the gate changes only Caminito's row. Xvfb has no touchpad, so the X11 row's single page is the stock answer there.

Not tested: a real XWayland, a real touchpad, and a setting applying live in Caminito. They're in the checklist item, for the Session.
