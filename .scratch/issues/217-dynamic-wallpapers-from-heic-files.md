# Dynamic wallpapers from HEIC files

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 216
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The third Dynamic format in [the spec](../wallpaper/spec.md): Apple's HEIC files, whose pictures are keyed to the sun's position, or to clock times in the older variant.

- **The drawer** reads the file's pictures and its metadata, and shows the picture for the sun's elevation and azimuth Caminito publishes, cross-fading between neighbours.
- **`libheif` is an optional dependency** of `caminito-git`. Without it the build and the programs still work, and the dialog greys these files out with a line that says why.
- **The dialog's "Add…"** takes a `.heic`.

## Acceptance criteria

- [ ] Unit tests of the metadata's parsing and of the picture chosen for a sun position, from fixtures that are not Apple's pictures.
- [ ] The workspace builds and its tests pass with and without `libheif`.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, by the developer with a HEIC file of their own: it is on the right picture at noon and at night.
