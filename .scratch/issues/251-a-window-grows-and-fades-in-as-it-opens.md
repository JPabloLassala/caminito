# A window grows and fades in as it opens

Epic: open-close-animation
Type: implementation
Status: resolved
Blocked by: None
Spec: [Opening and Closing animations](../open-close-animation/spec.md)

## What to build

The first slice, and the one a user can try: the **Opening animation** (CONTEXT.md), under "What opens and what closes", "Which windows", "Timing and shape", "Policy and input" and "Settings" in [the spec](../open-close-animation/spec.md). Closing is 252, interruptions 253, and the other effects 254.

- **A window that is shown** for the first time, or again after its client hid it, grows from 0.8 of its size to its own about its centre and fades in from no opacity, over 200 ms on a cubic ease-out, with its Frame, shadow and popups. Both Wayland (`xdg_shell_commit`) and X11 (`xwayland_commit`) reach this through `window_committed` → `map_pending` (`window/lifecycle.rs`).
- **Which windows:** one with a Frame, or of type Normal, Dialog or ModalDialog. Not override-redirect windows, popups, Layer surfaces, or the other types without a Frame. Not a window on a workspace no output shows.
- **Drawn on every frame** of its output from Caminito's clock, with a deadline (`caminito/src/deadlines.rs`), through `scaled_window_elements` (`scene.rs`) as the Overview draws a window at a place, size and opacity.
- **Policy maps it at once.** Focus, input and what the client is told are as with no animation, and a click part-way reaches the window at its real place.
- **`open-close-animation`** in `org.mate.Caminito`, a boolean, default `true`, in both copies of the schema. The animation runs when it is on and the minimise animation's three keys allow (`settings/translate.rs`). A change applies to the next window.
- The fixture keeps the key off unless a test sets it, as it does `workspace-slide`.
- The debug dump lists each opening window with the rectangle and opacity it is drawn at.

## Acceptance criteria

- [x] Headless tests that advance the clock: a Wayland and an X11 window read part-way and at the end, smaller and fainter part-way about the same centre, and at 200 ms at their own place with full opacity.
- [x] A Utility window with a Frame animates; a Menu, a Splashscreen without a Frame, an override-redirect window and a Layer surface don't.
- [x] An X11 window unmapped and mapped again opens again.
- [x] A key press part-way reaches the new window, and a click reaches it at its real place outside the drawn rectangle.
- [x] With `open-close-animation` off, and with each of the three other keys against it, a window is there in one frame.
- [x] The parity tests pass unchanged.

## Done (2026-10-15)

- **Built in a cloud session, not the OrbStack machine.** The worktree was branched from `ccr-6449ec4e-3ps3tn`'s tip by a merge: local `master` (c66c98e) was 34 commits behind it and had neither this ticket nor ADR 0034. Ubuntu 24.04's packages stood in for `tools/check --deps`, and `/tmp/.X11-unix` had to be made by hand for Xwayland to start.
- **Tested first: the maths, then the headless tests.** `opening_rect` and the ease-out have unit tests; `tests/open_close_animation.rs` reads the debug dump's new `opening_windows` and the output's pixels with the clock stopped. To check the drawing can fail its test, the scene's opening branch was turned off on purpose, and the part-way pixel test failed for both kinds.
- **One hook, in `map_pending`.** A window maps into policy as a new id each time its client shows it, so the first show and a show after the client hid it are the same event for both protocols. The refresh pass, after the slide's, starts an opening for each window noted that `is_showing` and animates, so one mapped on a workspace no output shows doesn't open, nor when a switch later shows it.
- **Which windows: `is_ssd()`, not `shows_frame()`,** or type Normal, Dialog or ModalDialog. A fullscreen window has no Frame showing, and the spec has it animate like any other.
- **Drawn through `scaled_window_elements`,** which now takes a `whole` window: its shadow and popups too, cut off only at the output's edge, with what's drawn worked back from the output through the scale. The Overview's callers pass `false` and are drawn as before. 252's picture can use the same path. A window opening during a slide is scaled at each place the slide draws it. Frame debug's outlines aren't drawn for it while it opens; the tile preview is.
- **The ease-out is the slide's,** as `workspace_slide::eased_way_left`, which takes the length.
- **An opening ends when its window stops showing,** whichever way: a minimise, a move away, a switch away. 253 decides what a closing or a minimise part-way hands over from; nothing here reads the reached size at unmap yet.
- **Not tested: a window that maps minimised.** Caminito can't map one (`MapStates` has no such state), so there's no case to test.
- **The schema's text is about opening alone** for now; 252 widens it when windows close with the animation.
- **What ran:** fmt; clippy `-D warnings` on `caminito` (`--no-deps`: this container's clippy 1.97 flags three `manual_range_contains` in `caminito-theme`, untouched here); `caminito`'s full suite (2100, parity included); the rest of the workspace with `--no-fail-fast`, all green except `caminito-theme`'s `goldens`, which differ from Marco's pixels by text rendering on Ubuntu's Pango and fonts, a crate this change doesn't touch. Not run: a real Session; the checklist item the spec asks for is the epic's to add.
- **From the review:** taken were one scaled-window path instead of a copy, the shared ease-out, the tile preview kept part-way, the double look-ups gone, the Scale effect's source cited, and the hidden-workspace test. Left as they are: the `OpenCloseAnimations` names, which 252 fills; the `(id, rect, alpha)` tuple of the dump, since the Overview's `DrawnWindow` is from the output's corner; and whole-pixel rects, as the Overview draws.
