# Keybindings read from `org.mate.Marco.*`, live

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] Policy-free unit tests, written before the code, of the table: built from a set of key values, a changed key, `disabled`, the empty string, an unparseable value, two keys with the same combination, and the `-backward` rule.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: every binding that works today works from its key's default; changing a key with the test backend moves the binding at once; `disabled` unbinds; `switch-windows-all` opens the Window switcher over every workspace's windows once it's bound; a 98 grab of a combination Caminito stops binding is signalled.
- [ ] `org.mate.Marco.global-keybindings` and `window-keybindings` leave `UNSUPPORTED` if 147 put them there, and their rows in `docs/parity.md` say how they're read, with the tests above.
- [ ] [101](101-mate-control-center-caminito.md)'s checklist item no longer says a changed shortcut changes no binding: it says the shortcut applies at once, and checks it. The item is tagged M8 for that step, which is the developer's, run in a Session, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

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
