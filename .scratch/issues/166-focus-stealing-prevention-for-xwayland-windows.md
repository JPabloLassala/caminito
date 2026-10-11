# Focus-stealing prevention for XWayland windows, and urgency in the window list

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 165, 155
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

[11](11-xwayland-integration.md)'s focus-stealing rules for XWayland windows, on [165](165-focus-stealing-prevention-for-native-windows.md)'s *smart*, *strict* and urgency, and urgency sent to mate-panel's window list for both kinds of window.

- **XWayland windows carry no token** (X11 startup notification is later, 05), so they take the no-token path:
  - under *smart*, a new XWayland window gets the focus if it's its client's first window, or transient for the focused window; any other becomes urgent;
  - under *strict*, only a transient for the focused window gets it.
- **`_NET_ACTIVE_WINDOW`:** a request with source indication 2 (a pager) is honoured; one with source 1 (an application) is honoured only if that client already owns the focused window, and otherwise makes the window urgent.
- **X11 urgency:** `XUrgencyHint` and `_NET_WM_STATE_DEMANDS_ATTENTION` already make a window urgent, and the Window switcher marks it. What's new is Caminito making one urgent itself, which it writes back as `_NET_WM_STATE_DEMANDS_ATTENTION` alongside [155](155-above-below-and-sticky.md)'s states, and clearing it.
- **The window list:** the Xfce foreign-toplevel protocol's `demands_attention` state is sent for an urgent window of either kind ([ADR 0013](../../docs/adr/0013-window-workspaces-through-xfce-foreign-toplevel-protocol.md)), so that mate-panel's window list can flash it. [65](65-foreign-toplevel.md) kept the window's demand for attention for this.

## Acceptance criteria

- [x] Policy tests, written before the code: each X11 rule under *smart* and *strict*; the two source indications; urgency Caminito sets, and its clearing.
- [x] Headless tests through the fixture, on XWayland windows with the `x11rb` client: a first window focused; a second refused and urgent; a dialog transient for the focused window focused; `_NET_ACTIVE_WINDOW` with each source; `_NET_WM_STATE_DEMANDS_ATTENTION` written back. And through the foreign-toplevel test client, on both kinds: `demands_attention` sent and cleared.
- [x] The `_NET_ACTIVE_WINDOW` and urgency rows in `docs/parity.md`'s keyless section, and `focus-new-windows`' row for XWayland windows, say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: an X11 app that opens a second window by itself under *smart*, its button in mate-panel's window list showing it wants attention, and the urgency clearing when it's clicked. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Whether `mate-panel-caminito` needs another Carried patch** to show `demands_attention` from libxfce4windowing's state, or shows it already. Found here; a patch, if needed, is written here and packaged by [173](173-packaging-and-closing-m8.md).
- **When urgency clears:** proposed, when the window is focused, or when the client clears its hint, as in Marco.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: how Marco treats `_NET_ACTIVE_WINDOW` with source 0 (old clients), and whether it clears `DEMANDS_ATTENTION` itself on focus.

## Done (2026-10-11)

- **The developer's two questions were put to the developer with what was found**, who answered "merge to main" and changed neither. So both stand as below.
- **`mate-panel-caminito` needs no other Carried patch.** libxfce4windowing 4.20.7 reads the Xfce protocol's `demands_attention` as `XFW_WINDOW_STATE_URGENT`, and the panel's Wayland window list gives an urgent window's button the class `urgent`, which its own CSS draws with a bold label (`#tasklist-button.urgent`). It doesn't flash, as libwnck's list does on X11. Seen in a nested Caminito with the panel: `166-checks/panel-urgent.sh`, and `166-checks/strip.png`, the window list before and after an xterm's bell made it urgent.
- **Urgency clears when the window is focused, or when its client takes the demand back**, as in Marco. Policy keeps one demand for attention for a window, Marco's `wm_state_demands_attention`: Caminito makes it for a window it refuses the focus, an X client makes it with `_NET_WM_STATE_DEMANDS_ATTENTION`, and either can end it. Caminito writes it to `_NET_WM_STATE` in the refresh pass, beside 155's states. `WM_HINTS`'s urgency hint is apart, and the client's to end.
- **From Marco's source.** Marco judges `_NET_ACTIVE_WINDOW` by its timestamp against the user's last input, and looks at the source indication only to raise the window (`window_activate`, window.c). A request with source 0 and no timestamp is taken, so here source 0 is honoured as a pager's is, and so is a number Marco doesn't know, which it reads as 0. `wmctrl -a` sends source 0. Marco clears `DEMANDS_ATTENTION` itself on focus (`meta_window_focus`), the client's own demand too, and so does policy.
- **Smithay hands `_NET_ACTIVE_WINDOW` on without its source indication.** Caminito's X connection, which X sends the message too, keeps the source of each one it reads. When Smithay hands a request on, a round trip on that connection makes sure the message has arrived, and the oldest source kept for the window is taken. One kept for a window is forgotten when the window is destroyed.
- **An XWayland window takes 165's no-token path as it is**, with 165's two rules from Marco. So under *strict* a second window of the focused window's process takes the focus too, where this ticket's text gives it to a transient only. A transient of the focused window, or of one of its transients, takes it under both modes, for native windows too, as Marco never refuses one.
- **An application's request (source 1) is honoured while nothing has the focus**, as a window that maps then takes it. `focus-new-windows` doesn't change how a request is judged.
- **A client is told by `_NET_WM_PID` alone.** An XWayland window with none counts as its client's first under *smart* and takes the focus, and its source 1 requests are refused. A fallback to the process the X-Resource extension gives was tried and taken out: it changed how the Window switcher groups XWayland windows, which isn't this ticket's.
- **A window that has the focus demands no attention**, which is nearest to Marco's rule that a window in full view isn't marked. That rule itself isn't here, as it isn't in 165.
- **Tested first:** the policy tests in `caminito-policy/src/tests/focus_stealing.rs`, which failed to compile before the code. Then the headless ones in `caminito/src/tests/focus_stealing.rs`. Three older tests changed: the two launcher tests in `xwayland_windows.rs` set `_NET_WM_PID`, as Qt does, and `a_window_demanding_attention_is_urgent` asks for a window that hasn't the focus.
- **Found in review and fixed:** a source kept for a destroyed window could be taken for a later window with its id; the source is decoded where it's read; source 0 is tested under *strict* too.
- **Not run:** the checklist's *Focus-stealing prevention for XWayland windows, and urgency in the window list*, which needs a Session and is 173's. The full test suite, by the developer's rule: clippy, `caminito-policy`'s 514 tests and `caminito`'s tests of the modules the change touches (478, by the filters `x11`, `xwayland`, `switcher`, `foreign_toplevel`, `debug`, `window_properties`, `root_properties`, `override_redirect`, `focus_modes`, `focus_stealing` and `parity`) passed, but for `xwayland_crashing_again_and_again…`, which fails now and then on `master` and passed alone.

## After (2026-10-11): an application's request is judged by its timestamp

Seen in a Session: Albert, shown by a key binding's command (`albert toggle` on Alt+Space), didn't get the keyboard. Its window asks with `_NET_ACTIVE_WINDOW` and source 1 while another client has the focus, which the rule above refused, and the window was left with `_NET_WM_STATE_DEMANDS_ATTENTION`.

- **An application's request (source 1) is now honoured too if its timestamp is no older than the seat's last change of focus**, which is what 165 takes for the user's last input. It's Marco's rule (`window_activate`, window.c:3143-3154 in 1.28.1), which this ticket left out because X's timestamp wasn't thought to be on Caminito's clock. It is: Xwayland's time and Caminito's clock are both the monotonic clock's milliseconds, X's in 32 bits. Measured in the Session, Albert's `_NET_WM_USER_TIME` as it showed was 17 ms after a reading of that clock. Caminito's X connection keeps the message's timestamp and puts it on its clock as the time nearest now that it can stand for.
- **Looser than before, as Marco is:** any X client that asks with a fresh timestamp takes the focus, and Qt's `requestActivate` gives one every time.
- **Still stricter than Marco:** a source 1 request with no timestamp (0) is refused unless its client has the focus or nothing has, where Marco takes it. The rest of the rule above stands, and `focus-new-windows` still doesn't change how a request is judged.
- **Tested first:** two policy tests and two headless ones, a timestamp since the focus changed honoured under both modes, and an older one refused. `docs/parity.md`'s `_NET_ACTIVE_WINDOW` row says the new rule.
- **Not run:** Albert in a Session on this build. The full test suite: `caminito-policy`'s 530 tests, `caminito`'s `focus_stealing` and `xwayland_windows`, and clippy passed. Of a wider run by the filters `x11`, `xwayland` and `focus`, `the_state_dump_shows_an_ssd_and_a_csd_window_on_two_workspaces`, `a_sides_binding_pressed_again_cycles_its_width_then_untiles::x11` and `the_command_starts_once_xwayland_is_ready…` failed, and fail the same without this change, run inside a Session; `the_commands_status_ends_the_session…` failed too and wasn't run without it.
