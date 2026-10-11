# Policy says when what the Scene reads changed, and the refresh pass queues the redraw

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] A headless test through the fixture, written first and run before the code changes: with a pointer focus mode and `auto-raise` on, a window under a still pointer is raised after `advance_clock` past the delay, read from the output's pixels and not from `policy.stacking`. If it passes before the change, the Done section says what queued the redraw, and the rest still goes ahead.
- [ ] The proptest invariant, written before the flag is set anywhere and seen to fail.
- [ ] A headless test for one handler whose call went: its change is drawn.
- [ ] Every remaining `queue_redraw_all()` in `caminito/src` is for something Policy doesn't hold; the Done section lists them by file with the reason.
- [ ] `CODING_STANDARDS.md` has the new line.
- [ ] `cargo test -p caminito-policy` passes, and the headless tests that read pixels pass (`tests::frames`, `tests::decoration`, `tests::bell`, `tests::switcher`, `tests::workspace_popup`, `tests::lock`, `tests::focus_modes`).
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **What the Scene reads from Policy,** from `scene.rs`: the invariant compares that list, and the list is in `caminito-policy`'s tests, so say how the two are kept the same.
- **Where the flag is set** in Policy, and whether one place covers it or several.
- **Where in the refresh pass it's taken,** with its "after" and "before" comment.
- **The Window switcher, the Workspace popup and the window menu:** their state is partly Policy's. Say for each whether its redraw now comes from the flag or stays with its own module.
