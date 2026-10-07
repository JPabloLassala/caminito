# Marco feature inventory

Type: research
Status: resolved
Blocked by:
Map: [Caminito v0.1](../map.md)

## Question

What does Marco actually do? List every user-visible behaviour and setting, taken from Marco's source (github.com/mate-desktop/marco) and its `org.mate.Marco.*` gsettings schemas. Cover focus modes, placement, the Alt+Tab and workspace popups, edge tiling and snapping, the window menu, keybindings (the full list of actions), workspaces, 'show desktop', the compositor's effects and shadows, and anything else. For each, note the gsettings keys that control it, and flag anything that can't be done on Wayland or that X11 makes easier. Output: a categorised inventory that a later ticket can sort into v0.1, later, and never.

## Answer

Full findings are on branch `research/marco-feature-inventory` (commit 8928527), in `research/marco-feature-inventory.md`. They cover Marco at 41c50d5 and mate-control-center at 24bc243. Mappings marked *(W)* are unverified Wayland protocol guesses.

- **Settings surface:** 5 Marco schemas with 194 keys (40 general, 51 global and 52 per-window keybindings, 15 commands, 36 workspace names). Marco also reads `org.mate.interface`, `org.mate.applications-terminal` and `org.mate.peripherals-mouse`. The file lists all 103 keybinding actions and the window-menu items.
- **Carries over as compositor-internal:** focus modes, auto-raise, placement, modifier+drag, titlebar actions, edge and keyboard tiling, edge resistance and snapping, dynamic workspaces, Alt+Tab options, shadows, and the tile preview and animations.
- **Breaks at the X11 seams with other MATE programs:** `_MATE_PANEL_ACTION` (Alt+F1/F2), `_NET_DESKTOP_LAYOUT` (pager grid layout, which has no gsettings key), `_NET_SHOWING_DESKTOP`, the window list, docks and struts, and mate-control-center's X11-based WM detection and keybinding listing.
- **Unavailable to native Wayland clients** (XWayland keeps them): XSMP restore, per-window opacity, client-requested positions and states, size increments, and multi-monitor fullscreen.
- **Droppable:** `--replace`, marco-message, `disable-workarounds`, `application-based` (never implemented), and `run-command-13`–`32`.
