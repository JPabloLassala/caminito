# One bell event

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Every bell an app rings reaches Caminito as the same event, whatever the app's toolkit ([06](06-v01-protocol-and-integration-set.md)'s *Bell sources* row, [17](17-milestone-slicing.md)). What the user sees and hears is M8's.

- **The three sources,** each logged at debug level today, feed one bell event:
  - GTK3: `gtk_shell1.system_bell` ([67](67-gtk-shell.md));
  - GTK4 and others: xdg-system-bell, from Smithay;
  - XWayland: `XkbBellNotify` on Caminito's X connection ([38](38-x-connection-and-root-properties.md)).
- **The event** carries the window the bell is for, or none. A bell with a surface is for that surface's window, as a window beep; a bell with none is for the focused window, as a display beep (06). A bell for a surface that's gone, or that isn't a window's, is a bell for no window, which settles what 67 left open.
- **One place handles it.** Until M8 it counts the bell and keeps the last one's window and source for the debug dump. It reads `org.mate.sound` and Marco's bell keys where M8 will, without acting on them.

## Acceptance criteria

- [x] Headless tests, written before the code: a `gtk_shell1` bell with a surface, one without, and an xdg-system-bell each show in the debug dump with the right window; a bell for a destroyed surface is a bell for no window.
- [x] An X11 tier test: an X client's bell shows in the dump with its window.

## Notes

- **The visual and audible bell** are M8's (17), with the keys that turn them on.
- **A bell from a sandboxed client** counts like any other.

## Resolution

The event is `caminito/src/bell.rs`: `Caminito::ring_bell` takes the source and what the bell named, counts it, and keeps the last one's source and window in `Caminito::bells`. The debug dump shows them as `bell`, `{ "count", "last": { "source", "window" } }`, with the source `gtk_shell1`, `xdg_system_bell` or `xkb`. The tests are `caminito/src/tests/bell.rs`, and the settings' are in `caminito/src/settings/translate/tests.rs`.

- **The three sources:** `GtkShellHandler::system_bell` and Smithay's `XdgSystemBellHandler::ring` call it, and so does `XConnection::requests`, which gives `XkbBellNotify` to `Caminito::x_request` as `Request::Bell`.
- **The window:** a bell that names nothing is for the focused window. One that names a surface is for its root surface's window, so a subsurface's bell is its window's. A surface of no window (a popup's, a layer surface's, one with no role) and a surface that's gone give no window. An X bell that names a window Caminito doesn't manage gives none too, where Marco took the focused window (`bell.c:meta_bell_notify`): the ticket's rule, the same for every source.
- **`gtk_shell1`'s handler now takes the surface as a `Weak`,** because a `gtk_surface1` has no destructor at version 1 and outlives its surface. Before, a bell for a gone surface came through as a bell for none, which would have been the focused window's.
- **The settings:** `Settings::bell` reads Marco's `audible-bell`, `visual-bell` and `visual-bell-type`. `ring_bell` logs them at debug level with `org.mate.sound theme-name`, which was already read, and doesn't act on them. `visual-bell` is in `UNSUPPORTED`, so turning it on is logged once until M8 flashes. `audible-bell` isn't, since Caminito matches it only when it's off, and neither is `visual-bell-type`, which only matters with the visual bell on.

## Notes from the work (2026-10-08)

- **Written before the code:** the bell tests, seen to fail with no `bell` in the dump. The settings' tests were written with their code, so they weren't seen to fail.
- **`bell-mode` isn't read.** 09 has `off` silence the audible bell, but 53 made the key *never*, and `UNSUPPORTED`'s doc says so. That's for M8 to settle when it plays the bell (17).
- **The test client** rings xdg-system-bell, and X bells with `XkbBell` for a window, as GDK3 does, and with the core `Bell` for none, as `XBell` does. It enables x11rb's `xkb` feature for that.
- **The run:** on Ubuntu 24.04 with Xwayland 23.2.6, the bell tests, the settings tests and the debug tests pass. The whole suite has five failures there that the base commit has too: four `keymap` tests and `window_properties::the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow`, which look like that host's xkeyboard-config and Xwayland. CI runs on Arch. Clippy on that toolchain (1.97) also flags three `manual_range_contains` in `caminito-theme`, which this change doesn't touch.
- **Left after the review:**
  - An X bell for a child of a managed X window gives no window. Only the managed window's own id is looked up, while a Wayland bell walks from a subsurface up to its root. GDK3 rings for the toplevel, so that's left as it is.
  - Each source has its own few lines turning what it named into `BellFor`, since each finds the window its own way.
  - One full run had `image_capture::a_tool_that_quits_with_a_frame_waiting_is_forgotten` fail. It passed in three runs alone and in two more full runs, and this change doesn't touch image capture.
