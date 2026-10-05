# 4. Run mate-session as Camino's child, on XWayland, with a carried patch

Date: 2026-09-28
Status: Accepted. Qualified on 2026-10-05 by ticket 78: with `GDK_BACKEND` unset, X11-only autostart entries crash, so two more carried patches skip them or start them on XWayland (ADR 0014).

## Context

On X11, mate-session starts first and launches Marco as its `windowmanager` component. On Wayland that order can't work: mate-session calls `gtk_init` and needs a display before it can do anything, and the compositor is the display. mate-session has never been ported to Wayland. Its logout, shutdown and inhibit dialogs and its X error traps call X11-only GDK functions, and it exports `DISPLAY=$WAYLAND_DISPLAY` when GDK is on Wayland. The Wayfire-based MATE session avoids the problem by not running mate-session at all, which loses autostart, XSMP, the logout handshake and inhibitors.

`XDG_CURRENT_DESKTOP` must be exactly `MATE`: caja and mate-menus compare the whole string. So a desktop-specific gschema override (`[org.mate.session:Camino]`) can't be used to stop mate-session starting Marco.

## Decision

- The session entry runs `camino --session`. Camino brings up its outputs, the Wayland socket and Xwayland, exports the environment, and then spawns `mate-session --disable-acceleration-check` as its child.
- mate-session runs with `GDK_BACKEND=x11` against XWayland. A carried patch, active only when `MATE_WM_NAME=Camino`:
  - unsets `GDK_BACKEND` right after `gtk_init`, so the variable reaches neither its children nor the D-Bus and systemd environments;
  - skips the `windowmanager` required component.
- mate-session exiting ends the session. Camino then tears down and exits with mate-session's status.

## Consequences

- MATE keeps its whole session machinery: autostart, the start-up phases, restarting components, XSMP, inhibitors and the logout dialog. The patch is a few lines and does nothing under Marco.
- mate-session's dialogs are X11 windows, and its idle detection runs on XWayland's X SYNC counter and stays inaccurate (Camino owns idle, per 06). Without Xwayland, mate-session can't start, so Camino treats Xwayland as a supervised helper and restarts it on the same display.
- Rejected:
  - mate-session launching Camino, as with Marco (it needs a display first);
  - native Wayland mate-session (a bigger patch that ports its X11 calls);
  - no mate-session, the Wayfire approach (no logout handshake, inhibitors or autostart);
  - `mate-session --autostart DIR...` to drop Marco (it replaces the default and required-apps logic, which Camino would have to re-create);
  - `XDG_CURRENT_DESKTOP=Camino:MATE` with a gschema override (caja and mate-menus break).
