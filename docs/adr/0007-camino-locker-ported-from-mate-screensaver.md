# 7. Camino ships its own locker, ported from mate-screensaver's unlock dialog

Date: 2026-09-28
Status: Accepted

## Context

Camino owns `org.mate.ScreenSaver` as a shim and locks through ext-session-lock (06), so something has to draw the lock screen. mate-screensaver's own dialog is X11-only: `GSLockPlug` is a `GtkPlug` embedded over XEmbed, and its on-screen keyboard is a `GtkSocket`. Existing Wayland lockers (swaylock, waylock, gtklock) have their own look and read none of the `org.mate.screensaver` keys for the dialog, user switching, logout or the status message.

## Decision

- **Camino ships `camino-locker`**, a small C GTK3 program on gtk-session-lock. It ports mate-screensaver's dialog, lock plug and PAM code (about 3.7k lines, GPL-2.0-or-later), turning the plug into a widget in one lock window per monitor. It loads mate-screensaver's installed `lock-dialog-*.ui` themes and uses its `mate-screensaver` PAM service.
- **Camino locks first and the locker only draws.** Camino blanks and stops input the moment a lock is requested, and the locker attaches to that state. A crashed locker leaves the session locked and only a locker Camino spawned may take the lock over; if the locker keeps failing, Camino tells the user to unlock from a text console with `loginctl unlock-session <id>`.
- **The locker is replaceable** through the `org.mate.Camino` key `locker-command`.

## Consequences

- The lock screen is MATE's: same dialog, same themes, same keys, same PAM stack (fingerprint and keyring included). The price is a second C helper to maintain beside `camino-gtk-palette` (ADR 0003), and following mate-screensaver's dialog by hand.
- gtk-session-lock (GTK3) is barely maintained. If it stops building, Camino vendors it.
- mate-screensaver stays installed for its data, and its daemon exits because Camino already owns the name.
- The embedded on-screen keyboard and the libmatekbd layout indicator are lost.
- Rejected:
  - swaylock, waylock or gtklock as the default (no MATE look, keys unhonoured; they remain usable through `locker-command`);
  - drawing the lock screen inside Camino (password entry, PAM and GTK theming in the compositor process);
  - porting mate-screensaver's daemon (Camino already owns idle and blanking, 06 and 09).
