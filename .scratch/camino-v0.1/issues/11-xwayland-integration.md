# XWayland integration

Type: grilling
Status: resolved
Blocked by: 10
Map: [Camino v0.1](../map.md)

## Question

Using the [XWayland integration audit](10-xwayland-integration-audit.md), how do XWayland windows get Marco's behaviour in v0.1? Decide:

- where each missing X11 feature is implemented (Smithay's window-manager connection or Camino's own), and whether any of it is worth sending upstream to Smithay;
- which root properties Camino publishes and when;
- how X work reconnects after an Xwayland restart;
- how XWayland windows map to Camino's window types, workspaces, stacking and focus policy;
- the v0.1 stance on clipboard and DnD bugs;
- how XWayland windows are scaled on HiDPI outputs (coordinate with [13](13-hidpi-and-accessibility-scope.md)).

This ticket takes over what 05, 06 and 08 handed on: position, above and sticky hints, `_NET_WM_PID`, struts, bells, the EWMH desktop atoms, `_NET_WORKAREA` and `_NET_FRAME_EXTENTS`, and reconnecting after a restart.

## Answer

Decided on 2026-09-28. The user asked for the recommended answer to every question to be taken, so there was no round-by-round grilling. Inputs were the [XWayland integration audit](10-xwayland-integration-audit.md) (branch `research/xwayland-integration-audit`, commit 713f172) and what [05](05-v01-marco-feature-cut.md), [06](06-v01-protocol-and-integration-set.md), [07](07-decoration-strategy.md), [08](08-session-launch-model.md) and [09](09-gsettings-mapping.md) handed on. Source citations are the audit's, at the same revisions (Smithay `928d4a9`, cosmic-comp `9f746ea`, Marco `41c50d5`, msd `927e9dc`, GTK `gtk-3-24` `28aa479`).

The hard-to-reverse part, Camino serving XSETTINGS and msd's xsettings plugin running mirror-only, is recorded in [ADR 0006](../../../docs/adr/0006-camino-serves-xsettings.md).

### Two X connections

- **Confirmed: two connections.** Smithay's `X11Wm` keeps the privileged `-wm` connection and the per-window protocol. **Camino's X connection** is an ordinary x11rb client that connects to `:n` after `Ready` and does everything Smithay doesn't. No Smithay fork and no dependency on an upstream change (03: pin master, don't fork).
- **What Camino's X connection selects:** `SubstructureNotify | PropertyChange` on the root (so it receives the EWMH root client messages Smithay drops), and XKB `BellNotify` (06). It never selects `SubstructureRedirect` and never grabs the server.
- **Property-change events come from Smithay.** Changes to properties Smithay doesn't parse arrive as `XwmHandler::property_notify(Other(atom))`, and Camino reads the new value over its own connection. Camino doesn't select `PropertyChange` on client windows itself.
- **Ownership rule:** Camino's X connection never writes a property Smithay maintains:
  - never: `_NET_CLIENT_LIST`, `_NET_CLIENT_LIST_STACKING`, root `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE`, `WM_STATE`;
  - only through Smithay's API: `_NET_SHOWING_DESKTOP` (`set_showing_desktop`), the XSETTINGS table (`set_xsettings`), the root cursor, the RandR primary output;
  - written once, right after `start_wm`: `_NET_SUPPORTED` (replaced by a superset) and the check window's `_NET_WM_NAME` ("Camino", replacing "Smithay X WM", for mcc's `wm-common`).
- **Order at start-up and after every restart:** `Ready` → `start_wm` → connect Camino's X connection → rewrite `_NET_SUPPORTED` and the WM name → publish the root properties below → apply XSETTINGS, cursor, primary output and client scale → only then export `DISPLAY` (08's steps 4–5). GTK3 caches `_NET_SUPPORTED` and never re-reads it.
- **Spawning:** `XWayland::spawn` clears the environment, so `XCURSOR_THEME` and `XCURSOR_SIZE` (08) go through `envs`. No extra Xwayland arguments in v0.1 (cosmic's `-enable-ei-portal` isn't needed).

### Which X11 features are in v0.1

Everything the audit marks **Smithay** is used as is, with Camino supplying the policy below. Of the rows marked **Camino X connection**:

| v0.1 | Feature |
|---|---|
| **Yes** | `_NET_SUPPORTED` superset and "Camino" identity |
| **Yes** | `_NET_NUMBER_OF_DESKTOPS`, `_NET_CURRENT_DESKTOP`, `_NET_DESKTOP_NAMES` and their root requests; per-window `_NET_WM_DESKTOP` and its request |
| **Yes** | `_NET_WORKAREA` and `_GTK_WORKAREAS_D<n>` |
| **Yes** | `_NET_FRAME_EXTENTS` and `_NET_REQUEST_FRAME_EXTENTS` |
| **Yes** | `_NET_WM_STRUT(_PARTIAL)` and the XKB bell (already 06) |
| **Yes** | `_GTK_SHOW_WINDOW_MENU` (opens Camino's window menu, per 05) |
| **Yes** | `_NET_WM_MOVERESIZE` keyboard move and resize (directions 9 and 10) and cancel (direction 11) |
| **Yes** | `_NET_CLOSE_WINDOW` and `_NET_MOVERESIZE_WINDOW` (a few lines each, reusing `close()` and the `configure_request` path) |
| **Yes** | skip-taskbar and skip-pager changes after map, kept in Camino's own window state (never written back to `_NET_WM_STATE`) |
| **Yes** | `_NET_WM_ICON`, read over Camino's X connection at map and on `property_notify(Other(atom))`, keeping 07's icon order (xdg-toplevel-icon, then `_NET_WM_ICON`, then the `.desktop` icon through `WM_CLASS`). If Smithay [#2160](https://github.com/Smithay/smithay/pull/2160) lands on the pinned master, use it instead. |
| Later | `_NET_WM_USER_TIME` (see Focus), `WM_CLIENT_LEADER` groups, SHAPE (waits for Smithay [#2162](https://github.com/Smithay/smithay/pull/2162)), `_NET_WM_ALLOWED_ACTIONS` and `_NET_RESTACK_WINDOW` (only X11 pagers use them, none in scope), `_NET_WM_FULLSCREEN_MONITORS` (05) |

The audit recommended the first six rows plus the moveresize cancel. Keyboard move and resize, `_NET_CLOSE_WINDOW`, `_NET_MOVERESIZE_WINDOW`, skip-taskbar changes and `_NET_WM_ICON` come from its optional list and are taken because each reuses a path Camino already has (05's keyboard move and resize, `close()`, `configure_request`, the window list, 07's icon order).

Accepted Smithay limitations, documented as known gaps: a `_NET_WM_STATE` message with two different atoms applies only the first; `WM_TAKE_FOCUS` carries `CurrentTime`.

### Root properties Camino publishes, and when

All values are in **X client space**: Smithay logical coordinates multiplied by the X11 client scale (see HiDPI). Struts read from X are divided by it.

| Property | Written |
|---|---|
| `_NET_SUPPORTED`, check-window `_NET_WM_NAME` | once per `start_wm` |
| `_NET_NUMBER_OF_DESKTOPS`, `_NET_DESKTOP_NAMES` | at start and whenever `num-workspaces`, dynamic workspaces or `workspace-names` change |
| `_NET_CURRENT_DESKTOP` | on every workspace switch |
| `_NET_WORKAREA` (one rectangle per desktop) and `_GTK_WORKAREAS_D<n>` (per desktop, one rectangle per monitor) | when outputs, layer-shell exclusive zones, X11 struts, the workspace count or the client scale change. Built as Marco does (`screen.c:2354-2435`). |
| `_NET_WM_DESKTOP` (per window) | at map, on every move between workspaces, and when sticky changes (`0xFFFFFFFF`) |
| `_NET_FRAME_EXTENTS` (per window) | at map, on `_NET_REQUEST_FRAME_EXTENTS` (answered before map, from the theme and window type), and when the frame geometry changes (theme reload, maximise, shade, fullscreen). SSD windows get the Marco theme's borders; CSD, fullscreen and undecorated windows get zero. |

Root requests (`_NET_CURRENT_DESKTOP`, `_NET_NUMBER_OF_DESKTOPS`, `_NET_WM_DESKTOP`) run the same code as Camino's own bindings. `_NET_NUMBER_OF_DESKTOPS` writes `num-workspaces`, as the pager does (06).

### Xwayland restarts

- **Accepted: an Xwayland crash logs the user out** in a MATE on Camino session. mate-session is an X11 GTK client (08), GTK3's X IO error handler calls `_exit(1)`, and mate-session exiting ends the session. Patching mate-session to survive losing its display isn't worth it in v0.1: its dialogs and inhibitors are X11 anyway. This is a documented known limitation.
- **Camino still survives it, and restarts Xwayland on the same display in every mode**, keeping 08's rule rather than limiting the restart to bare `camino`. One code path, which matters for bare `camino` (08, 14) and means Camino itself never dies with X:
  1. `XwmHandler::disconnected`: drop `X11Wm` and Camino's X connection; purge every X11 and override-redirect window from the scene, focus, foreign-toplevel and ext-workspace state by hand (dropping `X11Wm` doesn't call `destroyed_window`).
  2. Remove the `XWayland` calloop source, so its `X11Lock` deletes `/tmp/.X<n>-lock` and the socket.
  3. Respawn with `Some(n)`, the same `envs`, and 08's backoff and give-up rule.
  4. On `Ready`, run the start-up order above. Nothing per window is restored, since X clients don't survive.
- In a session, mate-session's exit usually wins the race and Camino tears down (08). That's fine: the restart path doesn't block teardown.
- Smithay's `-terminate` flag shouldn't fire while Camino's connections are attached. That's unverified at runtime and goes to 14's test loop.

### Window types, workspaces, stacking and focus

XWayland windows are ordinary Camino windows. The policy is the same as for native ones, and the rules below only say where X11 differs.

- **Window types:** `WmWindowType` maps to Marco's types, and Camino applies Marco's own rules for them (`recalc_window_features`, `meta_window_update_layer`, `placement.c`): which types get frames (07), which skip the taskbar and pager, which layer they stack in, and which can take focus. Desktop and dock types from third-party X11 apps are honoured as in Marco (desktop at the bottom, docks with their struts). MATE's own desktop and panel are layer surfaces and don't use these.
- **Modal and transient:** `WM_TRANSIENT_FOR` (mapped through `window_id()`) and `_NET_WM_STATE_MODAL` give the same transient and modal-dialog behaviour as native windows: same workspace as the parent, and stacked above it.
- **Workspaces:** an XWayland window starts on the current workspace unless it set `_NET_WM_DESKTOP` before mapping (honoured, as in Marco, with `0xFFFFFFFF` meaning sticky). Transients follow their parent. Sticky requests are in v0.1 (05's "may slip" is lifted: Smithay forwards them and the desktop atom is cheap).
- **Position and size requests:** honoured for windows that aren't maximised, fullscreen or tiled, following Marco's placement rules for `USPosition`/`PPosition` at map. Later `configure_request`s and `_NET_MOVERESIZE_WINDOW` are clamped to the work area as Marco does. This is cosmic's shape with Marco's rules.
- **Stacking:** Camino's scene is the truth. Above and below requests map to Camino's above and below layers. After every restack that involves an XWayland window, Camino calls `update_stacking_order_upwards` with the XWayland windows in scene order, leaving out override-redirect windows.
- **Focus:** `focus-mode`, `raise-on-click` and `auto-raise` apply unchanged. Focus goes through Smithay's `KeyboardTarget`, which handles the ICCCM input models and `WM_TAKE_FOCUS`.
- **Focus-stealing prevention (05) for X11:** XWayland windows carry no activation token (X11 startup notification is later, 05), so they take 05's no-token path. Under *smart*, a new XWayland window gets focus if it's its client's first window (05's freshly launched app) or transient for the focused window; any other becomes urgent. Under *strict*, only a transient for the focused window gets focus. The transient rule is Camino's addition to 05, standing in for Marco's user-time check so that dialogs opened from the focused X11 app aren't refused. A `_NET_ACTIVE_WINDOW` request with source indication 2 (pager) is honoured; one with source 1 (application) is honoured only if that client already owns the focused window, and otherwise marks the window urgent (Camino policy, the X11 counterpart of 05's token rule). `XUrgencyHint` and demands-attention both mean urgent. `_NET_WM_USER_TIME` is later.

### Override-redirect windows

- **Their own scene layer:** above every toplevel layer (normal, above, fullscreen) and the `top` layer-shell layer; below `overlay` layer-shell surfaces and the session lock. Within it they're ordered by `configure_notify`'s `above` sibling. They're placed in global logical coordinates, not per workspace, and left out of the X stacking sync.
- **Never focused on click.** X11 menus get the keyboard through `zwp_xwayland_keyboard_grab_v1`, which Camino implements through Smithay's handler.
- **A keyboard grab never beats the lock** (Smithay [#1714](https://github.com/Smithay/smithay/issues/1714)). While the session is locked, grab requests are refused and existing grabs are released. The lock itself is 12's.

### Clipboard, primary selection and drag-and-drop

- **Policy (cosmic's):** `allow_selection_access` returns true only while an XWayland window has keyboard focus. The Wayland selection is pushed into X lazily, when an XWayland window next gains focus, so background X11 clients can't read the clipboard.
- **DnD** uses Smithay's bridge; Camino's pointer and touch focus types implement `DndFocus`, and hit-testing uses `X11Surface::surface_under`.
- **Known bugs are accepted, with no Camino workarounds in v0.1:** XDND Ask and Private actions, the unimplemented `CLIPBOARD_MANAGER` `SAVE_TARGETS`, and drop bug [#1953](https://github.com/Smithay/smithay/issues/1953). They're fixed upstream or not at all, and picked up by bumping the pin.
- **Clipboard persistence** after an X11 app quits relies on msd's ext-data-control clipboard manager (06). That's inferred, not tested, and goes to 14.

### XSETTINGS

- **Camino is the XSETTINGS manager**, through `X11Wm::set_xsettings`. It ports msd's translation table (`msd-xsettings-manager.c:205-240` and `:541-597`: theme, icons, fonts and font rendering, DPI, cursor, double-click and drag threshold, event sounds, decoration layout; see ADR 0006), and adds the X11 scale settings (see HiDPI). Values apply live and are re-applied after every `start_wm`.
- **msd's xsettings plugin runs mirror-only under Camino.** The carried msd patch from 09 (ADR 0005) is extended: when `MATE_WM_NAME=Camino`, the plugin skips claiming `_XSETTINGS_S0` and starts only the GNOME-key mirror. Without this the plugin would find Smithay's owner, abort, and take the mirror down with it.

### HiDPI

- **One global X11 scale**, applied through Smithay's `set_client_scale` on the Xwayland client and published through XSETTINGS (`Xft/DPI`, `Gdk/UnscaledDPI`, `Gdk/WindowScalingFactor`, `Gtk/CursorThemeSize`). When it changes, Camino re-sends XSETTINGS, reloads the X cursor, republishes the work area and frame extents, and reconfigures every XWayland window. Per-output X11 scale isn't feasible.
- **Which value** is 13's decision. The recommendation handed on is cosmic's "largest integer output scale", [13](13-hidpi-and-accessibility-scope.md) took it, with msd's XSETTINGS DPI formulas. `window-scaling-factor` only affects output scales, and so the X11 scale only indirectly.

### Upstream Smithay

- **v0.1 is entirely Camino-side** and depends on no upstream change.
- **Optional PRs, after v0.1 or whenever convenient,** each of which would shrink Camino's side: extra `_NET_SUPPORTED` atoms passed to `start_wm` (closes the start-up race); forwarding unhandled client messages to `XwmHandler`; calling `destroyed_window` for every window when `X11Wm` is dropped; an `Exited` event ([#1410](https://github.com/Smithay/smithay/issues/1410)); `_NET_WORKAREA` ([#1181](https://github.com/Smithay/smithay/issues/1181)). If any lands on the pinned master first, Camino uses it.

### Handed on

- **12 Screen locking:** override-redirect windows sit below the lock, and Camino refuses and releases `zwp_xwayland_keyboard_grab_v1` grabs while locked ([#1714](https://github.com/Smithay/smithay/issues/1714)).
- **13 HiDPI:** the X11 scale value (recommended: largest integer output scale) and its interaction with `window-scaling-factor`. The mechanism is fixed above. ([13](13-hidpi-and-accessibility-scope.md) took the recommendation.)
- **14 Development and testing loop:** verify at runtime that `-terminate` doesn't fire while Camino is attached, that X11 clipboard contents survive the app quitting (msd's data-control manager), and exercise the restart path under bare `camino`.
- **15 Crate and module architecture:** the X11 side is two parts, Smithay's `XwmHandler` policy and Camino's X connection (x11rb), sharing Camino's window state.
- **16 Packaging:** the carried msd patch grows (skip the XSETTINGS claim under Camino). The patched msd is a hard dependency of the session package: without it, a Camino session loses the GNOME mirror (ADR 0006).
