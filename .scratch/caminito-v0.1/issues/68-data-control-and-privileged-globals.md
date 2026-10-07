# Data-control and the privileged globals

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 63, 65, 66
Map: [Caminito v0.1](../map.md)

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

- [x] Headless tests, written before the code:
  - a client connected through a security context sees none of the privileged globals and not `wp_security_context_manager_v1`, and still sees xdg-shell and `wl_data_device_manager`; an ordinary client sees them all;
  - a data-control client receives the clipboard's offer when another client copies, without focus;
  - a data-control client that reads the clipboard and sets it again keeps it after the first owner disconnects, and a focused client pastes it;
  - the same for the primary selection.
- [x] X11-tier test: text copied in an X11 client, taken over by a data-control client, is pasted into a native client after the X11 client exits.
- [x] `a_client_lists_the_globals` gains the two data-control managers and `wp_security_context_manager_v1`.
- [x] Nested, `wl-paste --watch` prints each copy, and in a Flatpak app's sandbox `wayland-info` lists no privileged global.

## Notes

- **The checklist item** for clipboard persistence with the real msd is written in [73](73-packaging-and-closing-m5.md) (11, 17).
- **Whether msd's clipboard plugin loads on Wayland,** and which protocol it binds, is checked in [71](71-mate-settings-daemon-caminito.md): 11 called it inferred, not tested.
- **Checked on 2026-10-05,** in the OrbStack machine (Arch Linux, aarch64): the tests in `tests/security_context.rs` and `tests/data_control.rs`, the two new X11-tier tests in `tests/selection.rs`, and the full suite on a private bus (`dbus-run-session`). caminito-theme's goldens fail on aarch64 by one pixel in 4 cases, as they do on `master`.
- **Nested, on Xvfb:** `wl-paste --watch` printed each of three `wl-copy`s, and `wl-paste --primary --watch` a `wl-copy --primary`. `wayland-info` listed the eight globals for an ordinary client. In Flatpak 1.18.4's sandbox (`flatpak run --socket=wayland --filesystem=host-os:ro --command=/run/host/usr/bin/wayland-info org.freedesktop.Platform//25.08`) it listed 34 globals, none of the eight, with `xdg_wm_base` and `wl_data_device_manager` among them.
- **Written before the code, but first run after it:** the test clients and the tests came first, but the machine that runs them was ready only once the code was written. The security-context tests were then seen to fail with the filter turned off.
- **Decided here, beyond what the ticket says:**
  - **The filter is `handlers::sees_privileged_globals`.** A client is sandboxed when its `ClientState` has a security context. Xwayland, whose client data is Smithay's, is never sandboxed. A client with any other data is offered no privileged global, so a new way to connect fails closed.
  - **Caminito's own protocol states take the filter as an argument,** as Smithay's do, so `protocols/` still knows nothing of `ClientState`.
  - **A context's listener leaves the loop by itself** when the engine closes the context. The clients connected through it stay, and stay sandboxed.
  - **The primary selection is offered through both data-control protocols.** wlr's needs version 2 of its manager, which is what Smithay offers.
  - **Nothing shows a client's app id and instance yet:** they're kept in its client data for M6's portal work. The debug dump doesn't print them.
- **One test of the 555 failed once,** in a run of the whole workspace while other builds loaded the machine. Its name wasn't kept. The suite then passed four times in a row.
- **After the review:** the filter fails closed, the X11-tier test waits for the X client's window to go before the paste, and a test has an X client paste a selection set through data-control.
- **Review left as is:** the three protocol states in `protocols/` each repeat the filter's field and `can_view`. The list of privileged globals is in `state.rs`, the test's `PRIVILEGED` and the checklist, which M6 and M7 change together. No test reads a sandboxed client's app id.
- **Not tested:** a sandboxed client that binds a privileged global by a guessed name. libwayland refuses that for a global the client can't see.
