# caminito-locker: the background, the status message and the buttons

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the checklist item is 136's
Milestone: M7
Blocked by: 131
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The rest of [12](12-screen-locking.md)'s *What the lock screen shows* and of its table of `org.mate.screensaver` keys.

- **The background:** the desktop background through `MateBG` on every output's lock window, replaced by `picture-filename` when that file exists.
- **The status message,** with `status-message-enabled`: read by the locker from mate-session's presence `StatusText`.
- **Switch User,** with `user-switch-enabled` and without `org.mate.lockdown disable-user-switching`, keeping mate-screensaver's display-manager paths (MDM, GDM, and LightDM over `XDG_SEAT_PATH`).
- **Log Out,** with `logout-enabled`: shown only if `logout-command` is set, after `logout-delay` minutes.
- **Never:** `embedded-keyboard-enabled` and `-command`, `power-management-delay`, `screensaver-arguments`.

## Acceptance criteria

- [x] Each key above is read where mate-screensaver reads it, and the ticket's notes name the function for each.
- [ ] The checklist has an item, tagged M7, and it was run in a Session with two monitors: both show the wallpaper, `picture-filename` replaces it, the status message shows, Switch User reaches the greeter with the session still locked behind it, and Log Out shows after its delay and runs its command. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **Switch User with sddm,** the developer's display manager, isn't one of mate-screensaver's paths. What the button does there is found here; if it does nothing, hide it and say so.

## Done (2026-10-09)

- **Where each key is read,** in the locker as in mate-screensaver:
  - The background: `apply_background_to_window`, ported from `gs-manager.c` into `caminito-locker.c`, loads `MateBG` from `org.mate.background` with `mate_bg_load_from_preferences` and puts `picture-filename` in its place when `g_file_test` finds the file, for each lock window at its size. `draw_background`, ported from `gs-window-x11.c`'s `gs_window_real_draw`, paints it under the window's content.
  - `status-message-enabled`, `user-switch-enabled`, `org.mate.lockdown disable-user-switching`, `logout-enabled`, `logout-delay` and `logout-command`: `gs_prefs_load_from_settings` at the start and `key_changed_cb` on a change, in `gs-prefs.c`, ported from mate-screensaver's and cut to these keys. The locker gives them to the dialog in `update_from_prefs`, ported from `gs-monitor.c`'s `_gs_monitor_update_from_prefs`: Switch User is `user-switch-enabled` and not `disable-user-switching`, and the status message is shown only with `status-message-enabled`.
  - The status message: `connect_presence_watcher` in `gs-watcher.c`, ported from `gs-watcher-x11.c` and cut to the presence's `status-text` and `StatusTextChanged`, on GDBus where mate-screensaver had dbus-glib. `on_watcher_status_message_changed` puts a new one in the dialog, as `gs-monitor.c`'s does. `status-text` is read asynchronously, so a mate-session that doesn't answer can't hold up the lock windows (mate-screensaver's daemon read it at its own start, not at a lock), and again when mate-session gets a new owner, where mate-screensaver stayed with the first and warned without one.
  - Log Out: `update_logout_enabled`, ported from `gs-window-x11.c`'s `is_logout_enabled`: shown with `logout-enabled` and a command once `logout-delay` has passed, and asked again by a timeout at that time, as the locker's dialog lasts the whole lock where mate-screensaver's daemon asked once per dialog. The delay runs from the locker's start, where mate-screensaver's timer starts when its windows show at the blank.
  - Switch User's display-manager paths are `gs-lock-plug.c`'s, as 131 ported them: MDM's and GDM's `flexiserver` when their process runs, and LightDM's `SwitchToGreeter` on `XDG_SEAT_PATH`.
  - *Never:* `embedded-keyboard-enabled`, `embedded-keyboard-command`, `power-management-delay` and `screensaver-arguments` are read by nothing: `gs-prefs.c` leaves them out, and its `key_changed_cb` ignores them rather than warn.
- **Switch User with sddm:** it works through mate-screensaver's LightDM path, from sddm's source (`da0b64d`): sddm sets `XDG_SEAT_PATH` in the session's environment (`Display.cpp`), its system-bus policy lets any user call `org.freedesktop.DisplayManager.Seat`, and its `SwitchToGreeter` activates the greeter's session or starts one (`SeatManager.cpp`). Caminito passes its environment to the locker, so the button shows and reaches the greeter. It wasn't run here; the checklist item's step 4 checks it.
- **What the port changed,** each said in the file's header:
  - MateBG makes a solid colour's surface 1×1, which X tiled as the lock window's background pixmap; cairo needs `CAIRO_EXTEND_REPEAT` to do the same, or only the first pixel has the colour.
  - The dialog was a `GtkPlug`, a window, with the theme's window background. As a `GtkBin` in a lock window whose background is now the desktop's, it has a window's CSS node and the `background` class and draws that background itself, so the theme's `window` and `window.lock-dialog` rules still give it its look.
  - The status message's label is no-show-all, as the buttons are, since a lock window's `show_all` showed it empty.
  - An empty `logout-command` is no command, so Log Out isn't shown for one. In mate-screensaver it is, and does nothing but warn: the spec says the button appears only if the command is set.
  - mate-screensaver applies a new status message with `status-message-enabled` off, which is its bug: here the key holds for a new message too.
  - From 131: the plug cleared the clipboards through `gtk_widget_get_clipboard`, which a widget outside a window can't answer, so they weren't cleared and GTK logged six criticals at each start. It clears the display's.
- **The tests:**
  - `helpers/caminito-locker/test-watcher.c`, a Meson test, runs `gs-watcher.c` against a stand-in mate-session presence on a `GTestDBus` bus: the status text at the start, a `StatusTextChanged`, no presence, and a presence that comes after the watcher.
  - `caminito/src/tests/caminito_locker.rs` gives the real locker a keyfile GSettings backend, where it had a memory one, so a test can set keys and change them while it runs. On two outputs, both lock windows show `org.mate.background`'s colour; `picture-filename` replaces it when the file exists and not when it's missing; and `disable-user-switching`, `logout-enabled`, `logout-delay`, `logout-command` and `status-message-enabled` reach the dialog, at the start and when changed. The background tests failed before the repeat was fixed, with the dialog on black.
  - The test schemas have mate-desktop 1.28.2's `org.mate.background`, which MateBG reads.
- **Not tested headless:** Log Out after a delay of a minute or more, the status message from a real mate-session, and the buttons' clicks; the checklist item covers them.
- **The checklist item** is *caminito-locker's background, status message, Switch User and Log Out*, tagged M7. It wasn't run: the container has no display or display manager. It's left to [136](136-packaging-and-closing-m7.md). 131's item now expects the desktop's background, not a plain one.
