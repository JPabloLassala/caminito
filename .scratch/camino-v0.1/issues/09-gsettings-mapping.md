# gsettings mapping

Type: grilling
Status: resolved
Blocked by: 01, 05
Map: [Camino v0.1](../map.md)

## Question

For the v0.1 feature cut, which `org.mate.Marco.*` keys does Camino honour (and with what semantics on Wayland), which does it ignore, and what goes in the new `org.mate.Camino` schema?

## Answer

Decided on 2026-09-28. The user asked for the recommended answer to every question to be taken, so there was no round-by-round grilling. Inputs:

- the [Marco feature inventory](01-marco-feature-inventory.md) (branch `research/marco-feature-inventory`);
- the cut in [05](05-v01-marco-feature-cut.md) and the handoffs from [06](06-v01-protocol-and-integration-set.md) and [08](08-session-launch-model.md);
- the installed MATE 1.28 schemas;
- a source read of mate-settings-daemon `927e9dc`, mate-wayland-session `ad2a5c1` and GTK `gtk-3-24` `28aa479`.

The relevant facts are quoted inline. The one hard-to-reverse part, mirroring MATE's settings into the GNOME schemas that native GTK reads, is recorded in [ADR 0005](../../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md).

### Rules for every key

- **Camino reads schemas it doesn't own.** Camino reads `org.mate.Marco.*`, `org.mate.interface`, `org.mate.peripherals-*`, `org.mate.session`, `org.mate.screensaver`, `org.mate.power-manager` and `org.mate.sound`, and never ships or overrides them. Marco's schemas are a runtime dependency (06). If they're missing, Camino exits at start-up with a clear error, instead of running with made-up defaults.
- **Live.** Every key Camino honours takes effect on change, with no restart, as in Marco. The only exceptions are `XCURSOR_THEME` and `XCURSOR_SIZE`, which are exported once at start-up (08).
- **Four kinds of key.** Each key Camino reads is exactly one of these:
  - *Honoured*: applied with the semantics below.
  - *Never* (05's list): read, ignored, never logged.
  - *Not Camino's*: keys that another MATE component applies (for example msd, mate-power-manager's backlight, or the matekbd indicator). Camino doesn't read them.
  - *Unsupported*: ignored for now. Camino logs it **once per run, and only when the user's value differs from the schema default**, since a log line at every default login would be noise. The list below, of unsupported keys whose defaults Camino doesn't match, is the roadmap.
- **Out-of-range values** (for example `num-workspaces=50` or an unparseable accelerator) are clamped, or fall back to the schema default, and are logged once, as Marco does.

### `org.mate.Marco.*`

05 settled that every key except its "never" list is honoured. These are the Wayland-specific semantics:

| Key | Semantics under Camino |
|---|---|
| `focus-mode`, `raise-on-click`, `auto-raise`, `auto-raise-delay` | As in Marco, for toplevels. Layer surfaces take focus only by their own keyboard-interactivity. |
| `focus-new-windows` | xdg-activation tokens, as in 05. |
| `mouse-button-modifier` | Parsed as a GTK accelerator. `<Super>` and `<Mod4>` both mean the logo key. An empty string or `disabled` turns modifier+drag off. Anything unparseable falls back to `<Alt>`, with a log line. |
| `resize-with-right-button` | As in Marco. |
| `button-layout`, `action-*-titlebar` | SSD frames only (05). CSD apps get the equivalent through the mirror below. |
| `theme` | Looked up as `themes/<name>/metacity-1/` in `$XDG_DATA_HOME` then `$XDG_DATA_DIRS`. Changes reload live (05). An unknown theme falls back to the default theme and is logged. |
| `titlebar-font`, `titlebar-uses-system-font` | The "system font" is `org.mate.interface font-name`. |
| `num-workspaces`, `dynamic-workspaces`, `wrap-style`, `workspace-names name-N` | As in Marco, clamped to 1–36. The count and names reach the pager over ext-workspace (06). |
| `compositing-manager`, `compositing-fast-alt-tab`, `reduced-resources` | As in 05: effects off, icons instead of thumbnails, and no minimise animation. |
| `icon-size`, `alt-tab-*`, `show-tab-border` | As in Marco. `icon-size` is in logical pixels. |
| `visual-bell`, `visual-bell-type`, `audible-bell` | For every bell source in 06. `audible-bell` is also gated by `bell-mode` (below). |
| `show-desktop-skip-list` | Matches `app_id` or `WM_CLASS` (05). |
| `center-new-windows`, `placement-mode`, `allow-tiling`, `allow-top-tiling`, `allow-tile-cycling` | As in Marco. |
| `global-keybindings`, `window-keybindings`, `keybinding-commands` | As in Marco, and the command strings are spawned through the shell. `run-command-terminal` uses `org.mate.applications-terminal exec`. |
| `application-based`, `disable-workarounds` | *Never* (05). |

### Foreign keys Marco read

- **`org.mate.interface enable-animations`:** honoured. When it's off, there is no minimise animation, as in Marco.
- **`org.mate.interface accessibility`:** *never*. Marco used it only to force opaque moves over wireframe, and Camino has no wireframe.
- **`org.mate.peripherals-mouse cursor-theme` and `cursor-size`:** honoured live for Camino's own cursor, cursor-shape-v1 and XWayland's root cursor. Clients get them through the mirror.
- **`org.mate.peripherals-mouse double-click` and `drag-threshold`:** honoured for Camino's frames (titlebar double-click, click-versus-drag). Marco used to get these from GTK.
- **`org.mate.sound`:**
  - `event-sounds` gates the workspace-switch sounds;
  - `theme-name` selects the sound theme for those sounds and the bell.
  
  Marco got both through GTK's canberra integration, so Camino reads them directly.
- **`org.mate.interface window-scaling-factor`:** unsupported, and handed to the HiDPI fog.

### Input devices (the 06 handoff)

**Rule:** Camino applies to libinput exactly what msd's mouse and keyboard plugins apply to libinput devices under X11, and nothing more. On Wayland those plugins are `X11Only` and don't load (`mouse.mate-settings-plugin.desktop.in:4`, `keyboard…:4`), so the Mouse and Keyboard capplets mean the same thing on both sessions. Settings are re-applied when a device is hot-plugged.

**`org.mate.peripherals-mouse`** (non-touchpad pointers):

| Key | libinput |
|---|---|
| `left-handed` | `left_handed` |
| `motion-acceleration` | `-1` means the device's default speed. Otherwise the speed is `(m − 1) × 2/9 − 1`, clamped to [−1, 1] (msd `msd-mouse-manager.c:751-754`, so 1 → −1 and 10 → +1). |
| `accel-profile` | `default` means the device's default, otherwise `adaptive` or `flat` (`:1030-1050`). |
| `middle-button-enabled` | `middle_emulation` on non-touchpads only (`:851-876`). |
| `natural-scroll` | `natural_scroll` on non-touchpad scroll devices. |
| `motion-threshold` | *Never*. msd uses it only for non-libinput drivers (`:661`). |
| `locate-pointer` | Honoured by Camino itself. Pressing and releasing Ctrl alone draws the ripple around the pointer in the scene graph. msd's `msd-locate-pointer` helper is X11-only. |
| `cursor-*`, `double-click`, `drag-threshold` | See "Foreign keys Marco read" above. |

**`org.mate.peripherals-touchpad`:**

| Key | libinput |
|---|---|
| `touchpad-enabled` | `send_events` disabled. |
| `left-handed` (`right`, `left`, `mouse`) | `mouse` follows the mouse key (`:1085-1098`). |
| `motion-acceleration`, `accel-profile`, `natural-scroll` | As for the mouse, but using the touchpad keys. |
| `tap-to-click` | `tap_enabled`. |
| `disable-while-typing` | `dwt`. |
| `two-finger-click`, `three-finger-click` | Click method clickfinger if either is non-zero, else button areas. The button numbers are ignored, as in msd (`:1275-1287`). |
| `vertical-two-finger-scrolling`, `vertical-edge-scrolling` | The scroll method: two-finger wins over edge, and neither means no scrolling (`:1520-1537`). |
| `horizontal-two-finger-scrolling`, `horizontal-edge-scrolling` | Whichever matches the chosen method decides whether horizontal scrolling is on (`:1550-1561`). libinput has no switch for this, so Camino drops horizontal axis events from that device. |
| `tap-button-*-finger`, `motion-threshold` | *Never*. msd uses them only for synaptics. |

**`org.mate.peripherals-keyboard`:**

- `repeat`, `rate` and `delay` go to `wl_keyboard.repeat_info`, which Xwayland also follows:
  - `rate` is repeats per second, and `delay` is in milliseconds;
  - `repeat=false` or `rate ≤ 0` sends rate 0;
  - a `delay` of 0 or less becomes 1.
- `remember-numlock-state` and `numlock-state`: when remembering is on, Camino restores NumLock at start-up and writes the state back on every change (msd `:165-194`, `:260-265`).
- `bell-mode`:
  - `off` silences the audible bell, whatever `audible-bell` says; the visual bell is unaffected;
  - `on` and `custom` both behave as on. msd never reads `bell-custom-file` either.
- `click`, `click-volume`, `bell-pitch` and `bell-duration` are *never*. They are XKB key-click and PC-speaker settings.

**`org.mate.peripherals-keyboard-xkb`:**

- **`kbd` layouts, options and model:**
  - They are libmatekbd strings: `layout\tvariant` and `group\toption`.
  - Camino converts them to RMLVO (rules, model, layouts, variants, options) for xkbcommon, with rules `evdev`, and rebuilds the keymap live.
  - Group-switch options (`grp:*`) work through xkbcommon's own group actions.
  - Empty lists mean the system default: `XKB_DEFAULT_*`, then systemd-localed's X11 keymap.
- **`general group-per-window`** (default true) and **`default-group`:** *unsupported*. v0.1 has one global group. This is the one place where Camino's behaviour differs from a default MATE install, and it's documented rather than logged.
- **Everything else** in `general`, `indicator` and `preview` is *not Camino's*. It belongs to the matekbd indicator applet.

**`org.mate.accessibility-keyboard`:** *unsupported*. It is logged once when `enable=true`, and handed to the accessibility fog (06).

### Idle, blanking and locking (the 06 handoff)

Camino owns idle (06). It reads:

| Key | Meaning |
|---|---|
| `org.mate.session idle-delay` (minutes) | The idle threshold. After it, Camino blanks the outputs if `idle-activation-enabled` is on. |
| `org.mate.screensaver idle-activation-enabled` | Whether reaching idle blanks the outputs. It doesn't affect idle-notify clients. |
| `org.mate.screensaver lock-enabled`, `lock-delay` (minutes) | Whether to lock after blanking starts, and how long after. |
| `org.mate.power-manager sleep-display-ac`, `sleep-display-battery` (seconds, 0 = never) | Idle time before the outputs are powered off (DPMS). Camino picks the key by UPower's `OnBattery`. mate-power-manager's own DPMS goes to XWayland and does nothing there. `sleep-display-ups` is *never*: a UPS counts as AC unless UPower reports battery. |

- **Never:** `org.mate.screensaver mode`, `themes` and `cycle-delay`. Camino only blanks, and screensaver themes are X11 hacks.
- **The locker's appearance and options** (`lock-dialog-*`, `picture-filename`, `status-message-enabled`, `user-switch-enabled`, `logout-*`, `embedded-keyboard-*`) are left to the Screen locking fog, together with the choice of locker.
- **Not Camino's:** mate-power-manager's dim, brightness, suspend and lock-on-suspend keys. It still calls `org.mate.ScreenSaver.Lock` on suspend, which reaches Camino's shim (06).

### Settings for native Wayland clients

GTK3 on Wayland reads only GNOME schemas (`gdkscreen-wayland.c:525-565`):

- `org.gnome.desktop.interface`
- `org.gnome.desktop.peripherals.mouse`
- `org.gnome.desktop.wm.preferences`
- `org.gnome.desktop.sound`

It never reads an `org.mate.*` schema. msd's xsettings plugin feeds only XWayland (`msd-xsettings-manager.c:898-919`), and nothing in MATE syncs MATE's settings into the GNOME schemas. mate-wayland-session ships a static override and tells users to set both by hand (its `README.md:32-37` and `TODO:5`). Without a fix, native GTK apps under Camino would ignore MATE's theme, fonts and cursor.

- **A carried msd patch** in the xsettings plugin, active only when `MATE_WM_NAME=Camino`:
  - It mirrors MATE's settings one way, MATE to GNOME, live, into the GNOME keys GTK reads, reusing the plugin's existing translation table.
  - It writes only when a value differs, so it never loops.
  - Settings made on the GNOME side are overwritten at the next MATE change, and at start-up.
- **What's mirrored:**

| Source | Target |
|---|---|
| `org.mate.interface` `gtk-theme`, `icon-theme`, `font-name`, `document-font-name`, `monospace-font-name`, `cursor-blink`, `cursor-blink-time`, `gtk-im-module`, `gtk-enable-animations`, `gtk-enable-primary-paste`, `gtk-overlay-scrolling` | `org.gnome.desktop.interface`, key for key (`gtk-overlay-scrolling` becomes `overlay-scrolling`, and `gtk-enable-animations` becomes `enable-animations`) |
| `org.mate.peripherals-mouse` `cursor-theme`, `cursor-size` | `org.gnome.desktop.interface` |
| `org.mate.peripherals-mouse` `double-click`, `drag-threshold` | `org.gnome.desktop.peripherals.mouse` |
| `org.mate.font-rendering` `antialiasing`, `hinting`, `rgba-order`, and `dpi` as `dpi/96` (1.0 when `dpi` ≤ 0) | `org.gnome.desktop.interface` `font-antialiasing`, `font-hinting`, `font-rgba-order`, `text-scaling-factor` |
| `org.mate.interface gtk-decoration-layout` (kept in step with Marco's `button-layout` by mcc) | `org.gnome.desktop.wm.preferences button-layout` |
| `org.mate.Marco.general action-{double,middle,right}-click-titlebar` | `org.gnome.desktop.wm.preferences`, with `_` becoming `-` in the nick; values the GNOME enum lacks become `none` |
| `org.mate.sound event-sounds`, `input-feedback-sounds`, `theme-name` | `org.gnome.desktop.sound` |

- **The trade-off:** a user who also logs into GNOME gets MATE's choices there. The alternatives are recorded in ADR 0005.
- **Other toolkits:** Qt and libadwaita apps follow whatever they already read. Nothing extra is done for them in v0.1.

### The `org.mate.Camino` schema

- **Shape:** one flat schema, `org.mate.Camino`, at path `/org/mate/camino/`, shipped by Camino.
- **What goes in it:** a key is added only if it is Wayland-only **and** no existing MATE schema already expresses it. v0.1 has four keys. Three together are the pager grid layout from 05 and 06, mirroring `_NET_DESKTOP_LAYOUT` and libwnck's `try_set_workspace_layout`; the fourth, `locker-command`, was added by [12](12-screen-locking.md):

| Key | Type / default | Meaning |
|---|---|---|
| `workspace-layout-orientation` | enum `horizontal`/`vertical`, default `horizontal` | Fill order: along rows, or down columns. |
| `workspace-layout-rows` | i, default 1 | Grid rows. 0 means derive it from the columns. |
| `workspace-layout-columns` | i, default 0 | Grid columns. 0 means derive it from the rows. |
| `locker-command` | as, default `['camino-locker']` | The locker Camino spawns to lock the session ([12](12-screen-locking.md)). |

- **Derivation rules:**
  - If both keys are 0, the grid is one row.
  - If both are set but the grid is too small for `num-workspaces`, the fill-order dimension grows.
  - The starting corner is always top-left.
- **Uses:** the ext-workspace `coordinates` (06), the direction bindings, `wrap-style` and the workspace popup.
- **Who writes it:** under Camino, the patched pager's "rows" preference writes these keys. On a horizontal panel it writes orientation `horizontal` with rows = `num-rows` and columns = 0. On a vertical panel it writes `vertical`, with columns = `num-rows` and rows = 0. When several pagers disagree, the last writer wins, as with `_NET_DESKTOP_LAYOUT` on X11.
- **Not in it:**
  - decoration policy (07 fixed it);
  - output configuration (msd owns it);
  - privileged-protocol access (fixed by 06);
  - a quit binding (08: there isn't one);

### Unsupported in v0.1 (the "later" roadmap)

- `org.mate.peripherals-keyboard-xkb.general` `group-per-window` and `default-group`.
- `org.mate.accessibility-keyboard`, all keys.
- `org.mate.interface window-scaling-factor`.

These add to 05's "later" list (the XWayland-only hints and session restore), which involves no keys.

### Handed on

- **Packaging:**
  - ship the `org.mate.Camino` schema;
  - the runtime dependency on Marco's schemas;
  - the carried msd xsettings-mirror patch;
  - the carried mate-panel pager patch now writes the three `workspace-layout-*` keys.
- **Screen locking fog:** the locker's keys, and any `org.mate.Camino` key it needs.
- **HiDPI fog:** `window-scaling-factor`.
- **Accessibility fog:** `org.mate.accessibility-keyboard`.
- **Known gap:** the matekbd layout indicator runs on XWayland, so it sees Xwayland's keymap, not Camino's global group.
