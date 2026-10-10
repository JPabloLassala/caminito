# The visual bell

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

What the user sees when an app rings the bell ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)). [105](105-one-bell-event.md) brought every bell, whatever its source (`gtk_shell1`, xdg-system-bell, X11's `XkbBellNotify`), to one event that's counted and logged; this ticket acts on it.

- **`visual-bell`:** on, a bell flashes as `visual-bell-type` says:
  - `fullscreen`: every output flashes, as Marco's does;
  - `frame_flash`: the bell's window's Frame flashes, as Marco flashes its frame's title bar.
- **Drawn in the Scene,** for the length of one flash, with no change to Policy.
- **`visual-bell` leaves `UNSUPPORTED`,** where 105 put it.

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code, with the driven clock: a bell from an xdg client and from an X11 client flashing every output with `fullscreen`, and the window's Frame with `frame_flash`, read from the output's pixels during the flash and after; nothing with `visual-bell` off; a key change applying to the next bell.
- [ ] `visual-bell` leaves `UNSUPPORTED`; its row and `visual-bell-type`'s in `docs/parity.md` say honoured, with the tests above.
- [ ] [105](105-one-bell-event.md)'s checklist item, or a new one tagged M8: `printf '\a'` in a native and an X11 terminal with each type. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **`frame_flash` on a CSD window, or a bell with no window** (an X11 bell with no focus): proposed, the output the window, or the pointer, is on flashes instead, as Marco falls back to a full-screen flash where there's no frame.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the flash's colour and timing for each type, and what a second bell during a flash does.
