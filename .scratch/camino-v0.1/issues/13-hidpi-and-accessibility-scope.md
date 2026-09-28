# HiDPI and accessibility scope

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How much HiDPI and accessibility goes into v0.1?

- **HiDPI:**
  - Fractional scale, with frames at the output scale rounded up (07).
  - Mixed-scale outputs, and windows spanning outputs.
  - The default scale per output (msd persists output config, 06).
  - What `org.mate.interface window-scaling-factor` means under Camino (09 left it unsupported).
  - How XWayland apps are scaled (coordinate with [11](11-xwayland-integration.md)).
- **Accessibility:**
  - Orca: AT-SPI works through the toolkits, but what does the compositor owe it (key event forwarding, focus tracking)?
  - The magnifier.
  - AccessX, meaning sticky, slow, bounce and mouse keys, from `org.mate.accessibility-keyboard` (06 and 09 deferred it).
  - Typing break.

For each item decide v0.1, later, or never, and who implements it (Camino, a MATE component, or another existing tool).

## Answer

Decided in a grilling session on 2026-09-28. The user chose a fractional default scale following KWin, and took the recommended answer everywhere else. Inputs were what [06](06-v01-protocol-and-integration-set.md), [07](07-decoration-strategy.md), [09](09-gsettings-mapping.md), [10](10-xwayland-integration-audit.md), [11](11-xwayland-integration.md) and [12](12-screen-locking.md) handed on, and a source read of Smithay `928d4a9`, niri `1f03391`, cosmic-comp `9f746ea`, xwayland-satellite `63cdf17`, GTK `gtk-3-24` `28aa479`, gtk-layer-shell `eb78764`, mate-settings-daemon `927e9dc`, mate-desktop `cc52aec`, mate-control-center `24bc243`, mate-session-manager `57a9178`, Orca `16772ea` (51.1), at-spi2-core `cf54328` (2.62), mutter `888a7b7`, KWin `7f2f4b6`, libxkbcommon `66e702d` (1.14.0-beta2), labwc `c2a95cf` and Onboard `5efcdf6`. The relevant facts are quoted inline.

The scale policy is recorded in [ADR 0008](../../../docs/adr/0008-fractional-default-scale-integer-x11-scale.md): output scales default to KWin's fractional rule, `window-scaling-factor` can force an integer, and the X11 scale stays an integer.

### Summary

| Item | Scope | Who |
|---|---|---|
| Fractional output scales | v0.1 | Camino |
| Frames rendered exactly at a fractional scale | later | Camino |
| Mixed-scale outputs, windows spanning them | v0.1 | Camino |
| Default scale per output | v0.1: KWin's rule, 5% steps | Camino (msd's saved config wins) |
| 5% steps in the Wayland Display capplet | v0.1 | mate-control-center carried patch |
| `window-scaling-factor` | v0.1: 1 or 2 forces that output scale | Camino |
| `window-scaling-factor-qt-sync` | never | nobody |
| X11 scale | v0.1: largest integer output scale | Camino |
| Orca key events (`org.freedesktop.a11y.KeyboardMonitor`) | v0.1 | Camino |
| Orca mouse review (`org.freedesktop.a11y.PointerLocator`) | later | Camino |
| Orca focus tracking | v0.1, no compositor work | the toolkits over AT-SPI |
| Starting Orca | v0.1, no compositor work | mate-session |
| Accessible compositor UI (Alt+Tab, workspace popup) | later | Camino (AccessKit) |
| Magnifier | later | Camino |
| Sticky, slow and bounce keys, toggle-key beeps | v0.1 | Camino |
| Mouse keys | later | Camino |
| Typing break | later; in v0.1 the msd patch skips its plugin | mate-control-center carried patch (later), msd carried patch (v0.1) |
| On-screen keyboard in the session | v0.1, no compositor work | Onboard (uinput) |
| On-screen keyboard at the lock screen | later | Camino + `camino-locker` |

### HiDPI

**What clients can do.** GTK3, which is most of MATE, has no fractional scaling. `gdk/wayland` has no `wp_fractional_scale_v1` or viewporter code. It binds `wl_compositor` at `MIN(version,3)` (`gdkdisplay-wayland.c:411-412`), so it never receives `preferred_buffer_scale`. It renders at the largest integer `wl_output.scale` among the outputs a surface has entered (`gdkwindow-wayland.c:777-809`), and uses monitor 0's scale before the first enter (`:481-483`). The Wayland backend reads neither `GDK_SCALE` (only `gdk/x11/gdkscreen-x11.c:1110` does) nor the gsettings `scaling-factor`. gtk-layer-shell has no scale code at all, so mate-panel and caja's desktop behave like any GTK3 window. On a 1.5 output, every GTK3 surface renders at 2 and Camino downsamples it, just like 07's frames.

**Fractional output scales: v0.1, Camino.**

- Outputs are Smithay `Scale::Fractional`, snapped to N/120 as fractional-scale-v1 requires. Integer-only protocols get the scale rounded up (`output.rs:175-214`).
- Clients that support fractional-scale-v1 (GTK4, Qt 6, Firefox, Chromium) render exactly, through the fractional-scale and viewporter globals that 06 already enables. Everything else renders at the rounded-up scale and is downsampled.
- **Frames stay as 07 decided:** strips are rendered at the scale rounded up and downsampled. Exact fractional rendering of frames (cairo at the fractional scale with pixel-snapped geometry) is *later*. Most client content on a fractional output is GTK3 and downsampled anyway, so frames rendered exactly would stand out rather than blend in.
- **Cursors:** Camino loads the xcursor theme at `cursor-size` × the output's rounded-up scale, per output, and downsamples on fractional outputs, as niri (`cursor.rs:108-114`) and cosmic-comp (`render/cursor.rs:762-790`) do. GTK3 uses cursor-shape-v1 (`gdkdisplay-wayland.c:559-563`), so most cursors are Camino's own.

**Mixed-scale outputs and spanning windows: v0.1, Camino.**

- **Enter and leave** go to every output a surface overlaps (`output_update`, `desktop/space/wayland/mod.rs:24`). GTK3 then picks the largest scale itself.
- **One preferred scale per surface**, from its *primary output*: Smithay's `update_surface_primary_scanout_output` with the default comparison (switch when another output shows at least twice the visible area, `renderer/element/mod.rs:428-460`), run after each render as cosmic-comp does (`state.rs:957-1000`). Camino sends the primary output's fractional scale through `with_fractional_scale`, and its rounded-up scale as `preferred_buffer_scale` through `send_surface_state`. Before the first render, the preferred scale falls back to the parent's primary output, then the output under the pointer.
- **Frames use GTK3's rule:** strips are rendered at the largest rounded-up scale among the outputs the window overlaps, so an SSD window's frame and its GTK3 content always have the same resolution. A window moving between outputs re-renders its strips once the set of outputs changes (07's cache key already includes scale).
- **Layer surfaces** belong to one output and use its scale, sent before their first configure (as niri does, `handlers/layer_shell.rs:199-206`).
- Output-management scale changes apply live, and every surface on that output gets new preferred scales.

**Default scale per output: v0.1, Camino. msd's saved config wins.**

- msd's wlrandr plugin stores fractional scales in `monitors-wayland.xml` and applies them at login (`msd-wlrandr-manager.c:1424-1425`, `:711`). With no saved config it only turns disabled heads on, and never picks a scale itself (`:1698-1785`). The Wayland Display capplet offers 1.00–2.00 in quarter steps per output (`wlrandr-capplet.c:1093-1110`). So Camino's default is what a user sees until they save a layout, and on every output msd has no entry for.
- **Camino's default is KWin's rule, ported as-is** (`chooseScale`, `outputconfigurationstore.cpp:859-931`):
  - **Target DPI:** 125 for an internal output when a lid switch exists (a laptop panel), 150 for an internal output without one, 30.5 for an external output more than 500 mm tall (a TV), and 96 otherwise.
  - **Raw scale:** DPI ÷ target on each axis, clamped to 1 and to at most mode size ÷ 800 (and at most 3), so at least 800 logical pixels remain. The smaller axis wins.
  - **Rounding:** to 5% steps (6/120, exact in fractional-scale-v1). Below 1.20 it becomes 1, and within 0.06 of an integer it snaps to that integer.
  - **Bad sizes:** an EDID size under 3 mm on either axis gives 1.

  Camino learns about lid switches from libinput, as KWin does.
- **Why fractional:** the default should suit the panel, even though GTK3 is downsampled on a fractional output. MATE's X11 auto rule (`msd-xsettings-manager.c:247-292`) gives 2 only above 172.8 DPI, so a 14" 1080p laptop panel (about 157 DPI) stays at 1 and everything is tiny. Mutter's heuristic (which niri ports, `utils/scale.rs:18-58`), cosmic-comp's `calculate_scale` and sway's fixed 1 were the other candidates; KWin's 5% steps with integer snaps were chosen. KWin's snaps keep the scales where softness costs most at 1 or 2.
- **The Display capplet gets 5% steps.** Its list is 1.00, 1.25, 1.50, 1.75 and 2.00 (`wlrandr-capplet.c:1095`). With any other current scale, `combo_select` finds nothing and the combo is blank. A carried mcc patch lists 1.00 to 3.00 in 5% steps, so every scale Camino picks can be seen and chosen.

**`org.mate.interface window-scaling-factor`: v0.1, Camino.** (It was unsupported in 09.)

- On X11 it's GTK's global integer scale: 0 = auto, range 0–2, default 0 (`org.mate.interface.gschema.xml:178-188`). msd computes the scale from it and publishes `Gdk/WindowScalingFactor`. The X11 Display capplet exposes it as "auto / 100% / 200%" (`xrandr-capplet.c:548-585`), but the Wayland Display capplet doesn't.
- Under Camino, **0 means use KWin's rule**, and **1 or 2 forces that integer scale** on outputs with no saved config: "I want crisp 1× or 2×". It doesn't set the X11 scale directly (below).
- **Who wins:** Camino tracks, per output, whether the current scale is its own default or was set over output management. A live change to the key re-applies the default only to outputs still at Camino's default, so msd's saved layout and capplet choices are never overridden.
- **`window-scaling-factor-qt-sync`:** *never*. On X11, msd uses it to push `QT_SCALE_FACTOR` and `QT_FONT_DPI` into the session environment (`msd-xsettings-manager.c:541-570`). Qt on Wayland scales from the compositor, so setting `QT_SCALE_FACTOR` would scale twice. That code sits in the XSETTINGS path that ADR 0006's mirror-only patch skips, so no extra patch is needed; packaging must keep it that way (below).
- The GNOME mirror (ADR 0005) doesn't mirror it: native GTK3 on Wayland ignores `org.gnome.desktop.interface scaling-factor`.

**X11 scale: v0.1, Camino. It is the largest integer output scale.**

- **The value** is the largest of the outputs' rounded-up scales. This is cosmic-comp's `Enabled` mode (`xwayland.rs:639-750`), rather than its fractional default or xwayland-satellite's smallest scale (`server/mod.rs:719-741`). The mechanism is 11's.
- **Why integer, even with fractional output scales:** GTK3 on X11 takes only an integer `Gdk/WindowScalingFactor`. With a fractional X11 scale, X11 GTK apps would render at 1 with scaled fonts, so they would look different from the same app running natively. With the largest scale, X11 windows are sharp on the densest output. Smithay's client scale downscales them elsewhere (`compositor/mod.rs:655-665`), and on a fractional output they are downsampled like native GTK3 surfaces. **The mixed-DPI limit:** on any output less dense than the densest one, X11 windows are downsampled and slightly soft. Only native clients can be sharp everywhere.
- **XSETTINGS use msd's formulas**, so X11 apps get the same DPI as on MATE under X11 (`msd-xsettings-manager.c:356-374`, `:411-421`). With `s` the X11 scale and `dpi` the `org.mate.font-rendering dpi` key, or 96 when that is ≤ 0 (Xwayland's physical size is fake, so msd's own "measure the screen" fallback doesn't apply):

  | Setting | Value |
  |---|---|
  | `Xft/DPI` | `dpi × s × 1024` |
  | `Gdk/UnscaledDPI` | `dpi × 1024` |
  | `Gdk/WindowScalingFactor` | `s` |
  | `Gtk/CursorThemeSize` | `cursor-size × s` |

  This agrees with the GNOME mirror's `text-scaling-factor = dpi/96` (09), so native and X11 GTK text are the same size.
- The X11 scale changes only when the set of outputs or their scales change. 11 already says what that triggers: XSETTINGS, the X cursor, the work area, frame extents, and reconfiguring every XWayland window.

### Accessibility

**Orca key events: v0.1, Camino.**

- **What Orca needs:** Orca 51 opens its keyboard device with `Atspi.Device.new_full("org.gnome.Orca")` (`ax_device_manager.py:61`). On Wayland, at-spi2-core (2.55 and later) tries the compositor's **`org.freedesktop.a11y.KeyboardMonitor`** first. The fallback is the legacy device, which only sees keys that toolkits report from inside accessible apps (`atspi-device.c:246-266`). Without the monitor, the Orca modifier and Orca's global commands don't work when focus is somewhere unexpected.
- **Camino implements it** on the session bus, with the well-known name `org.freedesktop.a11y.Manager` and path `/org/freedesktop/a11y/Manager`. The interface is Mutter's (`data/dbus-interfaces/org.freedesktop.a11y.xml`):
  - `GrabKeyboard` and `UngrabKeyboard`;
  - `WatchKeyboard` and `UnwatchKeyboard`;
  - `SetKeyGrabs(au modifiers, a(uu) keystrokes)`, including the rule that a lone grabbed modifier is swallowed unless it's pressed again within the repeat delay, so Orca's modifier can still work as itself on a double press;
  - the `KeyEvent(b u u u q)` signal.

  All of it is per sender and dropped when the sender leaves the bus. niri's version is 718 lines of Rust (`dbus/freedesktop_a11y.rs`) and cosmic-comp's is 304 lines (`dbus/a11y_keyboard_monitor.rs`).
- **Who may call it:** only a client that owns `org.gnome.Orca.KeyboardMonitor`, the name at-spi2-core requests for the app ID `org.gnome.Orca` (`atspi-device-a11y-manager.c:552`). This is Mutter's allowlist (`meta-a11y-manager.c:409-411`). An allowlisted client can read every keystroke, but so can any process of the user on X11, so this is the same trust boundary as MATE today.
- **Order in the key path:** first the lock state (12), then AccessX (below, because it decides what counts as a key press), then the keyboard monitor's grabs and watches, then Camino's bindings, then the focused client. So Orca sees keys after sticky and slow keys have applied, and an Orca grab outranks a Camino binding.
- **While locked,** the monitor keeps working, so Orca can read `camino-locker`'s dialog (GTK3, so AT-SPI). This is an exception to 12's "only VT and layout switching": KeyEvents go only to the allowlisted client, and Orca doesn't echo password fields.

**Orca's other needs.**

- **Focus tracking:** v0.1, with no compositor work. Orca follows AT-SPI `window:activate` and focus events from the toolkits, and those follow xdg-toplevel `activated` and X11 focus, which Camino already sends correctly.
- **Mouse review:** *later*, Camino. It needs `org.freedesktop.a11y.PointerLocator` (`QueryPointer` and `PointerPositionChanged`, same file), which has to map the toplevel under the pointer to its AT-SPI bus name.
- **Camino's own UI:** *later*, Camino. The Alt+Tab popup, the workspace-switch popup, the window menu and 12's give-up message are drawn by the compositor, so Orca can't read them. The model is niri's AccessKit adapter (`a11y.rs`, 372 lines). In v0.1, Orca announces the newly focused window once Alt+Tab is released.
- **Starting Orca:** v0.1, with no compositor work. mate-session starts `org.mate.applications-at-visual exec` (default `orca`) when `startup` is true (`mate-session/main.c:304-340`), and Orca's own autostart file follows `org.gnome.desktop.a11y.applications screen-reader-enabled`. GTK3's AT-SPI bridge is built in, and `org.mate.interface accessibility` only adds `GTK_MODULES=gail:atk-bridge` (`main.c:717-722`). It stays *never* for Camino (09).

**Magnifier: later, Camino.** MATE ships no magnifier: `org.mate.applications-at-visual` names only a screen reader, and no MATE schema configures zoom. Marco had none either, so parity doesn't require one. On Wayland, only the compositor can magnify. The later design is labwc's (`magnifier.c`, 303 lines): a scene-level zoom that follows the pointer, with Zoom In, Zoom Out and Toggle bindings. Its keys go in `org.mate.Camino` under 09's rule.

**AccessX: v0.1 for sticky, slow and bounce keys and the toggle-key beeps; mouse keys later. Camino.**

- On X11 the X server does AccessX inside XKB. msd's `a11y-keyboard` plugin only writes `org.mate.accessibility-keyboard` into the XKB controls (`msd-a11y-keyboard-manager.c:313-405`) and shows notifications when a gesture toggles a feature. The plugin is `X11Only=true`, so under Camino nothing applies the keys. Camino owns input (06), so it owns AccessX.
- **Honoured in v0.1:**
  - `enable`, `timeout-enable` and `timeout`;
  - `stickykeys-enable`, `-latch-to-lock`, `-two-key-off` and `-modifier-beep`;
  - `slowkeys-enable`, `-delay` and the three `slowkeys-beep-*` keys;
  - `bouncekeys-enable`, `-delay` and `-beep-reject`;
  - `togglekeys-enable`, `capslock-beep-enable` and `feature-state-change-beep`.

  They apply live, in Camino's keyboard filter, before xkbcommon, so they cover Wayland and XWayland clients and the lock screen alike.
- **Implemented in Camino, not in xkbcommon:** libxkbcommon 1.14 adds sticky and mouse keys (`XKB_KEYBOARD_CONTROL_A11Y_STICKY_KEYS`, `…_MOUSE_KEYS`, `xkbcommon.h:3470-3520`), but at the time of the source read it's a beta, and Smithay's bindings don't expose it. Slow and bounce keys aren't in xkbcommon at all. KWin's plugins show the size: sticky keys is 225 lines, slow keys 98 and bounce keys 66. Camino switches its sticky keys to xkbcommon's once Smithay exposes it.
- **Beeps** go through Camino's bell path (06), so they follow `audible-bell`/`visual-bell` and the sound theme.
- **Toggle gestures** (`enable`: Shift pressed five times toggles sticky keys, Shift held for eight seconds toggles slow keys): Camino detects them, flips the gsettings key and posts a desktop notification through `org.freedesktop.Notifications`. msd's X11 confirmation dialog isn't reproduced.
- **`togglekeys-backend`:** both values mean Camino's implementation.
- **Mouse keys** (`mousekeys-*`): *later*. They need pointer emulation with msd's acceleration curve. They stay *unsupported* under 09's rule and are logged once when enabled.

**Typing break: later, mate-control-center carried patch.**

- MATE's typing break is `mate-typing-monitor` from mate-control-center. msd's `typing-break` plugin spawns it when `org.mate.typing-break enabled` is true (default false, `msd-typing-break-manager.c:107-111`). The plugin isn't `X11Only`.
- The monitor polls `XScreenSaverQueryInfo` on the X root every 3 s (`drw-monitor.c:148`, `:182`) and never restricts GDK to X11. Under Camino it starts as a native Wayland client (08 unsets `GDK_BACKEND`) and crashes. Even on XWayland it would see only X11 input.
- **v0.1:** the carried msd patch keyed on `MATE_WM_NAME=Camino` also skips the typing-break plugin, and Camino logs `enabled=true` once as unsupported.
- **Later:** a carried mate-control-center patch replaces `drw-monitor` with ext-idle-notify-v1 (Camino sees all input, X11 included) and puts the break window on gtk-layer-shell's overlay layer with exclusive keyboard.

**On-screen keyboard.**

- **In the session:** v0.1, with no compositor work. Onboard now runs on Wayland: it uses gtk-layer-shell for its window and a uinput device for typing (`WaylandUtils.py`, with a udev rule). The uinput device reaches Camino through libinput like any keyboard, so it works for every client and gets Camino's keyboard settings. virtual-keyboard stays out (06): it would let any client inject keys, and Onboard doesn't need it. Squeekboard and wvkbd, which need it, aren't supported.
- **At the lock screen:** *later*, Camino and `camino-locker`. The lock sits above every layer (12), so Onboard can't be seen there, although its uinput keys would still reach the locker. The options are a keyboard widget inside `camino-locker`, or Camino letting one supervised layer surface sit above the lock. That's decided when it's built.

### Handed on

- **[07](07-decoration-strategy.md):** frame strips render at the largest rounded-up scale among the outputs the window overlaps. Exact fractional frames are later.
- **[09](09-gsettings-mapping.md):** `window-scaling-factor` is now honoured, and `window-scaling-factor-qt-sync` is *never*. Most of `org.mate.accessibility-keyboard` is now honoured, and `mousekeys-*` stays unsupported. `org.mate.typing-break enabled` is unsupported.
- **[11](11-xwayland-integration.md):** the X11 scale value, and the XSETTINGS DPI formulas above.
- **[12](12-screen-locking.md):** the keyboard monitor keeps working while locked, and AccessX applies to the lock screen.
- **14 Development and testing loop:** Orca in a nested session (does it bind the monitor?), reading `camino-locker` with Orca, a GTK3 window dragged across a 1.0 and a 2.0 output, and the auto scale rule against real EDIDs. ([14](14-development-and-testing-loop.md): the scale rule and AccessX are unit tests, KeyboardMonitor and mixed-scale enter/leave are headless tests, and Orca and the visual drag are checklist items.)
- **15 Crate and module architecture:** the keyboard path (lock filter, AccessX, keyboard monitor, bindings, client), and the `org.freedesktop.a11y.Manager` service beside the other D-Bus names. ([15](15-crate-and-module-architecture.md): one function in `input` calls the stages in this order; AccessX is a pure filter in `input::accessx`, and the service is `dbus::a11y`.)
- **16 Packaging:** a new carried mcc patch (5% scale steps in the Wayland Display capplet). The carried msd patch grows (skip the typing-break plugin under Camino) and must keep skipping the qt-sync `Setenv`. Suggest Onboard's udev rule for users who need an on-screen keyboard. ([16](16-packaging.md): the 5% steps are gated on Camino, not Wayland, [ADR 0011](../../../docs/adr/0011-patched-mate-packages-replace-stock-and-gate-on-camino.md); onboard is an optdepend.)
