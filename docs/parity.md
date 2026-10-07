# Parity with Marco

Each of Marco's `org.mate.Marco.*` keys and keybinding actions, with what Camino does with it. M8's audit fills this table from ticket 01's inventory, and M8 closes when no row is still to do ([17](../.scratch/camino-v0.1/issues/17-milestone-slicing.md)). Until then it has only the rows a ticket has settled.

A row's status is one of:

- **honoured:** Camino does what Marco does with it, or what the row says.
- **never:** Camino will not implement it, for the reason beside it.
- **left to another component:** another program of the Session does it.
- **unsupported:** not yet done, and on the roadmap. It's logged once when the user has changed it from its default (09).

## The Window switcher

Camino has one Window switcher, a grid of thumbnails, in place of Marco's Alt+Tab popup ([ADR 0016](adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md), ticket [108](../.scratch/camino-v0.1/issues/108-window-switcher.md)). Which windows an action lists, their order, and what a quick Alt+Tab goes to are Marco's.

### Actions (`org.mate.Marco.global-keybindings`)

The bindings are Marco's defaults, hard-coded until M8 reads the keys.

| Action | Status | What Camino does | Covered by |
| --- | --- | --- | --- |
| `switch-windows` | honoured | Alt+Tab opens the Window switcher over the active workspace's windows. | `camino-policy/src/tests/switcher.rs`, `camino/src/tests/switcher.rs` |
| `switch-windows-backward` | honoured | Marco's default is `disabled`: Shift with `switch-windows`'s binding opens the switcher on its last entry, and steps it backward. | the same |
| `switch-windows-all` | honoured | Every workspace's windows. Unbound, as in Marco, so nothing reaches it until M8 reads the key. | `camino-policy/src/tests/switcher.rs` |
| `switch-windows-all-backward` | honoured | As `switch-windows-backward`. Unbound. | the same |
| `switch-group` | honoured | Alt+\` opens the switcher over the focused application's windows on the active workspace: those of its process, or of its app id where a window names no process. | both |
| `switch-group-backward` | honoured | Shift with `switch-group`'s binding. | both |
| `switch-panels` | honoured | Ctrl+Alt+Tab opens the switcher over docks and the desktop. Only XWayland windows are docks or a desktop to policy: a Wayland panel is a Layer surface, and isn't listed. | both |
| `switch-panels-backward` | honoured | Shift with `switch-panels`'s binding. | both |

### Keys (`org.mate.Marco.general`)

| Key | Status | What Camino does | Covered by |
| --- | --- | --- | --- |
| `alt-tab-max-columns` | honoured | The grid wraps at this many columns. | `camino/src/switcher/layout.rs`, `camino/src/tests/switcher.rs` |
| `alt-tab-raise-windows` | honoured | The selected window is raised while the switcher is open, and goes back if it's cancelled. | both test modules |
| `compositing-manager` | honoured | Off: the grid shows icons with no thumbnails. Camino always composites, so the key does nothing else. | `camino/src/tests/switcher.rs` |
| `compositing-fast-alt-tab` | honoured | On: the grid shows icons with no thumbnails. | the same |
| `icon-size` | honoured | The size of the icons in the icons-only grid, in logical pixels. A thumbnail's corner icon is a quarter of the thumbnail's height instead. | the same |
| `alt-tab-expand-to-fit-title` | never | The grid's width comes from its thumbnails, and a title wider than the popup is ellipsised (ADR 0016). | `camino/src/tests/switcher.rs` |
| `show-tab-border` | never | Nothing is drawn round the selected window where it sits: its thumbnail shows which it is (ADR 0016). | |
