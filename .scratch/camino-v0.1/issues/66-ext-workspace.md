# ext-workspace

Type: implementation
Status: resolved
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

`ext_workspace_v1`, read-only, written by Camino in `protocols::ext_workspace` ([06](06-v01-protocol-and-integration-set.md), [15](15-crate-and-module-architecture.md)), for mate-panel's workspace switcher through libxfce4windowing.

- **Two parts, as in [58](58-wlr-output-management.md) and [65](65-foreign-toplevel.md):** a protocol module that knows nothing of policy, refreshed with plain data in the refresh pass, and a handler trait for requests.
- **One group** holding every output, since Marco's workspaces span all monitors. Outputs enter and leave it as they're plugged.
- **One workspace per policy workspace,** with:
  - `id`, stable for the workspace's number;
  - `name`, from `workspace-names` or Marco's default name (29 keeps them for this);
  - `coordinates`, **always two values**, column then row, because libxfce4windowing reads index 1 without a proper bounds check (06). Until M6's layout keys, the workspaces are one row: (n, 0);
  - `state`: `active` for the current workspace. `urgent` and `hidden` aren't used;
  - `capabilities`: `activate` only.
- **Read-only (06):** the group has no `create_workspace` capability, and workspaces have no `remove`, `assign` or `deactivate`. Requests without a capability are ignored, not protocol errors.
- **`activate` and `commit`:** requests take effect at the manager's `commit`, as the protocol says. `activate` switches workspace through policy, as the keybinding does.
- **Live count and names:** `num-workspaces` and `workspace-names` changes (29) add, remove and rename workspaces in the view, with one `done` per refresh.
- **Test client:** ext-workspace in `camino-test-client`, as a pager would use it (14).

## Acceptance criteria

- [x] Headless snapshot tests, written before the code:
  - a client gets one group with the outputs, four workspaces with names, two-value coordinates and `activate`, the first `active`, then `done`;
  - switching workspace by binding moves `active`, with one `done`;
  - `activate` then `commit` switches workspace, and `activate` without `commit` doesn't;
  - `num-workspaces` 4 to 6 adds two, 6 to 2 removes four, and a new name is sent;
  - `remove` and `create_workspace` from a client change nothing;
  - a new output enters the group.
- [x] `a_client_lists_the_globals` gains `ext_workspace_manager_v1`.

## Notes

- **The grid** from `org.mate.Camino`'s three `workspace-layout-*` keys, and the pager's patch that places by coordinates, are M6 (09, 17). One row is what an unpatched pager shows anyway.
- **Window-to-workspace** isn't in either protocol: libxfce4windowing can't tell which workspace a toplevel is on, so the window list shows every workspace's windows. Whether that's acceptable for v0.1 is checked in [72](72-session-file-and-first-login.md). 72 saw it in a session, and [75](75-window-list-by-workspace.md) fixed it with Xfce's foreign toplevel protocol, which does say (ADR 0013).
- **The security-context filter** on the global is [68](68-data-control-and-privileged-globals.md).
- **Smithay may have gained `ext_workspace`** since 03's audit at `928d4a9`. Check the pinned `rev` first; if it has it, use it and keep this ticket's behaviour.
- **Resolved on 2026-10-04.** Checked in the virtual machine (aarch64): the tests in `tests/ext_workspace.rs` and the full suite on a private bus (`dbus-run-session`). camino-theme's goldens fail on aarch64, as they do on `master`. No pager was run against it; [72](72-session-file-and-first-login.md) sees mate-panel's workspace switcher in a real session.
- **Smithay has no `ext_workspace`** at the pinned `rev` (`928d4a9`), so `protocols::ext_workspace` is Camino's, written from the XML. `wayland-protocols` 0.32.13 has the bindings.
- **Written before the code:** the test client and the ticket's tests were written and seen to fail (no global) before the protocol module. The tests for `stop` and for a removed handle came after the review.
- **Decided here, beyond what the ticket says:**
  - **`id` is the workspace's number, from 1,** as a string: "1" to "36".
  - **A workspace that comes back is a new handle with the same id.** A client's old handle for that id was sent `removed` and stays inert.
  - **Of several `activate`s before a `commit`, the last counts,** since one workspace is active. An `activate` for a workspace that goes before the `commit` is dropped. `commit` with nothing pending does nothing.
  - **`stop`:** the client is sent `finished`, which destroys the manager, and gets no event after it: each later event would need the manager's `done`.
  - **A client that destroys its group handle** gets no new one, so no more output events and no `workspace_enter` for new workspaces. The protocol allows that.
  - **`output_enter` is by `wl_output`,** so a client that binds an output after the manager is told on the next refresh, as in [65](65-foreign-toplevel.md).
  - **`deactivate` is ignored too,** with `remove`, `assign` and `create_workspace`.
- **Review left as is:** the diff of a client's `wl_output`s is written twice, here and in `protocols::foreign_toplevel`, and the workspace id is a `String`, not a type. No test sends `assign`.
