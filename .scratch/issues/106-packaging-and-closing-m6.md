# Packaging and closing M6

Epic: caminito-v0.1
Type: implementation
Status: open
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

- [ ] The checklist has an item for every row of [06](06-v01-protocol-and-integration-set.md)'s integration table, or the row is one an earlier milestone's item covers, and the ticket's notes say which.
- [ ] CI is green on push, with a package job for each of the seven packages.
- [ ] 17's exit, run by hand in a Session: Alt+F1 and Alt+F2; the panel's Show Desktop and Ctrl+Alt+D agreeing; the pager's rows, names and count; the minimise animation target reaching Caminito; volume, brightness and custom shortcuts from msd, with its OSD; the Windows, Keyboard Shortcuts, Mouse and Keyboard dialogs and caminito-displays; `Print`, `<Shift>Print` and `<Alt>Print`.
- [ ] X11 MATE and MATE on Wayfire still work with all the Patched packages installed (ADR 0011).
- [ ] Every checklist item tagged M1 to M6 that still applies was run, and the results are in the closing commit's message.

## Notes

- **Two changes to 17's M6 text,** both from [ADR 0014](../../docs/adr/0014-caminito-keeps-the-layouts-and-caminito-displays-edits-them.md), agreed with the developer on 2026-10-06 and recorded here and on the map, with 17 left as it was written:
  - the exit's "mcc's Display dialog" is caminito-displays ([84](84-caminito-displays.md)), which replaced the capplet;
  - mcc's 5% scale steps patch is dropped: the capplet it changed isn't used under Caminito.
- **81 to 86 and 89 to 91 are M6's,** by the developer's decision on 2026-10-06. They're resolved, and their checks left to the developer are in this ticket's closing run.
- **Brightness** is in 17's exit under msd. [99](99-msd-media-keys-and-osd.md) finds who handles it; if it isn't msd, the closing run records that and the ticket 99 filed.
- **[93](93-nested-caminito-slow-in-the-session.md)** gates no milestone and isn't part of the run.
