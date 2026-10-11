# Focus-stealing prevention for native windows

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] Policy tests, written before the code: *smart* and *strict* with a recent token, an old one, one with no input serial, none at all for a first window and for a second; a refused window made urgent and not raised; urgency cleared on focus; `alt-tab-urgent-placement`'s three values in the switcher's list.
- [x] Headless tests through the fixture, on xdg windows: a token from the focused client activating its second window; a token from a window that isn't focused refused under *strict*; a first window with no token focused under *smart*; an urgent window marked in the switcher's grid.
- [x] `focus-new-windows` and `alt-tab-urgent-placement` leave `UNSUPPORTED` if 147 put them there; their rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: a link opened from a terminal into a running Firefox (it passes a token), a new window from a menu, and an app that opens a window by itself after a delay, under each mode. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **What counts as recent and tied to user input** for a token: the age past which a token is old, and whether a token whose serial is the seat's last key or button press is the only one tied to input, or one from any input event of the focused client.
- **How a client's first window is identified:** by the client (its Wayland connection), by its pid, or by its app id having no other window. A launched app that reuses a running process (a second Caja window) is the case that decides it.
- **108's "an urgent window keeps its place"** against Marco master's `alt-tab-urgent-placement`, whose default is `start`: with Marco 1.29.0, which has no key, which applies; and with master's, whether its default is followed.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: what Marco's *smart* and *strict* compare on X11 (user time against the focused window's), which this ticket maps onto tokens, and when Marco clears a window's urgency.

## Triage (2026-10-10)

No option was marked recommended; the agent shipping the ticket took these defaults, and they're recorded in the Done section too.

- **Recent:** issued by Caminito in the last 10 seconds (`TOKEN_LIFETIME`), and not before the seat's focus last changed. **Tied to user input:** the asking client has the keyboard, and the serial is no older than the keyboard's entering it (any input event of the focused client), or than the pointer's, for the click that focused it.
- **A client's first window:** by pid, policy's own fact (socket credentials for a native client, and `_NET_WM_PID` for 166). No criterion makes a second Caja window from the menu a first window, since Caja's desktop is a Layer surface and the earlier browser window is the same process, connection and app id alike.
- **108 against `alt-tab-urgent-placement`:** with Marco 1.29.0's schema, which has no key, an urgent window keeps its place (108); with master's, its value is followed, default `start` included.

## Done (2026-10-11)

- **Landed from the patch of 2026-10-10.** `165-wip.patch` was applied on `master` with `git apply --3way` and its conflicts with 155 and 156 resolved; the patch file is deleted. `master` has one stack for all workspaces since 155, so a refused window is put under the focused one in that stack, and a sticky window is judged on the active workspace as any other.
- **Marco's *smart* and *strict*.** Marco compares the new window's `_NET_WM_USER_TIME` with the focused window's (`intervening_user_event_occurred`, window.c): a window launched before the user's last interaction with the focused window is refused, and a window with no time, or one that maps while nothing has the focus, takes it. *strict* differs from *smart* in Marco only when the focused window is a terminal. Here a token's time is when Caminito issued it, the focused window's user time is when the seat's focus last changed, and *strict* asks for a token tied to user input, as 05 has it.
- **When Marco clears urgency:** when the window is focused (`meta_window_focus` calls `meta_window_unset_demands_attention`). Policy clears it at the same point.
- **The developer's three questions took the Triage section's defaults**, as no option was marked: a token is recent for 10 seconds and not older than the last focus change; it is tied to input if the client with the keyboard asked with a serial no older than the keyboard's or the pointer's entering it; a client's first window is told by pid; and an urgent window keeps its place with Marco 1.29.0's schema and follows `alt-tab-urgent-placement` with master's.
- **Two rules the ticket doesn't name, both Marco's.** A window that maps while nothing has the focus takes it, and so does a window of the focused window's process with no token, under both modes. So a second window with no token is refused only while another process has the focus.
- **`alt-tab-urgent-placement` has two values**, `start` and `mru`, in master's schema, not three. The tests cover `start`, `mru` and a schema with no key.
- **Tested first:** the policy tests in `caminito-policy/src/tests/focus_stealing.rs`, then the headless ones in `caminito/src/tests/focus_stealing.rs` and `switcher.rs` (`an_xdg_window_refused_the_focus_is_marked`).
- **Found in review and fixed, test first:** a window refused while the Window switcher was open with `alt-tab-raise-windows` went back over the focused window at the next step. The stacking the switcher saved now has it under the focused window too.
- **Known and left as they are.** Under *smart* a token gives the focus whatever it was asked with, so an app that asks for its own token and uses it at once takes the focus; *strict* refuses it. Smithay drops an activation request whose token it doesn't know, so that request does nothing: the window isn't made urgent. Old tokens are pruned only when a new one is created, and an old one gives no focus meanwhile.
- **Not run:** the checklist's *Focus-stealing prevention for native windows*, which needs a Session and is 173's. The full test suite was stopped part way at the developer's request, with 1318 tests passed and only the five failures `master` has on aarch64 (four `tests::keymap` tests and `alt_middle_drag_resizes_from_the_nearest_corner::x11`); after the last edits, clippy, `caminito-policy`'s 416 tests and `caminito`'s `focus_stealing`, `switcher`, `translate` and `focus_modes` tests (291) passed.
