# gtk_shell1

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

A minimal `gtk_shell1` ([06](06-v01-protocol-and-integration-set.md)) in `protocols::gtk_shell`, generated from `resources/gtk-shell.xml` with `wayland-scanner` in `protocols::raw` ([15](15-crate-and-module-architecture.md)). GTK3 reports modality and rings the bell only through this GNOME protocol.

- **The XML** is GTK's `gtk-shell.xml` from `gtk-3-24`, vendored with its licence header.
- **The version offered** is the lowest at which GTK3 uses `set_modal` and `system_bell`, read from `gdkdisplay-wayland.c` and `gdkwindow-wayland.c` when the ticket starts. GTK3 skips what a lower version lacks.
- **`get_gtk_surface`** gives a `gtk_surface1` for a `wl_surface`.
- **`set_modal` and `unset_modal`:** the window is modal or not, joining xdg-dialog's modal hint. It feeds the frame's style (`modal_dialog`, or `attached` with `attach-modal-dialogs`, 30) and policy's transient rules (19), live.
- **`system_bell`:** accepted, with its surface or none. It's logged at debug level; M6 joins it to the one bell event (17).
- **Everything else is accepted and ignored:** `set_dbus_properties`, `present`, `request_focus`, `set_startup_id`, `notify_launch`, `titlebar_gesture`, `release`. Caminito sends no `configure`, `configure_edges` or `capabilities` events beyond what GTK3 needs to start; none, if it needs none.
- **Test client:** `gtk_shell1` in `caminito-test-client`, from the same XML.

## Acceptance criteria

- [x] Headless tests, written before the code: a transient toplevel that calls `set_modal` is a modal dialog in policy and gets the `modal_dialog` frame style, and `unset_modal` takes both back; `system_bell` with and without a surface is accepted; the ignored requests raise no protocol error.
- [x] `a_client_lists_the_globals` gains `gtk_shell1`.
- [x] Nested, a GTK3 modal dialog (pluma's *Open*, over its window) has the modal frame, and its parent can't be focused over it, as under Marco.

## Notes

- **Not offered to hide client-side decorations.** GTK3's SSD still goes through the KDE server-decoration protocol (07).
- **GTK4 doesn't bind `gtk_shell1`;** it uses xdg-dialog and xdg-system-bell, which are already in (22).
- **Resolved on 2026-10-04,** with the three criteria met.
- **Version 1 is offered.** Read from gtk-3-24 at `2776f156`: `set_modal`, `unset_modal` and `system_bell` are all in version 1. GTK3 binds the lower of the global's version and 5 (`gdkdisplay-wayland.c:429-436`), and at version 1 it skips `request_focus` (`gdkwindow-wayland.c:4056`), `release` (3584-3588, it only drops its proxy), `titlebar_gesture` (5094) and the edge constraints of `configure_edges` (1764), which it then works out itself. It needs no `capabilities` or `configure` event to start, so Caminito sends none.
- **`notify_launch`, `request_focus`, `titlebar_gesture` and `release` can't reach Caminito at version 1:** the Wayland library refuses a request newer than the version bound (`invalid method 3 (since 1 < 3)`). Their arms are in the code because the XML has them, and no test sends them. The headless test sends `set_startup_id`, `set_dbus_properties` and `present`.
- **The XML has no licence header upstream.** The vendored file is GTK's, unchanged, under a comment that names the branch and commit and gives GTK's LGPL notice.
- **The modal hint** is kept on the `wl_surface`, and `window::xdg` reads it beside xdg-dialog's. It outlives the `gtk_surface1`, since the protocol has no word for what destroying one undoes. GTK3 sends `set_modal` or `unset_modal` each time it maps a window, before the first commit, so a pending window maps with the right type.
- **A new policy rule was needed for the third criterion.** Policy (19) had no rule that keeps a parent from being focused over a modal dialog; modality only chose the frame's style and the placement. `Policy::focus_window` now gives the focus to the last modal dialog over the window, as Marco's `get_modal_transient` does (`window.c:4381-4417`), and so does the focus a closed transient hands to its parent. It applies to every modal dialog: xdg-dialog's and `_NET_WM_STATE_MODAL`'s too. One difference from Marco, on purpose: with the modal dialog minimised, Marco focuses nothing, and policy focuses the parent, so that a seat's focus is always on screen.
- **`attach-modal-dialogs` has no headless test here.** No MATE schema has the key (`settings::translate` reads it only "when a schema has it"), so the fixture can't set it. `decoration::style`'s tests have the `attached` style.
- **`system_bell` for a `gtk_surface1` whose `wl_surface` is gone** is logged as a bell for no window. M6 decides what the one bell event does with it.
- **Nested, checked in the virtual machine** (winit on Xvfb, a private bus, GTK 3.24.52). pluma isn't installed there, so a PyGObject program stood in: a `GtkWindow` and a modal `GtkFileChooserDialog` transient for it, with `gtk-dialogs-use-header` off as in a MATE session, which is what pluma's *Open* is. With `WAYLAND_DEBUG=1`, GTK3 binds `gtk_shell1` at version 1 and sends `set_modal` for the dialog before its first commit, and `unset_modal` for the window. The dialog's frame has the close button only, which is the `modal_dialog` style (a `dialog` frame has minimise and maximise too). A click on the parent, sent with XTest, leaves `DumpState`'s `focus` on the dialog. `gdk_window_beep` and `gdk_display_beep` are logged as the bell for the dialog's surface and for no window.
- **The first nested run showed the parent focused, and it was the build, not Caminito:** the guest's shared `CARGO_TARGET_DIR` had linked a `caminito-policy` built before the rule. The files copied from the Mac kept their older times, so cargo took its build as fresh. Copy with `tar -m`.
- **Review left as is:** `wayland-scanner` is declared in both crates, as `zbus` is; the generated modules take a blanket `allow` with a reason, since an `expect` of lints generated code may not raise would itself warn.
