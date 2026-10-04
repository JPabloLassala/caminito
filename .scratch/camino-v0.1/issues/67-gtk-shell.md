# gtk_shell1

Type: implementation
Status: open
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

A minimal `gtk_shell1` ([06](06-v01-protocol-and-integration-set.md)) in `protocols::gtk_shell`, generated from `resources/gtk-shell.xml` with `wayland-scanner` in `protocols::raw` ([15](15-crate-and-module-architecture.md)). GTK3 reports modality and rings the bell only through this GNOME protocol.

- **The XML** is GTK's `gtk-shell.xml` from `gtk-3-24`, vendored with its licence header.
- **The version offered** is the lowest at which GTK3 uses `set_modal` and `system_bell`, read from `gdkdisplay-wayland.c` and `gdkwindow-wayland.c` when the ticket starts. GTK3 skips what a lower version lacks.
- **`get_gtk_surface`** gives a `gtk_surface1` for a `wl_surface`.
- **`set_modal` and `unset_modal`:** the window is modal or not, joining xdg-dialog's modal hint. It feeds the frame's style (`modal_dialog`, or `attached` with `attach-modal-dialogs`, 30) and policy's transient rules (19), live.
- **`system_bell`:** accepted, with its surface or none. It's logged at debug level; M6 joins it to the one bell event (17).
- **Everything else is accepted and ignored:** `set_dbus_properties`, `present`, `request_focus`, `set_startup_id`, `notify_launch`, `titlebar_gesture`, `release`. Camino sends no `configure`, `configure_edges` or `capabilities` events beyond what GTK3 needs to start; none, if it needs none.
- **Test client:** `gtk_shell1` in `camino-test-client`, from the same XML.

## Acceptance criteria

- [ ] Headless tests, written before the code: a transient toplevel that calls `set_modal` is a modal dialog in policy and gets the `modal_dialog` frame style, and `unset_modal` takes both back; `system_bell` with and without a surface is accepted; the ignored requests raise no protocol error.
- [ ] `a_client_lists_the_globals` gains `gtk_shell1`.
- [ ] Nested, a GTK3 modal dialog (pluma's *Open*, over its window) has the modal frame, and its parent can't be focused over it, as under Marco.

## Notes

- **Not offered to hide client-side decorations.** GTK3's SSD still goes through the KDE server-decoration protocol (07).
- **GTK4 doesn't bind `gtk_shell1`;** it uses xdg-dialog and xdg-system-bell, which are already in (22).
