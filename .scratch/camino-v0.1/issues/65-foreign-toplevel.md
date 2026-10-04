# Foreign toplevel

Type: implementation
Status: open
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

`zwlr_foreign_toplevel_management_unstable_v1` version 3, written by Camino in `protocols::foreign_toplevel`, and `ext_foreign_toplevel_list_v1` from Smithay ([06](06-v01-protocol-and-integration-set.md), [15](15-crate-and-module-architecture.md)). mate-panel's window list and Window Selector use the first through libxfce4windowing; M6's `GetActiveToplevel` and image capture name windows by the second's identifier.

- **Two parts, as in [58](58-wlr-output-management.md):** the protocol module owns each client's handles and knows nothing of windows. Its `refresh` takes the toplevels there are, as plain data, in the refresh pass (15), and sends only what changed, then `done`. A handler trait answers requests.
- **A view, not inline (15):** no handler sends foreign-toplevel events. Handlers change policy, and the refresh pass compares against what each client was last sent.
- **Which windows:** every managed window on every workspace, xdg and XWayland alike, except those that skip the taskbar by Marco's type rules (35). Pending windows and override-redirect windows aren't listed.
- **What's sent:**
  - `title`;
  - `app_id`: the raw xdg `app_id`, or `WM_CLASS`'s class for an XWayland window, since the window list finds icons through its `.desktop` file (06);
  - `state`: maximized, minimized, activated (policy's focused window) and fullscreen;
  - `output_enter` and `output_leave`, from the outputs the window overlaps;
  - `parent` (v3), for transients.
- **Requests,** each through policy as the same user action would be:
  - `activate`: switches to the window's workspace, unminimises, raises and focuses, as Marco's `_NET_ACTIVE_WINDOW` from a pager does;
  - `close`;
  - `set_maximized` and `unset_maximized`, `set_minimized` and `unset_minimized`, `set_fullscreen` (on the output named, or the window's) and `unset_fullscreen`;
  - `set_rectangle`: kept per window as the minimise animation's target. Nothing reads it until M8's animation.
- **ext-foreign-toplevel-list:** Smithay's state, fed in the same refresh pass, with a stable identifier per window, kept where M6 can look a window up by it.
- **Icons:** the lookup of 30 is by `app_id`, and already shared.
- **Test client:** wlr-foreign-toplevel in `camino-test-client`, as a panel would use it (14).
- **Written fresh:** niri's and cosmic-comp's implementations are GPL-3, so they're design references only (03).

## Acceptance criteria

- [ ] Headless snapshot tests, written before the code:
  - a client that binds late gets every window, with title, app_id, state and outputs, then `done`;
  - a title change, a focus change, minimise and maximise each send one update and one `done`, and a burst of changes sends one;
  - a closed window's handle gets `closed`;
  - `activate` on a minimised window on another workspace switches workspace, restores and focuses it;
  - `close`, `set_minimized`, `set_maximized` and `set_fullscreen` do what the frame's buttons and bindings do;
  - a transient's `parent`;
  - a second client gets its own handles, and one leaving doesn't disturb the other.
- [ ] X11-tier test: an XWayland window is listed with its `WM_CLASS` as `app_id`, and a skip-taskbar one isn't.
- [ ] `a_client_lists_the_globals` gains `zwlr_foreign_toplevel_manager_v1 v3` and `ext_foreign_toplevel_list_v1`.
- [ ] Nested, `wlrctl toplevel list` lists the windows, and `wlrctl toplevel focus` and `minimize` act on them.

## Notes

- **The security-context filter** on both globals is [68](68-data-control-and-privileged-globals.md).
- **The window's demand for attention** (`window/mod.rs`) has no state in wlr-foreign-toplevel v3. It stays for M8's Alt+Tab and focus-stealing prevention.
- **`xdg.rs`'s minimise-rectangle note** ("only foreign-toplevel's clients would ask") is resolved here.
