# Parity with Marco

Each of Marco's `org.mate.Marco.*` keys and keybinding actions, with what Caminito does with it and what covers it. M8 closes when no row is still *to do* ([17](../.scratch/issues/17-milestone-slicing.md), [173](../.scratch/issues/173-packaging-and-closing-m8.md)).

The list is the schema itself ([147](../.scratch/issues/147-the-parity-table.md)): Marco 1.29.0's, which Arch ships and which is vendored for the tests in `tests/schemas/`, and the five keys Marco's master adds to it, which Caminito reads where the installed schema has them: `dynamic-workspaces`, `alt-tab-minimized-placement` and `alt-tab-urgent-placement` in `general`, `run-command-area-screenshot` in `global-keybindings` and `command-area-screenshot` in `keybinding-commands`. That's 40 keys in `general`, 36 in `workspace-names`, 15 in `keybinding-commands`, and 103 actions: 51 in `global-keybindings` and 52 in `window-keybindings`.

A row's status is one of:

- **honoured:** Caminito does what Marco does with it, or what the row says.
- **never:** Caminito will not implement it, for the reason beside it.
- **left to another component:** another program of the Session does it.
- **unsupported:** not yet done, and on the roadmap. It's logged once when the user has changed it from its default (09).
- **to do:** M8's, settled by the ticket the row names. *To do* is allowed until 173, which checks none is left.

A numbered family (`name-1` to `-36`) shares a row. `caminito/src/tests/parity.rs` checks that every key and action of the five schemas, master's five included, has exactly one row, that every *to do* row names one of M8's tickets, and that every key in the settings translation's `UNSUPPORTED` has an *unsupported* or *to do* row. A pass over the rows no test covers is the checklist's *The parity table's rows that no test covers*, run by 173.

## `org.mate.Marco.general`

The Window switcher's keys are 108's: Caminito has one Window switcher, a grid of thumbnails, in place of Marco's Alt+Tab popup ([ADR 0016](adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md), [108](../.scratch/issues/108-window-switcher.md)).

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `mouse-button-modifier` | to do (162) | Alt is hard-coded for modifier-drag, Marco's default. | |
| `resize-with-right-button` | to do (162) | With Alt, the middle button resizes, and no button opens the window menu. | |
| `show-tab-border` | never | Nothing is drawn round the selected window where it sits: its thumbnail shows which it is (ADR 0016). | the checklist's *The parity table's rows that no test covers* |
| `button-layout` | honoured | The SSD Frame's buttons, in Marco's syntax. CSD windows get the layout through the mirror (09). | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/frames.rs` |
| `focus-mode` | honoured | `click`, `sloppy` and `mouse`, live, for xdg and XWayland windows alike. The pointer entering a window focuses it, or its modal dialog, and a dock, the panel or another Layer surface never; leaving it for the desktop (nothing, or a Layer surface below the windows, such as caja's) takes the focus away under `mouse`. A crossing of a still pointer, as a window maps, goes or restacks under it, counts only until the keyboard moves the focus, as Marco's `mouse_mode` has it. The Window switcher and a move or resize hold the modes off. Layer surfaces take the keyboard by their own keyboard interactivity (153). | `caminito-policy/src/tests/pointer_focus.rs`, `caminito/src/tests/focus_modes.rs`, the checklist's *The focus modes and auto-raise* |
| `focus-new-windows` | to do (165) | A new window takes the focus; xdg-activation requests activate nothing. | |
| `raise-on-click` | honoured | Off, a click on a window's body focuses it, or its modal dialog, without raising anything. A click on its titlebar raises it once released, and a move or resize, by the Frame or with Alt, raises it only if the pointer went less than 8 pixels, as in Marco; a drag doesn't. | the same |
| `action-double-click-titlebar` | honoured | An SSD titlebar's double click, by `double-click`'s time (05: SSD frames only). `toggle_above` and `toggle_stick`, here and on the two keys below, keep the window above and make it sticky, or stop, by what the Frame shows (155). | `caminito/src/tests/frames.rs` (`double_clicking_the_titlebar_runs_its_action`, `the_titlebar_actions_toggle_above_and_sticky`) |
| `action-middle-click-titlebar` | honoured | An SSD titlebar's middle click. | `caminito/src/tests/frames.rs` (`middle_and_right_clicks_on_the_titlebar_run_their_actions`) |
| `action-right-click-titlebar` | honoured | An SSD titlebar's right click. `menu` logs and does nothing until the window menu (161). | the same |
| `action-scroll-up-titlebar` | to do (162) | A scroll on the titlebar does nothing, Marco's default `none`. | |
| `action-scroll-down-titlebar` | to do (162) | As `action-scroll-up-titlebar`. | |
| `auto-raise` | honoured | With `sloppy` or `mouse`, a window the pointer focuses is raised after `auto-raise-delay` if the pointer is still in it and it still has the focus; leaving it first cancels the raise. | the same |
| `auto-raise-delay` | honoured | The delay, in milliseconds, within Marco's 0 to 10000. | `caminito/src/settings/translate/tests.rs`, `caminito-policy/src/tests/pointer_focus.rs`, `caminito/src/tests/focus_modes.rs` |
| `theme` | honoured | The Marco theme the Frames are drawn in, reloaded live. | `caminito/src/settings/translate/tests.rs`, `caminito-theme/tests/` |
| `titlebar-uses-system-font` | honoured | On, the title is in `org.mate.interface font-name`. | `caminito/src/settings/translate/tests.rs` |
| `titlebar-font` | honoured | The title's font. | the same |
| `num-workspaces` | honoured | 1 to 36 workspaces, live, as M2 and 97 settled. | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/settings.rs`, `caminito/src/tests/ext_workspace.rs` |
| `dynamic-workspaces` | honoured | Master's key, read where the installed schema has it, and off where it hasn't. On, one empty workspace follows those with windows, as in Marco's master: an emptied one is closed up by moving the windows of those after it down one, the view and the focus with them, and `num-workspaces` is written back with the count (151). | `caminito-policy/src/tests/dynamic_workspaces.rs`, `caminito/src/tests/dynamic_workspaces.rs`, `caminito/src/settings/translate/tests.rs`, the checklist's *Dynamic workspaces grow and shrink the switcher* |
| `wrap-style` | honoured | `no wrap`, `classic` and `toroidal`, live, for the four directions of the Workspace grid and the moves of a window there. An empty cell of a part-filled grid is past the edge of its row or column, as in Marco: `toroidal` goes round the filled part of the row or column, and `classic` on to the start or end of the filled part of the next or previous one (150). | `caminito-policy/src/tests/grid.rs`, `caminito-policy/src/tests/workspace_moves.rs`, `caminito/src/tests/workspace_moves.rs`, `caminito/src/settings/translate/tests.rs`, the checklist's *The four directions in a 2 × 2 grid, with each wrap style* |
| `visual-bell` | honoured | On, every bell flashes as `visual-bell-type` says, from any source (105), for 100 ms on Caminito's clock; a second bell during a flash starts it again (169). | `caminito/src/tests/bell.rs`, the checklist's *The visual bell flashes* |
| `audible-bell` | to do (170) | Read; nothing plays (105). | the same |
| `visual-bell-type` | honoured | `fullscreen` flashes every output white; `frame_flash` draws the bell's window's Frame in the other focus state, or flashes the output of a window with no Frame (CSD, fullscreen, hidden) or of the pointer for a bell for no window (169). | the same |
| `compositing-manager` | honoured | Off: the Window switcher's grid shows icons with no thumbnails. Caminito always composites; the shadows (171) and the minimise animation (172) it turns off come with their tickets. | `caminito/src/tests/switcher.rs` |
| `compositing-fast-alt-tab` | honoured | On: the grid shows icons with no thumbnails. | the same |
| `reduced-resources` | to do (172) | There is no minimise animation for it to turn off yet. | |
| `application-based` | never | Marco never implemented it (05). | the checklist's *The parity table's rows that no test covers* |
| `disable-workarounds` | never | Marco's misfeatures for old or broken X11 applications, which Caminito doesn't carry (05). | the checklist's *The parity table's rows that no test covers* |
| `center-new-windows` | honoured | On, Marco's default, the `automatic` mode puts a new window in the middle of the work area of the focused window's output, over any windows there; one that doesn't fit there goes in its corner. It doesn't change `pointer` or `manual` placement, a position the client asks for, a dialog centred on its parent or the output, or the place a window that maps maximised or fullscreen goes back to (154). | `caminito-policy/src/tests/placement_modes.rs`, `caminito/src/tests/placement.rs` |
| `allow-tiling` | honoured | On, Marco's default, a window dropped with the pointer at an output's side fills that half of its work area, and at a corner that quarter, as the keyless section's *Edge and corner tiling* says; off, nothing tiles or maximises at an edge (158). | `caminito-policy/src/tests/tiling.rs`, `caminito/src/tests/tiling.rs`, `caminito/src/settings/translate/tests.rs` |
| `allow-top-tiling` | honoured | On, with `allow-tiling`, a window dropped with the pointer on an output's top edge, down to its work area, is maximised; off, the top does nothing and the sides and corners still tile (158). | the same |
| `allow-tile-cycling` | honoured | On, Marco's default, each press of a tiling keybinding on a window already tiled there steps the tile's width through half, a third, a quarter, all, three quarters and two thirds of the work area's, a corner's too, and the next press untiles it; a window tiled by a drag takes one press to start, at the half it's at. Off, a second press untiles it (159). | `caminito-policy/src/tests/keyboard_tiling.rs`, `caminito/src/tests/tiling.rs`, `caminito/src/settings/translate/tests.rs` |
| `placement-mode` | honoured | `automatic`: the first fit, else a cascade. `pointer`: the frame centred on the pointer, kept within the work area of the output the pointer is on. `manual`: as `pointer`, then the window follows the pointer, onto another output too, until a click or a key puts it down: the arrows, the keypad's and `hjkl` move it by 10 pixels, or 1 with Ctrl or Shift, Escape puts it back where it began, and any other key puts it down where it is. A position the client asks for, a dialog, and a window that maps maximised or fullscreen keep the `automatic` rules, as in Marco (154). | `caminito-policy/src/tests/placement_modes.rs`, `caminito/src/tests/placement.rs` |
| `show-desktop-skip-list` | honoured | A comma-separated list, each entry trimmed and matched in any ASCII case against a native window's app id or an XWayland window's `WM_CLASS` class, as Marco's prefs.c reads it. A listed window, and its transients, stay where they are with their focus whichever way the desktop is shown, aren't minimised when a new window ends it, and don't end it when they map. A change applies at the next Show Desktop (164). | `caminito-policy/src/tests/show_desktop.rs`, `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/show_desktop.rs` |
| `icon-size` | honoured | The size of the icons in the icons-only grid, in logical pixels. A thumbnail's corner icon is a quarter of the thumbnail's height instead. | `caminito/src/tests/switcher.rs` |
| `alt-tab-max-columns` | honoured | The grid wraps at this many columns. | `caminito/src/switcher/layout.rs`, `caminito/src/tests/switcher.rs` |
| `alt-tab-expand-to-fit-title` | never | The grid's width comes from its thumbnails, and a title wider than the popup is ellipsised (ADR 0016). | `caminito/src/tests/switcher.rs` |
| `alt-tab-raise-windows` | honoured | The selected window is raised while the switcher is open, and goes back if it's cancelled. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/switcher.rs` |
| `alt-tab-minimized-placement` | to do (163) | Master's key, read nowhere yet: minimised windows come last, as its default `end` places them. | `caminito-policy/src/tests/switcher.rs` (`minimised_windows_come_after_the_others`) |
| `alt-tab-urgent-placement` | to do (165) | Master's key, read nowhere yet: an urgent window keeps its place (108). | |

## `org.mate.Marco.workspace-names`

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `name-1` to `-36` | honoured | Each workspace's name, live, to the pager over ext-workspace (M2, 97). An empty name is Marco's *Workspace N*. | `caminito/src/settings/translate/tests.rs`, `caminito-policy/src/tests/workspaces.rs`, `caminito/src/tests/ext_workspace.rs` |

## `org.mate.Marco.keybinding-commands`

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `command-1` to `-12` | honoured | Run through the shell by `run-command-1` to `-12` (149). One that's empty, as Marco's default of a space is, runs nothing, is logged and shows Marco's error dialog, as the keyless section's *The error dialog for a command that can't run* says (167). | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/commands.rs`, `caminito/src/tests/dialogs.rs` |
| `command-screenshot` | honoured | Run through the shell by `run-command-screenshot` (104). | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/commands.rs` |
| `command-window-screenshot` | honoured | Run by `run-command-window-screenshot` (104). | the same |
| `command-area-screenshot` | honoured | Master's key, read where the schema has it, with master's default otherwise; run by `run-command-area-screenshot` (104). | the same |

## `org.mate.Marco.global-keybindings`

Every action's key is read and watched, and a change applies at once ([148](../.scratch/issues/148-keybindings-from-settings.md)). Its value is parsed as 98's accelerator grabs are, so `<Super>`, `<Mod4>` and `<Primary>` read alike, and `disabled` and the empty string bind nothing; a value that doesn't parse binds nothing and is logged once per key and value. An honoured action binds its key's combination, Marco's default unless the user changed it; a *to do* action's is read and parsed, and binds nothing until its ticket. Of two keys with one combination, the first in Marco's order wins, the global actions before the window ones, but for the moves to the workspace beside, which follow the switches there, and `switch-to-workspace-prev`, which is last, as `all-keybindings.h` has them. The `switch-*` actions are bound with Shift too, which turns them round, as in Marco, and refuse a combination with no modifier but Shift. A combination Caminito doesn't bind is free for mate-settings-daemon's accelerator grabs, which Caminito signals ([98](../.scratch/issues/98-accelerator-grabs.md), `caminito/src/tests/accelerators.rs`); one it binds isn't signalled, and that follows the keys as they change. The debug dump's `bindings` lists the combinations in force with their keys. Tests: `caminito/src/input/bindings/tests.rs` (the table), `caminito/src/tests/bindings.rs` (the keys through the fixture, on both kinds of window) and `caminito/src/settings/translate/tests.rs`. The `switch-*` rows are 108's: which windows an action lists, their order, and what a quick Alt+Tab goes to are Marco's.

| Action | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `run-command-1` to `-12` | honoured | Unbound by default, as in Marco; once bound, runs `command-1` to `-12` (149). | `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/commands.rs` |
| `switch-to-workspace-1` to `-12` | honoured | Unbound by default, as in Marco; once bound, switches to that workspace, and a number past `num-workspaces` does nothing (149). | `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/window_actions.rs`, `caminito-policy/src/tests/window_actions.rs` |
| `switch-to-workspace-left` | honoured | Ctrl+Alt+Left: the workspace to the left in the Workspace grid (96), past the row's end as `wrap-style` says (150). While the binding's modifiers are held the Workspace popup shows, as Marco's does: the grid on the output the pointer is on, each workspace's cell with its name and an outline of its windows, and the new workspace marked. Each further direction switches again, and letting go of a modifier, any other key or a press of a pointer button closes it. It shows at the grid's edge too, where the switch goes nowhere, and not with one workspace, while the session is locked, during a move or resize, or with the Window switcher open (152). | `caminito/src/tests/bindings.rs`, `caminito/src/tests/ext_workspace.rs`, `caminito-policy/src/tests/grid.rs`, `caminito-policy/src/tests/workspace_moves.rs`, `caminito/src/tests/workspace_moves.rs`, `caminito/src/tests/workspace_popup.rs` |
| `switch-to-workspace-right` | honoured | Ctrl+Alt+Right, as `switch-to-workspace-left`. | the same |
| `switch-to-workspace-up` | honoured | Ctrl+Alt+Up: the workspace above in the Workspace grid, past the column's end as `wrap-style` says (150), with the Workspace popup as `switch-to-workspace-left` shows it (152). | `caminito-policy/src/tests/grid.rs`, `caminito-policy/src/tests/workspace_moves.rs`, `caminito/src/tests/workspace_moves.rs`, `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/workspace_popup.rs` |
| `switch-to-workspace-down` | honoured | Ctrl+Alt+Down, as `switch-to-workspace-up`. | the same |
| `switch-to-workspace-prev` | honoured | Unbound, as by default. It shows no Workspace popup, as in Marco, nor does a switch or a move by number (152). Bound, back to the workspace active before this one, however the switch came: a binding, the pager, an activation or a move of a window to the workspace beside. It's forgotten when its workspace is removed, and does nothing until the next switch; with `dynamic-workspaces` it follows its windows (150). | the same |
| `switch-group` | honoured | Alt+\` opens the switcher over the focused application's windows on the active workspace: those of its process, or of its app id where a window names no process. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/switcher.rs` |
| `switch-group-backward` | honoured | Shift with `switch-group`'s binding. Its own key, when set, binds its combination backward, and with Shift forward (148). | the same, `caminito/src/input/bindings/tests.rs` |
| `switch-windows` | honoured | Alt+Tab opens the Window switcher over the active workspace's windows. | the same |
| `switch-windows-backward` | honoured | Marco's default is `disabled`: Shift with `switch-windows`'s binding opens the switcher on its last entry, and steps it backward. Its own key, when set, binds its combination backward, and with Shift forward (148). | the same, `caminito/src/input/bindings/tests.rs` |
| `switch-windows-all` | honoured | Every workspace's windows. Unbound by default, as in Marco, and bound by its key once set (148). | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/bindings.rs` |
| `switch-windows-all-backward` | honoured | As `switch-windows-backward`. Unbound by default. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/input/bindings/tests.rs` |
| `switch-panels` | honoured | Ctrl+Alt+Tab opens the switcher over docks and the desktop. Only XWayland windows are docks or a desktop to policy: a Wayland panel is a Layer surface, and isn't listed until 163. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/switcher.rs` |
| `switch-panels-backward` | honoured | Shift with `switch-panels`'s binding. Its own key, when set, binds its combination backward, and with Shift forward (148). | the same, `caminito/src/input/bindings/tests.rs` |
| `cycle-group` | to do (163) | Alt+F6 reaches the focused client. | |
| `cycle-group-backward` | to do (163) | Unbound, as by default. | |
| `cycle-windows` | to do (163) | Alt+Escape reaches the focused client. | |
| `cycle-windows-backward` | to do (163) | Unbound, as by default. | |
| `cycle-panels` | to do (163) | Ctrl+Alt+Escape reaches the focused client. | |
| `cycle-panels-backward` | to do (163) | Unbound, as by default. | |
| `show-desktop` | honoured | Ctrl+Alt+D shows the desktop with Marco's per-workspace semantics, shared with the panel's button through `org.mate.Caminito`'s `ShowingDesktop` (94). | `caminito-policy/src/tests/show_desktop.rs`, `caminito/src/tests/show_desktop.rs` |
| `panel-main-menu` | honoured | Alt+F1 asks mate-panel for its main menu (95). | `caminito/src/tests/panel.rs` |
| `panel-run-dialog` | honoured | Alt+F2 asks mate-panel for its run dialog (95). | the same |
| `run-command-screenshot` | honoured | Print runs `command-screenshot` (104). | `caminito/src/tests/commands.rs` |
| `run-command-window-screenshot` | honoured | Alt+Print runs `command-window-screenshot` (104). | the same |
| `run-command-area-screenshot` | honoured | Master's action: Shift+Print, master's default, runs `command-area-screenshot` (104). | the same |
| `run-command-terminal` | honoured | Unbound by default, as in Marco; once bound, runs `org.mate.applications-terminal exec`, `mate-terminal` by default (149). | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/commands.rs` |
| `rename-workspace` | honoured | Unbound by default, as in Marco; once bound, shows Marco's zenity entry dialog with the active workspace's name, and writes the answer to its `name-N`, which the pager then shows. Cancelling, or an empty answer, leaves the name; another press while it shows does nothing (167). | `caminito/src/tests/dialogs.rs` |

## `org.mate.Marco.window-keybindings`

Read and watched as `global-keybindings` is ([148](../.scratch/issues/148-keybindings-from-settings.md)), with the same tests: an honoured action binds its key's combination, and an action that's *to do* binds nothing, so its combination reaches the focused client.

| Action | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `activate-window-menu` | to do (161) | Alt+Space reaches the focused client. | |
| `toggle-fullscreen` | honoured | Unbound by default; once bound, makes the focused window fullscreen or leaves it, if Marco would let it be (a normal window or a dialog that can be resized) (149). | `caminito-policy/src/tests/window_actions.rs`, `caminito/src/tests/window_actions.rs` |
| `toggle-maximized` | honoured | Alt+F10 maximises the focused window, or restores it (M1); a fixed-size window isn't maximised (149). | `caminito/src/tests/bindings.rs` (`alt_f10_toggles_maximised`), `caminito-policy/src/tests/window_actions.rs` |
| `toggle-above` | honoured | Unbound by default, as in Marco. Bound, it keeps the focused window above the normal windows, as Marco's layers do, or lets it go, raising it either way (155). | `caminito-policy/src/tests/above_below_and_sticky.rs`, `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/bindings.rs` (`toggle_above_and_toggle_on_all_workspaces_toggle_the_focused_window`) |
| `maximize` | honoured | Unbound by default; once bound, maximises the focused window if it can be, as from its titlebar: a fixed-size window isn't (149). | the same |
| `unmaximize` | honoured | Alt+F5 unmaximises the focused window, both ways, from either (149). | the same |
| `toggle-shaded` | honoured | Unbound by default; once bound, shades the focused SSD window or unshades it. A CSD window, or one with a border alone, has no titlebar to shade to, and nothing happens (149). | the same |
| `minimize` | honoured | Alt+F9 minimises the focused normal window or dialog (149). | the same |
| `close` | honoured | Alt+F4 asks the focused window to close (M1). | `caminito/src/tests/bindings.rs` (`alt_f4_asks_the_focused_window_to_close`) |
| `begin-move` | honoured | Alt+F7 moves the focused window by the keys, as Marco's keyboard move: the arrows, the keypad's and `hjkl` step it 10 pixels, 1 with Ctrl, and Shift snaps it to the next edge; a step stops at an edge it would cross. The pointer is warped to the window's middle and moves it too. Return, any other key or a click puts it down, and Escape puts it back. A maximised or fullscreen window isn't moved, and X11's `_NET_WM_MOVERESIZE` direction 10 starts the same grab (156). | `caminito-policy/src/tests/keyboard_grab.rs`, `caminito/src/tests/keyboard_grab.rs`, `caminito/src/tests/window_properties.rs`, the checklist's *Alt+F7 and Alt+F8 move and resize a window by the keys* |
| `begin-resize` | honoured | Alt+F8 resizes the focused window by the keys, as Marco's keyboard resize: the first arrow picks the edge it points at, or the pointer's first motion the edge or corner it goes towards, and the pointer is warped onto it; the next arrows move it as `begin-move`'s step. Escape puts the window back as it was. A maximised, fullscreen or fixed-size window isn't resized, and X11's direction 9 starts the same grab (156). | the same |
| `toggle-on-all-workspaces` | honoured | Unbound by default, as in Marco. Bound, it puts the focused window, with its transients, on every workspace in its place, or back on the active one only (155). | `caminito-policy/src/tests/above_below_and_sticky.rs`, `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/bindings.rs` (`toggle_above_and_toggle_on_all_workspaces_toggle_the_focused_window`) |
| `move-to-workspace-1` to `-12` | honoured | Unbound by default; once bound, moves the focused window there, and the active workspace stays, so the focus goes to the window now on top. A number past `num-workspaces` does nothing, nor does the desktop or a dock move (149). | `caminito/src/input/bindings/tests.rs`, `caminito-policy/src/tests/window_actions.rs`, `caminito/src/tests/window_actions.rs` |
| `move-to-workspace-left` | honoured | Ctrl+Shift+Alt+Left: the focused window, with its transients, to the workspace on the left in the Workspace grid, by `wrap-style`, which becomes the active one with the window focused and raised, as in Marco. The desktop and docks don't move (149, 150). While the binding's modifiers are held the Workspace popup shows, as for `switch-to-workspace-left`, and each further direction moves the window on, whichever of the two its key is bound to; with no window to move it doesn't show (152). | `caminito-policy/src/tests/window_actions.rs`, `caminito-policy/src/tests/workspace_moves.rs`, `caminito/src/tests/window_actions.rs`, `caminito/src/tests/workspace_popup.rs` |
| `move-to-workspace-right` | honoured | Ctrl+Shift+Alt+Right, as `move-to-workspace-left`. | the same |
| `move-to-workspace-up` | honoured | Ctrl+Shift+Alt+Up: the focused window, with its transients, to the workspace above, by `wrap-style`, which becomes the active one with the window focused and raised, as in Marco. The desktop and docks don't move (150). The Workspace popup shows as for `move-to-workspace-left` (152). | `caminito-policy/src/tests/grid.rs`, `caminito-policy/src/tests/workspace_moves.rs`, `caminito/src/tests/workspace_moves.rs`, `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/workspace_popup.rs` |
| `move-to-workspace-down` | honoured | Ctrl+Shift+Alt+Down, as `move-to-workspace-up`. | the same |
| `raise-or-lower` | honoured | Unbound by default; once bound, raises the focused window if a showing window of its own layer overlaps it from above, and lowers it otherwise (149). | `caminito-policy/src/tests/window_actions.rs`, `caminito/src/tests/window_actions.rs` |
| `raise` | honoured | Unbound by default; once bound, raises the focused window with its transients (149). | the same |
| `lower` | honoured | Unbound by default; once bound, lowers the focused window to the bottom of its layer, and it keeps the focus, unlike the titlebar's middle click (149). | the same |
| `maximize-vertically` | honoured | Unbound by default; once bound, maximises the focused window vertically or restores it, if it can be resized (149). | the same |
| `maximize-horizontally` | honoured | Unbound by default; once bound, as `maximize-vertically`, horizontally (149). | the same |
| `tile-to-side-e` | honoured | Unbound by default; once bound, tiles the focused window to the right half of the work area of the output it's most on, as a drag there does, unmaximising it first, if it could be maximised and fits; pressed again it cycles or untiles by `allow-tile-cycling` (159). | `caminito-policy/src/tests/keyboard_tiling.rs`, `caminito/src/input/bindings/tests.rs`, `caminito/src/tests/tiling.rs` |
| `tile-to-side-w` | honoured | As `tile-to-side-e`, to the left half (159). | the same |
| `tile-to-corner-nw` | honoured | As `tile-to-side-e`, to the top-left quarter (159). | the same |
| `tile-to-corner-ne` | honoured | As `tile-to-side-e`, to the top-right quarter (159). | the same |
| `tile-to-corner-se` | honoured | As `tile-to-side-e`, to the bottom-right quarter (159). | the same |
| `tile-to-corner-sw` | honoured | As `tile-to-side-e`, to the bottom-left quarter (159). | the same |
| `move-to-corner-nw` | to do (160) | Unbound, as by default. | |
| `move-to-corner-ne` | to do (160) | Unbound, as by default. | |
| `move-to-corner-sw` | to do (160) | Unbound, as by default. | |
| `move-to-corner-se` | to do (160) | Unbound, as by default. | |
| `move-to-side-n` | to do (160) | Unbound, as by default. | |
| `move-to-side-s` | to do (160) | Unbound, as by default. | |
| `move-to-side-e` | to do (160) | Unbound, as by default. | |
| `move-to-side-w` | to do (160) | Unbound, as by default. | |
| `move-to-center` | to do (160) | Unbound, as by default. | |
| `move-to-monitor-n` | to do (160) | Unbound, as by default. | |
| `move-to-monitor-s` | to do (160) | Unbound, as by default. | |
| `move-to-monitor-e` | to do (160) | Unbound, as by default. | |
| `move-to-monitor-w` | to do (160) | Unbound, as by default. | |

## Keys of other schemas Marco read

09's *Foreign keys Marco read*: Marco took these from GTK or read them itself, so Caminito reads them for the same things.

### `org.mate.interface`

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `enable-animations` | to do (172) | Not read: there's no minimise animation for it to turn off yet. | |

### `org.mate.sound`

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `event-sounds` | to do (170) | Passed to XWayland's XSETTINGS and the mirror (09); Caminito plays no workspace switch sound yet. | `caminito/src/xwayland/xsettings/tests.rs` |
| `theme-name` | to do (170) | Passed on as `event-sounds` is; Caminito plays nothing from the theme yet. | the same |
| `input-feedback-sounds` | left to another component | GTK plays the input feedback sounds in each app; Caminito passes the key to XWayland's XSETTINGS and the mirror (09). | the same |

### `org.mate.peripherals-mouse`

| Key | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `double-click` | honoured | The time for a double click on an SSD titlebar (M2). | `caminito/src/tests/frames.rs` (`two_clicks_further_apart_than_the_double_click_time_are_two_clicks`) |
| `drag-threshold` | honoured | How far a press on an SSD Frame moves before it's a drag (M2); twice it, how far tiling's side and corner zones reach into the work area, and how far a move takes a tiled window before it untiles (158). | `caminito/src/tests/frames.rs` (`dragging_the_titlebar_past_the_drag_threshold_moves_the_window`) |

## What has no key of Marco's

Most of 05's and 09's roadmap has no key in Marco's schemas: what Caminito never does, what it does later, and M8's work that no key turns on.

### Never

| What | Status | Why |
| --- | --- | --- |
| `run-command-13` to `-32` | never | Not in the schema: Marco's schema stops at 12 (05). |
| `set-spew-mark` | never | Not in the schema; a debugging aid of Marco's (05). |
| Wireframe moves and resizes | never | Caminito always moves and resizes the window itself (05). |
| The empty close and unminimise effect hooks | never | Marco's hooks draw nothing (05). |
| The command-line options `--replace`, `--display`, `--sync`, `--sm-*` and `--no-force-fullscreen` | never | X11 options: Caminito is the compositor, started by its Session (05). |
| marco-message and the `_MARCO_*` messages | never | Changing the `theme` key reloads the theme live instead (05). |
| The `_NET_DESKTOP_GEOMETRY` and `_NET_DESKTOP_VIEWPORT` stubs | never | Marco's are stubs with nothing behind them (05). |
| marco-theme-viewer, marco-window-demo and wm-tester | never | Marco's X11 test programs (05). |
| `org.mate.interface` `accessibility` | never | Marco used it only to force opaque moves over wireframe, and Caminito has no wireframe (09). |
| `org.mate.interface` `window-scaling-factor-qt-sync` | never | Caminito honours `window-scaling-factor` itself (13), and Qt reads the scale from Wayland (09). |

### Later

The roadmap after v0.1: each is *unsupported*. The keys among them that Caminito doesn't match are logged once when set away from their defaults (09).

| What | Status | Why |
| --- | --- | --- |
| XWayland's `_NET_WM_WINDOW_OPACITY` | unsupported | X11-only hints, later (05). |
| XWayland's size increments and the resize popup | unsupported | As `_NET_WM_WINDOW_OPACITY` (05). |
| XWayland's `_NET_WM_FULLSCREEN_MONITORS` | unsupported | As `_NET_WM_WINDOW_OPACITY` (05). |
| X11 startup-notification | unsupported | XWayland windows take the no-token path of focus-stealing prevention until then (05, 166). |
| Session restore | unsupported | Native clients only, once xdg-session-management is stable. No XSMP (05). |
| `org.mate.peripherals-keyboard-xkb.general` `group-per-window` | unsupported | v0.1 has one global keyboard group. Documented rather than logged: Caminito behaves as if it were off (09, 54). |
| `org.mate.peripherals-keyboard-xkb.general` `default-group` | unsupported | As `group-per-window` (09, 54). |
| `org.mate.accessibility-keyboard` `mousekeys-*` | unsupported | Mouse keys need pointer emulation with msd's acceleration curve (13, 09). |
| `org.mate.typing-break` `enabled` | unsupported | The typing break is later (13, 09). |
| `org.mate.peripherals-mouse` `locate-pointer` | unsupported | Caminito will draw the ripple itself (53); 147 put it here, for the developer to bring into M8 with a ticket after 172 if wanted. |
| `org.mate.accessibility-keyboard` `timeout-enable` | unsupported | 13 had it in v0.1, and the AccessX filter doesn't apply it yet (134); 147 put it here, as `locate-pointer`. |
| `org.mate.accessibility-keyboard` `capslock-beep-enable` | unsupported | As `timeout-enable`. Caminito plays no sound until 170. |
| `org.mate.accessibility-keyboard` `feature-state-change-beep` | unsupported | As `capslock-beep-enable`. |

### To do in M8

| What | Status | Why |
| --- | --- | --- |
| The Workspace popup | honoured | Shown by the eight bindings to the workspace beside while their modifiers are held, at once, as in Marco, which has no key for it: see `switch-to-workspace-left` and `move-to-workspace-left`. It's drawn in the GTK theme's colours and font, as the Window switcher is (152). Covered by `caminito/src/tests/workspace_popup.rs`. |
| Edge resistance and snap-move | honoured | Always on, as in Marco: a window being moved or resized stops at the work areas' edges and the other windows' until the pointer goes 32 (16 for a window) pixels past, and with Shift held it snaps to the nearest edges instead (157). Covered by `caminito-policy/src/tests/edge_resistance.rs` and `caminito/src/tests/edge_resistance.rs`. |
| Edge and corner tiling, with the tile preview | honoured | With `allow-tiling` on, a window being moved with the pointer within twice the drag threshold of its output's work area's side, or of a corner, shows the tile preview: where letting go puts its frame, in the palette's selected background, translucent inside a line of it, beneath the moved window. Letting go there tiles it, and moving away hides the preview; a Shift move tiles nowhere. A tiled window gets xdg's `tiled_*` states for the edges it lies along, an SSD Frame drawn in the theme's `tiled_left` and `tiled_right` styles, X configure requests refused, and follows its work area. A titlebar drag past `drag-threshold` untiles it under the pointer, any other move twice that, and maximising it untiles it too (158). A resize by its inner sides, those not along the work area's edges, keeps it tiled at the new size, which it keeps until it's tiled afresh; by any other side it changes nothing, as in Marco (159). Covered by `caminito-policy/src/tests/tiling.rs` and `caminito/src/tests/tiling.rs`. |
| The error dialog for a command that can't run | honoured | A `run-command-*` binding, a screenshot's or the terminal's, whose command is empty or can't be started shows Marco's zenity error dialog, titled *Marco*, with GLib's message, after the command unless it's empty. The arguments are Marco's but `--display` and `--class`, which zenity 4 refuses, and the command and the message are escaped for zenity's markup. The text is Marco's English: Caminito sets no locale. The failure is logged too, and zenity missing is logged with nothing shown, as in Marco (167). Covered by `caminito/src/tests/dialogs.rs`. |
| "Not responding" and Force Quit | to do (168) | Shown when a window doesn't close; Marco has no key for it. |
| Shadows | to do (171) | Turned off by `compositing-manager`, with no key of their own. |
