# 16. One window switcher, a thumbnail grid, in place of Marco's Alt+Tab popup

Date: 2026-10-07
Status: Accepted

## Context

Marco is Camino's reference, and M8's exit is a parity table with no row still to do (17). Tickets 05 and 09 put Marco's Alt+Tab popup in v0.1 "with thumbnails and all its options": a row of icons or small thumbnails, a border drawn round the selected window where it sits, and the `alt-tab-*`, `show-tab-border` and `icon-size` keys honoured as in Marco.

The developer wants a switcher like KDE Plasma's instead: its Thumbnail Grid, with live thumbnails large enough to tell windows apart, the mouse, and the arrow keys. Until M8 Alt+Tab has no popup at all and activates the most recently used window.

Two switchers, Marco's by default and the grid behind a key, would be two popups to draw, test and keep, for a difference most users would never choose between.

## Decision

- **Camino has one window switcher, and it's a thumbnail grid.** It stands in for Marco's popup, and there's no key that brings Marco's back. This amends 05's "Alt+Tab with thumbnails and all its options" and 09's "`alt-tab-*` as in Marco".
- **What Marco decides, Camino still decides as Marco does:** which windows are listed and in what order, which action lists what, and what a quick Alt+Tab goes to. Only what the popup looks like, and how the pointer and the arrow keys work in it, is taken from KDE.
- **Camino draws it itself, in the scene, as it draws frames.** It lives on the keyboard's grab and on the modifier's release, which only the compositor sees, and a thumbnail is the window's own surface drawn smaller.
- **Its state is policy's** (ADR 0010): the windows listed, their order, the selection, and committing or cancelling. The compositor crate lays the grid out, draws it, and tells policy of keys and clicks.
- **Marco's keys are honoured where they still mean something.** `alt-tab-max-columns` wraps the grid, `alt-tab-raise-windows` raises the selected window while the switcher is open, and `compositing-manager` off or `compositing-fast-alt-tab` on shows icons with no thumbnails. `show-tab-border` is never honoured, since nothing is drawn round the selected window: its thumbnail already shows which it is.
- **No effect is taken from KDE.** The other windows aren't faded or hidden while one is selected, as showy effects are out of scope.

Ticket 108 has the behaviour in full.

## Consequences

- The parity table gets rows that say *never* with this ADR as the reason, where 05 and 09 promised *honoured*. They're a decision, not a gap, and the audit reads them so.
- A user coming from Marco sees a different popup. The keys they know still work the same way: Alt+Tab, Alt+Shift+Tab, Alt+\`, Ctrl+Alt+Tab, and releasing Alt.
- Camino needs to draw a window's surface at another size and place than its own, for a window that may be minimised or on another workspace. The scene draws each window once today, and capture (103) is the nearest thing to it.
- Marco's popup has no goldens to compare against, as frames have (ADR 0009), so the switcher's look is tested against Camino's own.
- Rejected:
  - Marco's popup alone (what the developer set out to replace);
  - both, chosen by an `org.mate.Camino` key (two popups to keep for one job);
  - a client on a layer surface that Camino feeds (it would need a private protocol for the grab, the modifier's release and the thumbnails, where Camino has all three already);
  - KDE's other layouts, the sidebar, large icons and the compact list (the grid is the nearest to Marco's centred popup, and thumbnails were already v0.1's).
