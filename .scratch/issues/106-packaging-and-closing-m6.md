# Packaging and closing M6

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M6's exit criterion holds on the developer's machine, and the closing commit says so ([17](17-milestone-slicing.md)).

- **Packaging ([16](16-packaging.md)):**
  - `caminito-git` depends on every Patched package by its real name: the five of 16 and `caja-caminito`, which [81](81-primary-output.md) added;
  - the weekly patch-on-head job covers `mate-control-center-caminito` and `mate-utils-caminito`;
  - `pkgver` and every `pkgrel` are bumped, and all the packages are installed on the developer's machine from the built files.
- **The checklist:** one item per row of 06's integration table, tagged M6. Tickets 94 to 104 wrote theirs; write the ones no ticket did:
  - the minimise animation target reaching Caminito: the window list's button rectangle shows for a window in the debug dump (`mate-panel-caminito`'s first patch, from M5);
  - any other row of the table with no item.
- **The closing run:** every item tagged M6 or earlier that still applies, on the installed packages, in a Session on the developer's machine, with a wizard script beside this ticket as [73](73-packaging-and-closing-m5.md) had. The M6 items already written are part of it: the window list by workspace ([75](75-window-list-by-workspace.md)), the saved layout, the primary output, turning an output off, and caminito-displays' four.
- **The closing commit,** `Close M6 MATE integrations`, with the results in its message. It moves the tag of any item that's deferred, and updates the README's status and the map.

## Acceptance criteria

- [ ] The checklist has an item for every row of [06](06-v01-protocol-and-integration-set.md)'s integration table, or the row is one an earlier milestone's item covers, and the ticket's notes say which. (Not met: the notes say which item each row has, and four rows have none.)
- [x] CI is green on push, with a package job for each of the seven packages. (CI passed on `master` on 2026-10-08 (run 37853654941, at `b886f7a`), and the Packaging job passed on `master` on 2026-10-08 (run 37823877022, at `88710e2`), the last push that changed `packaging/arch/`. It builds eleven packages, the seven among them. The job runs again with this ticket's bumps.)
- [ ] 17's exit, run by hand in a Session: Alt+F1 and Alt+F2; the panel's Show Desktop and Ctrl+Alt+D agreeing; the pager's rows, names and count; the minimise animation target reaching Caminito; volume, brightness and custom shortcuts from msd, with its OSD; the Windows, Keyboard Shortcuts, Mouse and Keyboard dialogs and caminito-displays; `Print`, `<Shift>Print` and `<Alt>Print`. (Seen by the developer on 2026-10-08, in a Session: `Print`, `<Shift>Print` and `<Alt>Print` work ([104](104-screenshots.md)). The rest wasn't reported item by item: the developer closed M6 that day, as the closing note says.)
- [x] X11 MATE and MATE on Wayfire still work with all the Patched packages installed (ADR 0011). (X11 MATE seen by the developer on 2026-10-08, the checklist item's steps 1 to 3: the session is `x11`, Marco runs, a theme change restyles open apps, no applet is missing, and the window list, the switcher, *Log Out…* and *Shut Down…* work. Also seen there that day: *Displays* opens the stock Display capplet, not caminito-displays (84); *Windows* and *Keyboard Shortcuts* work (101); *Mouse* and *Keyboard* work (102). MATE on Wayfire seen that day too: the panel, the desktop icons and the wallpaper are in place; *Mouse* and *Keyboard* crash at start as the stock ones do (102); *Windows* says the window manager is unsupported and *Keyboard Shortcuts* opens (101); the switcher's preferences have neither the count nor the names (97). msd run by hand there, on the Wayland display, has no `/org/mate/SettingsDaemon/MediaKeys`, so the media-keys plugin isn't loaded (99); the session's own msd is started with `GDK_BACKEND=x11` by `mate-wayland-components.sh`, and loads it on XWayland through the X11 code, as stock. On X11 the switcher's count and names follow its preferences (97); the volume keys work (99); a custom shortcut runs its command and follows a change of its keys (100); and with the switcher's rows set to 2 and back, `gsettings list-recursively org.mate.Caminito` kept its rows at 1 (96).)
- [ ] Every checklist item tagged M1 to M6 that still applies was run, and the results are in the closing commit's message. (Not met: no run was recorded. The closing commit's message has what was seen.)

## Notes

- **Two changes to 17's M6 text,** both from [ADR 0014](../../docs/adr/0014-caminito-keeps-the-layouts-and-caminito-displays-edits-them.md), agreed with the developer on 2026-10-06 and recorded here and on the map, with 17 left as it was written:
  - the exit's "mcc's Display dialog" is caminito-displays ([84](84-caminito-displays.md)), which replaced the capplet;
  - mcc's 5% scale steps patch is dropped: the capplet it changed isn't used under Caminito.
- **81 to 86 and 89 to 91 are M6's,** by the developer's decision on 2026-10-06. They're resolved, and their checks left to the developer are in this ticket's closing run.
- **Brightness** is in 17's exit under msd. [99](99-msd-media-keys-and-osd.md) finds who handles it; if it isn't msd, the closing run records that and the ticket 99 filed.
- **[93](93-nested-caminito-slow-in-the-session.md)** gates no milestone and isn't part of the run.

## Closed by the developer's decision (2026-10-08)

- **The developer said M6 is finished** and that M7 starts, with three criteria above unticked. This note says what the record has, so that M7's closing run knows what it inherits. 17 has every item tagged with a milestone or an earlier one run at each closing, so the M6 items are M7's to run too, and no tag was moved.
- **Seen by the developer in a Session on Caminito:** `Print`, `<Shift>Print` and `<Alt>Print` work ([104](104-screenshots.md)). Nothing else of 17's exit was reported item by item: Alt+F1 and Alt+F2, Show Desktop, the pager's rows, count and names, the volume keys and custom shortcuts with the OSD, the four control center dialogs, caminito-displays, and the earlier M6 items (the window list by workspace, the saved layout, the primary output, turning an output off).
- **Seen by the developer in X11 MATE and on Wayfire** that day, with every Patched package installed: the fourth criterion's note.
- **CI:** green on `b886f7a` (run 37853654941), and the Packaging job on `88710e2` (run 37823877022), with the eleven packages.
- **Installed on the developer's machine** at the closing: `caminito-git` r333.0fb70d7-1, `mate-session-manager-caminito` 1.28.0.r23.g57a9178-3, `mate-panel-caminito` 1.28.7.r111.g58ac6c1-10, `mate-settings-daemon-caminito` 1.29.1-6, `mate-control-center-caminito` 1.29.1-2, `caja-caminito` 1.28.0-3, `mate-utils-caminito` 1.29.1-2, `mate-power-manager-caminito` 1.28.1-1, `vala-panel-appmenu-mate-caminito` 25.04-4 and `appmenu-gtk-module-caminito` 25.04-2. The tree has `mate-control-center-caminito` at `pkgrel` 3 ([102](102-mouse-and-keyboard-capplets.md)'s two patches) and `caminito-git` at r358, neither installed. What `caminito-git` gained since r333 is the menu claim of the global menu's epic, which is after v0.1.
- **Not written:** the wizard script for the closing run.
- **06's integration table, row by row:**
  - Alt+F1, Alt+F2: *Alt+F1 opens the main menu, and Alt+F2 the Run dialog* (95).
  - Show Desktop shared with Ctrl+Alt+D: *The Show Desktop button and Ctrl+Alt+D show the same desktop* (94).
  - Pager grid layout: *The workspace switcher's rows are Caminito's grid* (96).
  - Workspace count and names: *The workspace switcher's preferences set the count and the names* (97).
  - Minimise animation target: *The window list sends each button's rectangle*, written here. It reads the panel's `set_rectangle` requests and not the debug dump, as this ticket asked: the dump has no member for the rectangle.
  - Caminito's identity for mate-control-center: *Windows and Keyboard Shortcuts open under Caminito, and name it* (101).
  - Active window for mate-screenshot: *Print, Shift+Print and Alt+Print* (104).
  - Media keys and custom shortcuts: *The volume keys change the volume and show the OSD* (99) and *A custom shortcut runs its command* (100). The brightness keys' item is M7's (107).
  - Input device settings and keyboard layout: M4's *Switching keyboard layouts in native and X11 apps*, and the live changes in *Mouse and Keyboard open under Caminito, and apply live* (102).
  - Mouse and Keyboard capplets: the same item of 102.
  - Display configuration: the row is superseded by ADR 0014. *Caminito restores the saved layout from the first frame*, the two *Outputs* items of M6 and caminito-displays' four.
  - Wallpaper on outputs caja doesn't cover: M5's *Logging into MATE on Caminito*, whose expected result has msd's wallpaper on every output, and the primary output's item.
  - Screen locking, and Idle and presence: M7's.
  - **No item:** Bell sources ([105](105-one-bell-event.md) has headless tests), Window types and Struts (M5's headless tests; the login item only has the panel in place). Whether they need one is M7's closing to decide.
