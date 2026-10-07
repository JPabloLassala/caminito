# Xwayland restart

Type: implementation
Status: resolved
Milestone: M3
Blocked by: 36, 37, 41
Map: [Caminito v0.1](../map.md)

## What to build

Caminito surviving an Xwayland crash, and bringing it back on the same display ([08](08-session-launch-model.md), [11](11-xwayland-integration.md)). It's one code path in every mode. In a session, mate-session usually dies first and ends it (08), and the restart doesn't block that teardown. 17 names this as one of the two places a bug ends the session, so it has its tests before M3 closes.

- **`XwmHandler::disconnected` (11):**
  1. drop `X11Wm` and Caminito's X connection;
  2. purge every X11 and override-redirect window from policy, the scene, focus, decoration caches and selection state by hand, since dropping `X11Wm` doesn't call `destroyed_window`;
  3. remove the `XWayland` calloop source, so its `X11Lock` deletes `/tmp/.X<n>-lock` and the socket.
- **Respawn** with `Some(n)` and the same `envs`, using the `helpers` backoff and give-up policy (08). After giving up, Caminito logs once and carries on without X.
- **On `Ready`:** the full start-up order from [38](38-x-connection-and-root-properties.md): `start_wm`, the connection, `_NET_SUPPORTED` and the name, root properties, XSETTINGS ([40](40-xsettings.md)), the client scale ([41](41-x11-scale.md)). `DISPLAY` is unchanged. Nothing per window is restored.
- **Known limitation (11):** in a MATE on Caminito session, an Xwayland crash logs the user out. It's documented in the module and the README's known limitations.

## Acceptance criteria

- [x] X11-tier tests, written before the code: kill Xwayland with two mapped X windows and an override-redirect one; policy, the scene and focus have no X windows left; Xwayland comes back on the same display; the root properties, `_NET_SUPPORTED` and XSETTINGS are republished; a new x11rb client maps and is managed.
- [x] A test that repeated immediate crashes back off and then give up, leaving Caminito running without X.
- [x] Nested, `pkill Xwayland` with xterm open removes the xterm, and a new xterm on the same `DISPLAY` gets a frame.

## Notes

- From [37](37-clipboard-and-dnd.md): the Wayland selections X is owed live in `RunningXwayland` and go with it, and a new Xwayland starts owing nothing. After a restart, the seat's current Wayland selections should be owed to the new X, which means Caminito keeps the last Wayland selection's mime types outside `RunningXwayland`.
- **The path** is `xwayland::restart`. `XwmHandler::disconnected`, if it's from the current `X11Wm`, stops Xwayland: `RunningXwayland` goes, with `X11Wm`, Caminito's X connection and the override-redirect windows, and so does the `XWayland` source, whose `X11Lock` deletes the lock and socket. Every pending and managed XWayland window is then destroyed through `window::lifecycle`, and the X selections the seat holds as Caminito's are cleared. Focus, the scene and the frames follow from policy in the refresh pass, and a dead keyboard grab is dropped there as before (36).
- **Restarts:** `Restart` keeps the display Xwayland was first ready on, the exits, counted by `helpers::Restarts`, and the policy (the helpers' default). Every later spawn takes `Some(n)`. A failed spawn, `XWaylandEvent::Error`, a failed `start_wm`, and Xwayland dying before it's ready count as exits too once it has been ready, and before that still mean Caminito runs without X. The last case is noticed in the refresh pass by Xwayland's Wayland client having gone, since Smithay's `XWayland` source takes the end of the display fd for "not ready yet" (`xserver.rs:330-345`) and would otherwise spin. Each exit logs one restart `warn!`; a crash after `Ready` also logs Caminito's X connection closing. Giving up also removes `DISPLAY` from `Caminito::environment`, so spawned clients don't try a dead display. Until the restart is ready, `DISPLAY` stays exported, as 08 wants it unchanged.
- **Selections (37's note):** `WaylandSelections` on `Caminito` keeps the seat's Wayland selections' mime types, and every `RunningXwayland` starts owing them, so after a restart X is told of them as one of its windows next takes the keyboard.
- **Tests** (`tests/xwayland_restart.rs`): SIGKILL Xwayland with two managed XWayland windows and an override-redirect one, then policy, the override-redirect list, the scene's layers and the keyboard have no XWayland windows; nothing restarts within the first 500 ms; Xwayland comes back on the same display with the same `DISPLAY`; `_NET_DESKTOP_NAMES`, `_NET_SUPPORTED` and XSETTINGS are there for a new x11rb client, and its window is managed. Under a quick policy, a crash and then two more as Xwayland restarts, before it's ready, give up, leave no Xwayland and no `DISPLAY`, and an xdg window still maps. A Wayland clipboard set before a crash pastes into an XWayland window after it. Each was checked to fail without its part of the change. The doubling of the delay is `helpers`' unit tests'.
- **Left alone:** `X11Wm`'s event source and its focus-release ping stay registered after its channel closes, since Smithay gives no tokens for them: two idle sources per restart.
- **The nested check** ran winit on Xvfb as 41 did. With gtk3-demo's application demo open on `:2`, SIGKILL of the nested Xwayland (by pid, not `pkill`, which would reach the host's) logged the restart, and Xwayland was ready on `:2` again 500 ms later. The demo exited with its display. New gtk3-demos on `:2` mapped, `_NET_DESKTOP_NAMES` was back, and one of their windows got a Menta frame (`_NET_FRAME_EXTENTS` 1, 1, 31, 1). xterm isn't installed, so the xterm half stays open, and is added to [46](46-nested-x11-checks-left-over.md). SIGTERM left `/tmp/.X2-lock` behind, since Caminito's exit doesn't drop the `XWayland` source; that predates this ticket.
