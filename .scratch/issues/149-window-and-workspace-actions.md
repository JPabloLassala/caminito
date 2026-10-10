# The window and workspace actions policy already has

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in; the X11 halves of the new fixture tests are unrun here (Xwayland doesn't start in the cloud container) and are the developer's to run, and the checklist item is 173's closing run
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The actions whose behaviour Policy already has, bound from their keys ([148](148-keybindings-from-settings.md)) with Marco's defaults:

- **Window state:** `minimize`, `maximize`, `unmaximize`, `maximize-horizontally`, `maximize-vertically`, `toggle-fullscreen`, and `toggle-shaded`, which shades an SSD window and does nothing on a CSD window ([05](05-v01-marco-feature-cut.md)).
- **Stacking:** `raise`, `lower` and `raise-or-lower`.
- **Workspaces:** `switch-to-workspace-1` to `-12`, and `move-to-workspace-1` to `-12`, `-left` and `-right`, through the Workspace grid ([96](96-workspace-grid.md)). A number past `num-workspaces` does nothing.
- **Commands:** `run-command-1` to `-12` run `org.mate.Marco.keybinding-commands`' `command-1` to `-12` through the shell, as the screenshots' do ([104](104-screenshots.md)); an empty command does nothing and is logged ([167](167-zenity-dialogs.md) adds Marco's dialog). `run-command-terminal` runs `org.mate.applications-terminal exec` ([09](09-gsettings-mapping.md)).

Each window action acts on the focused window, and does nothing with none, or on a window that doesn't allow it (a fixed-size window isn't maximised, as from its titlebar).

## Acceptance criteria

- [x] Policy tests, written before the code, for any rule this ticket adds to policy: `raise-or-lower`'s test, and whether `move-to-workspace-*` takes the focus and the active workspace with the window.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: each action from its default binding, or from a binding set in the test where Marco's default is `disabled`; a number past the count; the shade on an SSD window and nothing on a CSD one; a command run and an empty one logged; `run-command-terminal` running the terminal key's command.
- [x] Their rows in `docs/parity.md` say honoured, with the tests above; any of these keys in `UNSUPPORTED` leaves it.
- [x] The checklist has an item, tagged M8: each action from mate-control-center's *Keyboard Shortcuts*, on a native and an X11 window. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **`raise-or-lower`'s test:** Marco raises a window that something overlaps and lowers one on top; which windows count as overlapping it (same workspace, every layer or its own).
- **`move-to-workspace-*`:** whether Marco's active workspace follows the window, and whether the window keeps the focus.
- **`toggle-shaded` on a window with no titlebar** (a Frame with none, or a borderless one).

**For the developer:** none.

## Done (2026-10-10)

- **Tests first:** `caminito-policy/src/tests/window_actions.rs` was written against stub `Action` variants that did nothing, and failed (11 of 13) before policy had the rules. The table's tests (`caminito/src/input/bindings/tests.rs`) and the fixture's (`caminito/src/tests/window_actions.rs`, on both kinds of window, and `caminito/src/tests/commands.rs`) were written with the binding code, and the translation's in `caminito/src/settings/translate/tests.rs`.
- **`raise-or-lower`'s test (open question):** Marco's `handle_raise_or_lower` lowers the top of the stack, and otherwise walks `meta_stack_get_above(…, TRUE)`, which stops at the window's own layer, raising if a mapped window's outer rect intersects it. So Caminito raises the focused window if a *showing* window (on the active workspace, not minimised) of *its own layer* above it overlaps its frame, and lowers it otherwise. A dock over it, or a minimised window, doesn't count.
- **`move-to-workspace-*` (open question):** a numbered move is `meta_window_change_workspace` alone: the active workspace stays, and the focus goes to the default window. The moves left and right (`handle_workspace_switch_or_move`) take the active workspace along with the window focused (`meta_workspace_activate_with_focus`). After merging 150, left and right use 150's `move_to_workspace_towards`, which also raises the window and follows `wrap-style`; Marco's grab popup isn't drawn. The desktop and docks don't move (`always_sticky`). A number past the count does nothing, for the switches too (`SwitchToWorkspace` no longer clamps).
- **`toggle-shaded` on a window with no titlebar (open question):** nothing, as Marco's `has_shade_func` is false for an undecorated or border-only window. Caminito asks the focused window's frame flags (`allows_shade`, which an attached dialog's border-only frame and a CSD window lack), as the titlebar's actions do; policy's `set_shaded` refuses a CSD window anyway. Unshading follows the same check.
- **Which window allows what:** policy takes Marco's `recalc_window_features` for the actions: minimise, maximise and fullscreen only a normal window or a dialog; maximise, the one-way maximises and fullscreen need it resizable (not fixed size, not a desktop, dock or splash). `toggle-maximized` now refuses a fixed-size window too. Marco's exception, fullscreen for a fixed-size window the size of the screen, isn't taken. `unmaximize` undoes either way alone. `lower` keeps the focus (`meta_window_lower`), unlike the titlebar's lower-and-unfocus.
- **Order of the table:** Marco's, with the numbered families generated (`twelve!`): `switch-to-workspace-1..12`, the switches and moves beside, the switchers, the panel's, `run-command-1..12`, the screenshots, `run-command-terminal`, then the window actions in `all-keybindings.h`'s order, and 150's `switch-to-workspace-prev` last. The new defaults Alt+F5, Alt+F9 and Ctrl+Shift+Alt+Left/Right no longer reach the client; the accelerators test that pressed Alt+F9 as an ungrabbed combination now presses Ctrl+F9.
- **Commands:** `command-1` to `-12` are read, and spawned through `shell_parse_argv` as the screenshots'. A blank line (Marco's default is a space) runs nothing and logs `can't run org.mate.Marco.keybinding-commands command-N for a keybinding: it's empty` at `warn`; the dialog is 167's. `run-command-terminal` runs `org.mate.applications-terminal exec` (`mate-terminal` by default); that schema is read, not required, and vendored from mate-desktop 1.28.2 for the tests.
- **`UNSUPPORTED`:** none of these keys was there; its comment no longer names `command-1` to `-12`.
- **Review:** no sub-agent tool was available to this run, so the two-axis review was done by hand against the spec and CODING_STANDARDS.md.
- **Tests run (only the related ones, per the run's instructions):** `cargo test -p caminito-policy --lib` (all 321, the crate being the one changed), and in `caminito` the filters `tests::window_actions`, `tests::workspace_moves`, `tests::commands`, `tests::bindings`, `tests::debug`, `tests::accelerators`, `tests::settings`, `tests::keybinding_manifests`, `tests::parity`, `settings::translate` and `input::bindings`, before and after merging the integration branch. Every xdg and non-X11 test passed; every `::x11` half, and the xsettings tests the `settings` filter caught, failed at "Xwayland starts", as in this container. The X11 state-dump snapshot was edited by hand to match the xdg one. Clippy is clean on both crates but for caminito-theme's three pre-existing `manual_range_contains` lints.

