# The README's Ubuntu section, the checklist step, and the first run in a VM

Epic: debian-packaging
Type: verification
Status: ready-for-human
Milestone: none: after v0.1
Blocked by: 205, 206
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Docs" in [the spec](../debian-packaging/spec.md).

- README: *Building on Ubuntu 26.04*, beside "Building". It covers `tools/debian-build` and `tools/bootstrap-debian`, installing the `.deb`s, the pins and how to drop them, marco 1.29.1, and mate-panel's `.so.1` left to Ubuntu.
- `docs/testing/checklist.md`: a step that installs the `.deb`s in an Ubuntu 26.04 VM and logs into *MATE on Caminito* from LightDM. It's run before each milestone closes.
- The first run of that step, by the developer.

## Acceptance criteria

- [ ] The README's section and the checklist step are in.
- [ ] By the developer, in an Ubuntu MATE 26.04 VM:
  - [ ] *MATE on Caminito* is listed by LightDM and logs in and out, with the panel, caja's desktop and msd.
  - [ ] The X11 MATE session still logs in.
  - [ ] Ubuntu's applets load in the panel.
  - [ ] mate-terminal's menu is in the global menu applet.
  - [ ] VS Code's `.deb` gets its menu in the panel.
- [ ] The spec's unconfirmed points are confirmed or ticketed: caja extensions, the 1.27.1 applets, and LightDM's `wayland-sessions`.
