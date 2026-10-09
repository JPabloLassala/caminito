# Idle, blanking and the lock after it

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the Session run of the checklist item is 136's
Milestone: M7
Blocked by: 126
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito owns idle ([06](06-v01-protocol-and-integration-set.md)'s *Idle and presence* row, [09](09-gsettings-mapping.md)'s *Idle, blanking and locking*): the `idle` module, with the timers as something a test can drive with its own clock.

- **The threshold:** `org.mate.session idle-delay` (minutes). After it, the outputs are blanked if `org.mate.screensaver idle-activation-enabled` is on.
- **The lock after it:** with `lock-enabled`, a lock is requested `lock-delay` minutes after blanking starts.
- **Activity** on any input device unblanks and restarts the timer. It never unlocks.
- **ext-idle-notify,** with idle clients notified whatever `idle-activation-enabled` says.
- **Inhibition:** the idle-inhibit protocol, counted while its surface is visible, and mate-session's `IsInhibited` (`dbus::session_manager`). Neither applies while locked, when no client surface is visible ([12](12-screen-locking.md)).
- **The debug dump** has the idle state and the inhibitors.

## Acceptance criteria

- [x] Unit or headless tests with a driven clock, written before the code:
  - the outputs blank at `idle-delay`, and not with `idle-activation-enabled` off;
  - a key or a pointer motion unblanks and restarts the timer;
  - the lock comes `lock-delay` after blanking, and not with `lock-enabled` off;
  - an ext-idle-notify client gets `idled` and `resumed`;
  - an idle-inhibit surface that's visible holds idle off, and one on another workspace or minimised doesn't;
  - mate-session's `IsInhibited`, from a stand-in on the bus, holds it off;
  - while locked, the inhibitors hold nothing off.
- [x] A change of each key applies without a restart.
- [ ] The checklist has an item, tagged M7, and it was run in a Session with `idle-delay` at one minute: the screen blanks, a key brings it back, a playing video holds it off, and the session locks after the delay. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **mate-session's own idle presence stays inaccurate:** it's X SYNC over XWayland (06). Whether mate-session then calls anything on idle is found here and written down.
- **Powering the outputs off** is [129](129-dpms.md)'s.

## Done (2026-10-09)

- **The module:** `idle`, with the timers as a pure type, `IdleState`, which takes `now` in every call and says when it next needs to run (15). One loop timer is set for its deadline, as AccessX's is (134). Activity only moves the deadline later, so a timer due earlier is kept, finds nothing, and sets the next: input doesn't set a timer at each event.
- **The driven clock:** in tests, `Caminito::now` adds a `clock_offset`, and `Fixture::advance_clock` moves it on and runs the timers that are due, as the loop timer would. No headless test waits for a timer.
- **ext-idle-notify is Caminito's own,** in `protocols::idle_notify`, version 2. Smithay's runs a loop timer for each notification on the loop's own clock, which a test can't drive, and decides inhibition apart from the blank. Here every notification is a timer of `IdleState`'s, held off by the same inhibitors, and `get_input_idle_notification`'s by none. It's offered to sandboxed clients too: it isn't one of 68's privileged globals, and neither is idle-inhibit, which is Smithay's.
- **The blank** is the scene's top layer, `Blank`, above the cursor and the lock: black, with nothing under it drawn. A lock asked for under it waits to show until activity ends the blank; its `locked` doesn't wait for that, as no frame shows the session either way.
- **The lock after the blank** is `State::request_lock` (126), at the blank's start plus `lock-delay`, counted from when the blank was due. With `disable-lock-screen` it's refused and logged at each blank. Activity before it ends the blank with no lock, as mate-screensaver's lock delay is a grace period. Activity after it ends the blank and shows the lock; it never unlocks.
- **The count:** an inhibitor keeps the session from going idle and resumes the notifications it holds off, as Smithay's ext-idle-notify does; the count starts again when the last inhibitor goes. An inhibitor that comes during a blank leaves it, as mate-screensaver's do. A shorter `idle-delay` that's already passed blanks at once, `idle-activation-enabled` turned on blanks a session that's idle, and `lock-enabled` turned off during a blank asks for no lock. `idle-delay` 0 never blanks, as in mate-session (`gsm-presence.c:170-172`).
- **Visible:** an idle-inhibit surface counts while it's part of a window that policy shows, on the active workspace and not minimised, or of a mapped layer surface. A surface of a popup, or of no role, doesn't count. While the session is locked nothing counts (12).
- **mate-session's inhibitors:** `dbus::session_manager` asks `IsInhibited(8)` when Caminito joins the bus, when the name's owner changes and at every signal of its interface (`InhibitorAdded`, `InhibitorRemoved`), with `NoAutoStart`. The answer is applied in the refresh pass, as the idle-inhibit surfaces are.
- **What mate-session does on idle,** found in mate-session-manager 1.28.0: its presence goes idle when `org.mate.ScreenSaver` says `ActiveChanged(true)` (`gsm-presence.c:183-195`), or on X SYNC's idle time from XWayland, which only X input resets (06); on a change it calls logind's `SetIdleHint`, and nothing else (`gsm-manager.c:2814-2835`). It calls nothing of Caminito's. Once 127 sends `ActiveChanged`, its presence follows Caminito's blank.
- **Activity** is a key, a pointer motion, a button, a scroll, and any gesture, touch or tablet event. The key or button that ends a blank goes on to the client, as with any other: on X11 mate-screensaver's grab while active swallows it. That's left as is.
- **Refocusing isn't activity:** the refresh pass gives the focus to what's come under a pointer that didn't move, through the same motion path as the mouse, so that path's activity is only the devices' entry points'. Found in review: a locker's lock surface mapping under the pointer ended the blank it came under. `a_surface_that_comes_under_the_pointer_leaves_the_blank` covers it.
- **Settings:** idle's new settings are applied after the rest, so a lock that comes of them reads the new `disable-lock-screen`.
- **The debug dump** has `idle`: `blanked`, `inhibited`, the `inhibitors` (each idle-inhibit surface with its window and whether it's `shown`, and `mate-session`), and the number of `notifications`.
- **For 127 and 129:** the shim's inhibitors join `State::refresh_idle`'s sum, and `SimulateUserActivity` is `State::idle_activity`. `SetActive(true)` needs a way to blank at once, which `IdleState` doesn't have yet. 129's DPMS timer is another of `IdleState`'s.
- **Test first:** the settings tests and `IdleState`'s unit tests were written first and failed to build before the code. The headless tests were written after the code that applies the state, against the criteria, and each was then checked by taking out the line it covers: the lock's exception, a window's visibility, the activity of a key and of the pointer, mate-session's answer and the lock request each failed their tests. Leaving every layer under the blank in the scene failed none, since the blank covers them, so `the_blank_is_all_the_scene_has` was added for it.
- **The full suite** passed but for five tests that fail on `master` too in the container, whose xkeyboard-config and Xwayland aren't CI's Arch ones: four of `tests::keymap` and `the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow`. Its clippy, 1.97, flags three `manual_range_contains` in `caminito-theme` that `master` has too; the rest of the workspace is clean.
- **Not tested:** gestures, touch and tablets as activity, which the fixture can't make, and `get_input_idle_notification` through the protocol, which the test client lacks; the unit tests cover its rule.
- **The checklist item** is *Idle blanks, a key brings it back, a playing video holds it off, and the lock follows*, tagged M7. It wasn't run: the ticket was done in a container with no display, so it's left to [136](136-packaging-and-closing-m7.md).
- **The schema:** `org.mate.session` is vendored into `tests/schemas/` from mate-session-manager 1.28.0.
