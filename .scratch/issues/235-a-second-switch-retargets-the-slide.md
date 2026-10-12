# A second switch retargets the slide

Epic: workspace-slide
Type: implementation
Status: ready-for-agent
Blocked by: 233
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

"Timing" and "Policy and input" in [the spec](../workspace-slide/spec.md): a slide that is cut short or sent elsewhere.

- **A second switch during a slide retargets it** from where the windows are, so the movement is continuous. The workspaces passed over are drawn as they go by, each at its cell's place.
- **A switch back** to the workspace the slide left returns from where it is.
- **The session lock ends a slide at once**, as does the output being disabled.
- A change of `workspace-slide` or of the three other keys during a slide lets the running one finish.

## Acceptance criteria

- [ ] A headless test: three switches right, each made part-way through the last, give offsets that never jump, and three workspaces' windows are drawn in turn.
- [ ] A switch right and back part-way returns the first workspace's windows to their places without a jump.
- [ ] Locking part-way leaves no slide in the debug dump, and the unlocked output shows the new workspace at rest.
