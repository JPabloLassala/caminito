# The two bridges to org.mate.background

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 210
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The settings" in [the spec](../wallpaper/spec.md): `org.mate.background` is no longer the store, and is bridged one way in each direction so that its readers and writers keep working.

- **Out:** the drawer writes the primary output's current picture to `picture-filename`, and its positioning to `picture-options`, whenever either changes. For a Colour it writes the colour and an empty filename.
- **In:** a write to `picture-filename` by anything else, such as caja's "Set as wallpaper" or the `caja-wallpaper` extension, makes that file the Picture of every output.
- The drawer's own writes must not come back in as someone else's.

## Acceptance criteria

- [ ] Tests of the loop guard: the drawer's own write changes no output's settings, and another's write of the same file the primary already shows changes the other outputs.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: caja's "Set as wallpaper" on a picture sets every output, and `gsettings get org.mate.background picture-filename` follows a change made to the primary output.
