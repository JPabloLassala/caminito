# Policy says which Interaction a seat has, and none opens over another

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

Marco has one slot for what the user is in the middle of, `display->grab_op`: a move or resize, the Alt+Tab popup, the workspace popup or a menu, one at a time. Caminito has four states in two crates, and each has its own list of the others it gives way to:

| Interaction | Where its state is | What it checks before it opens |
| --- | --- | --- |
| A move or resize, by the pointer or the keys | Policy, `grabs` | another grab (`policy/keyboard_grab.rs`, `begin_grab` in `policy.rs`) |
| The Window switcher | Policy, `switcher`, and the compositor | a grab (`policy/switcher.rs`), and the pointer's grab in the compositor (`switcher/mod.rs`) |
| The Window menu | Policy, `window_menu` | a grab and the switcher (`policy/window_menu.rs`), and the lock in the compositor |
| The Workspace popup | the compositor only (`workspace_popup/mod.rs`) | the lock, the pointer's grab, a keyboard grab, the switcher (`workspace_popup_gives_way`) |

Each new one had to be added to every other's list, and two lists were missed. By reading, not run:

- `pointer_modes_held` (`policy/pointer.rs`) holds the pointer focus modes off for the Window switcher and a grab only. Its comment says "The workspace popup is to hold them off too when it comes, with 152". [152](152-workspace-popup.md) came, and Policy still can't know the popup is open, so with `focus-mode` sloppy or mouse a crossing while it shows moves the focus.
- The same function doesn't name the Window menu, which [161](161-window-menu.md) added after it. Marco's menu has a pointer grab, so the focus doesn't follow the pointer there.

The keyboard path's order, `bindings_stage` in `caminito/src/input/mod.rs`, is fine as it is: one function, in order, with a comment on each step. What it lacks is one answer to read.

`CONTEXT.md` has the term: an **Interaction** is what a seat is in the middle of. This was decided in an architecture review on 2026-10-11.

## What to build

- **One answer from Policy:** which Interaction a seat has: a move or resize, the Window switcher, the Window menu, the Workspace popup, or none. It's derived from the state Policy has; `grabs`, `switcher` and `window_menu` keep their fields.
- **The Workspace popup's open state goes to Policy,** with whether a move opened it (`moves`). The compositor asks Policy to open it once the binding's action has run, as it does for the Window menu, since only it knows that a binding with no modifier shows none; Policy refuses if the seat has another Interaction. The compositor tells Policy when it closes. The modifiers that hold it open, its output, its layout and its drawing stay in `workspace_popup/`. The module doc's "Policy has no state for it" changes to say what Policy holds and why.
- **One rule:** a seat has one Interaction at most, and one opens only when the seat has none. Each pairwise check in Policy becomes a question to the one answer. What's there today stays: the Window menu opens in place of itself, and a `switch-*` or `cycle-*` key steps the open Window switcher.
- **The pointer focus modes are held off during every Interaction.** `pointer_modes_held` asks the one answer, and its comment about 152 goes.
- **The lock stays the compositor's,** and isn't an Interaction. The compositor checks it before it asks Policy to open one, and closes the open one when the session locks, as now. Its three checks may become one function if they're the same check.
- **The keyboard path reads the answer.** `bindings_stage` asks Policy for the seat's Interaction and matches on it. The keyboard move or resize and the Workspace popup each get a `*_key_pressed` function in their own module, with the shape `switcher_key_pressed` and `window_menu_key_pressed` have: `None` when the key isn't theirs. `workspace_popup_gives_way` goes, but for the lock and whatever of the pointer's grab Policy doesn't hold.

Left alone: the pointer button path. Each Interaction keeps its own button handler.

[179](179-policy-says-that-it-changed.md) changes who queues the redraw when Policy's state changes, and the Workspace popup's open state becomes Policy's here. Neither ticket blocks the other; the one that merges second follows the first.

## What the user sees change

With `focus-mode` sloppy or mouse, the focus doesn't follow the pointer while the Workspace popup or the Window menu is open.

## Acceptance criteria

- [ ] Two headless tests through the fixture, written first and run before the code changes, with `focus-mode` sloppy: a pointer crossing into another window while the Workspace popup shows leaves the focus where it was, and the same with the Window menu open. If one passes before the change, the Done section says what held the focus.
- [ ] A Policy table test, written before the code: with each Interaction open, each other one doesn't open.
- [ ] A proptest invariant in `caminito-policy/src/tests/invariants.rs`, with operations that open and close each Interaction: no seat has two. A pair that fails on the code as it is goes into the Done section, as a bug fixed or as an exception Marco allows, written into the rule.
- [ ] `grep -rn "workspace_popup_gives_way" caminito/src` finds nothing, or what's left of it names only the lock and the pointer's grab.
- [ ] `tests::switcher`, `tests::workspace_popup`, `tests::window_menu`, `tests::keyboard_grab`, `tests::bindings` and `tests::focus_modes` pass with no test changed but the two new ones.
- [ ] `cargo test -p caminito-policy` passes.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **The seat of the Window switcher and the Window menu.** `grabs` is per seat; `switcher` and `window_menu` are one each. Say which seat each counts for.
- **The compositor's `window_grab`** (the pointer's move or resize, `state.rs`) beside Policy's `grabs`: whether the one answer covers it, so that the compositor's own checks of it can go.
- **Marco's pairs,** from `meta_display_begin_grab_op` and its callers: whether any Interaction opens over another there.
