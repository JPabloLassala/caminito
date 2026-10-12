# The two bridges to org.mate.background

Epic: wallpaper
Type: implementation
Status: resolved
Blocked by: 210
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The settings" in [the spec](../wallpaper/spec.md): `org.mate.background` is no longer the store, and is bridged one way in each direction so that its readers and writers keep working.

- **Out:** the drawer writes the primary output's current picture to `picture-filename`, and its positioning to `picture-options`, whenever either changes. For a Colour it writes the colour and an empty filename.
- **In:** a write to `picture-filename` by anything else, such as caja's "Set as wallpaper" or the `caja-wallpaper` extension, makes that file the Picture of every output.
- The drawer's own writes must not come back in as someone else's.

## Acceptance criteria

- [x] Tests of the loop guard: the drawer's own write changes no output's settings, and another's write of the same file the primary already shows changes the other outputs.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: caja's "Set as wallpaper" on a picture sets every output, and `gsettings get org.mate.background picture-filename` follows a change made to the primary output.

## Done (2026-10-12)

- **The tests came first, at the bridge's own seam.** `caminito-wallpaper/src/bridge.rs` is pure, and `bridge/tests.rs` drives it against the store in memory that `settings/tests.rs` now shares: the drawer's own write heard back changes no output's settings, another's write of the file the primary already shows reaches the other output, a picture brought in keeps each monitor's positioning and reaches `all/`, an empty filename brings nothing in, and a failed write isn't taken for the next one's echo. Two ignored tests in `caminito/src/tests/caminito_wallpaperd.rs` run the real drawer: MATE's keys follow the primary output (a colour as `''` and its colour, then a tiled picture as `'wallpaper'`) while the second output keeps its own, and a `picture-filename` written as caja would is drawn on both. Breaking the guard on purpose fails the first.
- **The loop guard notes the drawer's own `picture-filename` before writing it** and passes over its change when heard, not a comparison of values: another's write of the same file must still count. GSettings signals a process's own write at once, and only once, on both dconf and the keyfile backend. Only the keys whose values differ are written, since a key written signals even with its old value, and `picture-filename` goes last so a reader hearing it finds the options and colour in place.
- **Out writes more than the filename:** `picture-options` from the positioning (Fill `zoom`, Fit `scaled`, Stretch `stretched`, Centre `centered`, Tile `wallpaper`, each read back as itself by `Positioning::from_mate`), and `primary-color` with `color-shading-type` `solid` for every kind, since MATE's default shading is a gradient and Caminito has none. A Colour leaves `picture-options` as it was. Out runs after every refresh, and a change of Caminito's `PrimaryOutput` now schedules one.
- **In** writes the file and kind Picture to `all/` and to every monitor in `outputs`, plugged in or not (`Set::Seen`), keeping each one's positioning and colour. An empty `picture-filename` isn't a picture, so it brings nothing in.
- **Not covered, on purpose:** the keyfile backend doesn't signal another's write that changes nothing, and dconf may drop one, so in a Session caja's "Set as wallpaper" on the file `picture-filename` already names may never be heard; the checklist doesn't ask for it. A write made while the drawer isn't running is overwritten by its first refresh, not brought in, since a change to Caminito's settings made meanwhile can't be told from it. A dconf write the service rejects later would come back as its old value and be brought in; nothing here handles that.
- **Run in an Ubuntu 24.04 cloud container.** The workspace's tests, caminito's 1833 and the five ignored drawer tests pass; the X11 tier needed `/tmp/.X11-unix` made first. clippy is clean on the crates this touches, and still fails on the base's three `manual_range_contains` in `caminito-theme`. The checklist item, under "The wallpaper", is the developer's, with 220.
