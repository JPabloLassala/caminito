# Moves, sticky windows, families and activation, per output

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"Moving and activating" in [the spec](../per-output-workspaces/spec.md), with the sticky and family rules of "The model".

- **`move-to-workspace-N` and the beside moves** move a window within its own output's workspaces. The beside moves switch that output with the window.
- **The Window menu's workspace items** are relative to the window's output's active workspace.
- **`move-to-monitor-*`** puts the window on the target output's active workspace.
- **A sticky window** is on every workspace of its Main output, and dragged across is sticky on the other. Unsticking puts it on its output's active workspace.
- **A transient family** is on the workspace of its root's Main output: a dialog dragged to another output shows and hides with its root.
- **Activating a window that isn't showing** switches that window's output to its workspace and focuses it, from wlr-foreign-toplevel's `activate`, a client's activation request and policy's own `activate`. No other output changes.

## Acceptance criteria

- [x] Policy unit tests with two outputs for each rule above, among them: a dialog on the second output hides when its root's output switches away and the second hasn't; activation of a hidden window on the second output leaves the first output's active workspace alone; a beside move on the second output switches only the second.
- [x] The Window menu's items for a window on the second output name moves from the second output's active workspace.
- [x] A headless test: `activate` over wlr-foreign-toplevel on a hidden window of the other output brings it into view there.
- [x] With the key off, each behaves as today.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: a Find dialog dragged to the other monitor follows its editor's workspace; a sticky window stays through its monitor's switches.

## Done (2026-10-13)

- **Tested first.** Policy's `tests/per_output_moves.rs` was written before the code, and ten of its nineteen tests failed: the beside moves, the three activations, and each test with a dialog on the other output. The other nine passed already, because ticket 230 left them right: `move-to-workspace-N`, the Window menu's items for a root, `move-to-monitor-*`, and a sticky window. The headless tests were written after the code, so the code was then broken on purpose (activation made to switch every output; the two compositor fixes taken out), and each of them failed.
- **A family shows by its root's Main output.** `Policy::output_showing(id)` now gives the Main output of the root of `id`'s transient family, so a dialog dragged to another output shows and hides with its root. `Policy::main_output_of(id)` is the window's own Main output. A dialog that crosses alone keeps its family's workspace, as in 230. A root that crosses takes its family, the dialog left behind too.
- **The Active output and the default focus go by a window's own Main output**, as the spec says ("It is the focused window's Main output"). A focused dialog on the second output makes the second the Active output. A switch binding then switches the second output, and the dialog stays in view with the focus, because its root's output did not switch. When the root's output switches away, the dialog hides and the focus goes to a window of the dialog's own output.
- **The beside moves** go from the active workspace of the output the window shows by (its root's) and switch that output alone. `output_switched_for(id)` gives that output, or every output with the key off.
- **Activation** of a window that isn't showing switches the output it shows by, and no other. Every path goes through `Policy::activate`: wlr-foreign-toplevel's `activate`, a client's request (`request_activation`, `request_activation_from`), the not-responding dialog and the Window switcher.
- **The Window menu, `move-to-workspace-N`, `move-to-monitor-*` and unsticking needed no new code.** They read `active_workspace_of`, which follows `output_showing`, and `follow_main_outputs` (230) puts a moved window on the target output's active workspace.
- **From the code review, in the compositor.** The Workspace slide now takes an output's windows by `main_output_of`, where they are drawn. With `output_showing` a dragged dialog was in its root's output's slide, and was drawn moving across an output it was never on. The Workspace popup of a beside move opens on the output the focused window shows by, the one the move switches. It opened on the Active output, which is the other one for a dragged dialog.
- **Not done, on purpose:** the Workspace popup's thumbnails show a dragged dialog on neither output. It is its root's output's window, and it is not in that output's rectangle. `Policy::switch_workspace` still switches every output, for ext-workspace and X11 (226, 228). The Window switcher's lists are ticket 224's.
- **The checklist** has the new item "A dialog follows its window's workspace, and a sticky window stays, on two monitors". It was not run: the run in a Session with two monitors is ticket 229's. `docs/parity.md`'s rows for `move-to-workspace-*` and `move-to-monitor-*` name Per-output workspaces.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito-policy` and `caminito`; policy's 633 tests, with the property tests; and caminito's headless tests filtered by `per_output_workspaces`, `dialogs`, `workspace_popup`, `workspace_slide`, `foreign_toplevel`, `window_actions`, `workspace_moves`, `window_menu`, `show_desktop`, `switcher`, `focus_stealing`, `move_to`, `tests::debug`, `xwayland_windows`, `dynamic_workspaces`, `placement`, `not_responding` and `ext_workspace` (517 passed).
