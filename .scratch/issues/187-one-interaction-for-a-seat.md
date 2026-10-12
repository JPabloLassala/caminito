# Policy says which Interaction a seat has, and none opens over another

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] Two headless tests through the fixture, written first and run before the code changes, with `focus-mode` sloppy: a pointer crossing into another window while the Workspace popup shows leaves the focus where it was, and the same with the Window menu open. If one passes before the change, the Done section says what held the focus.
- [x] A Policy table test, written before the code: with each Interaction open, each other one doesn't open.
- [x] A proptest invariant in `caminito-policy/src/tests/invariants.rs`, with operations that open and close each Interaction: no seat has two. A pair that fails on the code as it is goes into the Done section, as a bug fixed or as an exception Marco allows, written into the rule.
- [x] `grep -rn "workspace_popup_gives_way" caminito/src` finds nothing, or what's left of it names only the lock and the pointer's grab.
- [x] `tests::switcher`, `tests::workspace_popup`, `tests::window_menu`, `tests::keyboard_grab`, `tests::bindings` and `tests::focus_modes` pass with no test changed but the two new ones.
- [x] `cargo test -p caminito-policy` passes.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **The seat of the Window switcher and the Window menu.** `grabs` is per seat; `switcher` and `window_menu` are one each. Say which seat each counts for.
- **The compositor's `window_grab`** (the pointer's move or resize, `state.rs`) beside Policy's `grabs`: whether the one answer covers it, so that the compositor's own checks of it can go.
- **Marco's pairs,** from `meta_display_begin_grab_op` and its callers: whether any Interaction opens over another there.

## Done (2026-10-12)

- **The two headless tests were written and run first,** in `caminito/src/tests/focus_modes.rs`, on both kinds. With the Workspace popup showing, the crossing moved the focus, so that test failed. With the Window menu open the crossing left the focus alone before the change: the compositor's `pointer_target` tells Policy the pointer is over `Other` while the menu is open, so Policy never heard of the window. That check is still there, and Policy now holds the modes off for the menu too.
- **When the Window menu closes, the focus still goes to the window under the still pointer,** as it did before this ticket: the target changes from `Other` to the window, which Policy takes as a crossing. Marco ignores the enter that the end of a grab makes (`NotifyUngrab`, display.c:2120). After the Workspace popup closes the focus stays, since Policy heard of each crossing while it showed. Making the menu's close the same is a change this ticket didn't ask for, and it's not done.
- **The one answer is `Policy::interaction(seat)`,** in `caminito-policy/src/policy/interaction.rs`, with the rule in that module's doc. `Interaction` has four variants and carries nothing: `grab`, `keyboard_grab`, `switcher`, `window_menu` and the new `workspace_popup` still answer what each one holds.
- **The seat of the Window switcher and the Window menu: every seat's.** Each is one for the display, as Marco's `grab_op` is, and so is the Workspace popup, which shows the one active workspace. A move or resize is its own seat's. So none of the three opens while any seat has an Interaction, and no seat begins a move or resize while one of the three is open. Before, the switcher and the menu looked only at the opening seat's move or resize. Caminito has one seat, so only Policy's tests see the difference.
- **The pairs that opened on the code as it was,** from the table test run against the old checks, all fixed as bugs: a move or resize, by the pointer or the keys, over the Window menu and over the Workspace popup; the Window switcher and a `cycle-*` over the Window menu and over the Workspace popup; the Window menu over the Workspace popup and the popup over the menu; and, for a second seat, the Window switcher and the Window menu over the first seat's move or resize. None was reachable from the keyboard, since the open one takes the keys first. A new window's manual placement (154) was: it began under the Window menu or the Workspace popup, and now the window is placed where it maps, as under the Window switcher.
- **Marco's pairs: none.** `meta_display_begin_grab_op` returns `FALSE` whenever `display->grab_op` isn't `META_GRAB_OP_NONE` (display.c:3583-3590), and each caller ends there. So the rule has no exception for one Interaction over another. Three things open in place of themselves, as before: the pointer's move or resize in place of the seat's own, the Window menu in place of the open one, and a `switch-*` or `cycle-*` action steps the open Window switcher.
- **The compositor's `window_grab` isn't covered, and its checks stay.** It's the pointer held for a move or resize, and it outlasts one that Policy ended itself (the window went, or was maximised or hidden) until the pointer next moves or its button goes up. `nothing_shows_during_a_move` is that case: the switch hides the window, Policy ends the move, and the popup would open. So `switch_action` and `workspace_popup_gives_way` still ask it, and `workspace_popup_gives_way` names the lock and `window_grab` only. `open_window_menu` doesn't ask it, as before.
- **The Workspace popup is asked about twice.** `workspace_popup_shows_for` asks the seat's Interaction before the action runs, since a switch closes the Window switcher and the Window menu and Policy would then let the popup open; the code review found that. Policy is asked to open it after.
- **The lock's three checks stay three.** They aren't one check: the keyboard path stops at the lock before any binding, `open_window_menu` returns at it, and the popup asks it with `window_grab`. `session_locked` now closes the popup with the switcher and the menu, where it used to close at the next refresh pass.
- **One of Policy's tests changed:** `a_window_with_its_titlebar_on_an_output_has_no_move_titlebar_on_screen` began a pointer move with the Window menu left open, and now closes the menu first. The six headless modules pass with nothing changed but the two new tests.
- **The proptest has the Window menu's and the Workspace popup's operations** and `activate-window-menu`, with the invariant that no seat has two. It passed 20000 cases once, and runs the default count in the suite.
- **`tools/check` was run, and one step fails here that fails on `master` too.** rustfmt, clippy, cargo-deny, the build, every crate's tests, the palette helper, the helpers' tests and caminito's 1812 tests pass. In *caminito-locker against caminito*, `the_password_typed_at_caminito_lockers_dialog_unlocks_after_a_wrong_one` times out: `pam_wrapper` isn't installed on this machine (`libpam_wrapper.so` can't be preloaded). The script stops there, so the extension's step was run by hand, and passes.
