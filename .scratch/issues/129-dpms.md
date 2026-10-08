# Powering the outputs off when idle

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 128
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

After the idle time of mate-power-manager's keys, the outputs are powered off, and any activity powers them on ([09](09-gsettings-mapping.md), [17](17-milestone-slicing.md)'s exit: idle blanks, then powers the outputs off).

- **The keys:** `org.mate.power-manager sleep-display-ac` and `sleep-display-battery` (seconds, 0 for never), picked by UPower's `OnBattery`. `sleep-display-ups` is never read.
- **On the tty** the outputs' DPMS is set through DRM. Nested, powering off is the blank of [128](128-idle-and-blanking.md) and nothing more.
- **Powering on** restores each output's mode and its frame, with no layout change: clients, the panel and msd see no output go or come.
- **While locked** the timers run without inhibitors ([12](12-screen-locking.md)).

## Acceptance criteria

- [ ] Tests with a driven clock, written before the code: the outputs are off at the key's time, the key is chosen by `OnBattery` from a stand-in UPower, 0 never powers off, and an inhibitor holds it off.
- [ ] No client sees an output removed or added across a power-off and a power-on.
- [ ] The checklist has an item, tagged M7, and it was run on the tty with the developer's monitors: both go to standby, a key lights both in their saved layout, and a playing video keeps them on. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **mate-power-manager's own DPMS** goes to XWayland and does nothing (09). If it gets in the way under Caminito, say so here: `mate-power-manager-caminito` exists since [107](107-brightness-and-power-keys.md).
- **[87](87-hot-plug-blanks-the-other-monitor.md)'s AOC** goes black when the other monitor is turned off or on. Whether a power-on does the same is seen in the check.
