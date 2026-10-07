# Screenshots: Print, Shift+Print and Alt+Print

Type: implementation
Status: open
Milestone: M6
Blocked by: 103
Map: [Camino v0.1](../map.md)

## What to build

`Print` takes a screenshot of the screen, `<Shift>Print` of an area, and `<Alt>Print` of the active window with its Frame, as under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Active window for mate-screenshot* row, [17](17-milestone-slicing.md)'s exit).

- **The bindings:** find first who binds each key on X11 MATE (Marco's screenshot commands, or msd), and bind them the same way: Marco's join the hard-coded set that stays until M8, and running the command from `org.mate.Marco.keybinding-commands`; msd's come with [99](99-msd-media-keys-and-osd.md).
- **D-Bus:** `GetActiveToplevel() → s` on `org.mate.Camino`. It returns the ext-foreign-toplevel-list identifier of the most recently focused toplevel that doesn't belong to the caller's pid, or an empty string when there's none. [65](65-foreign-toplevel.md) kept the identifier lookups for this.
- **The package:** `mate-utils-camino`, the fifth Patched package of [16](16-packaging.md), with its PKGBUILD, series, CI job and place in the weekly job. `camino-git` depends on it by its real name.
- **The carried patch:** when `org.mate.Camino` has an owner, mate-screenshot's `--window` asks for the active toplevel and captures it through the ext toplevel image-capture source ([103](103-image-capture.md)). With no owner it keeps its refusal of `--window` on Wayland (ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] Headless tests with a D-Bus client, written before the code: `GetActiveToplevel` returns the focused window's identifier; with the caller's own window focused it returns the one focused before; with no other window it returns the empty string.
- [ ] A headless test that each of the three keys runs its command.
- [ ] `mate-utils-camino` builds, and its `makepkg` and `namcap` job passes in CI.
- [ ] The checklist has the item, and it was run in a Session: `Print` saves the whole screen, `<Shift>Print` an area, and `<Alt>Print` the window that had the focus, with its Frame, when another window covers part of it. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).

## Notes

- **The area screenshot** is mate-screenshot's own selection over an output capture. If it doesn't work on Wayland at 16's baseline, file a ticket; this one doesn't write it.
- **mate-screenshot's dialog** is a window of the caller's pid, which is why the method skips the caller's.
