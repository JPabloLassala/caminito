# A window shows by its Main output's workspace

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 221
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The second of three slices: "The key" and "The model" in [the spec](../per-output-workspaces/spec.md). After it the outputs' active workspaces can differ, and what is drawn and hit follows. Only a test, or the debug interface, can make them differ yet: the bindings are ticket 231.

- **The key** `workspaces-per-output` in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml`), default `true`, translated into policy's config. Off, every switch sets every output, as ticket 221 left it.
- **A switch of one output's workspace** in policy, which leaves the others alone when the key is on. The existing switch paths call it for every output until ticket 231 gives them one.
- **A window is showing** by its Main output's active workspace. The scene, hit-testing, the surfaces' output enter and leave, Xwayland's stacking and the redraw trigger go by the windows showing on every output.
- **A window that straddles two outputs** is drawn on both while it's showing.
- **A window dragged across** takes the other output's active workspace when its Main output changes, so it stays in view.
- **A change of the key applies at once**, with no window's number changed: turned on, every output keeps the workspace that was active; turned off, every output takes one workspace. Until ticket 231 there is no Active output, so the Primary output's is the one taken.

Sticky windows, families, moves and activation keep their one-output reading (the window's own output) and are finished by ticket 223.

## Acceptance criteria

- [x] Policy unit tests with two outputs and the key on: a switch of one output leaves the other's active workspace and its showing windows alone; a window's showing follows its Main output; a drag across joins the other output's active workspace when most of the window has crossed, and not before.
- [x] The same with the key off: both outputs switch together, and ticket 221's invariant holds whenever the key is off.
- [x] The property tests' other invariants hold with the key on.
- [x] A headless test with two outputs on different workspaces: each output's scene has its own workspace's windows, a straddling window is drawn on both, and input reaches only windows that are showing.
- [x] Turning the key off and on in a running compositor does what is said above.
- [x] With one output, nothing differs between the key on and off.
- [x] Every existing test passes with the key off.

## For the implementing agent to settle

Recorded in the Done section:

- how the debug interface switches one output, for the headless tests and for trying it by hand before ticket 231.

## Done (2026-10-12)

- **Tested first.** Policy's `per_output_workspaces.rs` tests were written against the new API before the code: one output's switch, showing by Main output, the drag across (most of the window crossed, and not before), the key off and on, and one output with the key either way. With `follow_main_outputs` disabled, the two drag tests failed.
- **The key.** `workspaces-per-output` in `org.mate.Caminito`, default `true`, is read as `Config::workspaces_per_output`. `Config::default()` and the headless fixture keep it off unless a test sets it, as with `center-new-windows`, so every test written before it runs Spanning workspaces.
- **The switch of one output.** `Policy::switch_output_workspace(output, workspace)` switches that output alone with the key on, and every output with it off. `switch_workspace` and every existing path switch every output, as ticket 231 expects. Switching one output closes the switcher and the Window menu, and sounds from that output's workspace. Seats whose focused window is on that output get its default focus, or none. Other seats keep theirs, so focus never moves to another output's window.
- **The Primary output stands in for the Active output.** `everywhere()` is gone. `primary_shown()` gives the bindings' neighbour, `switch-to-workspace-prev`, the count's clamp and the dynamic renumbering their from, and `active_workspace()` (the tests' query) is the primary's. With the key off they are all the same.
- **The drag across.** `Managed::main_output` is a memo of the window's Main output, not its output. At the top of `apply`, `follow_main_outputs` sees a change and moves the window's family to the new output's active workspace, if the window was showing on the old output. `set_outputs` refreshes the memo first, so an output coming, going or moving isn't a drag; what happens to the windows then is ticket 225's. Only a root's crossing counts, and a sticky window is skipped. A transient dragged across alone keeps its family's number and shows by its own output, so it can hide. Families are ticket 223's. Any change of Main output counts, not only a pointer drag: a client's move, tiling, or `move-to-monitor-*`, as the spec asks of the latter. A new window doesn't follow: it opens on the primary output's workspace wherever it's placed, which is ticket 222's.
- **The key's change.** Turned off, `span_workspaces` gives every output, and the entry an added output takes, the primary output's active and previous workspace. It closes the switcher and Window menu if anything changed, and `apply` refocuses. Turned on, nothing changes.
- **The debug interface.** `org.mate.Caminito.Debug.SwitchOutputWorkspace(output, workspace)` takes the connector name (as `wlr-randr` gives it) and a workspace numbered from 0, and returns whether both exist. It calls `switch_output_workspace`, so with the key off it switches every output. Try it by hand with `busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug SwitchOutputWorkspace su HDMI-A-1 1`.
- **The compositor needed no change of its own.** The scene, hit-testing, the surfaces' output enter and leave, Xwayland's stacking and the redraw trigger already read `active_stacking`/`is_showing`, which go by the Main output since 221. A straddling window is drawn clipped on every output it overlaps while it's showing. The headless test checks it with pixels, without forcing a frame, so the redraw trigger is covered too.
- **Property tests.** Two new ops: one output's switch and the key's toggle. The settings ops keep the key, and the proptest runs once starting off and once starting on. The every-output-equal invariant holds whenever the key is off. They passed at 2000 cases each.
- **Run in an Ubuntu 24.04 cloud container**, Rust 1.97. The build's dependencies came from apt (`libgtk-session-lock-dev` isn't packaged there). `/tmp/.X11-unix` had to be created for the X11 tier. Results:
  - fmt is clean.
  - clippy is clean but for three `manual_range_contains` errors in `caminito-theme`, which this toolchain's clippy is newer than, and which this ticket doesn't touch.
  - policy's 574 tests and caminito's 1847 tests pass. One run of caminito had `locker::a_killed_locker_comes_back…` fail, and it passed three times alone and in the next full run.
  - `caminito-theme`'s goldens fail on pixels (fonts), as in 221.
  - `tools/check`'s cargo-deny, meson, locker, wallpaperd and extension steps weren't run.
