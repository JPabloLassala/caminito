# Night Light in a real Session, by hand

Epic: night-light
Type: verification
Status: open
Milestone: none: after v0.1
Blocked by: 139, 141, 142
Map: [Night Light](../night-light/map.md)

## What to build

Nothing: a run of Night Light in a Session on the tty, by the developer with Claude reading the logs, to close the epic. Each item failed is a ticket of its own.

## Acceptance criteria

- [ ] Over an evening: the sun schedule warms the screen gradually with no visible step, and reaches the night temperature after dusk. A log-out and log-in applies it at once.
- [ ] The lock screen and caminito-locker are warm. After DPMS off and on, and after a suspend and resume, the temperature is right.
- [ ] A screenshot, and a screencast through the portal if one runs, are true to the colours.
- [ ] Two monitors get the same temperature. Plugging one in gives it the temperature at once.
- [ ] gammastep on one monitor leaves the other to Night Light, and quitting it fades Night Light back.
- [ ] Offline after a reboot, the stored location is used. With geoclue removed and the zone set to `Etc/UTC`, the tab shows the fixed times' note.
- [ ] A pause made at night lasts until morning across a log-out, and ends by itself.
- [ ] The shortcut and the Preferences entry work.
- [ ] Nothing in the log but the spec's once-only fallback lines.
