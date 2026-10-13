# A workspace switch slides

Epic: workspace-slide
Type: implementation
Status: resolved
Blocked by: 230
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

The first slice, and the one a user can try: the **Workspace slide** (CONTEXT.md) on one output, for every direction, under "Direction and distance", "Which switches slide", "Timing", "Policy and input" and "Settings" in [the spec](../workspace-slide/spec.md). The windows that stay still are ticket 234, a second switch during a slide is 235, and two outputs are 236.

- **On a change of the output's active workspace**, whatever its cause, the old workspace's windows leave and the new one's arrive, with their Frames, shadows and popups. Layer surfaces and the wallpaper stay still.
- **The line is the Workspace grid's**: one output's width for each column between the two cells and one output's height for each row. A jump is one straight line, and no workspace between is drawn. A wrap under `wrap-style` slides one cell in the direction of the key. There is no gap.
- **250 ms, ease-out**, from Caminito's clock, drawn on every frame of the output, with a deadline as the minimise animation has (`caminito/src/deadlines.rs`). Choose the curve and record it in the spec's "Not decided".
- **Policy switches at once.** Focus, input, ext-workspace and X11 are as they are with no slide, and a click part-way reaches the new workspace's window at its final place.
- **The Workspace popup** is drawn over the sliding windows.
- **`workspace-slide`** in `org.mate.Caminito`, a boolean, default `true`, in both copies of the schema. The slide runs when it is on and the minimise animation's three keys allow (`settings/translate.rs`). A change applies to the next switch.
- A second switch during a slide ends the running one at once and starts its own, until 235.
- The debug dump shows the slide: its output, its two workspaces, its offset, and each sliding window's drawn rectangle.

## Acceptance criteria

- [x] Headless tests that advance the clock, as `caminito/src/tests/minimise_animation.rs` does: a switch right, left, up and down in a grid with rows, read part-way and at the end; a jump across two columns and a row; a wrap.
- [x] Part-way through, the old workspace's windows and the new one's are both drawn, the panel hasn't moved, and after 250 ms only the new one's are drawn, at their own places.
- [x] A switch from the pager's ext-workspace request and one from activating a window that isn't showing both slide.
- [x] A key press part-way reaches the new workspace's focused window, and a click reaches the window at its final place.
- [x] With `workspace-slide` off, and with each of the three other keys against it, a switch is a cut.
- [x] The parity tests pass unchanged, with the key off where they read a frame after a switch.

## Done (2026-10-13)

- **How it's built:** policy notes each output's switch (`Policy::take_output_switches`), with the way it goes in the Workspace grid: the cells between the two workspaces, or one cell the way of the binding, which a wrap keeps to. Turning `workspaces-per-output` on or off, an output that comes, and a change of the workspaces' count note none. The refresh pass starts a slide from them (`caminito/src/workspace_slide.rs`), after the last step that changes policy, so no frame shows the switch before its slide. The Scene draws the windows that leave and those that arrive away from their places, on the output that switched alone, each in its place in the one stack (`stacking_all`, a new Scene query). The output is drawn again after each of its frames while a slide runs, and `deadlines` ends the slide at 250 ms.
- **Which windows slide:** those that leave are the old workspace's windows that showed at the refresh pass before the switch and don't now; those that arrive are the new workspace's that show now and didn't. A window that shows before and after is left in its place. So a sticky window and the window a `move-to-workspace-*` binding carries already stay still, which is part of 234; 234 still has the dragged window, the Window menu's moves, the tests and the dump's list of the windows a slide leaves.
- **The curve:** a cubic ease-out, `(1 − t)³` of the way left at the part `t` of the 250 ms. It's in the spec under "Timing", and "Not decided" now has nothing. The feel in a Session is 237's to check.
- **The fixture keeps `workspace-slide` off** unless a test sets it, as it keeps `workspaces-per-output` and `center-new-windows` off. The tests written before this read a frame straight after a switch (`a_switch_of_workspace_a_pager_asks_for_is_drawn` is one), and Caminito's clock runs in real time in a test, so each would wait 250 ms or read a frame part-way. This is wider than the criterion's "where they read a frame after a switch": only `tests::workspace_slide` runs with the shipped default.
- **Two switches in one refresh pass** are one slide, from the workspace shown at the pass before to the last, with the cells added up. A wrap and then a jump in one pass so give a line that isn't the grid's between the two. 235 changes this code for the retarget.
- **Not done, on purpose:** an X11 override-redirect window (a menu open during the switch) stays still, since it's on no workspace in the Scene. Dynamic workspaces removing a workspace and a lower `num-workspaces` give no slide: the first is 236's, and the second isn't in the spec's list. The checklist's items are 237's. On a second output a window of the sliding output is drawn at its place with no offset, and one that left isn't drawn; the tests of it are 236's.
- **Tests:** written with the code, not before it, so none was seen to fail first, but for one whose click spot was wrong. `caminito/src/tests/workspace_slide.rs` moves the clock on and reads the dump and the pixels: each way round a grid of two rows and the pixels part-way, on both kinds of window; a jump, a wrap, the pager's request, an activation, a key and a click part-way, the four keys, the key turned off while Caminito runs, the Workspace popup's layer and a second switch, on a Wayland client. The curve's and the merge's arithmetic have unit tests beside the code, and policy's notes have theirs in `caminito-policy/src/tests/per_output_workspaces.rs`. The part-way reads allow about 60 ms of a slow run between the switch and the frame.
- **What was run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, 187 of `caminito-policy`'s tests (invariants, scene_changed, the workspace modules, move_to, window_menu, grid) and 724 of `caminito`'s: `tests::parity`, `workspace_slide`, `workspace_moves`, `workspace_popup`, `ext_workspace`, `per_output_workspaces`, `dynamic_workspaces`, `minimise_animation`, `debug`, `settings`, `show_desktop`, `frames`, `switcher`, `layer_shell`, `shadows`, `image_capture`, `tiling`, `window_menu`, `foreign_toplevel`, `sounds`, `bindings`, `move_to`, `lock`, and the unit tests of `settings`, `deadlines`, `minimize_animation` and `workspace_slide`. All passed. **Not run:** the full suite and the other steps of `tools/check`, at the developer's standing word. After the review's fix only the slide's tests and policy's per-output ones ran again.
- **Code review:** two reviews, of the standards and of the spec, found nothing that blocks. Fixed from them: a window that maps or is restored in the pass of a switch no longer slides in. Left: the test helpers `pixel`, `clear` and `middle` are copied once more, and the set of shown windows is kept once more beside the minimise animation's.
