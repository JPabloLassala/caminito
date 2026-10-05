# camino-displays takes a long time to start

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

camino-displays ([84](84-camino-displays.md)) takes a long time to show its window. The developer saw it on 2026-10-05, with no time measured. This ticket measures it, finds where the time goes, and says whether start-up time is to be watched from here on.

- **To measure first:** the time from the start of the process to the first frame of the window, cold and warm, in a Session and with `--demo`. `--demo` talks to no compositor beyond GTK's own connection, so the two together say whether the time is GTK's or the program's.
- **Where it may go, none of it checked:**
  - GTK4's start: its renderer (the GL or Vulkan one against cairo, `GSK_RENDERER`), the theme and the fonts.
  - The second Wayland connection, for output management and Identify, and the round trips before the first set of monitors is known.
  - The bus: `org.mate.Camino`, and a call that waits for its answer before the window is shown.
  - The build: whether what was run was a debug build or the package's.
  - The launcher: the control center's entry and the shadowed desktop entry, before the process starts at all.
- **Whether to keep an eye on it** is to decide here: a number written down for the next ticket to compare with, or a check that fails when it gets worse.

## Acceptance criteria

- [ ] The times are written here, with how they were taken and on which build.
- [ ] The cause of the longest part is named, and fixed here or left to a ticket of its own.
- [ ] It is decided whether start-up time is watched, and how.

## Notes

- **"The panel"** in the developer's report is read as camino-displays's window, since it came with [85](85-camino-displays-hangs-on-a-resolution.md). If mate-panel was meant, this ticket is about the wrong program.
