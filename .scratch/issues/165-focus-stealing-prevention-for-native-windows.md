# Focus-stealing prevention for native windows

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
Milestone: M8
Blocked by: 153
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`focus-new-windows`, *smart* and *strict*, for native windows, on xdg-activation tokens ([05](05-v01-marco-feature-cut.md)). Today xdg-activation's request handler is empty, so a client that asks to activate a window activates nothing, and `focus-new-windows` isn't read.

- **xdg-activation:** a token Caminito hands out records what it was asked for with (the seat's last input serial, the surface that had the focus, the app id), and when. An activation request with a token activates its window, as `focus-new-windows` allows.
- ***smart*:** a new window, or an activation, takes the focus with a valid and recent token. A client's first window with no token still gets the focus, because MATE's launchers don't pass tokens yet ([05](05-v01-marco-feature-cut.md)).
- ***strict*:** only a token tied to user input gives the focus.
- **Refused:** a window refused the focus isn't raised over the focused one, and becomes urgent.
- **Urgency:** native windows can be urgent now; today an xdg window never is, as no protocol tells Caminito of it ([65](65-foreign-toplevel.md)). The Window switcher marks an urgent window ([108](108-window-switcher.md) has the mark, and nothing has set it for a native window), the debug dump lists it, and urgency clears when the window is focused. `alt-tab-urgent-placement`, where the schema has it, places urgent windows in the switcher's list.
- **The window list** sees urgency through [166](166-focus-stealing-prevention-for-xwayland-windows.md), which sends it for both kinds.

## Acceptance criteria

- [ ] Policy tests, written before the code: *smart* and *strict* with a recent token, an old one, one with no input serial, none at all for a first window and for a second; a refused window made urgent and not raised; urgency cleared on focus; `alt-tab-urgent-placement`'s three values in the switcher's list.
- [ ] Headless tests through the fixture, on xdg windows: a token from the focused client activating its second window; a token from a window that isn't focused refused under *strict*; a first window with no token focused under *smart*; an urgent window marked in the switcher's grid.
- [ ] `focus-new-windows` and `alt-tab-urgent-placement` leave `UNSUPPORTED` if 147 put them there; their rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: a link opened from a terminal into a running Firefox (it passes a token), a new window from a menu, and an app that opens a window by itself after a delay, under each mode. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **What counts as recent and tied to user input** for a token: the age past which a token is old, and whether a token whose serial is the seat's last key or button press is the only one tied to input, or one from any input event of the focused client.
- **How a client's first window is identified:** by the client (its Wayland connection), by its pid, or by its app id having no other window. A launched app that reuses a running process (a second Caja window) is the case that decides it.
- **108's "an urgent window keeps its place"** against Marco master's `alt-tab-urgent-placement`, whose default is `start`: with Marco 1.29.0, which has no key, which applies; and with master's, whether its default is followed.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: what Marco's *smart* and *strict* compare on X11 (user time against the focused window's), which this ticket maps onto tokens, and when Marco clears a window's urgency.

## Work in progress (2026-10-10)

- **Stopped before landing, at the developer's request.** An agent built most of the ticket in one commit (policy's `focus_stealing.rs` with `focus-new-windows` smart/strict judging xdg-activation tokens; a refused window opens below the focused one and stays urgent until focused; urgency shown by the Window switcher and the debug dump and placed by `alt-tab-urgent-placement`; settings translation, policy and headless tests, xdg-activation in the test client, a parity row and a checklist item). Its own ticket notes are inside the patch.
- **Not done:** the Done section, the code review, a test run after the last edits, and the merge.
- **The patch:** [165-wip.patch](165-wip.patch), made with `git diff --binary bd6403f <its commit>`. It conflicts with later merges (155, 156) in `caminito-policy/src/lib.rs`, `policy.rs`, `docs/parity.md` and the checklist, so apply it with `git apply --3way` on a branch that contains bd6403f and resolve those.
