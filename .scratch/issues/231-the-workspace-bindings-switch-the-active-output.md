# The workspace bindings switch the Active output

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The third of three slices, and the first a user can try: the **Active output** (CONTEXT.md) as far as focus decides it, under "The Active output" in [the spec](../per-output-workspaces/spec.md). Clicks and where a new window opens are ticket 222.

- **Each seat has an Active output**: the focused window's Main output. When a switch leaves no window focused it stays that output. While there is none, the Primary output stands in.
- **The `switch-to-workspace-*` bindings** switch the Active output only: by number, beside, and to the previous one, which is that output's previous.
- **Focus after a switch** is the top window showing on that output, or nothing. It doesn't go to a window on another output because the workspace is empty. A switch of an output the focused window isn't on leaves focus alone.
- **The Workspace popup** opens on the Active output, marks its active workspace, and outlines its windows.
- **The switch sounds** play for the output that switched.
- **Turning the key off** takes the Active output's active workspace for every output, in place of the Primary output's (ticket 230).
- The debug dump shows each output's active workspace and each seat's Active output.

## Acceptance criteria

- [x] Policy unit tests with two outputs: a binding switches the focused window's output and not the other; a switch to an empty workspace leaves nothing focused and the Active output where it was, and a second switch acts on the same output with the pointer on the other; `switch-to-workspace-prev` goes back on that output only.
- [x] Focus moves to a window on the other output only when the user focuses one, and the Active output follows.
- [x] A headless test: the Workspace popup is on the Active output with the pointer on the other.
- [x] With the key off, the bindings switch every output and focus is as today.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: switch each monitor's workspace with the bindings, switch one to an empty workspace twice running, drag a window across, and turn the key off and on with `gsettings`.

## Done (2026-10-13)

- **Tested first.** Policy's tests in `per_output_workspaces.rs` were written before the code. They couldn't fail before `active_output` existed, so the code was then broken on purpose (the bindings made to switch every output, the lost focus taken from any output): seven of them failed, and the code was put back.
- **The Active output is a memo of the focus.** `Policy::active_output(seat)` reads `active_outputs`, which `apply` sets to the output each seat's focused window shows by (`output_showing`: its Main output, or the Primary output for a window on none). A seat with no focus keeps its entry. The Primary output stands in for a seat with no entry. `set_outputs` drops the entry of an output that went, so the output doesn't become the Active output again when it comes back; the rest of an output's going is ticket 225's.
- **The bindings.** `switch-to-workspace-N`, the four beside and `-prev` go through `switch_active_output`, which switches the seat's Active output alone with the key on, and every output with it off or with no output. The neighbour and the previous workspace are that output's.
- **Focus after a switch of one output** goes to the seats whose Active output it is, not only to those whose focused window is on it. So a seat with no focus after a switch to an empty workspace gets the top window back on the way back.
- **A lost focus stays on the Active output**, with the key on: `default_focus_for(seat)` is used by `fix_focus` (the focused window closed, minimised or put away) and by `lower`. With the key off both take the default focus from any output, as before. The end of Show Desktop still takes it from any output: that is ticket 224's.
- **Turning the key off** takes the Active output of the seat that last acted. The sound of a switch of every output, of a lower count and of the dynamic renumbering is from that output's workspace too.
- **Not changed, on purpose:** `move-to-workspace-*` beside moves still go by the Primary output's neighbour and switch every output (223). Activation, ext-workspace's `activate` and `_NET_CURRENT_DESKTOP` still switch every output (223, 226, 228). `rename-workspace` and the dump's top `active_workspace` still read the Primary output's (222).
- **The Workspace popup** opens on the Active output with the key on, and on the pointer's output with it off, as before. With the key on it outlines only the windows the output is the Main output of. That filter came from the code review and has no test of its own.
- **The debug dump** has `active_output` (the seat's, by name) and an `active_workspace` in each of `outputs`. The two state dump snapshots changed by those three lines.
- **Tests changed for the new rule:** policy's `turning_the_key_off_…` tests and the headless `turning_the_key_off_and_on_applies_at_once` and the slide's `turning_per_output_workspaces_on_or_off…` now make the left output the Active output with a click before the key goes off, or expect the Active output's workspace.
- **The checklist** has the new item "The workspace bindings switch the Active output". The Workspace popup's and the two-monitor slide's items said "the monitor with the pointer", and now say the monitor of the focused window. None of them was run: the run in a Session with two monitors is ticket 229's.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito-policy` and `caminito`; policy's 600 tests; and caminito's headless tests filtered by `per_output_workspaces`, `workspace_popup`, `tests::debug`, `workspace_slide`, `sounds`, `workspace_moves`, `dynamic_workspaces`, `focus_modes`, `show_desktop` and `window_actions` (164 passed), and before the review fixes also `ext_workspace`, `switcher` and `window_menu` (356 passed).
