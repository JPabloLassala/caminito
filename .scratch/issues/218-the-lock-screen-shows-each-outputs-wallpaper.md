# The lock screen shows each output's wallpaper

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 210
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The lock screen" in [the spec](../wallpaper/spec.md). caminito-locker reads `org.mate.background` today, which now carries the primary output's picture alone.

- **The drawer writes a small state file** whenever an output's picture changes: per output, a picture the locker can show as it is, the positioning and the colour. For a dynamic wallpaper mid-fade it is the nearer of the two pictures.
- **caminito-locker reads the file** and shows each output's own wallpaper. Reading it never waits on the drawer, which may have crashed or hung.
- **With no file, or an output the file doesn't have,** the locker does as it does today.

## Acceptance criteria

- [ ] The file is written whole or not at all, and a test reads it while it is being replaced.
- [ ] caminito-locker's existing tests pass, with new ones for a file with two outputs, a missing file and a file naming a picture that is gone.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: with a different wallpaper per output, each lock screen shows its own, and with the drawer killed the lock still comes up.

## For the implementing agent to settle

Recorded in the Done section: where the file is and its format.
