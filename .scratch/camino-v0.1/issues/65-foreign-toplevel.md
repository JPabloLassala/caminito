# Foreign toplevel

Type: implementation
Status: resolved
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

- [x] Headless snapshot tests, written before the code:
  - a client that binds late gets every window, with title, app_id, state and outputs, then `done`;
  - a title change, a focus change, minimise and maximise each send one update and one `done`, and a burst of changes sends one;
  - a closed window's handle gets `closed`;
  - `activate` on a minimised window on another workspace switches workspace, restores and focuses it;
  - `close`, `set_minimized`, `set_maximized` and `set_fullscreen` do what the frame's buttons and bindings do;
  - a transient's `parent`;
  - a second client gets its own handles, and one leaving doesn't disturb the other.
- [x] X11-tier test: an XWayland window is listed with its `WM_CLASS` as `app_id`, and a skip-taskbar one isn't.
- [x] `a_client_lists_the_globals` gains `zwlr_foreign_toplevel_manager_v1 v3` and `ext_foreign_toplevel_list_v1`.
- [x] Nested, `wlrctl toplevel list` lists the windows, and `wlrctl toplevel focus` and `minimize` act on them.

## Notes

- **The security-context filter** on both globals is [68](68-data-control-and-privileged-globals.md).
- **The window's demand for attention** (`window/mod.rs`) has no state in wlr-foreign-toplevel v3. It stays for M8's Alt+Tab and focus-stealing prevention.
- **`xdg.rs`'s minimise-rectangle note** ("only foreign-toplevel's clients would ask") is resolved here.
- **Resolved on 2026-10-04.** Checked in the virtual machine (aarch64): the tests in `tests/foreign_toplevel.rs`, the full suite, and the nested criterion with wlrctl built from its source (c6bc608), on a konsole and an xterm: `toplevel list` shows both, `minimize` minimises the xterm, and `focus` restores and focuses it. The compositor's tests need a private bus there (`dbus-run-session`, as in CI), since the guest's desktop owns the screensaver names. camino-theme's goldens fail on aarch64, as they do on `master`.
- **wlrctl aborts after `minimize` and `focus` have acted,** with libwayland 1.26's "data is non-NULL with zero alloc". It sends `stop`, and then gets the `state` update for the handle it still has; with Camino's first `stop`, which wrongly dropped the client's handles, it didn't. The wire log shows well-formed events (`finished`, then `state(array[4])`), and the test client takes the same sequence. It's wlrctl's handling of a second `state` after an empty one, not Camino's. libxfce4windowing doesn't stop its manager; 72 sees the panel's window list in a real session.
- **Written before the code** holds for the first test only. The protocol module was then written whole, and the other tests after it. The review's findings (below) were fixed with a test for each.
- **Decided here, beyond what the ticket says:**
  - **ext-foreign-toplevel-list has every managed window,** the ones that skip the taskbar too. The ticket's *Which windows* reads as if it's for both protocols, but an xdg toplevel with a parent is a dialog, which skips the taskbar (35), and M6 has to name a focused dialog by its identifier. The taskbar rule is for the taskbar's protocol only.
  - **`unset_minimized` activates the window:** policy has no other way to show a minimised window again, and a window list restores a window by activating it. It switches to the window's workspace.
  - **Outputs are those the window's geometry overlaps, shown or not.** Policy puts a minimised window, or one on another workspace, on no output, and sending that would take it off a window list that shows one monitor's windows.
  - **`parent` is sent only for a transient the taskbar shows whose parent it shows too.** With Marco's rules that's an XWayland window that says it's a normal window; an xdg toplevel with a parent isn't listed.
  - **`stop`:** the manager is destroyed by `finished`, and the client's handles stay up to date until it destroys them.
  - **A handle that was sent `closed` is inert,** though its window may still be managed (it started to skip the taskbar).
  - **The seat `activate` names is ignored:** Camino has one.
- **Kept for later:** `Mapped::minimize_rectangle` for M8, and `Camino::toplevel_identifier` and `window_for_toplevel_identifier` for M6. None has a caller outside the tests yet, so each has an `expect(dead_code)` to take off.
- **The test client's `destroy_window`** now destroys the xdg-decoration object first, which xdg-decoration requires, and `Fixture::disconnect` takes a Wayland client.
- **Review left as is:** the tests read maximised, minimised and fullscreen from policy, as earlier tests do. The minimise rectangle is a tuple in four places, not a type.
