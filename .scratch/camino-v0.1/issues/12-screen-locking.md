# Screen locking

Type: grilling
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

Which ext-session-lock locker does Camino's `org.mate.ScreenSaver` shim (06) spawn, and how does it look?

- **Options:** an existing locker such as swaylock, gtklock, or waylock, or a small MATE-styled GTK3 locker Camino ships built on gtk-session-lock.
- **Behaviour:** what the lock screen shows (clock, user, the `picture-filename` wallpaper, the session status message) and which of the locker-specific `org.mate.screensaver` keys 09 left open it honours (`lock-dialog-*`, `status-message-enabled`, `user-switch-enabled`, `logout-*`, `embedded-keyboard-*`).
- **Robustness:** how PAM and fingerprint unlock work, and what happens if the locker crashes (ext-session-lock keeps the session locked).
- **Control:** the D-Bus shim's full method set, and whether the locker command goes in `org.mate.Camino` under 09's rule.
