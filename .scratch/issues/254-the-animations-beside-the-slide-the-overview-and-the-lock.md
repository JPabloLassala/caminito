# The animations beside the slide, the Overview and the lock

Epic: open-close-animation
Type: implementation
Status: ready-for-agent
Blocked by: 252
Spec: [Opening and Closing animations](../open-close-animation/spec.md)

## What to build

"The other effects" in [the spec](../open-close-animation/spec.md).

- **Workspace slide:** a window that opens on the workspace arriving, or closes on the one leaving, moves with its workspace and zooms and fades as well. The closing window's picture belongs to its workspace and slides with it.
- **Overview:** neither animation runs while the Overview is open or during its transition. A picture still running when the Overview opens is dropped. A window opening or closing in the Overview joins or leaves its layout as now.
- **The session lock** ends every running animation at once, and none starts under it. **An output that goes** drops the pictures on it. As the slide has them (235).
- Under Per-output workspaces and under Spanning workspaces (ADR 0032), with one output and with two.
- [checklist.md](../../docs/testing/checklist.md) gets an item for the animations in a real Session, the developer's to run.

## Acceptance criteria

- [ ] Headless tests that advance the clock: a window opening on the arriving workspace and one closing on the leaving one during a slide, drawn moved and scaled part-way.
- [ ] A window opening and one closing with the Overview open, and during its transition, are not animated; a picture running when it opens is gone in the next frame.
- [ ] Locking part-way ends both kinds at once; a window that opens under the lock isn't animated after it.
- [ ] An output removed part-way drops the pictures on it.
- [ ] The tests run with two outputs under both workspace modes.
- [ ] The checklist has its item.
