# Spec: Night Light

Written on 2026-10-09 from a grilling session with the developer, whose decisions are in [the map](map.md). It gists them and links the ADR that holds the one hard to reverse. Where the two differ, the ADR is right and this file is to be fixed.

Nothing here is built.

## Problem

MATE has no Night Light. On X11 users run redshift; on Wayland a compositor has to offer it, and Caminito offers neither its own nor `wlr-gamma-control`, so gammastep and wlsunset can't run either. Plasma has Night Light with a day and a night temperature. macOS has True Tone, which follows the room's light through a sensor that the developer's machine doesn't have.

## Solution

**Night Light** (CONTEXT.md) warms every output toward a lower colour temperature on a **schedule**. The **sun schedule** ("Follow the sun") stands in for True Tone: it follows the sun's elevation at the user's location, warmest at night and a little warm at noon, changing gradually all day. The **constant schedule** holds one temperature. Caminito writes it as each output's gamma table ([ADR 0023](../../docs/adr/0023-caminito-owns-night-light-and-serves-wlr-gamma-control.md)), and also serves `wlr-gamma-control`: a **gamma client** wins its own output while it holds it. The settings are a tab of caminito-displays.

## User stories

1. As a user, I want my screen warmer in the evening and at night without running anything, so that it's easier on my eyes.
2. As a user, I want it to change gradually with the daylight, a little warm at noon and warmest at night, so that I never see it switch.
3. As a user, I want it to follow the real sun where I am, through the seasons, without entering anything.
4. As a user with no location service, or offline, I want it to still follow a day, and to be told why its times may be off.
5. As a user, I want one warm temperature all the time instead, if I'd rather.
6. As a user, I want to set the noon, night and constant temperatures.
7. As a user, I want to turn it on and off from its settings and from a shortcut I choose.
8. As a user, I want to pause it until tomorrow's sunrise, for a film or a photo, and have it come back by itself.
9. As a user, I want screenshots and screencasts to show the true colours.
10. As a user of gammastep or wlsunset, I want them to work, and Night Light to stay off the outputs they hold.
11. As a user, I want changes to fade rather than jump, and the screen to be right at once when I log in.
12. As a user, I want it on the lock screen too, and still right after the screen wakes or a monitor is plugged in.

## Implementation decisions

### Settings

Keys in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml`), read by the settings thread and translated like the others:

| Key | Type | Default | |
| --- | --- | --- | --- |
| `night-light-enabled` | `b` | `false` | |
| `night-light-schedule` | enum `sun`, `constant` | `'sun'` | |
| `night-light-noon-temperature` | `u` | `5500` | 1000 to 6500 |
| `night-light-night-temperature` | `u` | `3400` | 1000 to 6500 |
| `night-light-constant-temperature` | `u` | `4000` | 1000 to 6500 |
| `night-light-paused-until` | `x` | `0` | Unix time; 0 is not paused |
| `night-light-last-location` | `m(dd)` | `nothing` | latitude, longitude; written by Caminito |

Every output gets the same temperature. There are no per-output settings.

### The temperature

- **The sun schedule:**
  - While the sun is below −6° (civil twilight), the night temperature.
  - At that day's highest elevation (solar noon), the noon temperature, so every day reaches it, short winter ones too.
  - In between, it follows the elevation linearly from −6° up to the day's highest, interpolated in **mireds** (10⁶ / K), not kelvin, so the change looks even.
  - If the sun never climbs above −6° that day, it's the night temperature all day. Under the midnight sun, it bottoms out where the sun does and never reaches the night temperature.
  - It's worked out again about once a minute, in steps too small to see.
- **The constant schedule:** the constant temperature, for as long as Night Light is on.
- **The sun's position** is computed in Caminito from the time and the location, with no service and no crate that pulls in more than arithmetic.
- **The white point for a temperature** comes from a blackbody approximation computed from a formula, as wlsunset (MIT) does. redshift's and gammastep's tables are GPL-3.0 and Caminito is GPL-2.0-or-later, so nothing is copied from them.

### The location

These sources are tried in order. Each fallback is logged once, naming the source it fell to.

1. **geoclue**, over zbus, at city accuracy, with desktop id `caminito`. Caminito isn't sandboxed, so geoclue authorises it without asking. It still needs an agent registered for the user: MATE ships none, and geoclue's demo agent autostarts outside GNOME. `caminito-git` lists `geoclue` as an optional dependency. With no agent, geoclue's start fails after about 5 s, and the next source is used meanwhile, never waited for. Every location geoclue gives is written to `night-light-last-location`.
2. **The last location geoclue gave**, from that key.
3. **The time zone's coordinates.** The zone is the `/etc/localtime` link's target under `/usr/share/zoneinfo/`, and the coordinates are its row in `zone.tab`, following a backward link through `tzdata.zi` when the name isn't there. `zone1970.tab` drops merged zones such as `Europe/Amsterdam`, so it isn't used.
4. **None,** for example under `Etc/UTC`. The sun schedule then uses fixed sun times: night until 07:00 and from 19:00, the noon temperature at 13:00, and a half-sine elevation in between, as though the sun rose at 07:00 and set at 19:00. The Night Light tab says "Location unknown, using fixed times".

### Turning it off and pausing it

- **`night-light-enabled`** is flipped by the tab, by `ToggleNightLight()` on `org.mate.Caminito`, and by a custom shortcut the user binds in Keyboard Shortcuts (ticket 100) to `caminito-displays --toggle-night-light`. That command calls the method and exits with no window. No shortcut is bound by default.
- **Pause:**
  - `PauseNightLight()` writes `night-light-paused-until` with the next sunrise: the next time the sun climbs past −6°, from the same location sources, or 07:00 with none. It is capped at 24 hours.
  - It means the same thing under either schedule.
  - Pausing at 10:00 lasts until tomorrow's sunrise.
  - `ResumeNightLight()` writes 0.
  - Turning `night-light-enabled` off, or on, writes 0.
  - Because the end time is a key, a pause survives a crash and a log-out.
  - When the time passes, Caminito writes 0 and the schedule takes over.
- **No inhibit API** for apps.

### What the user sees, and when

- **Every change fades over about 2 s,** from the gamma table the output has to the one it should have. That includes Night Light on and off, a pause or resume, a change of schedule or temperature, and a gamma client letting go.
- **At session start the table is applied at once,** with no fade from neutral.
- **The gamma table applies to everything on the output,** the lock screen and caminito-locker included, so Night Light stays on there.

### Caminito: the gamma tables

[ADR 0023](../../docs/adr/0023-caminito-owns-night-light-and-serves-wlr-gamma-control.md). niri, which is on Smithay too, is the reference.

- **The tty backend sets each CRTC's table:**
  - by the atomic `GAMMA_LUT` property where the CRTC has it, as a property blob set outside DrmCompositor's frame commits, through `DrmOutputManager::device()`;
  - by the legacy gamma call otherwise, where a linear ramp has to be written out because the legacy call can't reset;
  - by neither where the CRTC has no gamma, which is logged once per output.

  DrmCompositor's commits never touch `GAMMA_LUT`, so a table stays across frames and modesets.
- **When the table is reset or applied again:**
  - It's reset when Caminito takes the device, since a previous compositor's survives `DrmDevice::new`.
  - It's applied again after a session resume, a new CRTC for an output, a modeset, and DPMS on.
  - It's never set while the session is paused: the change is kept and applied on resume.
- **The nested run and the headless backend have no gamma tables.** Night Light keeps its state and tints nothing.
- **`DumpState`** shows Night Light's state: enabled, schedule, pause, the location and its source, the target temperature, and per output the temperature applied and whether a gamma client holds it.
- **The `NightLight` property** on `org.mate.Caminito` is read-only and tells when it changes. It's an `a{sv}` with:
  - `temperature` (`u`, the target in kelvin, 6500 when off or paused);
  - `location-source` (`s`: `geoclue`, `stored`, `time-zone` or `none`);
  - `next-sunrise` (`x`, Unix time).

  The tab reads it.

### Caminito: wlr-gamma-control

- **Version 1, on the tty backend only, privileged** (06): not offered to a client in a `wp_security_context_v1` sandbox.
- **`get_gamma_control`:**
  - It sends `gamma_size` from the CRTC.
  - It sends `failed` at once when the output has no gamma, or another gamma client already holds it.
- **`set_gamma(fd)`:** reads exactly `size × 3` u16s and nothing more, or sends `failed`. Either way the table is set at once, with no fade, while Night Light steps back from that output alone.
- **When the client lets go:** if it destroys the control, fails, or exits, Night Light fades back in on that output.
- **When the output is removed,** the control gets `failed`.
- niri's `src/protocols/gamma_control.rs` ports nearly as it is. Smithay re-exports `wayland-protocols-wlr`.

### caminito-displays

- **A second tab, "Night Light,"** beside the displays page. It shows:
  - the on/off switch;
  - the schedule, "Follow the sun" or "Constant";
  - the temperatures the schedule uses, as sliders from warm to neutral that show kelvin;
  - "Pause until sunrise" or "Resume", with "Paused until 06:42";
  - the location note when the source is `none`.
- **It writes the keys through gio,** and calls `PauseNightLight()` and `ResumeNightLight()`. It reads `NightLight` for the note and the pause time. Its page state is toolkit-free, as `page.rs` is.
- **`--night-light`** opens the window on that tab.
- **`caminito-night-light.desktop`,** in Preferences, runs `caminito-displays --night-light`.
- **`--demo`** shows the tab too.

## The cut into tickets

Seven slices, filed in `.scratch/issues/` as tickets 137 to 143. Each is run by itself once the ones it's blocked by are in.

| | Slice | Blocked by |
| --- | --- | --- |
| 137 | [**The constant schedule on the tty.**](../issues/137-night-light-constant-schedule-on-the-tty.md) The keys, the white point, the gamma tables, the fades, `DumpState`, `NightLight`, and `ToggleNightLight()`. | |
| 138 | [**The sun schedule.**](../issues/138-night-light-sun-schedule.md) The sun's position, the curve, the time zone's coordinates, and the fixed times. | 137 |
| 139 | [**The location from geoclue.**](../issues/139-night-light-location-from-geoclue.md) The client, the stored location, and `optdepends`. | 138 |
| 140 | [**Pause until sunrise.**](../issues/140-night-light-pause-until-sunrise.md) | 138 |
| 141 | [**wlr-gamma-control.**](../issues/141-wlr-gamma-control.md) | 137 |
| 142 | [**The Night Light tab.**](../issues/142-night-light-tab-in-caminito-displays.md) The tab, `--night-light`, `--toggle-night-light`, and the desktop entry. | 137, 138, 140 |
| 143 | [**Night Light in a real Session, by hand.**](../issues/143-night-light-in-a-real-session.md) | 139, 141, 142 |

## Out of scope

- **True Tone:** a light sensor or a camera.
- **An inhibit API** for apps, such as Plasma's, and **a pause by the shortcut**.
- **Per-output temperatures.**
- **Plasma's sunset-to-sunrise and custom-times schedules.** A neutral noon temperature on the sun schedule acts like the former.
- **Caminito as the session's geoclue agent,** which would answer Flatpak apps' location requests.
- **Coordinates the user enters.**
- **A panel applet** for Night Light.
- **A shader for the nested run.**
- **Colour management, ICC profiles, CTM-based gamma** (niri's last resort for Asahi), and HDR.

## Further notes

- **The demo agent on Arch** hasn't been checked: whether `geoclue` ships it and its autostart file, and whether mate-session starts it. 139 finds out, and if it doesn't, the time-zone fallback carries a fresh Arch install.
- **Three things were settled while writing this spec,** not in the grilling. Each is easy to change:
  - the shortcut's command;
  - the fixed times' half-sine;
  - the `NightLight` property.
