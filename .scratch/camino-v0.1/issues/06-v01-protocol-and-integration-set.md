# v0.1 protocol and integration set

Type: grilling
Status: resolved
Blocked by: 02, 03
Map: [Camino v0.1](../map.md)

## Question

Using the MATE Wayland audit and the Smithay capability audit, which Wayland protocols must Camino v0.1 implement, and for each, does Smithay provide it or do we build it, so that mate-panel with its window list and workspace switcher, mate-session, mate-settings-daemon and caja's desktop work? Where MATE components are missing Wayland support, is the fix in Camino, in the MATE component (out of our tree), or deferred?

## Answer

Decided in a grilling session on 2026-09-28. Inputs were the [MATE Wayland audit](02-mate-wayland-audit.md), the [Smithay capability audit](03-smithay-capability-audit.md) and the channels [05](05-v01-marco-feature-cut.md) handed on. The session also read source for mate-panel `58ac6c1`, libxfce4windowing `bf84c7b`, mate-control-center, mate-settings-daemon and mate-utils master, GTK `gtk-3-24`, gtk-layer-shell 0.10.1 and Smithay `928d4a9`; the relevant facts are quoted inline.

The integration rule is recorded in [ADR 0002](../../../docs/adr/0002-mate-integration-channels.md): a standard protocol first, then an environment variable for static facts, then D-Bus on `org.mate.Camino`. A Camino-private Wayland protocol is used only when the channel must name a surface, and none is needed in v0.1. The MATE side of every non-standard channel is a **carried patch**.

This ticket decides *where each fix lives and which channel carries it* for every MATE gap, including the gaps listed as fog on the map. Detailed design stays with the owning tickets.

### Protocols Smithay provides (enabled in v0.1)

- **Core:** xdg-shell, xdg-output, viewporter, fractional-scale, xdg-activation, primary-selection, and data-control (both ext and wlr).
- **Input:** pointer-constraints, relative-pointer, pointer-gestures, tablet-v2, keyboard-shortcuts-inhibit, text-input-v3 and input-method-v2, cursor-shape.
- **Windows and clients:** xdg-foreign, xdg-dialog, xdg-toplevel-icon, xdg-system-bell (GTK4 only), security-context.
- **Rendering:** presentation-time, single-pixel-buffer, content-type, alpha-modifier, fifo and commit-timing, drm-syncobj.
- **Layers, capture, locking and idle:** wlr-layer-shell (v4 or newer; caja needs v4 for ON_DEMAND keyboard), ext-foreign-toplevel-list, ext-image-capture-source (output and toplevel sources) plus ext-image-copy-capture, ext-session-lock, ext-idle-notify, idle-inhibit.
- **Decorations:** KDE server-decoration and xdg-decoration. GTK3 honours only the KDE protocol. The policy belongs to [07](07-decoration-strategy.md).
- **Left out:** drm-lease and virtual-keyboard.

### Protocols Camino builds (v0.1)

- **wlr-foreign-toplevel-management v3**, for mate-panel's window list, Window Selector and Show Desktop fallback via libxfce4windowing. It reports the raw xdg `app_id`, or `WM_CLASS` for XWayland windows, because the window list finds icons through the app_id's .desktop file.
- **ext-workspace-v1**, which is **read-only**: no create or remove capability. It always sends **two-value** `coordinates` (column, row), because libxfce4windowing reads index 1 without a proper bounds check.
- **wlr-output-management v4**, for msd's wlrandr plugin and so for the Display capplet.
- **A minimal `gtk_shell1`**: `get_gtk_surface`, `system_bell` and `set_modal`/`unset_modal`. GTK3 rings the bell and reports modality only through this GNOME protocol, so without it native GTK3 bells are silent.

### Deferred protocols

- **wlr-screencopy.** mate-screenshot prefers ext-image-copy-capture and uses screencopy only as a fallback.
- **wlr-output-power-management, wlr-gamma-control and wlr-virtual-pointer.** Nothing in the v0.1 scope binds them.
- **The Xfce private toplevel and output protocols.** libxfce4windowing binds them only if they're offered.

### Access to privileged globals

Layer-shell, foreign-toplevel, ext-workspace, output management, data-control and screen capture are offered to every client **except those in a `wp_security_context_v1` sandbox** (Flatpak).

### Integration channels

| Need | Fix lives in | Channel |
|---|---|---|
| Alt+F1, Alt+F2 | mate-panel carried patch + Camino | `org.mate.Panel` `/org/mate/Panel`: `MainMenu(u ts)` and `RunDialog(u ts)`. Both reuse the `_MATE_PANEL_ACTION` code paths: drop from the Menu Bar, else anchor to the Menu Button, else open at the pointer. The panel never saw the keypress, so Camino accepts **one popup grab from the panel's client without a valid serial, for about 1 s** after calling. On an unpatched panel, Alt+F2 spawns `mate-panel --run-dialog` and Alt+F1 logs once. `KILL_DIALOG` is dropped in favour of Camino's own Force Quit. |
| Show Desktop shared with Ctrl+Alt+D | mate-panel carried patch + Camino | D-Bus property `org.mate.Camino` `ShowingDesktop` (read/write, with change signals, current workspace, Marco semantics). The applet falls back to libxfce4windowing's client-side minimise-all when Camino is absent. |
| Pager grid layout | mate-panel carried patch + Camino | ext-workspace `coordinates` computed from the `org.mate.Camino` layout key (name is for [09](09-gsettings-mapping.md)). The Wayland pager is patched to place workspaces by coordinates when all of them have coordinates, and under Camino its "rows" preference writes Camino's key. |
| Workspace count and names | mate-panel carried patch | The pager's preferences write Marco's `num-workspaces` and `workspace-names` directly, and Camino applies them live. ext-workspace stays read-only. |
| Minimise animation target | mate-panel carried patch | Standard wlr-foreign-toplevel `set_rectangle`. The window list is patched to call `xfw_window_set_button_geometry`. With no rectangle, Camino minimises without animating. |
| Camino's identity for mate-control-center | mcc carried patch + Camino | Environment variables `MATE_WM_NAME=Camino` and `MATE_WM_KEYBINDINGS=Camino`, exported into the session and the D-Bus activation environment. On non-X11 displays, `wm-common` reads them. The Windows capplet uses `wm-common`, accepts "Camino", and connects its X11-only signal only on X11. This also fixes the Keyboard Shortcuts crash, where a NULL keybinding list is dereferenced on Wayland. The `50-camino-*.xml` manifests use `wm_name="Camino"`. |
| Active window for mate-screenshot (`<Alt>Print`) | mate-screenshot carried patch + Camino | D-Bus `org.mate.Camino` `GetActiveToplevel() → s`. It returns the ext-foreign-toplevel-list identifier of the most recently focused toplevel that doesn't belong to the caller's pid. mate-screenshot captures it through the ext toplevel image-capture source, and Camino renders frame-without-shadow for toplevel captures. |
| Bell sources | Camino | GTK3: `gtk_shell1.system_bell`, with the surface for a window beep and the focused window for a display beep. GTK4: xdg-system-bell. XWayland: Camino's own XKB-enabled X connection selecting `XkbBellNotify`, mapped to the X11 surface. All three are in v0.1. |
| Window types | Camino | Native: GTK3 sends only a parent and `gtk_shell1` modality, and GTK4 uses xdg-dialog. Native toplevels are therefore *normal*, *transient* or *modal dialog*. Utility, splash, skip-taskbar and keep-above hints are lost, and GTK isn't patched. XWayland: `WmWindowType` from Smithay's X11 window manager. |
| Struts | Camino | Native: layer-shell exclusive zones. XWayland: `_NET_WM_STRUT(_PARTIAL)`, read over Camino's own X connection on `property_notify`, because Smithay never reads it. This is Camino-side for v0.1, not an upstream patch. |
| Media keys and custom shortcuts | msd carried patches + Camino | Camino provides D-Bus `GrabAccelerators` / `AcceleratorActivated`, in the style of `org.gnome.Shell`. msd's media-keys (about 250 lines) and keybindings (about 100 lines) plugins are patched to grab through it, and keep all their actions (libmatemixer volume, launchers, eject, power dialogs, MPRIS, rfkill). Other patches: the OSD window goes on gtk-layer-shell (overlay layer, no keyboard), `touchpad_is_present` is stubbed, and `X11Only` is dropped for these two plugins. |
| Input device settings and keyboard layout | Camino | Camino applies `org.mate.peripherals-mouse`, `-touchpad` and `-keyboard`, and `org.mate.peripherals-keyboard-xkb.kbd` (`layouts`, `options`, `model`), to libinput and xkb, live. The key-by-key mapping is for [09](09-gsettings-mapping.md). |
| Mouse and Keyboard capplets | packaging + mcc carried patch | Launched with `GDK_BACKEND=x11` under XWayland through `.desktop` overrides. Keyboard then works fully. A roughly 3-line patch makes `touchpad_is_present` return TRUE on Wayland, so Mouse shows its touchpad page. |
| Display configuration | msd (unchanged) | **msd owns persistence** (`monitors-wayland.xml`) and applies it over wlr-output-management at login. Camino persists nothing and starts each output in its preferred mode, left to right. |
| Screen locking | Camino | Camino **owns `org.mate.ScreenSaver`** as a shim. `Lock` spawns a configured ext-session-lock locker, so the panel, mate-session and the notification daemon keep working. Which locker is for the Screen locking fog ([12](12-screen-locking.md): `camino-locker`). |
| Idle and presence | Camino | **Camino owns idle.** It reads `org.mate.session idle-delay` and the lock settings, and drives lock and blanking. Idle counts as inhibited if the idle-inhibit protocol or mate-session's `IsInhibited` says so. |
| Wallpaper on outputs caja doesn't cover | msd carried patch + Camino | msd's background plugin stays on under Camino. Camino stacks the `desktop` layer surface (caja) above `mate-background` (msd) on the same output. |

### Deferred and known gaps

- AccessX (sticky, slow, bounce and mouse keys) and typing break. Both need Camino work, and they move to the accessibility fog ([13](13-hidpi-and-accessibility-scope.md): sticky, slow and bounce keys and the toggle-key beeps are in v0.1; mouse keys and typing break are later).
- Reading msd's `monitors-wayland.xml` at startup to avoid a re-modeset at login.
- mate-session's idle presence status stays inaccurate, because it's X SYNC over XWayland.
- mate-notification-daemon's fullscreen and screensaver suppression.
- Caja's desktop icons on one output only.
- `switch-panels` / `cycle-panels`, which work only if the panel's layer surfaces accept keyboard focus (as in 05).

### Handed on

- **[07](07-decoration-strategy.md):** SSD for GTK3 goes through the KDE server-decoration protocol. GTK3 doesn't bind xdg-decoration.
- **[08](08-session-launch-model.md):** mate-session's `DISPLAY` overwrite and `windowmanager` override. Also exporting `MATE_WM_NAME`, `MATE_WM_KEYBINDINGS`, `XDG_CURRENT_DESKTOP=MATE` and `DISPLAY` before msd starts.
- **[09](09-gsettings-mapping.md):** the layout key name, and the mapping of the peripherals and xkb keys.
- **XWayland fog:** the EWMH desktop atoms (`_NET_NUMBER_OF_DESKTOPS` and so on), `_NET_WORKAREA` and `_NET_FRAME_EXTENTS` go over the same Camino-side X connection as bells and struts.
- **Packaging** ([16](16-packaging.md): 1.29.x tags or pinned master commits; the capplet overrides are dropped in favour of an mcc patch that picks the X11 backend under Camino, and the touchpad patch is gated on Camino, not Wayland, [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md); `camino-git` depends on marco):
  - the MATE baseline for the carried patches to mate-panel, mcc, msd and mate-screenshot
  - the `.desktop` overrides for the Mouse and Keyboard capplets
  - the runtime dependency on Marco's `org.mate.Marco.*` schemas (the Windows capplet aborts without them)
