# The Power popup switches the power profile

Epic: power-applet
Type: implementation
Status: ready-for-agent
Blocked by: 262
Spec: [Power applet](../power-applet/spec.md)

## What to build

The profile tiles under "The popup" and power-profiles-daemon under "Its sources" in [the spec](../power-applet/spec.md), as in the prototype's "Popup C".

- **A tile for each profile the daemon lists,** two or three, never more. Each has an icon and a name (Power Saver, Balanced, Performance), and the active one is pressed. A profile not listed isn't drawn. A click sets `ActiveProfile`.
- **The bus name** is `org.freedesktop.UPower.PowerProfiles`, falling back to `net.hadess.PowerProfiles`. With neither, the section is hidden.
- **A dimmed line under the tiles:**
  - Why Performance is degraded, from `PerformanceDegraded` (`high-operating-temperature`, `lap-detected`), in the warning colour.
  - Who holds a profile, from `ActiveProfileHolds`.
- The tiles follow changes made elsewhere (`powerprofilesctl`, a hotkey).

## Acceptance criteria

- [ ] Check-script screenshots against dbusmock's `power_profiles_daemon` template: three profiles, two (no Performance), degraded, held, and no daemon.
- [ ] A click changes the mock's `ActiveProfile`, and an outside change moves the pressed tile.
