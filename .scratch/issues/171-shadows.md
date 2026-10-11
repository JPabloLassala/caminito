# Shadows

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's compositor shadows around the windows Caminito decorates, on [07](07-decoration-strategy.md)'s model, which M2 left to M8 since `compositing-manager` governs the effects together ([17](17-milestone-slicing.md)).

- **Which windows:** SSD windows only. A CSD window draws its own.
- **The shape:** a Gaussian shadow around the Frame's visible rectangle, following its corners, outside the invisible resize border, with Marco's parameters for each window type and for the focused window.
- **Cached** per size and parameters, so that a frame of the same size draws no new shadow.
- **Off** with `compositing-manager` off.
- **Left out of input,** so a click on a shadow goes to what's under it, and **of captures:** a toplevel capture is the Frame and the window, and [103](103-image-capture.md)'s test that pins it to the Frame still passes.

## Acceptance criteria

- [x] Unit tests of the shadow's geometry, written before the code: its rectangle for a Frame of each type, focused and not, at scales 1, 1.5 and 2.
- [x] Headless tests through the fixture, on xdg and XWayland SSD windows: the shadow's pixels outside the Frame, none for a CSD window, none with `compositing-manager` off; a click on the shadow reaching the window under it; 103's capture test unchanged.
- [x] `compositing-manager`'s row in `docs/parity.md` says it turns shadows off, and the keyless section's shadows say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: a native SSD window and an X11 one, focused and not, at a fractional scale, and with `compositing-manager` off. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: `compositor.c`'s per-type and focus parameters (radius, opacity, offsets), and which window types have none.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Caching and damage at fractional scales:** proposed, the shadow is drawn at the output's scale and cached per scale, and damaged with its Frame.
- **Shaded windows:** proposed, the shadow follows the shaded Frame, as Marco's does.

## Done (2026-10-11)

- **The tests and the code were written together.** The geometry tests of `decoration/shadow.rs` are in the same file as the code, and the two were first compiled together, so no run shows the tests red before the code. Two tests failed on the first run and were corrected (a limit that was too strict, and the fractional test below).
- **Marco's parameters, from `compositor-xrender.c` at `8522ed1`.** They depend on the focus only, not on the window type. The focused Frame has the large shadow: radius 12, offset (-15, -15), so it reaches 15 left and up and 21 right and down. Any other Frame has the medium shadow: radius 6, offset (-9, -7), so it reaches 9 left and right, 7 up and 11 down (`:227-236`, `:2208-2211`, `:3357`, `:3403`). The map is `(ceil(3 × radius) + 1) & ~1` wide, 36 and 18. Marco's small shadow is used for nothing.
- **The opacity is 16/25, not 0.66.** `make_shadow` reads its tables at `(int)(0.66 × 25)` = 16, and `presum_gaussian` multiplies the byte by 16 and divides by 25 (`:484`, `:410-417`). The darkest pixel has alpha 163.
- **Which windows have none.** A maximised Frame and a Frame tiled to the left or the right have no shadow, as in Marco (`:1025-1043`). A Frame of each other type that Caminito draws (normal, dialog, modal dialog, menu, utility, attached) has one. Marco also gives a shadow to windows with no frame (menus, tooltips, docks). Caminito does not, because the ticket says SSD windows only.
- **"Outside the invisible resize border" was read as "not a part of it".** Marco's shadow starts at the Frame's visible rectangle, below the invisible resize border, and Caminito's does too. A click less than 10 pixels from the Frame is the border's, and a click further out goes to what is below the shadow.
- **Decided by the developer: the scale.** The shadow is drawn at the integer scale of the Frame's strips, which is the output's scale rounded up, and the renderer makes it smaller on a fractional output. This is not the ticket's proposal (the exact fractional scale). The reason: the shape that is cut out of the shadow is then the strips' own corner shape, pixel for pixel. The radius and the offsets are logical pixels. A new shadow has a new buffer, so the damage is the renderer's, with the Frame's.
- **Decided by the developer: a shaded window.** The shadow follows the shaded Frame, as the ticket proposed and as Marco's does.
- **The shadow is four bands, not one image.** One band above and one below, each as deep into the Frame as its rounded corners, and one at each side. An image with the full window area in it is 8 MB for a 1920 × 1080 window at scale 1.
- **The cache: for the developer to confirm.** Each Frame keeps one shadow, with what it was drawn from (type, visible size, scale, corner shape). A new title, a hover or a new palette draws the strips again and keeps the shadow. A focus change draws the shadow again, as in Marco, because the cache has one entry and not one for each focus state.
- **At scale 1.5 the Frame can cover the first row of the shadow.** A strip that is one logical pixel high is two physical pixels there, so it goes one physical pixel past the Frame's logical edge. The Frame is above the shadow, so there is no gap. The fractional test starts one pixel out for this reason.
- **Captures and thumbnails.** The shadow is added in `scene::window_elements` only, so a toplevel capture and the Window switcher's thumbnail do not have it. 103's `a_window_capture_is_the_window_with_its_frame` is not changed and passes. A new test, `a_window_capture_leaves_the_shadow_out`, checks the pixel that the rounded corner cuts out.
- **An older test was changed.** `a_framed_window_is_drawn_with_the_themes_strips_around_it` compares each strip with the theme's over the background, and the shadow now shows in the corners' pixels. The test sets `compositing-manager` off first.
- **From the review.** The port first calculated the Gaussian at each pixel. Marco calculates the near half and mirrors it, and its map's centre is not in its middle, so the right and bottom sides were one pixel too far out. The profile is now mirrored. A `fullscreen` branch that Marco does not have was removed.
- **What was run.** In the OrbStack machine (aarch64): `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and one full run of the workspace's tests after the build. Its failures were the ones master has there: caminito-theme's goldens and four `tests::keymap` tests. After the review's corrections, a second full run of `caminito` had one more failure, `tests::lock::a_locker_that_is_killed_leaves_the_session_locked`, which then passed 5 times of 5 alone and does not read a shadow. After that, at the developer's request, only the affected tests were run: the decoration, shadow, capture, frames, settings, parity and lock tests (322 tests, all passed).
- **What was not run.** A third full run, and the shadows in a Session: that is the checklist's item *Shadows around a native and an X11 window*, for 173's closing run.
