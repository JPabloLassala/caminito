# Stay awake, and what keeps the computer awake

Epic: power-applet
Type: implementation
Status: ready-for-agent
Blocked by: 262
Spec: [Power applet](../power-applet/spec.md)

## What to build

**Stay awake** (CONTEXT.md) and the list of applications keeping the computer awake, under "Stay awake" and "The popup" in [the spec](../power-applet/spec.md), as in the prototype's "Popup C".

- **A row under the profile tiles:** a lock icon, "Stay awake" with "Don't sleep or lock the screen" below it, and a switch.
- **On** calls mate-session's `Inhibit("caminito-power-applet", 0, "Stay awake is on", 4 | 8)` and keeps the cookie. **Off** calls `Uninhibit`. The switch shows off again after a panel restart.
- **Below a separator:** one line for each mate-session inhibitor with the suspend or idle flag, with the application's icon, name and reason. Stay awake is one of them while it's on. Hidden when there are none. It follows `InhibitorAdded` and `InhibitorRemoved`.

## Acceptance criteria

- [ ] Check-script screenshots against a mock mate-session: Stay awake off, on, and an application's inhibitor listed.
- [ ] The mock sees `Inhibit` with flags 12 and then `Uninhibit` with the same cookie.
- [ ] A checklist item: in a real Session, with Stay awake on, the screen doesn't blank past `idle-delay` (Caminito's idle, 128).
