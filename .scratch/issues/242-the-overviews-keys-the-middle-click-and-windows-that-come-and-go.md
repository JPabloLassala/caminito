# The Overview's keys, the middle click and windows that come and go

Epic: overview
Type: implementation
Status: resolved
Blocked by: 241
Spec: [Overview](../overview/spec.md)

## What to build

The Overview used from the keyboard, and kept right while it is open, under "The keys", "The pointer" and "Which windows" in [the spec](../overview/spec.md). It builds on ticket 241's Overview.

- **The selection** is policy's. The focused window is selected at opening, or none when no window is focused. The selected window is marked in the layout.
- **The arrow keys** move the selection to the nearest window that way in the layout. With none selected, an arrow key selects the first.
- **Enter** chooses the selected window, as a click on it does: it is activated, unminimised if it must be, and the Overview closes. With none selected, Enter does nothing.
- **A middle click on a window** closes that window. The Overview stays open.
- **A window that opens** while the Overview is open joins the layout, and **one that closes** leaves it. The others take their new places in one frame, until 243. If the selected window closes, none is selected.
- The debug dump shows the selection.

## Acceptance criteria

- [x] Policy's tests, with no protocol objects: the selection at opening with a focused window and with none, each arrow key in a layout of two rows, an arrow key at the layout's edge, Enter with a selection and with none.
- [x] Headless tests that read the dump: the arrow keys and Enter activate the window chosen and close the Overview.
- [x] A middle click on a window sends it a close request, and the Overview is still open.
- [x] A window that maps while it is open is in the layout's next frame, and one that goes isn't. The selection is none after its window goes.

## Done (2026-10-14)

- **The selection is policy's**, as `Overview::selected`: the focused window at opening if the Overview lists it, otherwise none. It goes to none when its window leaves the list, at the same `apply` that takes the window out. The proptest invariants check that it's always a listed window, with ops for the arrows, Enter and the middle click.
- **Policy is given the layout's places with each arrow key** (`overview_select(direction, places)`), since the layout is the compositor's. So policy's tests use rectangles they write themselves, and need no compositor.
- **"The nearest window that way" is my rule**, as the spec doesn't give one: a window wholly past the selected one's edge on that side. Left and Right take only one on the same row, and stay at a row's end; the review found that a centred short row let Left jump up a row. Up and Down prefer a window in the same column, and then the nearest centre. **"The first"**, with none selected, is the top row's left window: the one whose bottom edge is highest, since a row's windows stand on one line, and then the left one. The top edge doesn't work: it picks the tallest window.
- **The arrow keys and Enter act only when they're no binding**, so Shift+Left moves the selection and Ctrl+Alt+Left is swallowed until 244 gives the workspace bindings their work. Keypad Enter is Enter. The keypad's arrows aren't arrows, as in the Window switcher.
- **The mark** is the selected window's label, filled in the theme's selection with its selected text, as the switcher's selected cell is.
- **A middle click on a window or its label** asks the window to close (`overview_close_window`, then `close`), as a left click on either chooses it. The Overview stays open, and the window leaves once its client destroys it.
- **A window that maps while it's open can take the focus**, as policy has it with no Interaction: after Esc that new window is focused. The spec says nothing on this. It's left for the developer, and for 247 in a real Session.
- **Tests:** policy's were written before the code; the headless tests after the first build. To check that they can fail, the arrow keys, the middle click, the label's mark and the selection's going with its window were broken on purpose, and nine headless tests failed.
- **What ran:** `caminito-policy`'s whole suite (712), and `caminito`'s tests filtered on `overview`, `debug` and `state_dump` (79), with fmt and clippy. Before the review fixes, also `switcher::draw` and `workspace_popup` (120 with the others). Not the full suite, by the standing rule. Not run in a real Session: that's 247's.
- **From the review, left as it is:** the Keysym-to-direction match beside the switcher's (they map to two different `Direction` types), and the fourth copy of the tests' `destroy` helper (moving the other three into `Fixture` is outside this ticket).
