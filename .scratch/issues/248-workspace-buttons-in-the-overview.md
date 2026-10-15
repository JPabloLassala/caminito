# Workspace buttons in the Overview, and its selection on the Active output

Epic: overview
Type: implementation
Status: ready-for-agent
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

- [ ] Policy tests: a click on another output's cell makes that output the Active output and the selection's output, with nothing selected, and the focus unchanged; a later workspace binding switches that output.
- [ ] Headless tests, with `switch-to-workspace-left` and `-right` set to `Button8` and `Button9`: a press of each over a window, over a cell and over the empty area switches the Active output's workspace and leaves the Overview open, with no Workspace popup; the window under the pointer isn't chosen or closed.
- [ ] Headless tests with two outputs, under Per-output workspaces and under Spanning workspaces: after a click on the other output's cell, a workspace key and a workspace button switch that output.
- [ ] A button bound to the Overview still closes it, and a bound button that switches no workspace still does nothing over a window and closes the Overview over the empty area.
- [ ] A break script shows each new test can fail.
- [ ] The tests of 241 to 246 pass unchanged.
- [ ] The spec's "Outputs", "The pointer" and "The keys" say what was built.
