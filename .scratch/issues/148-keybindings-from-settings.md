# Keybindings read from `org.mate.Marco.*`, live

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in and its xdg tests pass; the XWayland halves of the new fixture tests need the developer's `tools/check` (Xwayland doesn't start in the cloud container), and the checklist step is 173's closing run
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito's bindings come from the user's settings, as Marco's do, in place of M1's hard-coded defaults ([17](17-milestone-slicing.md), [22](22-xdg-shell-and-input.md)). A shortcut changed in mate-control-center's *Keyboard Shortcuts* applies at once.

- **The keys.** The settings thread reads `org.mate.Marco.global-keybindings` and `org.mate.Marco.window-keybindings`, every key, and watches them. A key's value is a GTK accelerator, parsed with the parser [98](98-accelerator-grabs.md) wrote for mate-settings-daemon's grabs, so both read `<Super>`, `<Mod4>`, `<Primary>`, `disabled` and the empty string alike.
- **The table.** The bindings stage of the keyboard path matches against a table built from those keys in place of today's hard-coded one (16 entries, for the 7 actions policy has, the panel's two and the screenshots'). It's rebuilt when a key changes, and a press that started under the old table ends under it.
- **Nothing that works today stops working.** Every action bound today keeps its default binding, now from its key: the workspace left and right, Alt+F1 and Alt+F2, Alt+F4, Alt+F10, the screenshots, Ctrl+Alt+D and every `switch-*` action.
- **Actions with no behaviour yet** are read and parsed but bind nothing until their ticket gives them an action; their rows stay *to do* ([147](147-the-parity-table.md)). This ticket adds no new behaviour except that `switch-windows-all` and `switch-windows-all-backward`, which [108](108-window-switcher.md) built but couldn't bind, are now bindable.
- **The `-backward` actions.** Today Shift with an action's binding steps backward. A `-backward` key that's set binds its own combination as well.
- **The accelerator grabs.** [98](98-accelerator-grabs.md) grants a grab of a combination Caminito binds and never signals it while Caminito binds it, since the bindings were fixed. That now follows the keys: a combination Caminito stops binding is signalled to its grab from then on, and one it starts binding stops being signalled.
- **The debug dump** lists the bindings in force, with the key each came from.

## Acceptance criteria

- [x] Policy-free unit tests, written before the code, of the table: built from a set of key values, a changed key, `disabled`, the empty string, an unparseable value, two keys with the same combination, and the `-backward` rule.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: every binding that works today works from its key's default; changing a key with the test backend moves the binding at once; `disabled` unbinds; `switch-windows-all` opens the Window switcher over every workspace's windows once it's bound; a 98 grab of a combination Caminito stops binding is signalled.
- [x] `org.mate.Marco.global-keybindings` and `window-keybindings` leave `UNSUPPORTED` if 147 put them there, and their rows in `docs/parity.md` say how they're read, with the tests above.
- [x] [101](101-mate-control-center-caminito.md)'s checklist item no longer says a changed shortcut changes no binding: it says the shortcut applies at once, and checks it. The item is tagged M8 for that step, which is the developer's, run in a Session, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Notes

- **The manifests** (`50-caminito-*.xml`) already point at the keys, and their test checks them against Marco's schema. [173](173-packaging-and-closing-m8.md) checks them against every action once all are bound.
- **The commands** of `run-command-1` to `-12` are `org.mate.Marco.keybinding-commands`, read for the screenshots only today. [149](149-window-and-workspace-actions.md) reads the rest.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **Keysym or keycode.** Today's table matches by keysym. Marco resolves an accelerator to keycodes at grab time. Which the table does, and what happens with a layout switch (54's group actions), follows from what Marco does, as far as Wayland allows.
- **How Shift-reverses composes with an explicit `-backward` key**, and whether Marco applies Shift-reverses to every `switch-*` and `cycle-*` action or only to some.
- **`disabled`, the empty string and an unparseable value:** what Marco binds for each, and the log line Caminito writes, once per key and value ([09](09-gsettings-mapping.md)'s rule for out-of-range values).
- **Two keys with one combination:** which wins in Marco.

**For the developer:** none foreseen. A 98 grab that collides with a combination after a change is settled by the rule above; if Marco's source shows another, the Done section says so for the developer to confirm.

## Done (2026-10-10)

- **Tests first:** `caminito/src/input/bindings/tests.rs`, the table built from key values with no fixture and no policy, failed against a stub table that bound nothing, then passed. The fixture's tests are in `caminito/src/tests/bindings.rs` (both kinds of window), the grabs' in `caminito/src/tests/accelerators.rs` (xdg only, as that module is), the translation's in `caminito/src/settings/translate/tests.rs`.
- **The table** (`input::bindings::Bindings`) is built on the settings thread as part of `Settings`, from every key of both schemas, which `SCHEMAS` now reads (required, as Marco's other schemas are) and so watches like the rest. A change replaces the table whole. The bindings stage's swallowed release is keyed by keycode, not by the table, so a press ends under the table it started under.
- **Only the actions with a behaviour bind:** the 18 in `BINDABLE`, which are today's 16 entries' actions plus `switch-windows-all` and the three other `-backward` keys. `switch-to-workspace-1` to `-12` have a policy action but stay 149's, unbound, as their rows say. Every other key is parsed and binds nothing.
- **Keysym or keycode:** keysym. Marco resolves an accelerator's keysym to a keycode when it grabs (`reload_keycodes`, `XKeysymToKeycode`), so a binding stays on its key whichever group is active, and a keysym on any level of the key names it. Caminito names a press by its first layout's first-level keysym, which stays put across a layout switch as Marco's keycode does, and by the keysym the press types, as 98's grabs already did (100), so `<Shift><Super>exclam` binds Shift+Super+1 as in Marco. The two stages now share that rule (`press_keysyms`).
- **Shift reverses** every action Marco flags `BINDING_REVERSES`: all the `switch-*` and `cycle-*` ones, `-backward` included (`all-keybindings.h`). A combination without Shift is bound with Shift too, right after it (`rebuild_binding_table`), and Shift held turns the action round (`do_choose_window`). So a `-backward` key set to `<Alt>q` binds Alt+q backward and Alt+Shift+q forward, and `switch-windows` set to `<Alt><Shift>w` steps backward. The `cycle-*` actions bind nothing until 163, whose actions are to take the same rule. Marco refuses a reversible binding with no modifier but Shift and writes the old value back; Caminito binds nothing for it, logs it, and writes no setting. Stepping the open switcher is unchanged (108's: its opening key or Tab, Shift backward).
- **`disabled`, the empty string, a value that doesn't parse:** Marco binds nothing for the first two, and for the third warns and keeps the binding it had (nothing at start-up). Caminito binds nothing for all three, whatever was bound before, so the table depends on the keys alone; the third is logged once per key and value, as 09's rule: `org.mate.Marco.window-keybindings close is '<Alt>NoSuchKey', which isn't a key combination Caminito can bind: NoSuchKey isn't a keysym; binding nothing`. 98's parser also refuses `<Mod2>`, `<Mod3>`, `<Mod5>` and a keycode, which Marco would bind; they're logged the same way.
- **Two keys with one combination:** Marco grabs both and `process_event` runs the first in its table, which is `all-keybindings.h`'s order, global actions before window ones. Caminito keeps that order. An action with no behaviour yet binds nothing, so a later one sharing its combination gets it until the first is given a behaviour; a grab of 98 for a combination is consulted only after the table, so no rule beyond the stages' order was needed for a collision after a change. For the developer to confirm: nothing in Marco's source differs from that.
- **`run-command-area-screenshot`** is master's key: read where the installed schema has it, with master's default `<Shift>Print` otherwise, as `command-area-screenshot` already was.
- **`UNSUPPORTED`:** 147 put neither keybinding schema there, so nothing left it; its comment now says why none of their keys is there.
- **The debug dump** has `bindings`, each combination in force with its key, in the order they're tried. The two `the_state_dump_shows_…` snapshots gained the list; the X11 one was edited by hand to match the xdg one, as its test can't run here.
- **The checklist item** (*Windows and Keyboard Shortcuts open under Caminito, and name it*) is tagged M8 for step 4, now on *Toggle maximization state*, which Caminito binds (`maximize` is 149's), and checks the changed shortcut applies at once. It's run in 173's closing run.
- **Not run here:** the cloud container's Xwayland 23.2 doesn't start under the fixture, so the X11 halves of the new `on_both_kinds!` tests, and every other X11-tier test, failed at "Xwayland starts", and `display_is_set_only_once_the_root_properties_and_xsettings_exist`, which waits on Xwayland, timed out. `a_killed_locker_comes_back_and_takes_the_lock_over_with_the_session_locked_meanwhile` failed once under the full suite's load and passed alone. caminito-theme's goldens differ in pixels on this container's fonts, untouched here. Every other test of the workspace passed. Clippy is clean on `caminito` apart from the three pre-existing `manual_range_contains` lints in caminito-theme.
