# Data-control and the privileged globals

Type: implementation
Status: open
Milestone: M5
Blocked by: 63, 65, 66
Map: [Camino v0.1](../map.md)

## What to build

`wp_security_context_v1`, the two data-control protocols, and the rule that hides every privileged global from sandboxed clients ([06](06-v01-protocol-and-integration-set.md)). Data-control waited for the other privileged globals ([17](17-milestone-slicing.md), 22), and output management has been offered to everyone since 58.

- **Security context:** Smithay's `SecurityContextState`. A client that connects through a context's listener is marked sandboxed in its client data, with its app id and instance.
- **One filter:** a single function says whether a client may see privileged globals, and every such global's filter calls it. The security-context global itself isn't offered to sandboxed clients either.
- **Privileged, as of M5:**
  - `zwlr_layer_shell_v1` ([63](63-layer-shell.md));
  - `zwlr_foreign_toplevel_manager_v1` and `ext_foreign_toplevel_list_v1` ([65](65-foreign-toplevel.md));
  - `ext_workspace_manager_v1` ([66](66-ext-workspace.md));
  - `zwlr_output_manager_v1` (58, whose client data kept room for this);
  - `ext_data_control_manager_v1` and `zwlr_data_control_manager_v1`.

  M6's image capture and M7's session lock join the list when they're built.
- **Data-control:** Smithay's ext and wlr states, for the clipboard and the primary selection. msd's clipboard manager uses it to keep the clipboard after its owner quits (11).
- **With XWayland (37):** a data-control client sets and reads the selection without keyboard focus. The X side's rule stays: X clients read the Wayland selection only while an XWayland window has the keyboard, and a selection set through data-control is pushed to X then (`xwayland/selection.rs` notes this).

## Acceptance criteria

- [ ] Headless tests, written before the code:
  - a client connected through a security context sees none of the privileged globals and not `wp_security_context_manager_v1`, and still sees xdg-shell and `wl_data_device_manager`; an ordinary client sees them all;
  - a data-control client receives the clipboard's offer when another client copies, without focus;
  - a data-control client that reads the clipboard and sets it again keeps it after the first owner disconnects, and a focused client pastes it;
  - the same for the primary selection.
- [ ] X11-tier test: text copied in an X11 client, taken over by a data-control client, is pasted into a native client after the X11 client exits.
- [ ] `a_client_lists_the_globals` gains the two data-control managers and `wp_security_context_manager_v1`.
- [ ] Nested, `wl-paste --watch` prints each copy, and in a Flatpak app's sandbox `wayland-info` lists no privileged global.

## Notes

- **The checklist item** for clipboard persistence with the real msd is written in [73](73-packaging-and-closing-m5.md) (11, 17).
- **Whether msd's clipboard plugin loads on Wayland,** and which protocol it binds, is checked in [71](71-mate-settings-daemon-camino.md): 11 called it inferred, not tested.
