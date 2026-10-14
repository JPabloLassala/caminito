# The Overview on two outputs

Epic: overview
Type: implementation
Status: resolved
Blocked by: 244, 245
Spec: [Overview](../overview/spec.md)

## What to build

The Overview on every output at once, and a window dragged from one to another, under "Outputs" and "The pointer" in [the spec](../overview/spec.md), checked under Per-output workspaces and under Spanning workspaces (ADR 0032). It builds on tickets 244 and 245.

- **It opens on every output at once.** Each shows its own active workspace's windows, the ones it is the Main output of, and its own bar. Each output's panel and caja's desktop fade, and each wallpaper is dimmed.
- **Under Per-output workspaces** a click on a cell switches that output alone. **Under Spanning workspaces** every output's bar shows the same workspaces, and a switch in one switches all.
- **The workspace bindings** switch the Active output's workspace.
- **The arrow keys** move the selection within the output that has it. The selection starts on the focused window's output, or on the Active output with none.
- **A drag onto another output:**
  - on its layout: the window goes to that output's active workspace;
  - on a cell of its bar: the window goes to that workspace of that output.

  Either way it is at the same place relative to the work area, shrunk to fit if it must be. A maximised or tiled window keeps that state there. The Overview stays open.
- **An output that comes or goes** while the Overview is open: it stays open, and each output's layout is made again from the windows it now has.
- The transition of 243 runs on every output at the same time.
- The debug dump shows each output's Overview.

## Acceptance criteria

- [x] Headless tests with two outputs, under Per-output workspaces and under Spanning workspaces: each output's listed windows, layout and bar, read from the dump.
- [x] A click on a cell switches one output under Per-output workspaces and both under Spanning workspaces.
- [x] A window dragged onto the other output's layout, and one onto a cell of its bar, is on that output and that workspace, at the same place relative to the work area.
- [x] A window larger than the other output's work area is shrunk to fit, and a maximised and a tiled window keep their state there.
- [x] A window that straddles two outputs is in the layout of its Main output alone.
- [x] An output removed while the Overview is open leaves it open, with the other output's layout holding the windows it took.
- [x] The tests of 241 to 245 pass unchanged.

## Done (2026-10-14)

- **Policy lists every output** (`Overview::others`, `OverviewOutput`). The fields `output`, `workspace` and `windows` stay, for the output the selection is on, so that the tests of 241 to 245 are unchanged. `outputs()` gives every output's part. That output is the focused window's (`output_showing`) at opening, or the seat's Active output with none focused. The arrow keys move the selection in its layout alone. When it goes, the output opening would choose takes its place. My choice: the ticket doesn't say.
- **The old calls stay as wrappers**: `overview_switch`, `overview_pointer` and `overview_windows_on` act on the selection's output, or on the dragged window's own. The compositor calls `overview_switch_on`, `overview_pointer_on` and `overview_windows_of`. Only tests call the wrappers.
- **A drop on another output** (`OverviewDrag::onto`): `move_to_output` moves the window's whole transient family by the difference of the two work areas, shrinks each to fit the new work area, and then moves it inside as `move-to-monitor-*` does. `move_to_monitor` and it share `move_between_outputs`, so a maximised, tiled or fullscreen window is so on the new output. Then `apply`: `follow_main_outputs` brings the window to that output's active workspace (Per-output workspaces). On a cell that isn't the active workspace, `move_to_workspace` follows, so a sticky window stops being sticky there, as in 245. On the layout or on the active cell, a sticky window stays sticky. Anywhere on the other output that isn't a cell counts as its layout. A dialog of the family that was already on the other output is moved by the same offset, so its place there is only approximate (from the review).
- **The compositor has one laid-out Overview for each output** (`Caminito::overview` is a `Vec`). Each has its own bar, layouts, element ids and reduced rate of frame callbacks. The transition is one: each output gets a copy of the first's, and an output that comes while it runs takes it from where it is. The dragged window is drawn on each output it's over, cut at that output's edge. The cell under the pointer is marked on the output the pointer is on.
- **The Workspace slide** is under the Overview whenever it's open, on any output: opening it cuts every slide, and closing it ends the slides that started under it. The removed doc line ("a cut until its bar has the workspaces") was out of date since 244.
- **An output that goes** leaves the Overview open while one is left. Its windows go to the primary output with their workspace numbers, as before (225), so they're in its layout when it shows the same number. The test has both outputs on their first workspace.
- **The dump**: `overview` keeps its fields for the selection's output (flattened), and gets `outputs`, each output's part. `drag` gets `onto`; its `rect` is from the selection's output's corner. `overview_transition` gets `outputs`, the outputs it runs on.
- **Tests**: policy's 15 new tests and the proptest's 2 new ops and wider invariant came after the code. The headless tests (`tests/overview_outputs.rs`, 18 with both kinds) too. To check that they can fail, `246-break.sh` breaks the drawing of the dragged window on the other output, the output policy hears during a drag, the layout of the second output and the click on the other output's cell. Each break failed its tests (2, 7, 15 and 1). The shrink and the maximised and tiled windows are read from the xdg client's last configure, since the test client takes no size by itself.
- **What ran:** `caminito-policy`'s whole suite (751), and `caminito`'s tests filtered on `overview`, `workspace_slide`, `deadlines`, `debug`, `state_dump`, `scene`, `switcher`, `per_output`, `move_to`, `lock`, `grabs`, `drag_icons`, `input`, `xfce_foreign_toplevel`, `ext_workspace`, `frames`, `workspace_popup`, `window_menu`, `button_bindings` and `modifier_click` (878, 5 ignored), with fmt and clippy at `-D warnings`. Not the full suite, by the standing rule. Not run in a real Session: that's 247's.
- **From the review, left as it is:** the lookup of an output by its id repeats in `debug.rs`; `outputs()` and `on()` clone the lists; `lay_out_overview` returns a bare `bool` for "fresh".
