# Xwayland restart

Type: implementation
Status: open
Milestone: M3
Blocked by: 36, 37, 41
Map: [Camino v0.1](../map.md)

## What to build

Camino surviving an Xwayland crash, and bringing it back on the same display ([08](08-session-launch-model.md), [11](11-xwayland-integration.md)). It's one code path in every mode. In a session, mate-session usually dies first and ends it (08), and the restart doesn't block that teardown. 17 names this as one of the two places a bug ends the session, so it has its tests before M3 closes.

- **`XwmHandler::disconnected` (11):**
  1. drop `X11Wm` and Camino's X connection;
  2. purge every X11 and override-redirect window from policy, the scene, focus, decoration caches and selection state by hand, since dropping `X11Wm` doesn't call `destroyed_window`;
  3. remove the `XWayland` calloop source, so its `X11Lock` deletes `/tmp/.X<n>-lock` and the socket.
- **Respawn** with `Some(n)` and the same `envs`, using the `helpers` backoff and give-up policy (08). After giving up, Camino logs once and carries on without X.
- **On `Ready`:** the full start-up order from [38](38-x-connection-and-root-properties.md): `start_wm`, the connection, `_NET_SUPPORTED` and the name, root properties, XSETTINGS ([40](40-xsettings.md)), the client scale ([41](41-x11-scale.md)). `DISPLAY` is unchanged. Nothing per window is restored.
- **Known limitation (11):** in a MATE on Camino session, an Xwayland crash logs the user out. It's documented in the module and the README's known limitations.

## Acceptance criteria

- [ ] X11-tier tests, written before the code: kill Xwayland with two mapped X windows and an override-redirect one; policy, the scene and focus have no X windows left; Xwayland comes back on the same display; the root properties, `_NET_SUPPORTED` and XSETTINGS are republished; a new x11rb client maps and is managed.
- [ ] A test that repeated immediate crashes back off and then give up, leaving Camino running without X.
- [ ] Nested, `pkill Xwayland` with xterm open removes the xterm, and a new xterm on the same `DISPLAY` gets a frame.

## Notes

- From [37](37-clipboard-and-dnd.md): the Wayland selections X is owed live in `RunningXwayland` and go with it, and a new Xwayland starts owing nothing. After a restart, the seat's current Wayland selections should be owed to the new X, which means Camino keeps the last Wayland selection's mime types outside `RunningXwayland`.
