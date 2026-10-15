# Workspace buttons in the Overview, and its selection on the Active output

Epic: overview
Type: implementation
Status: resolved
Blocked by: 246
Spec: [Overview](../overview/spec.md)

## Why

The developer's `switch-to-workspace-left` and `-right` are `Button8` and `Button9`, the mouse's back and forward buttons. In the Overview they do nothing over a window or a cell of the bar, and close it over the empty area. Ticket 244 let the workspace bindings through only when they are keys: `overview_key_pressed` returns a binding that `switches_workspace()` as `Bound::Action`, but `overview_button` takes every press first and never asks. A System shortcut can be a mouse button (CONTEXT.md), so the spec's "the workspace bindings switch the Active output's workspace" already promises this.

Decided in a grilling session with the developer on 2026-10-14.

## What to build

Under "The pointer" and "The keys" in [the spec](../overview/spec.md).

- **A button bound to a workspace switch** (`switch-to-workspace-left`, `-right`, `-up`, `-down`, `-prev` and `-N`) does in the Overview what its key does, wherever the pointer is: over a window, a cell of the bar or the empty area. It switches the Active output's workspace, with the Workspace slide and without the Workspace popup. The Overview stays open. Its release is swallowed.
  - A drag in progress ends as it does for a key (245).
  - The workspace that output gets is the one the binding names, not the one under the pointer.
- **While the Overview is open, the output its selection is on is the Active output.**
  - At opening they are the same already (`overview_output`).
  - A left click on a cell of another output's bar switches that output to the cell's workspace, as now, and makes that output the seat's Active output, as a press on its wallpaper does (`press_on_output`, 222). The selection goes to that output with nothing selected, so the first arrow key selects its top-left window. The focus is left alone.
  - A click on a cell of the selection's own output's bar is as now.
- **Unchanged:**
  - The arrow keys move the selection within its output.
  - A press on the empty area closes the Overview and makes no output Active.
  - A press of a button bound to the Overview closes it.
  - Every other bound or grabbed button is swallowed over a window or a cell, and closes the Overview over the empty area, as any press there does.
  - `move-to-workspace-*`, keys or buttons, stays swallowed.

## Acceptance criteria

- [x] Policy tests: a click on another output's cell makes that output the Active output and the selection's output, with nothing selected, and the focus unchanged; a later workspace binding switches that output.
- [x] Headless tests, with `switch-to-workspace-left` and `-right` set to `Button8` and `Button9`: a press of each over a window, over a cell and over the empty area switches the Active output's workspace and leaves the Overview open, with no Workspace popup; the window under the pointer isn't chosen or closed.
- [x] Headless tests with two outputs, under Per-output workspaces and under Spanning workspaces: after a click on the other output's cell, a workspace key and a workspace button switch that output.
- [x] A button bound to the Overview still closes it, and a bound button that switches no workspace still does nothing over a window and closes the Overview over the empty area.
- [x] A break script shows each new test can fail.
- [x] The tests of 241 to 246 pass unchanged, but for one assertion this ticket overturns (see Done).
- [x] The spec's "Outputs", "The pointer" and "The keys" say what was built.

## Done (2026-10-15)

- **Tests first**: the 6 policy tests and the headless tests (`caminito/src/tests/overview_buttons.rs`) were written before the code and failed against it, but for the two that keep what was there (the button bound to the Overview, and another bound button). `248-break.sh` shows that each test can fail: four breaks of the new code, one of the click on the selection's own output's cell, and two of the code that was there before, which those two tests keep.
- **A workspace button** is run in `overview_button` before any other arm but the Overview's own shortcut: wherever the pointer is, a binding that `switches_workspace()` goes through `run_action`, as the key's `Bound::Action` does. So the Workspace popup doesn't show under the Overview's Interaction, and a drag whose window the switch takes away ends, as with a key (245). The press is already in `suppressed_buttons`, so the release is swallowed.
- **A click on another output's cell** (`overview_switch_on`) switches first, then makes that output the seat's Active output and moves the selection there with nothing selected (`overview_select_output`). The order matters: a switch gives the default focus to the seats whose Active output it is, so making the output Active first would move the focus. Under Spanning workspaces the switch of every output gives the default focus anyway, as it always has. The spec says so now.
- **Opening makes the selection's output the Active output.** The ticket says they are the same at opening already. They aren't after a press on another output's wallpaper with the focus left where it was: the selection goes to the focused window's output, and the bindings would then have switched the other one. The Active output stays changed after the Overview closes. The spec's "Outputs" says so.
- **One assertion of 246 changed**: `a_click_on_a_cell_switches_that_output_alone_with_workspaces_per_output` expected the selection to stay on the other output's focused window after a click on this one's cell. This ticket says it goes there with nothing selected, so the assertion is now `None`. Every other test of 241 to 246 is unchanged.
- **Left open**: the selection follows the Active output only at opening and on a click on a cell. When something else moves the Active output while the Overview is open, the selection stays where it was: the focused window dropped on the other output (246), or a client's activation. Keeping them together needs `refresh_overview` to run after `note_active_outputs` in `apply`, which is wider than this ticket. It wants a ticket of its own if it matters in a Session.
- **Dropped from review**: a test that no client sees a workspace button's press. No client has the pointer while the Overview is open, so no break made it fail.
- **What ran:** the whole workspace suite, in a cloud container rather than the OrbStack machine. Everything passed (caminito's 2095, policy's 757) but `caminito-theme`'s `goldens`. Those are pixel comparisons with Marco's frames, and 342 of 680 differ by a few pixels under that container's cairo and pango. This ticket doesn't touch that crate. fmt is clean, and clippy is clean at `-D warnings` on `caminito` and `caminito-policy`. `caminito-theme` has three `manual_range_contains` errors from that container's newer clippy, which are older than this ticket. Not run in a real Session: that's 247's.
