# Windows that stay still through a slide

Epic: workspace-slide
Type: implementation
Status: open: the code is in, and the Window menu's carried window is the developer's to decide, since a Window menu move doesn't switch the workspace
Blocked by: 233
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

"What slides" in [the spec](../workspace-slide/spec.md): the windows that are the same on both workspaces don't move.

- **A sticky window** stays still, and the others slide beneath or over it as the stack has them.
- **The window carried by a `move-to-workspace-*` binding** or by the Window menu's workspace moves stays still while the rest slide.
- **A window being dragged** at the time of the switch stays under the pointer.
- **A focused fullscreen window** slides like any other, in the place the Scene gives it, so the panel shows as it leaves.

## Acceptance criteria

- [ ] Headless tests, read part-way (all but the Window menu's, below): a sticky window's drawn rectangle is its own while another window's is offset; the same for a carried window, by binding and by the Window menu; the same for a window in an interactive move.
- [x] A fullscreen window's drawn rectangle is offset part-way, and the panel is drawn.
- [x] The debug dump says which windows a slide moves and which it leaves.

## Done (2026-10-13)

- **Not met, and the developer's to decide: the Window menu's carried window.** A Window menu move (Move to Workspace Left, Right, Up, Down, and the submenu's workspaces) changes the window's workspace and leaves the active one, as Marco's does (window.c:7063-7071; `caminito/src/tests/window_menu.rs` has "the active workspace stays"). `move-to-workspace-N` is the same. So no output switches, no slide starts, and there is nothing to carry or to test. The spec's "Which switches slide" lists "the Window menu's moves", which has the same fault. Two ways out: strike the Window menu from this ticket and from the spec, or make the menu's moves switch to the window's new workspace, which leaves Marco and needs a line in `docs/parity.md`. Nothing was changed for either.
- **What 233 had already:** a sticky window and the window a `move-to-workspace-left`, `-right`, `-up` or `-down` binding carries show before and after the switch, so the slide leaves them. This ticket adds their tests and no code.
- **A window being moved goes with the switch.** Before, a switch during a move hid the window and ended the move. Now policy's `switch_shown` takes the window to the new workspace with its transient family, and the move goes on, as Marco's `meta_workspace_activate_with_focus` does (workspace.c:416-456, read in the OrbStack machine's copy). It applies to every cause of a switch and with `workspace-slide` off too, and it is in `docs/parity.md`'s `switch-to-workspace-left` row. A sticky window stays sticky. A window being resized stays on its workspace and its resize ends, as before. Under Per-output workspaces the family goes if its root's Main output is the one that switches (ADR 0032); only the debug call switches one output today, so no test of two outputs has it.
- **A fullscreen window needed no code.** As it leaves it has lost the focus, so it's in the windows' layer under the panel, and the panel is drawn over it at once. As it arrives it's focused and comes in over the panel, which shows where the window isn't yet. The test reads both.
- **The dump:** each slide has `still`, the ids of the windows on its output that it leaves in their places, beside `windows`, those it moves.
- **Tests:** in `caminito/src/tests/workspace_slide.rs`, read part-way from the dump and from the pixels, on a Wayland client only: a sticky window, one a binding carries, one being dragged (the drag goes on part-way and ends in its place), and a fullscreen one each way. The three first put the still window clear of the band where the others slide, so none reads the stack order of a window that slides over or under one that stays; that order is 233's code. Policy's are in `caminito-policy/src/tests/workspace_moves.rs`. The two tests of the dragged window were seen to fail without the policy change; the others passed when written.
- **Not tested:** a keyboard move and a held move of a maximised window through a switch, a dialog dragged apart from its parent, and two outputs.
- **What was run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, 253 of `caminito-policy`'s tests (invariants, scene_changed, the workspace modules, move_to, window_menu, the grabs, edge_resistance, tiling, interaction, per_output) and 149 of `caminito`'s (`tests::workspace_slide`, `grabs`, `workspace_moves`, `workspace_popup`, `debug`, `tiling`, `edge_resistance`, `per_output_workspaces`, `window_menu`, `move_to`, `parity`, `keyboard_grab`, and the slide's unit tests). All passed. After the review's fixes a smaller set ran again: 151 of policy's and 51 of `caminito`'s. **Not run:** the full suite, at the developer's standing word.
- **Code review:** two reviews, of the standards and of the spec, found nothing that blocks. Fixed from them: the carry reads the root's output, `still_windows` takes no time, the policy test is two, and small things in the tests and in CONTEXT.md.

