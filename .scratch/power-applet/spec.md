# Spec: Power applet

Written on 2026-10-14 from a grilling session with the developer and a prototype. The applet's shape follows [ADR 0035](../../docs/adr/0035-the-bluetooth-popup-is-an-in-process-mate-panel-applet.md). Where this file and the ADR differ, the ADR is right and this file is to be fixed.

Nothing is built yet. Its tickets are 262 to 266, under "Tickets".

## Problem

In a Session nothing shows the battery. mate-power-manager's tray icon is an XEmbed icon on XWayland, and mate-panel on Wayland doesn't host those. Nothing lets the user change the power profile, see a mouse's battery, limit charging, or keep the computer awake through a presentation.

## Solution

The **Power applet** (CONTEXT.md): a battery icon in mate-panel whose popup has:

- The power profile.
- Every battery: the laptop's and the peripherals'.
- **Stay awake** (CONTEXT.md), and the applications keeping the computer awake.

It is Plasma's Power and Battery, laid out as the prototype's "Popup C". Its settings window is mate-power-preferences, which already exists.

## User stories

1. As a laptop user, I want to see the charge in the panel, and in its tooltip how long is left or until full.
2. As a user, I want to switch between Power Saver, Balanced and Performance in one click.
3. As a user, I want to know why Performance isn't giving me performance (the laptop is hot, or on my lap).
4. As a user with two batteries, I want to see each one's charge, state and health.
5. As a user who keeps the laptop plugged in, I want to limit charging to protect the battery, from the panel, since MATE has nowhere else for it.
6. As a user, I want to see my mouse's and headphones' batteries, with a low one stand out.
7. As a presenter, I want to keep the computer from sleeping and locking until I say otherwise.
8. As a user whose laptop won't sleep, I want to see which application is keeping it awake.
9. As a desktop user with no battery, I still want the profile and Stay awake.
10. As a user on X11 MATE, I want the same thing, and only one battery icon.

## Implementation decisions

### The applet

- An in-process mate-panel applet in C and GTK3, in `helpers/caminito-power-applet/`, as a Meson subproject of the root [meson.build](../../meson.build). It's called "Power" in Add to Panel. It shares no code with the Bluetooth applet: whatever both need is copied.
- It reads its sources directly over GDBus. There's no Rust half and no private interface.
- Neither the applet nor its files depend on Caminito. They go in the normal XDG directories.

### Its sources

- **UPower** (`org.freedesktop.UPower`):
  - The display device for the icon and the tooltip.
  - The devices of type Battery with `PowerSupply` true, for the laptop's batteries.
  - The other devices that report a percentage, for peripherals: mouse, keyboard, headset, gaming input and the like.
  - The charge limit is `ChargeThresholdSupported`, `ChargeThresholdEnabled` and `ChargeEndThreshold`, set through `EnableChargeThreshold`, which may ask polkit.
- **power-profiles-daemon:**
  - It's `org.freedesktop.UPower.PowerProfiles`, falling back to `net.hadess.PowerProfiles`. `tuned-ppd` serves the same names.
  - Its properties are `Profiles`, `ActiveProfile`, `PerformanceDegraded` and `ActiveProfileHolds`.
  - It has two or three profiles, never more: `power-saver` and `balanced` always, and `performance` only when a driver provides it. Firmware's other platform profiles never reach it.
- **mate-session** (`org.gnome.SessionManager`): `Inhibit` and `Uninhibit` for Stay awake, and `GetInhibitors` with each inhibitor's app ID, reason and flags, for the list.

### The icon

- A battery glyph for the display device's percentage, with a charging variant. With no battery, a plug.
- Icon names come from a themed icon with fallbacks, because themes disagree: Breeze uses other names than MATE and Adwaita. Ticket 259 has the same problem.
- The percentage beside the glyph is an applet preference, off by default.
- **The tooltip** is the percentage and the time: "3 h 40 min left", "Full in 50 min" or "Plugged in".
- Left-click toggles the popup. Right-click shows the applet's Preferences and "Configure Power Management…".

### The popup

The developer chose the prototype's "Popup C", on the `prototype/power-applet-ui` branch (`--variant 3`). Redo it in GTK3, keeping its layout. From the top:

- **Profile tiles:**
  - A row of toggle tiles, one for each profile the daemon lists (two or three), each with an icon and its name: Power Saver, Balanced and Performance. The active one is pressed. A tile that isn't listed isn't drawn, not even greyed out.
  - Under the row, a dimmed line when there's something to say: why Performance is degraded (`high-operating-temperature`, `lap-detected`), in a warning colour, and which application holds a profile ("GameMode is holding Performance").
  - The section is hidden when there's no daemon.
- **Stay awake:** a row with a lock icon, "Stay awake" with "Don't sleep or lock the screen" under it, and a switch.
- **The batteries,** as rows of one table: icon, then name with state under it, then a bar, then the percentage.
  - The laptop's batteries come first. Their state is "Charging", "Discharging", "Not charging" or "Fully charged", with "· limit 80%" added when the limit is on.
  - Each laptop battery has a ⋮ button. Its popover holds "Health 58%" and, only where `ChargeThresholdSupported`, a switch "Limit charging to 80%" (the threshold is UPower's `ChargeEndThreshold`).
  - Then the peripherals, with their icon from their UPower type. They have no ⋮.
  - A percentage below 20% is in the warning colour.
- **Keeping the computer awake:** below a separator, one line for each mate-session inhibitor with the suspend or idle flag, with the application's icon and name and its reason. Stay awake shows here as its own line while it's on. The section is hidden when there are none.
- **"Configure Power Management…"** at the bottom runs `mate-power-preferences`.
- With no battery at all, the table is left out, or holds only the peripherals.

### Stay awake

- On: the applet calls mate-session's `Inhibit` with app ID `caminito-power-applet`, the reason "Stay awake is on", and the flags for suspend (4) and idle (8). It keeps the cookie.
- Off: `Uninhibit` with the cookie.
- The inhibitor dies with the applet's process, so if the panel restarts, the switch is off again.
- Caminito's idle and mate-power-manager already respect mate-session's inhibitors (128), so nothing else changes.

### mate-power-manager's own icon

- A gsettings override in the applet's package sets `org.mate.power-manager icon-policy` to `never`, so X11 MATE shows only one battery icon. The user can turn mpm's icon back on in mate-power-preferences.

### Not in v1

- Plasma's lid note. The lid is mpm's, and Caminito has no external-monitor rule.
- Screen and keyboard brightness. They belong to another applet.
- Notifications. mpm already sends low battery ones.
- Translations, which no Caminito program has.

## Testing

- Check scripts in the tickets' check directories run mate-panel with the applet against python-dbusmock's `upower` and `power_profiles_daemon` templates, and a small mock of mate-session's inhibitor calls. They take screenshots for the developer.
- They cover:
  - One battery, two batteries, and none.
  - Two and three profiles, and degraded.
  - The charge limit supported and not.
  - A low peripheral.
  - Stay awake on and off.
  - An application's inhibitor.
- [checklist.md](../../docs/testing/checklist.md) gets items for real hardware: unplug and replug, a profile change visible in `powerprofilesctl`, the charge limit, and Stay awake keeping a Session from blanking past `idle-delay`. These are the developer's to run.

## Not decided

Nothing.

## Tickets

Five slices, filed in `.scratch/issues/` as tickets 262 to 266.

| | Slice | Blocked by |
| --- | --- | --- |
| 262 | [**The Power applet's icon shows the battery.**](../issues/262-the-power-applets-icon-shows-the-battery.md) | None |
| 263 | [**The popup lists every battery.**](../issues/263-the-power-popup-lists-every-battery.md) | 262 |
| 264 | [**The popup switches the power profile.**](../issues/264-the-power-popup-switches-the-power-profile.md) | 262 |
| 265 | [**Stay awake, and what keeps the computer awake.**](../issues/265-stay-awake-and-what-keeps-the-computer-awake.md) | 262 |
| 266 | [**The Power applet in the default panel, and the packaging.**](../issues/266-the-power-applet-in-the-default-panel-and-the-packaging.md) | 261, 263, 264, 265 |
