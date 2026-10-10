# 24. A tiled window is a state of its own beside maximise

Date: 2026-10-10
Status: Accepted

## Context

Ticket 158 ports Marco's tiling by drag: a window dropped at an output's side fills that half of the work area, at a corner that quarter, and at the top all of it. Policy already has horizontal and vertical maximise, and 144 calls a window maximised one way "snapped". Marco builds a side tile on vertical maximise and a corner tile on horizontal maximise, with a `tile_mode` and a `tiled` flag beside them (window.c:2697-2730), and every check of a maximised window then has to tell the tiled ones apart (`META_WINDOW_TILED`, `META_WINDOW_SIDE_TILED`, `META_WINDOW_CORNER_TILED`). A left half is vertical maximise plus a place and a width, but a quarter isn't any maximise.

## Decision

- **Policy keeps a tile of its own**: which half or quarter (`Tile`), on which output's work area, beside the two ways of maximising. A tiled window isn't maximised either way, and a window maximised one way isn't tiled.
- **It shares maximise's floating geometry.** A tiled window goes back to `Managed::floating` when it's untiled, and maximising one leaves the tile and keeps that geometry, as Marco's `meta_window_maximize` does.
- **The top edge maximises.** `allow-top-tiling` has a drop at the top maximise the window both ways, as Marco's `META_TILE_MAXIMIZED` does, so it isn't a tile.
- **144's half-maximised windows stay what they are.** Tiling one clears its maximise; it keeps the floating geometry it had.

## Consequences

- `WindowStates` carries the tile, so the xdg `tiled_*` states and the Frame's tiled styles come from it, and an X11 window's configure requests are refused while it's tiled, as `Managed::constrained` covers it.
- An X11 client isn't told a tile through `_NET_WM_STATE`: Marco publishes a side tile as `_NET_WM_STATE_MAXIMIZED_VERT`, a side effect of building it on maximise that Caminito doesn't have.
- Tile cycling and the tiling keybindings (159) add to `Tile` and its area, not to maximise.
- Rejected: building tiles on vertical and horizontal maximise, as Marco does (a quarter isn't one, and every maximise check would have to exclude tiles).
