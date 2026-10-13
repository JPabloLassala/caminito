# The slide on two outputs

Epic: workspace-slide
Type: implementation
Status: resolved
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

- [x] Headless tests with two outputs of different sizes, key on: a switch on one leaves the other's drawn windows as they were, and nothing of a sliding window is drawn on the other, read part-way.
- [x] Key off: both outputs slide, each by its own size.
- [x] A straddling window is drawn on the output that didn't switch throughout.
- [x] Turning the key each way, plugging an output in and unplugging one give no slide in the debug dump.

## Done (2026-10-13)

- **Most of it was built already.** 233's Scene finds the slide by output, and its refresh pass gives a slide the windows its output is the Main output of. So the clip, the straddling window and the two slides under Spanning workspaces needed no new code, only tests. The one code change is in policy: `dynamic-workspaces` removing the workspace an output shows now notes that output's switch to the first workspace (`caminito-policy/src/policy/dynamic_workspaces.rs`), the way between the two cells of the grid as it was.
- **For the developer, a reading I made: a sliding window that straddles.** A window of the output that switches, with a part on the other output, slides on its own output alone. On the other output its part is at its place from the first frame if it arrives, and isn't drawn if it leaves. So for the 250 ms an arriving one shows at two places, and a leaving one's part goes at once. This is what 233 left and recorded; I read "keeps being drawn on the other at its place" this way, wrote it in the spec's "Outputs", and a test holds it. The other reading is that nothing of such a window shows on the other output until the slide ends. 237 can show which is right.
- **For the developer, a case with no slide: a middle workspace that empties under dynamic workspaces.** The output keeps its number and the next workspace's windows move down into it, as Marco has it. Its contents change with a cut, since its active workspace doesn't change. An output whose number changes with its windows shows what it did, and doesn't slide either. Both are in the spec's "Outputs" now.
- **A straddling window of the other output** (its Main output didn't switch) stays at its place on both outputs, and is none of the slide's windows, moved or still.
- **Tests** (`caminito/src/tests/workspace_slide.rs`, two outputs of 800 × 600 and 1024 × 768, the clock stopped): a switch of the left output with the arriving window drawn over the right output's area, and a switch back with the leaving one there, each read from the right output's pixels; a straddling window of the other output at the start, part-way and at the end; a straddling window of the output that switches, arriving and leaving; the key off, right and down, with offsets of each output's own width and height at the start and an eighth of them half-way; the key each way, an output plugged in, and one unplugged from another workspace, with no slide in the dump and no deadline; and the removed workspace. Policy's three are in `caminito-policy/src/tests/dynamic_workspaces.rs`.
- **What was seen to fail first:** policy's two tests of the removed workspace's switch. The compositor's tests passed on 233's code as written. To check that they can fail, I made the Scene draw the first output's slide on every output: the three tests that read the other output's pixels failed, and I put the Scene back.
- **Not tested:** under Spanning workspaces, that one output draws nothing of the other's sliding windows (the windows are one colour, and both outputs' slide in the same band); a retarget on two outputs; a real Session, which is 237's.
- **What was run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, 86 of `caminito-policy`'s tests (`dynamic_workspaces`, `per_output_workspaces`, `grid`, `workspaces`, and `invariants` and `scene_changed` before the review's fixes) and 98 of `caminito`'s (`workspace_slide`, `dynamic_workspaces`, `per_output_workspaces`, `ext_workspace`, `sounds`, `output_lifecycle`; `workspace_moves` and `workspace_popup` before the review's fixes). All passed. **Not run:** the full suite, at the developer's standing word.
- **Code review:** two reviews, of the standards and of the spec, found no bug. Fixed from them: the way between two cells is one function of the grid (`Grid::cells_between`), the unplugged output shows another workspace than the one that takes its windows, and the leaving straddling window has its test. Left: the tests put windows and switch through policy and the debug interface, as the file's older tests do, and the rule for a removed workspace is written two times in `update_dynamic_workspaces`.
