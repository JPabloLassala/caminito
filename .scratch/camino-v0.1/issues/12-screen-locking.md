# Screen locking

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

Which ext-session-lock locker does Camino's `org.mate.ScreenSaver` shim (06) spawn, and how does it look?

- **Options:** an existing locker such as swaylock, gtklock, or waylock, or a small MATE-styled GTK3 locker Camino ships built on gtk-session-lock.
- **Behaviour:** what the lock screen shows (clock, user, the `picture-filename` wallpaper, the session status message) and which of the locker-specific `org.mate.screensaver` keys 09 left open it honours (`lock-dialog-*`, `status-message-enabled`, `user-switch-enabled`, `logout-*`, `embedded-keyboard-*`).
- **Robustness:** how PAM and fingerprint unlock work, and what happens if the locker crashes (ext-session-lock keeps the session locked).
- **Control:** the D-Bus shim's full method set, and whether the locker command goes in `org.mate.Camino` under 09's rule.

## Answer

Decided on 2026-09-28. The user asked for the recommended answer to every question to be taken, so there was no round-by-round grilling. Inputs were what [06](06-v01-protocol-and-integration-set.md), [08](08-session-launch-model.md), [09](09-gsettings-mapping.md) and [11](11-xwayland-integration.md) handed on, and a source read of mate-screensaver `60b2db3`, gtk-session-lock `b9ddb27`, gtklock `3ba3671` and swaylock `44b82de`. The relevant facts are quoted inline.

The hard-to-reverse part, Camino shipping its own locker ported from mate-screensaver's lock dialog, is recorded in [ADR 0007](../../../docs/adr/0007-camino-locker-ported-from-mate-screensaver.md).

### Which locker

- **Camino ships `camino-locker`**, a small C GTK3 program built on gtk-session-lock. It's a port of mate-screensaver's unlock dialog: `mate-screensaver-dialog.c` (611 lines), `gs-lock-plug.c` (2305) and `gs-auth-pam.c` (810), all GPL-2.0-or-later. The one structural change is that `GSLockPlug` is a `GtkPlug` (XEmbed, `gs-lock-plug.c:157`) and becomes a widget inside one gtk-session-lock window per monitor.
- **Why not an existing locker:** swaylock and waylock can't show MATE's dialog or honour any `org.mate.screensaver` key; gtklock is GTK3 on gtk-session-lock too, but its layout and modules are its own, so matching MATE means configuring around it rather than reusing MATE's dialog. The same shape as `camino-gtk-palette` (ADR 0003): a small C GTK3 helper where the GTK side is what matters.
- **The dialog is MATE's own.** `camino-locker` loads `lock-dialog-<lock-dialog-theme>.ui` and `.css` from mate-screensaver's data directory, as mate-screensaver does (`gs-lock-plug.c:1939-1949`), and falls back to a vendored `lock-dialog-default.ui` in a gresource if the file is missing. Colours and widgets come from the GTK theme.
- **Any locker still works.** The locker command is the `org.mate.Camino` key `locker-command` (`as`, default `['camino-locker']`, an argv, no shell). It meets 09's rule: it's Wayland-only and no MATE schema expresses it. Set to `['swaylock']` or `['gtklock']`, the lock is still correct; only the MATE keys below stop applying. That's also the development loop's escape hatch (14).
- **Accepted risk:** gtk-session-lock (GTK3) last changed in April 2024, and gtklock depends on it too. If it stops building, the fallback is to vendor it; it's a small fork of gtk-layer-shell.

### What the lock screen shows

Exactly mate-screensaver's unlock dialog:

- **Background:** the desktop background through `MateBG`, replaced by `picture-filename` when that file exists (`gs-manager.c:1050-1056`). Every output gets its own lock surface with the background; the dialog goes on the output under the pointer, as in mate-screensaver.
- **Dialog:** the user's face image and real name, the password prompt and PAM messages, the clock and date, the session status message, and the Switch User and Log Out buttons when enabled.
- **Status message:** read by `camino-locker` itself from mate-session's presence `StatusText` (mate-screensaver's `gs-watcher-x11.c:408`), passed nowhere by Camino.
- **No keyboard layout indicator.** It's libmatekbd (X11 through libxklavier, `gs-lock-plug.c:46`), so it's compiled out. Layout switching still works, because Camino handles it (below).
- **Popups don't work** on a lock surface (gtk-session-lock's README), so the ported dialog must not use tooltips or menus. The default `.ui` has none.

### `org.mate.screensaver` keys

Completing 09's list:

| Key | Status |
|---|---|
| `lock-dialog-theme`, `lock-dialog-time-format`, `lock-dialog-date-format` | honoured by `camino-locker` |
| `picture-filename` | honoured by `camino-locker` |
| `status-message-enabled` | honoured by `camino-locker` |
| `user-switch-enabled`, and `org.mate.lockdown disable-user-switching` | honoured by `camino-locker`, keeping mate-screensaver's display-manager paths (MDM, GDM, and LightDM over `XDG_SEAT_PATH`, `gs-lock-plug.c:205-260`) |
| `logout-enabled`, `logout-delay` (minutes), `logout-command` | honoured by `camino-locker`. As in mate-screensaver, the button appears only if the command is set, after the delay (`gs-window-x11.c:1664-1680`). |
| `org.mate.lockdown disable-lock-screen` | honoured by Camino: locking is refused and logged. Blanking still happens. |
| `embedded-keyboard-enabled`, `embedded-keyboard-command` | *never*: the keyboard is an XEmbed `GtkSocket` (`gs-window-x11.c:1249-1259`), and a lock surface can't host another client. An on-screen keyboard at the lock screen would need a new mechanism, not these keys; that question goes to 13 ([13](13-hidpi-and-accessibility-scope.md): later). |
| `power-management-delay`, `screensaver-arguments` | *never*: blanking and DPMS already come from 09's keys, and screensaver arguments belong to the X11 themes. |

`mode`, `themes` and `cycle-delay` stay *never* (09).

### Locking, fail closed

- **Camino locks first; the locker only draws.** When a lock is requested (the shim, idle plus `lock-delay`, or logind `Lock`), Camino enters its locked state at once: every output shows a solid dark background, input stops going to clients, and it spawns the locker. The locker's `ext_session_lock_v1.lock` then attaches to that state. Nothing on screen is visible between the request and the locker's first frame, even if the locker is slow or never starts.
- **Only the locker unlocks.** The session unlocks only on `unlock_and_destroy` from the client holding the lock, or on logind `Unlock` (below). This deviates from mate-screensaver, whose D-Bus `Unlock` deactivates without authentication (`gs-listener-dbus.c:1533-1535`).
- **Who may lock:** 12 adds ext-session-lock to 06's list of privileged globals, so it's offered to every client except sandboxed ones. A `swaylock` started by hand counts as a lock like any other.
- **Who may take over a lock:** once the session is locked and the lock's client is gone, only a locker Camino itself spawned (matched by the client's pid) may take the lock over. Any other client's `lock` is refused, so killing the locker doesn't let another process take the lock and unlock it.
- **Over everything:** the lock sits above every layer, including override-redirect windows and `overlay` layer surfaces, and Xwayland keyboard grabs are refused and released while locked (11). The session isn't visible to screen capture while locked: capture sources render the lock background.
- **Keys while locked:** Camino handles only VT switching and keyboard layout switching. [13](13-hidpi-and-accessibility-scope.md) adds two exceptions: AccessX still applies, and Orca's keyboard monitor keeps working. Every other Camino binding, and every accelerator grabbed through `GrabAccelerators` (so msd's media keys), is suppressed, matching X11 where mate-screensaver's keyboard grab blocks msd's passive grabs. The power button is logind's.
- **Idle inhibitors don't apply while locked:** no client surface is visible, so blanking and DPMS run on their normal timers.

### Robustness

- **PAM service `mate-screensaver`**, the same one mate-screensaver uses (`gs-auth-pam.c:368`), so the distro's file and any user changes (pam_fprintd, pam_gnome_keyring) carry over and packaging ships no PAM file. PAM runs in a thread, as ported.
- **Fingerprint** works through that PAM stack, sequentially with the password, exactly as with mate-screensaver on X11. Parallel fprintd over D-Bus is *later*.
- **The locker crashes while locked:** the session stays locked (ext-session-lock, and Camino's own locked state). Camino restarts it with 08's helper backoff, and the new locker takes the lock over (above).
- **The locker keeps failing** (new here; 08 set no general give-up rule): after five failed starts within a minute, Camino stops respawning, stays locked and draws, with the theme engine's cairo and pango, a one-line message: switch to a text console and run `loginctl unlock-session <id>`, with the session's actual ID filled in (a bare `loginctl unlock-session` from a console acts on the console's own session). There's no way out from inside the session.
- **logind:** Camino listens to its session's `Lock` and `Unlock` signals, as mate-screensaver does (`gs-listener-dbus.c:1770-1790`). `Unlock` is the text-console recovery path: Camino ends the lock and stops the locker. logind lets the session's own user send it without authentication, so any process of that user can unlock, as on X11 with mate-screensaver. That's accepted for a single-user session. Camino sets `SetLockedHint` on every lock and unlock, so display managers and `loginctl` see the state.
- **Suspend:** whether to lock on suspend is still mate-power-manager's key; it calls `Lock` (09). Camino holds a logind `sleep` delay inhibitor, as mate-screensaver does (`gs-listener-dbus.c:1697-1700`). On `PrepareForSleep(true)`, it releases it once any pending lock has its lock surfaces rendered on every output, or after 2 s, and takes it again on resume. The machine never suspends with the desktop on screen and a lock pending.

### The screensaver shim

Camino owns `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` **before spawning mate-session** (08's order). mate-screensaver's autostart then finds the name taken, logs "screensaver already running in this session" and exits (`gs-listener-dbus.c:2362-2368`, `mate-screensaver.c:104`); its `.desktop` has no auto-restart. No carried patch is needed.

`org.mate.ScreenSaver`, `/org/mate/ScreenSaver`, matching mate-screensaver's interface (`gs-listener-dbus.c:1374-1420`):

| Member | Behaviour |
|---|---|
| `Lock()` | Lock as above, unless `disable-lock-screen`. Returns at once. |
| `Unlock()` | Refused with `AccessDenied`. Only the locker or logind unlocks. |
| `SetActive(b)` | `true` blanks now and starts the `lock-delay` timer, like reaching idle. `false` unblanks if not locked, and never unlocks. |
| `GetActive() → b`, `GetActiveTime() → u` | True while blanked or locked; seconds since then. |
| `SimulateUserActivity()` | Resets the idle timer and unblanks. Never unlocks. |
| `Inhibit(s s) → u`, `UnInhibit(u)`, `GetInhibitors() → as` | Idle inhibitors, added to 06's idle-inhibit and mate-session `IsInhibited`. Tracked per sender and dropped when it leaves the bus. |
| `Throttle(s s) → u`, `UnThrottle(u)` | Accepted, no effect: throttling only affects screensaver themes. |
| `Cycle()`, `ShowMessage(s s s)` | Accepted, no effect. |
| `Quit()` | Not provided (mate-screensaver handles it without introspecting it, `gs-listener-dbus.c:1538`). `mate-screensaver-command --exit` gets `UnknownMethod`: Camino's shim can't quit. |
| `ActiveChanged(b)` signal | On every blank or lock change. |

`org.freedesktop.ScreenSaver`, on `/org/freedesktop/ScreenSaver` and `/ScreenSaver`: `Inhibit` and `UnInhibit` only, sharing the same inhibitor set, as mate-screensaver does. This covers X11 apps that inhibit idle over D-Bus rather than through idle-inhibit.

### Handed on

- **13 HiDPI and accessibility:** an on-screen keyboard at the lock screen (the XEmbed one is gone), and screen-reader access to the dialog (GTK3's AT-SPI works on Wayland, but Orca under Camino is untested). ([13](13-hidpi-and-accessibility-scope.md): the lock-screen keyboard is later, and Orca's keyboard monitor keeps working while locked.)
- **14 Development and testing loop:** kill the locker while locked and check it comes back locked; run the give-up path; the suspend race with a slow locker; `loginctl unlock-session` recovery; `locker-command` set to `swaylock` for nested runs.
- **15 Crate and module architecture:** the lock state machine (blank, pending, locked, locker-lost), the two D-Bus names, logind, and `camino-locker` as a C subproject beside `camino-gtk-palette`.
- **16 Packaging:** ship `camino-locker` (build dependency gtk-session-lock, runtime libmate-desktop for `MateBG`); runtime dependency on mate-screensaver's data, meaning its PAM file and `lock-dialog-*.ui` (its daemon exits on its own); the `locker-command` key in the `org.mate.Camino` schema.
