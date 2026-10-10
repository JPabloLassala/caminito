# Above, below and sticky

Epic: caminito-v0.1
Type: implementation
Status: open: the code, the policy tests and the xdg headless tests are in; the X11 variants of the headless tests are unrun (Xwayland doesn't start in the build container), and running the checklist item in a Session is the developer's, at 173's closing run
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Keeping a window above or below the others, and sticky windows, which show on every workspace, as compositor state ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)). Today Policy records above, below and sticky as a client asks for them, and nothing acts on them.

- **Layering:** Policy's stacking honours above and below: an above window stays over the normal ones of its workspace, a below window under them, as Marco's layers do.
- **Sticky:** a sticky window shows on every workspace, keeps its place on each, and goes with no workspace switch. Today each workspace has its own stack, so a sticky window has no stack to be in: the model for it is this ticket's (below).
- **The ways in:**
  - the bindings `toggle-above` and `toggle-on-all-workspaces`;
  - the titlebar actions `toggle_above` and `toggle_stick` (today they log and do nothing), and the Frame's above and stick buttons where `button-layout` has them;
  - X11 `_NET_WM_STATE` above, below and sticky, and `_NET_WM_DESKTOP` `0xFFFFFFFF`, before map and as requests ([39](39-x-window-properties-and-requests.md), [11](11-xwayland-integration.md)), which reach Policy since M3 and now take effect;
  - the Xfce foreign-toplevel protocol's `sticky`, `above` and `below` states, sent to the window list and honoured as requests ([ADR 0013](../../docs/adr/0013-window-workspaces-through-xfce-foreign-toplevel-protocol.md)).
- **Written back:** `_NET_WM_STATE` and `_NET_WM_DESKTOP` on an XWayland window, and the foreign-toplevel states, follow every change, whichever way it came.
- **Struts:** a sticky dock's struts count on every workspace ([64](64-work-areas-from-struts.md) counts them on its own workspace until then).

## Acceptance criteria

- [ ] Policy tests, written before the code: above and below against normal windows, transients and fullscreen windows; sticky across a workspace switch, a `move-to-workspace-*`, the Window switcher's lists and Show Desktop; a sticky dock's struts in every workspace's work area.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the two bindings; the titlebar actions and buttons; X11 `_NET_WM_STATE` and `_NET_WM_DESKTOP` requests and their properties written back; the foreign-toplevel states sent and a request honoured.
- [ ] `toggle-above` and `toggle-on-all-workspaces`, and the `toggle_above` and `toggle_stick` values of the titlebar action keys, have rows in `docs/parity.md` that say honoured, with the tests above; any of them in `UNSUPPORTED` leaves it.
- [ ] The checklist has an item, tagged M8: a window kept above and one made sticky from the Frame, from the binding and from the window list, seen in the pager and on each workspace, and an X11 app (`xterm` with `wmctrl -r :ACTIVE: -b add,above`). Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Notes

- **The window menu's** *Always on Top* and *Always on Visible Workspace* items are [161](161-window-menu.md)'s, on top of this.

## Open questions

**For the developer**, to settle before building:

- **The data model for sticky with a stack per workspace** (ADR-worthy). Options: a sticky window is in every workspace's stack, kept in step; there's one stack for sticky windows, merged into each workspace's at its layer; or the stacks become one with a workspace filter. It touches stacking, focus, the Window switcher's lists and Show Desktop's per-workspace semantics ([94](94-show-desktop.md)), and isn't easily reversed.
- **Whether `mate-panel-caminito`'s window-list patch needs to read `above`**, so that its menu's *Always on Top* shows, or stays as it is with only `sticky` read.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: where a transient of an above window goes; how above and below stack against docks and fullscreen windows; whether a sticky window's place is the same on every workspace.

## Done (2026-10-10)

- **Triage (the open questions, settled without the developer, as this run asked):**
  - *Sticky's data model:* one stack with a workspace filter, Marco's own model ([ADR 0024](../../docs/adr/0024-one-stack-for-every-workspace-so-sticky-windows-have-a-place.md)). Policy keeps every window in one stack, bottom to top; a workspace's stacking is the windows on it, a sticky window being on all. The other two options are rejected there.
  - *`mate-panel-caminito` reading `above`:* left as it is, with only `sticky` (pinned) read. Caminito now sends `above` and `below` in the Xfce state, so a later patch can read them; the window menu's *Always on Top* is 161's.
- **From Marco's source:**
  - *A transient of an above window* goes over it, in the top layer: Marco's `ensure_above` promotes a transient into its parent's layer (stack.c:677-698), which policy already did. A transient kept below still stays over its parent, for the same reason.
  - *Above and below against docks and fullscreen windows:* `get_standalone_layer` (stack.c:272-315). Kept above shares the dock's layer and goes by stack position there; kept below wins over fullscreen, and a dock kept below goes down to the bottom layer; a focused fullscreen window goes over one kept above, an unfocused one stays under it. Making or unmaking above raises the window (window.c:2951-2971); below only changes the layer.
  - *A sticky window's place:* the same on every workspace, geometry and stacking both, as Marco has one of each.
  - *Stick and unstick* take the window's transients along (window.c:4646-4666); here the whole transient family goes, from its root, so a family stays on one workspace or on all. Unsticking puts it on the active workspace (window.c:4615-4624); moving it to a workspace unsticks it (window.c:4515-4554); activating a sticky window stays on the active workspace. Show Desktop hides a sticky window on the workspace that shows its desktop (window.c:1395-1401).
- **Struts:** the work area was already one for every workspace, from every XWayland window's struts (64), so a sticky dock's count everywhere; ADR 0024 keeps it so. The policy test checks a sticky dock's reserved edges bound a maximised window on another workspace. Docks aren't made sticky by default as Marco makes them, which would change 64's docks and wasn't asked.
- **Behaviour that changed:** moving windows between workspaces by a smaller `num-workspaces` or by dynamic workspaces now leaves the stacking order alone, as in Marco, where they went above the windows there (`a_smaller_workspace_count_moves_windows_to_the_new_last_workspace` updated). `Policy::stacking` returns a `Vec`; `Policy::stacking_all` gives the whole stack, which XWayland's stacking order is now made from.
- **The ways in:** `toggle-above` and `toggle-on-all-workspaces` are bound (Marco's defaults, `disabled`; `BINDABLE` grows to 25); the Frame's above, unabove, stick and unstick buttons and the `toggle_above` and `toggle_stick` titlebar actions act, by the frame flags `above` and `stuck`, which are now drawn from policy; X11's `_NET_WM_STATE` and `_NET_WM_DESKTOP` already reached policy and were written back, and now take effect; the Xfce handle sends `state` with sticky, above and below whenever they change (none is sent while there's none to say, so earlier snapshots stand), and honours the six requests.
- **Tested first:** the policy tests in `caminito-policy/src/tests/above_below_and_sticky.rs` were run against the old policy (21 of 22 red; a focused fullscreen window over an above one already held), then the model and layers written to pass them; the invariants proptest now checks the one stack, sticky windows on every workspace, and families sticky together, with `SetBelow` and `SetSticky` ops.
- **Tests run** (under `dbus-run-session`, filtered): `cargo test -p caminito-policy` (330 passed; policy's stack model is what changed, so the whole crate is its tests), the invariants proptest again with 3000 cases; `cargo test -p caminito --lib -- tests::xfce_foreign_toplevel tests::frames tests::bindings input::bindings tests::parity decoration:: tests::settings tests::dynamic_workspaces tests::workspace_moves tests::show_desktop tests::switcher tests::input:: tests::focus_modes tests::debug tests::window_properties` (198 passed; every `::x11` variant and the X11-only `window_properties`, the new X11 test among them, and six switcher tests fail at "Xwayland starts", which is the environment; `debug::the_state_dump_shows_an_ssd_and_a_csd_window_on_two_workspaces::xdg` fails on the bindings 150 added, which its snapshot lacks, before this ticket's change). The full suite wasn't run, as asked for this run.

