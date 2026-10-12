# Appearance hides its Background tab under Caminito

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The Control Center" in [the spec](../wallpaper/spec.md): with mate-settings-daemon's background plugin off, the Background tab of MATE's Appearance capplet has no effect in a Session.

A carried patch to mate-control-center, kept in `packaging/patches/mate-control-center` (ADR 0030) and gated on Caminito as the others are, that leaves the Background tab out. Anything that opens Appearance at that tab opens the Wallpaper dialog instead, if a way to do that is found in the capplet; otherwise it opens Appearance's first tab.

## Acceptance criteria

- [ ] The patched packages build with the patch in their series, on Arch and on Debian.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: in a Session Appearance has no Background tab, and caja's desktop menu "Change Desktop Background" opens something sensible; in the X11 session with Marco the tab is there and works (the gate).
