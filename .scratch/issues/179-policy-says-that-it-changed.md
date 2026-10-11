# Policy says when what the Scene reads changed, and the refresh pass queues the redraw

Epic: caminito-v0.1
Type: implementation
Status: resolved
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

A handler that changes Policy has an unwritten second step: it calls `queue_redraw_all()` after, or the change isn't drawn until something else asks for a frame. There are 71 `queue_redraw_all()` calls in 28 files of `caminito/src`, and a good part of them follow a Policy mutation for no other reason (`window/lifecycle.rs`, `input/frame.rs`, `handlers/foreign_toplevel.rs`, `handlers/ext_workspace.rs`, `xwayland/xwm.rs`, `dbus/caminito.rs`, among others).

One caller doesn't take the step. `deadlines` runs `policy.elapse(now)` for auto-raise (153), which raises a window and queues nothing. The window has the focus already, so `apply()` sends it nothing and `refresh_frames` sees no change. By reading, a raise on a pointer that doesn't move is drawn only at the next redraw from another cause. `tests::focus_modes`' auto-raise test asserts `policy.stacking`, not pixels, so it can't see this. It wasn't run.

`CODING_STANDARDS.md` says "Handlers change policy state and mark it dirty", so the manual mark is the written rule today.

This was decided in an architecture review on 2026-10-11.

## What to build

- **A flag in Policy.** Policy sets it wherever something the Scene reads from it changes: the stacking, a window's geometry or state, which windows show, the tile preview, the active workspace, and whatever else `scene.rs` reads. `apply()` isn't a complete place for it: the tile preview is Policy's state that no window is told of. Setting it when nothing changed is allowed, and costs one redraw.
- **The refresh pass takes it.** One step in `State::refresh_and_flush_clients` takes the flag and, if it was set, queues every output, as the handlers do today. It comes after the last step that changes Policy and before `redraw_queued_outputs`.
- **An invariant proves the flag.** `verify_invariants`, or `check_ops` around it (`caminito-policy/src/tests/invariants.rs`), checks after each random operation that a difference in what the Scene reads comes with the flag set. A mutation that forgets the flag then fails the proptest. A set flag with no difference isn't a failure.
- **Handlers lose the call.** A `queue_redraw_all()` that follows a Policy mutation, and is there for it, goes. The calls for what Policy doesn't hold stay: the lock, idle's blank, the bell, Layer surfaces, the cursor, a selection, a client's commit, an output's change.
- **`CODING_STANDARDS.md`'s line changes** to say: handlers change policy state; the refresh pass queues the redraw for a Policy change and updates the outward views; a handler queues a redraw only for what Policy doesn't hold.

Left alone:

- The `update_keyboard_focus()` calls in handlers. The refresh pass makes the same call, so none can be forgotten, and a handler that makes it early may need the focus before its next line.
- Which outputs are queued. Every output, as today; Policy doesn't say which.

## Acceptance criteria

- [x] A headless test through the fixture, written first and run before the code changes: with a pointer focus mode and `auto-raise` on, a window under a still pointer is raised after `advance_clock` past the delay, read from the output's pixels and not from `policy.stacking`. If it passes before the change, the Done section says what queued the redraw, and the rest still goes ahead.
- [x] The proptest invariant, written before the flag is set anywhere and seen to fail.
- [x] A headless test for one handler whose call went: its change is drawn.
- [x] Every remaining `queue_redraw_all()` in `caminito/src` is for something Policy doesn't hold; the Done section lists them by file with the reason.
- [x] `CODING_STANDARDS.md` has the new line.
- [x] `cargo test -p caminito-policy` passes, and the headless tests that read pixels pass (`tests::frames`, `tests::decoration`, `tests::bell`, `tests::switcher`, `tests::workspace_popup`, `tests::lock`, `tests::focus_modes`).
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **What the Scene reads from Policy,** from `scene.rs`: the invariant compares that list, and the list is in `caminito-policy`'s tests, so say how the two are kept the same.
- **Where the flag is set** in Policy, and whether one place covers it or several.
- **Where in the refresh pass it's taken,** with its "after" and "before" comment.
- **The Window switcher, the Workspace popup and the window menu:** their state is partly Policy's. Say for each whether its redraw now comes from the flag or stays with its own module.

## Done (2026-10-11)

- **There's no flag that a mutation sets: Policy compares the answers.** `Policy::take_scene_changed` builds what the Scene reads now (`SceneReads`, `caminito-policy/src/policy.rs`), compares it with what it built the last time it was asked, and keeps the new one. So one place covers it, no mutation can forget it, and it's true only when something the Scene reads is different, where a flag set in `apply()` would be true after each commit and each crossing. This is not the ticket's letter. The cost is one list of the windows, with `is_showing` and `layer` for each, in each refresh pass, which already goes through every window for the views and the frames.
- **The auto-raise test failed before the change.** `auto_raise_draws_the_window_it_raised_under_a_pointer_that_stays_still` (`tests::focus_modes`, on xdg and X11 windows) reads the output's pixel on the lower window's titlebar, under the other window's body, and asks for no frame. On master's code the pixel was still the upper window's after `advance_clock`, as the ticket read it. Nothing else queued the redraw.
- **The invariant failed against a stub.** `check_ops` compares `SceneView` before and after each operation and asks `take_scene_changed`. With a `take_scene_changed` that returned false it failed at the first `Map`. With the comparison in Policy it proves that `SceneReads` holds no less than `SceneView`, not that each mutation remembers.
- **What the Scene reads from Policy** is eleven queries: `active_workspace`, `stacking`, `window`, `geometry`, `frame_rect`, `is_showing`, `is_fullscreen`, `layer`, `grab`, `tile_preview` and `switcher` (its entries, for the thumbnails). A shaded window is in `frame_rect`, which is the titlebar alone then. The list is `SCENE_QUERIES` in `caminito-policy/src/tests/invariants.rs`, with `SceneView` beside it. **The two are kept the same by a test:** `the_scene_view_has_every_query_the_scene_makes` reads `caminito/src/scene.rs` as text and fails when the methods it calls on `policy` aren't that list. It doesn't see a read that `scene.rs` makes through another module's helper; there's none today. `SceneReads` is the third copy, which the proptest holds to `SceneView`. `scene.rs`'s module doc and `CODING_STANDARDS.md` say where a new query goes.
- **Where it's taken:** `Caminito::queue_redraw_for_policy`, in `State::refresh_and_flush_clients` after `update_keyboard_focus` and directly before `redraw_queued_outputs`, with its comment. The steps after it (`refresh_logind`, `refresh_captures`, `refresh_deadlines`) don't change Policy.
- **The Window switcher, the Workspace popup and the window menu keep their redraws in their own modules.** Each draws a buffer from Policy's state in the refresh pass and queues when it draws it again or lets it go (`refresh_switcher`, `refresh_switcher_popup`, `close_workspace_popup`, `refresh_workspace_popup`, `refresh_window_menu`). The calls their handlers made after a Policy mutation went. The switcher's thumbnails are the one part the Scene reads from Policy, and are in `SceneReads`; its selection isn't, since the popup's chrome is drawn from it.
- **29 calls went:** `work_area.rs` (1), `dbus/caminito.rs` (1), `input/mod.rs` (1), `input/frame.rs` (2), `input/grab.rs` (1), `window/lifecycle.rs` (4), `xwayland/mod.rs` (3), `xwayland/xwm.rs` (2), `handlers/foreign_toplevel.rs` (5), `handlers/xfce_foreign_toplevel.rs` (1), `handlers/ext_workspace.rs` (1), `switcher/mod.rs` (4), `window_menu/mod.rs` (3). Two more in `input/grab.rs` were for the cursor too, and each is now where the grab's kind changes: `keyboard_grab_key`'s, and the one in the grab's motion, which the review put back.
- **48 calls stay,** each for what Policy doesn't hold:
  - `bell.rs` (2): the flash.
  - `idle/mod.rs` (4): the blank, and the outputs' power.
  - `lock/mod.rs` (4), `lock/locker.rs` (2): the lock, and the locker's message.
  - `state.rs` (6): the palette (2), the cursor theme (1), and the frames' look: `frames_changed`, `decoration_changed` and `refresh_frames`. The first two follow `policy.frame_changed`, and by reading the flag and `refresh_frames` would cover them; I kept them, since a Frame that comes or goes is `Mapped`'s and I didn't test it without them.
  - `output.rs` (2): an output's change, and the refresh pass's own call.
  - `backend/tty.rs` (1): the session's return.
  - `minimize_animation.rs` (2): the animation's boxes.
  - `debug.rs` (1): frame debug.
  - `input/mod.rs` (1): the cursor moved.
  - `input/grab.rs` (3): the cursor's shape, as a grab ends and as a keyboard resize picks its edges.
  - `handlers/mod.rs` (1): a client's cursor.
  - `handlers/selection.rs` (3): the drag icon.
  - `handlers/compositor.rs` (1), `handlers/xdg_shell.rs` (1), `window/lifecycle.rs` (1): a commit of a subsurface, a popup and a window.
  - `switcher/mod.rs` (2), `workspace_popup/mod.rs` (2), `window_menu/mod.rs` (4): the popups' own buffers, drawn again or let go.
  - `xwayland/xwm.rs` (4), `xwayland/restart.rs` (1): override-redirect windows.
- **The handler's test is ext-workspace's.** `a_switch_of_workspace_a_pager_asks_for_is_drawn` maps a window with no frame, has a pager switch the workspace, and reads the pixel. With the refresh pass's step turned off it failed, but at the map, which isn't drawn then either; I didn't run the switch alone without the step.
- **What ran:** `cargo test -p caminito-policy` (506 pass), `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`, and 49 modules of `caminito`'s headless tests (860 pass), which have the seven the ticket names and those of each handler that lost a call. After the review's fixes, `tests::keyboard_grab`, `tests::grabs`, `tests::focus_modes` and `tests::ext_workspace` again. **What didn't:** the rest of `tools/check` (`cargo deny`, the other crates' tests, the palette helper, the extension) and the other modules of `caminito`'s suite. All of it ran in the OrbStack machine, with `.scratch/issues/179-test.sh`.
- **`/code-review` found no hard violation.** From it: the queue in the grab's motion for a keyboard resize's cursor, which leaned on its caller; "a Frame's look" in the coding standards' list; the field's name. Left: a `Fixture::pixel` for the five copies of the tests' `pixel`, which is not this ticket's.
