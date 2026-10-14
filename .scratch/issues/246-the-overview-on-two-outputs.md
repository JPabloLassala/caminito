# The Overview on two outputs

Epic: overview
Type: implementation
Status: ready-for-agent
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

- [ ] Headless tests with two outputs, under Per-output workspaces and under Spanning workspaces: each output's listed windows, layout and bar, read from the dump.
- [ ] A click on a cell switches one output under Per-output workspaces and both under Spanning workspaces.
- [ ] A window dragged onto the other output's layout, and one onto a cell of its bar, is on that output and that workspace, at the same place relative to the work area.
- [ ] A window larger than the other output's work area is shrunk to fit, and a maximised and a tiled window keep their state there.
- [ ] A window that straddles two outputs is in the layout of its Main output alone.
- [ ] An output removed while the Overview is open leaves it open, with the other output's layout holding the windows it took.
- [ ] The tests of 241 to 245 pass unchanged.
