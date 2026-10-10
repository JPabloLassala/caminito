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
| `raise-on-click` | honoured | Off, a click on a window's body focuses it, or its modal dialog, without raising anything; a click on its Frame still raises it, as in Marco. | the same |
| `action-double-click-titlebar` | honoured | An SSD titlebar's double click, by `double-click`'s time (05: SSD frames only). | `caminito/src/tests/frames.rs` (`double_clicking_the_titlebar_runs_its_action`) |
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
| `dynamic-workspaces` | to do (151) | Master's key: off, and read nowhere yet. | |
| `wrap-style` | to do (150) | The Workspace grid doesn't wrap, Marco's default `no wrap`. | |
| `visual-bell` | to do (169) | A bell is counted and logged, and nothing flashes (105). | `caminito/src/tests/bell.rs` |
| `audible-bell` | to do (170) | Read; nothing plays (105). | the same |
| `visual-bell-type` | to do (169) | Read with `visual-bell` (105). | the same |
| `compositing-manager` | honoured | Off: the Window switcher's grid shows icons with no thumbnails. Caminito always composites; the shadows (171) and the minimise animation (172) it turns off come with their tickets. | `caminito/src/tests/switcher.rs` |
| `compositing-fast-alt-tab` | honoured | On: the grid shows icons with no thumbnails. | the same |
| `reduced-resources` | to do (172) | There is no minimise animation for it to turn off yet. | |
| `application-based` | never | Marco never implemented it (05). | the checklist's *The parity table's rows that no test covers* |
| `disable-workarounds` | never | Marco's misfeatures for old or broken X11 applications, which Caminito doesn't carry (05). | the checklist's *The parity table's rows that no test covers* |
| `center-new-windows` | to do (154) | Windows are placed as Marco's `automatic` mode does. | |
| `allow-tiling` | to do (158) | Nothing tiles yet. | |
| `allow-top-tiling` | to do (158) | As `allow-tiling`. | |
| `allow-tile-cycling` | to do (159) | As `allow-tiling`. | |
| `placement-mode` | to do (154) | Always `automatic`, Marco's default. | |
| `show-desktop-skip-list` | to do (164) | Show Desktop hides every window (94). | |
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
| `command-1` to `-12` | to do (149) | Not read: nothing runs them until `run-command-1` to `-12` do. | |
| `command-screenshot` | honoured | Run through the shell by `run-command-screenshot` (104). | `caminito/src/settings/translate/tests.rs`, `caminito/src/tests/commands.rs` |
| `command-window-screenshot` | honoured | Run by `run-command-window-screenshot` (104). | the same |
| `command-area-screenshot` | honoured | Master's key, read where the schema has it, with master's default otherwise; run by `run-command-area-screenshot` (104). | the same |

## `org.mate.Marco.global-keybindings`

An honoured action's binding is Marco's default, hard-coded until 148 reads the keys. A combination Caminito doesn't bind is free for mate-settings-daemon's accelerator grabs, which Caminito signals ([98](../.scratch/issues/98-accelerator-grabs.md), `caminito/src/tests/accelerators.rs`); one it binds isn't signalled. The `switch-*` rows are 108's: which windows an action lists, their order, and what a quick Alt+Tab goes to are Marco's.

| Action | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `run-command-1` to `-12` | to do (149) | Unbound, as by default. | |
| `switch-to-workspace-1` to `-12` | to do (149) | Unbound, as by default. | |
| `switch-to-workspace-left` | honoured | Ctrl+Alt+Left: the workspace to the left in the Workspace grid, stopping at the row's end (96). | `caminito/src/tests/bindings.rs`, `caminito/src/tests/ext_workspace.rs`, `caminito-policy/src/tests/grid.rs` |
| `switch-to-workspace-right` | honoured | Ctrl+Alt+Right, as `switch-to-workspace-left`. | the same |
| `switch-to-workspace-up` | to do (150) | Ctrl+Alt+Up reaches the focused client. | |
| `switch-to-workspace-down` | to do (150) | Ctrl+Alt+Down reaches the focused client. | |
| `switch-to-workspace-prev` | to do (150) | Unbound, as by default. | |
| `switch-group` | honoured | Alt+\` opens the switcher over the focused application's windows on the active workspace: those of its process, or of its app id where a window names no process. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/switcher.rs` |
| `switch-group-backward` | honoured | Shift with `switch-group`'s binding. | the same |
| `switch-windows` | honoured | Alt+Tab opens the Window switcher over the active workspace's windows. | the same |
| `switch-windows-backward` | honoured | Marco's default is `disabled`: Shift with `switch-windows`'s binding opens the switcher on its last entry, and steps it backward. | the same |
| `switch-windows-all` | honoured | Every workspace's windows. Unbound, as in Marco, so nothing reaches it until 148 reads the key. | `caminito-policy/src/tests/switcher.rs` |
| `switch-windows-all-backward` | honoured | As `switch-windows-backward`. Unbound. | the same |
| `switch-panels` | honoured | Ctrl+Alt+Tab opens the switcher over docks and the desktop. Only XWayland windows are docks or a desktop to policy: a Wayland panel is a Layer surface, and isn't listed until 163. | `caminito-policy/src/tests/switcher.rs`, `caminito/src/tests/switcher.rs` |
| `switch-panels-backward` | honoured | Shift with `switch-panels`'s binding. | the same |
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
| `run-command-terminal` | to do (149) | Unbound, as by default. | |
| `rename-workspace` | to do (167) | Unbound, as by default. | |

## `org.mate.Marco.window-keybindings`

As in `global-keybindings`, an honoured action's binding is Marco's default, hard-coded until 148 reads the keys. An action that's *to do* binds nothing, so its default combination reaches the focused client.

| Action | Status | What Caminito does | Covered by |
| --- | --- | --- | --- |
| `activate-window-menu` | to do (161) | Alt+Space reaches the focused client. | |
| `toggle-fullscreen` | to do (149) | Unbound, as by default. | |
| `toggle-maximized` | honoured | Alt+F10 maximises the focused window, or restores it (M1). | `caminito/src/tests/bindings.rs` (`alt_f10_toggles_maximised`) |
| `toggle-above` | to do (155) | Unbound, as by default. | |
| `maximize` | to do (149) | Unbound, as by default. | |
| `unmaximize` | to do (149) | Alt+F5 reaches the focused client. | |
| `toggle-shaded` | to do (149) | Unbound, as by default. | |
| `minimize` | to do (149) | Alt+F9 reaches the focused client. | |
| `close` | honoured | Alt+F4 asks the focused window to close (M1). | `caminito/src/tests/bindings.rs` (`alt_f4_asks_the_focused_window_to_close`) |
| `begin-move` | to do (156) | Alt+F7 reaches the focused client. | |
| `begin-resize` | to do (156) | Alt+F8 reaches the focused client. | |
| `toggle-on-all-workspaces` | to do (155) | Unbound, as by default. | |
| `move-to-workspace-1` to `-12` | to do (149) | Unbound, as by default. | |
| `move-to-workspace-left` | to do (149) | Ctrl+Shift+Alt+Left reaches the focused client. | |
| `move-to-workspace-right` | to do (149) | Ctrl+Shift+Alt+Right reaches the focused client. | |
| `move-to-workspace-up` | to do (150) | Ctrl+Shift+Alt+Up reaches the focused client. | |
| `move-to-workspace-down` | to do (150) | Ctrl+Shift+Alt+Down reaches the focused client. | |
| `raise-or-lower` | to do (149) | Unbound, as by default. | |
| `raise` | to do (149) | Unbound, as by default. | |
| `lower` | to do (149) | Unbound, as by default. | |
| `maximize-vertically` | to do (149) | Unbound, as by default. | |
| `maximize-horizontally` | to do (149) | Unbound, as by default. | |
| `tile-to-side-e` | to do (159) | Unbound, as by default. | |
| `tile-to-side-w` | to do (159) | Unbound, as by default. | |
| `tile-to-corner-nw` | to do (159) | Unbound, as by default. | |
| `tile-to-corner-ne` | to do (159) | Unbound, as by default. | |
| `tile-to-corner-se` | to do (159) | Unbound, as by default. | |
| `tile-to-corner-sw` | to do (159) | Unbound, as by default. | |
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
| `drag-threshold` | honoured | How far a press on an SSD Frame moves before it's a drag (M2). | `caminito/src/tests/frames.rs` (`dragging_the_titlebar_past_the_drag_threshold_moves_the_window`) |

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
| The workspace popup | to do (152) | Shown by the workspace bindings while their modifier is held; Marco has no key for it. |
| Edge resistance and snap-move | to do (157) | Always on in Marco, with Shift for snap-move. |
| "Not responding" and Force Quit | to do (168) | Shown when a window doesn't close; Marco has no key for it. |
| Shadows | to do (171) | Turned off by `compositing-manager`, with no key of their own. |
