# The window and workspace actions policy already has

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
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

- [ ] Policy tests, written before the code, for any rule this ticket adds to policy: `raise-or-lower`'s test, and whether `move-to-workspace-*` takes the focus and the active workspace with the window.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: each action from its default binding, or from a binding set in the test where Marco's default is `disabled`; a number past the count; the shade on an SSD window and nothing on a CSD one; a command run and an empty one logged; `run-command-terminal` running the terminal key's command.
- [ ] Their rows in `docs/parity.md` say honoured, with the tests above; any of these keys in `UNSUPPORTED` leaves it.
- [ ] The checklist has an item, tagged M8: each action from mate-control-center's *Keyboard Shortcuts*, on a native and an X11 window. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **`raise-or-lower`'s test:** Marco raises a window that something overlaps and lowers one on top; which windows count as overlapping it (same workspace, every layer or its own).
- **`move-to-workspace-*`:** whether Marco's active workspace follows the window, and whether the window keeps the focus.
- **`toggle-shaded` on a window with no titlebar** (a Frame with none, or a borderless one).

**For the developer:** none.
