# 51. caminito-notifications is the notification daemon, and the applets read it over a private API

Date: 2026-10-25
Status: Accepted

## Context

A Session's notification daemon is mate-notification-daemon. It shows bubbles and has a do-not-disturb switch, but it has no Notification history, no per-app settings and no grouping. Its applet is built into the panel's process against the 1.28 applet library, so it doesn't load in `mate-panel-caminito` (README). The developer wants a notification system with Plasma's and macOS's features:
- per-app Delivery, sound, Previews and app icon;
- grouping;
- Do Not Disturb for a while or until turned off;
- a Notification center in the panel;
- a Badge on Dock items.

Apps never name a daemon. They call the well-known name `org.freedesktop.Notifications`, directly or through libnotify, GLib's fallback or the GTK portal, and whoever owns that name gets every notification. Two packages on the developer's machine claim it for D-Bus activation, mate-notification-daemon and Plasma, and mate-notification-daemon is XDG-autostarted too.

ADR 0035 split a feature into a thin C and GTK3 in-process applet and a Rust process, sharing nothing but a system service's D-Bus API, and rejected a private API between them. Notifications have no such service: the history and Do Not Disturb exist only in whatever process owns the name.

## Decision

- **`caminito-notifications`, in Rust and GTK4 without libadwaita (ADR 0014), is the notification daemon.** It owns `org.freedesktop.Notifications` and implements the Desktop Notifications spec with actions, markup, hyperlinks, sound, persistence and KDE's inline reply. It draws the Notification bubbles as layer surfaces in the namespace `caminito-notification`, keeps the Notification history, decides Do Not Disturb, and has the settings window in the same process.
- **It's a Helper.** Caminito starts it before it runs mate-session and restarts it when it exits, so it owns the name before any app could trigger D-Bus activation of another daemon. `mate-notification-daemon.desktop` joins the skipped list of mate-session's carried patch (ADR 0014), and `caminito-git` no longer depends on mate-notification-daemon.
- **The Notifications applet is a thin C and GTK3 in-process applet on `caminito-applet-common`,** in the manner of ADR 0035. It is the panel icon and the Notification center.
- **The daemon exports a private API, `org.mate.Caminito.Notifications` at `/org/mate/Caminito/Notifications`,** which ADR 0035 rejected for Bluetooth. It covers:
  - the Notification history: list, dismiss, clear an app's notifications or all of them, invoke an action, reply;
  - Do Not Disturb: its state, when it ends, why it's on, and setting it;
  - each Notifying app's count for the Badge;
  - opening the settings window, at an app's page;
  - opening the Notification center, which the daemon passes on to the Notifications applet with a signal, since the applet owns no name to be called at (CMN-263).

  Each comes with change signals. Its clients are the Notifications applet and the Dock applet, and Caminito, which calls it for `Super+N` and Do Not Disturb's System shortcut. It's unversioned, because all of them ship in the `caminito` package.
- **Settings are GSettings:** `org.mate.Caminito.notifications`, plus the relocatable `org.mate.Caminito.notifications.app`, one path per Notifying app. The C applets can read them, and `dconf` can edit them. The history is a file under `$XDG_STATE_HOME/caminito/`.
- **Caminito decides nothing about Do Not Disturb.** It publishes, on `org.mate.Caminito`, whether the output under the pointer has a fullscreen window and whether a screencast is running, and the daemon watches them.

## Consequences

- Without the daemon, the Notifications applet hides itself and the Dock shows only Launcher entry counts. Outside a Session, X11 MATE keeps mate-notification-daemon, and the applet hides there too.
- A daemon crash loses the bubbles on screen but not the history, which is on disk. Caminito restarts it at once. While the name is unowned, a call can still start another daemon by D-Bus activation, and that daemon keeps the name until it exits. mate-notification-daemon is activated with `--idle-exit`, so it does exit when idle. The restarted daemon takes the name back at once all the same (CMN-258): the activated mate-notification-daemon owns it with `ALLOW_REPLACEMENT`, the daemon asks for it with `REPLACE`, and mate-notification-daemon exits as it loses it. Plasma's activation file only runs `plasma_waitforname`, which owns nothing outside Plasma. The daemon doesn't allow its own replacement, and waits in the bus's queue for a name it's refused.
- The private API is the one interface between Caminito's processes that isn't a system service's or Caminito's own. Changing it means changing the daemon, both applets and Caminito's calls together.
- The Dock applet gains a second D-Bus client, and the Badge has two sources (CONTEXT.md).
- GNOME Shell's `org.gtk.Notifications` isn't served. GApplication apps fall back to the freedesktop name, but an action only reaches such an app while it's running.
- Rejected:
  - Carried patches on mate-notification-daemon. Its design has no history, per-app rules or grouping, so the patches would be a rewrite in C, carried forever.
  - The daemon in the compositor, with Caminito drawing the bubbles. It puts a lot of UI into the compositor, and it works only under Caminito.
  - The daemon in the applet, in the panel's process. A crash there takes the panel with it, which ADR 0035 rules out, and a panel restart would lose the name.
  - XDG autostart, as caminito-network is started. It races with D-Bus activation of the other daemons, and nothing restarts it before the session is up.
