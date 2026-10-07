# One bell event

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Every bell an app rings reaches Camino as the same event, whatever the app's toolkit ([06](06-v01-protocol-and-integration-set.md)'s *Bell sources* row, [17](17-milestone-slicing.md)). What the user sees and hears is M8's.

- **The three sources,** each logged at debug level today, feed one bell event:
  - GTK3: `gtk_shell1.system_bell` ([67](67-gtk-shell.md));
  - GTK4 and others: xdg-system-bell, from Smithay;
  - XWayland: `XkbBellNotify` on Camino's X connection ([38](38-x-connection-and-root-properties.md)).
- **The event** carries the window the bell is for, or none. A bell with a surface is for that surface's window, as a window beep; a bell with none is for the focused window, as a display beep (06). A bell for a surface that's gone, or that isn't a window's, is a bell for no window, which settles what 67 left open.
- **One place handles it.** Until M8 it counts the bell and keeps the last one's window and source for the debug dump. It reads `org.mate.sound` and Marco's bell keys where M8 will, without acting on them.

## Acceptance criteria

- [ ] Headless tests, written before the code: a `gtk_shell1` bell with a surface, one without, and an xdg-system-bell each show in the debug dump with the right window; a bell for a destroyed surface is a bell for no window.
- [ ] An X11 tier test: an X client's bell shows in the dump with its window.

## Notes

- **The visual and audible bell** are M8's (17), with the keys that turn them on.
- **A bell from a sandboxed client** counts like any other.
