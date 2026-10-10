# The visual bell

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in; the tests' X11 variants need a run where Xwayland starts (the developer's), and running the checklist item is 173's
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
- [x] `visual-bell` leaves `UNSUPPORTED`; its row and `visual-bell-type`'s in `docs/parity.md` say honoured, with the tests above.
- [x] [105](105-one-bell-event.md)'s checklist item, or a new one tagged M8: `printf '\a'` in a native and an X11 terminal with each type. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **`frame_flash` on a CSD window, or a bell with no window** (an X11 bell with no focus): proposed, the output the window, or the pointer, is on flashes instead, as Marco falls back to a full-screen flash where there's no frame.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the flash's colour and timing for each type, and what a second bell during a flash does.

## Done (2026-10-10)

The flash is the bell event's, in `caminito/src/bell.rs`; the scene draws it (`SceneLayer::BellFlash`, and the Frame's strips), and the tests are `caminito/src/tests/bell.rs`.

- **Written before the code:** the full-screen and frame-flash tests (seen to fail on the xdg side), then the two fallback tests (seen to fail with every output flashing). The visual-bell-off test, the key-change test and the second-bell test went in after the code they cover and passed at once; the second-bell test was checked to fail with a bell during a flash ignored. Each test reads the outputs' pixels during the flash and again after `Fixture::advance_clock(FLASH)`, which now ends a flash that's due as its loop timer would.
- **Not run here:** the `::x11` variants (full-screen, frame-flash, visual bell off), like every X11-tier test on this host, panic at "Xwayland starts" (Ubuntu's Xwayland 23.2); they need a run on the developer's machine. The checklist item is a Session's, for 173.
- **Colour and timing, from Marco's `bell.c`:** both types flash for 100 ms, the timeout of Marco's `bell_flash_window_frame`. Marco's full-screen flash maps a window, fills it white then black and unmaps it at once, with no length of its own; Caminito draws opaque white over each output for the same 100 ms, so the flash is visible on a compositor that draws frames at vblank. `frame_flash` sets the theme's `is_flashing` flag, which draws the Frame in the other focus state, as Marco's `META_FRAME_IS_FLASHING` does. The Frame keeps the extents it was laid out with, so a theme whose focused and unfocused Frames differ in size would draw the flash a little off; none of the stock themes do.
- **A second bell during a flash** starts the flash again, for 100 ms from that bell, and for that bell's window, which ends the first one's. Marco gave each frame flash its own timeout, so the first could end a second's early; one flash at a time is simpler and never shorter.
- **The full-screen flash is under the lock,** so a locked session shows none of it, as it shows nothing else of the session; the blank covers it too.
- **For confirmation, the open question:** a `frame_flash` bell for a window that shows no Frame (CSD, fullscreen, minimised or on another workspace) flashes the output that window is on, and a bell for no window flashes the output the pointer is on, as proposed. Marco falls back to a full-screen flash there (`bell.c:bell_visual_notify`), of the whole X screen; one output is the nearest thing per window. A display beep is the focused window's (105), so it flashes that window's Frame.
- **A bug from 105 fixed on the way:** the translation read `frame-flash` as the nick, but `org.mate.Marco.VisualBellType`'s is `frame_flash`, so a user's `frame_flash` was taken as invalid and fell back to `fullscreen` with a warning. The nick and its test are fixed, and the tests use the schema's spelling.
- **Policy doesn't change:** the flashing state lives in `Bells` and reaches the Frame through `refresh_frames`, and the full-screen flash is a scene element. `visual-bell` left `UNSUPPORTED`; its translation test now checks it's taken without a problem.
