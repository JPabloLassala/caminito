# Up, down, the previous workspace, and `wrap-style`

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The Workspace grid ([96](96-workspace-grid.md)) gains its other two directions and the previous workspace, and every direction follows Marco's `wrap-style`.

- **The actions:** `switch-to-workspace-up`, `-down` and `-prev`, and `move-to-workspace-up` and `-down`, with Marco's defaults (Ctrl+Alt+Up and Down for the first two).
- **`wrap-style`:** Marco's three styles, `no wrap`, `classic` and `toroidal`, for every direction, the left and right of [96](96-workspace-grid.md) included. Today the grid doesn't wrap.
- **The previous workspace:** the one active before the current one, whichever way the switch came (a binding, the pager, an activation).
- **`wrap-style` leaves `UNSUPPORTED`.** It's there today, as an example in the translation's tests too.

## Acceptance criteria

- [ ] Policy tests, written before the code: up and down in a one-row grid, a full grid and a part-filled one; each wrap style at each edge; `prev` after each kind of switch; `prev` after the count shrinks below it.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the five actions from their bindings; `move-to-workspace-up` carrying the focused window; a `wrap-style` change applying at once.
- [ ] `wrap-style` leaves `UNSUPPORTED`; the five actions' and the key's rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: the four directions in a 2 × 2 grid set from the pager, with each wrap style. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **`classic` and `toroidal` with empty cells,** in a grid whose last row or column is part-filled: where a move into an empty cell lands.
- **`prev` across a switch Marco doesn't count,** such as one done by moving a window.

**For the developer:**

- **What `prev` remembers across a change in the workspace count:** proposed, the previous workspace is forgotten when it's removed, and `prev` then does nothing until the next switch.
