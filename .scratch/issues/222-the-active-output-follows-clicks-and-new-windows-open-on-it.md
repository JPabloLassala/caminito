# The Active output follows clicks, and new windows open on it

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 231
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The rest of "The Active output" in [the spec](../per-output-workspaces/spec.md).

- **A click on anything on another output makes it the Active output**: a window, a panel or any other layer surface, caja's desktop or the wallpaper. Moving the pointer there changes nothing.
- **A new window opens on the Active output's active workspace**, in place of the focused window's output or else the Primary output. The `pointer` and `manual` placement modes keep the pointer's output. The Primary output is used only while a session has no Active output yet.
- **`rename-workspace`** reads the Active output's active workspace.

## Acceptance criteria

- [x] Headless tests with two outputs: a click on the other output's layer surface, and on its bare background, changes the Active output, and the next switch binding acts there; pointer motion alone doesn't.
- [x] A window mapped after a switch to an empty workspace on the second output opens there, on that workspace, with nothing focused before it.
- [x] A window mapped under `pointer` placement opens on the pointer's output.
- [x] With the key off, a new window opens where it does today.
- [x] The Window switcher's popup opens on the pointer's output still, with the Active output the other one.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: with the second monitor on an empty workspace, open a terminal with its binding and see it there; click the first monitor's panel and open one from the menu and see it on the first.

## Done (2026-10-13)

- **Tested first.** Policy's tests in `per_output_workspaces.rs` were written before the code, against stubs, and five of them failed. The headless tests were written after the compositor's code, so that code was then broken on purpose (the press made to do nothing, the placement put back): eight of them failed, and the code was put back.
- **A press says the Active output.** `Policy::press_on_output(seat, output)` is for a press on what is no window: a layer surface (the panel, caja's desktop, the wallpaper), an override-redirect window or an output's bare background. The compositor's `output_pressed` calls it with the output under the pointer. It leaves the focus alone. `Policy::press_on_window` is for every press on a window, so a Frame button's press and the window menu's say the window's Main output too.
- **The Active output follows the focus only when the focus changes.** In ticket 231 every `apply` set it from the focused window, which would undo a press at once. `followed_focus` now keeps the focused window and its Main output that each seat last followed, and the Active output changes when one of them changes. `focus_window` also sets it, so that activating the window that has the focus already brings it back.
- **A new window's workspace is that of the output it opens on** (`opening_output`): the Active output, or the pointer's output with the `pointer` and `manual` modes. From the code review: a window that asks for a position, or maps fullscreen on a named output, takes that output's active workspace, so it opens in view. A transient goes with its parent, and a window that asks for a workspace gets it, as before.
- **With the key off** `placement_output` is as before: the focused window's output, else the Primary output. A press still sets the seat's Active output with the key off. Nothing reads it there but the dump, and the next change of the focus sets it again.
- **`rename-workspace` and the dump's top `active_workspace`** read `Policy::active_output_workspace(SEAT)`. `primary_active_workspace` stays for X11 and ext-workspace (226, 228).
- **Not done, on purpose:** a press while the pointer is grabbed (a client's open popup menu, a drag) does not change the Active output. A press on the lock screen does not either.
- **The Window switcher's popup** needed no change. A headless test now holds it on the pointer's output.
- **The checklist** has the new item "A click says the Active output, and a new window opens on it". It was not run: the run in a Session with two monitors is ticket 229's. `docs/parity.md`'s `center-new-windows` row now names the Active output.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito-policy` and `caminito`; policy's 612 tests; and caminito's headless tests filtered by `per_output_workspaces`, `dialogs`, `placement`, `workspace_popup`, `tests::debug`, `focus_modes`, `layer_shell`, `switcher`, `window_actions`, `show_desktop`, `workspace_slide`, `sounds`, `outputs`, `lock::`, `work_area`, `window_menu`, `click`, `button`, `fullscreen`, `xwayland`, `frame` and `activation` (785 passed). `xwayland_crashing_again_and_again_as_it_starts…` failed one time in that run and passed alone, as it does on `master`.
