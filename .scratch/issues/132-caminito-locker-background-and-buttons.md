# caminito-locker: the background, the status message and the buttons

Epic: caminito-v0.1
Type: implementation
Status: open
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

- [ ] Each key above is read where mate-screensaver reads it, and the ticket's notes name the function for each.
- [ ] The checklist has an item, tagged M7, and it was run in a Session with two monitors: both show the wallpaper, `picture-filename` replaces it, the status message shows, Switch User reaches the greeter with the session still locked behind it, and Log Out shows after its delay and runs its command. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **Switch User with sddm,** the developer's display manager, isn't one of mate-screensaver's paths. What the button does there is found here; if it does nothing, hide it and say so.
