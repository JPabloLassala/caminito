# The slide on two outputs

Epic: workspace-slide
Type: implementation
Status: ready-for-agent
Blocked by: 233, 231, 225
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

"Outputs" and "Which switches slide" in [the spec](../workspace-slide/spec.md), under Per-output workspaces and under Spanning workspaces (ADR 0032).

- **A sliding window is clipped to the output that is switching.** It never shows on the other during the slide.
- **A window that straddles two outputs** keeps being drawn at its place on the output that isn't switching.
- **Under Spanning workspaces every output slides its own windows**, at the same time, each by its own width and height.
- **No slide** when `workspaces-per-output` is turned on or off, when an output appears, or when an output takes another's windows as it goes.
- Dynamic workspaces removing a workspace slides the outputs whose active workspace changes.

## Acceptance criteria

- [ ] Headless tests with two outputs of different sizes, key on: a switch on one leaves the other's drawn windows as they were, and nothing of a sliding window is drawn on the other, read part-way.
- [ ] Key off: both outputs slide, each by its own size.
- [ ] A straddling window is drawn on the output that didn't switch throughout.
- [ ] Turning the key each way, plugging an output in and unplugging one give no slide in the debug dump.
