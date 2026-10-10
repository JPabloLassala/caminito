# `cycle-*`, and panels that are Layer surfaces

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 148, 108
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The switching actions [108](108-window-switcher.md) left: those with no popup, and the panels that are Layer surfaces ([05](05-v01-marco-feature-cut.md)).

- **`cycle-windows` (Alt+Escape), `cycle-group` (Alt+F6) and `cycle-panels` (Ctrl+Alt+Escape),** and their `-backward` twins: each press activates the next window of the same list as its `switch-*` action, at once, with no Window switcher, as Marco's do.
- **Panels that are Layer surfaces.** Today `switch-panels` lists only docks and the desktop that are XWayland windows, since a Wayland panel is a Layer surface and not a window to Policy. `switch-panels` and `cycle-panels` now also reach the Layer surfaces that accept keyboard focus (on-demand or exclusive interactivity), and give one the keyboard when it's chosen, as 05 asked.
- **In the switcher:** a Layer surface in `switch-panels`' grid has a thumbnail as a window does, and its name from its namespace.

## Acceptance criteria

- [ ] Policy tests, written before the code: each `cycle-*` action's next window, both ways, and the list it walks; the panels' list with Layer surfaces in it.
- [ ] Headless tests through the fixture, on xdg and XWayland windows, and with a layer-shell client: each `cycle-*` action from its default binding (Alt+Escape, Alt+F6 and Ctrl+Alt+Escape), and the `-backward` twins from Shift and from a binding set in the test; `switch-panels` listing a Layer surface with on-demand interactivity and not one with none; committing to it giving it the keyboard.
- [ ] The six `cycle-*` actions' rows in `docs/parity.md` say honoured, and `switch-panels`' row no longer says Layer surfaces aren't listed, with the tests above.
- [ ] The checklist has an item, tagged M8: Ctrl+Alt+Tab and Ctrl+Alt+Esc reaching mate-panel and the desktop, and Alt+Escape and Alt+F6 among a native and an X11 window. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: whether `cycle-*` walks the most recently used order, as `switch-*` does, or the stacking order; whether a cycled window is raised at once; and what Marco's `cycle-panels` does with a single panel.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **How a Layer surface is focused for the panel:** the keyboard is given to it, and mate-panel, a GTK3 client on gtk-layer-shell, must then let a key move between its applets. Whether it does, and whether caja's desktop takes the keyboard, is found here.
- **Which cooperation from mate-panel or caja is a known gap:** what doesn't work without a carried patch, written in the ticket and in parity's row, rather than patched here.
