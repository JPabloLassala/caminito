# Ubuntu MATE's X11-only autostarts in mate-session's lists

Epic: debian-packaging
Type: research
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 191
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "Caminito and the patches" in [the spec](../debian-packaging/spec.md), and ADR 0014.

- List the `/etc/xdg/autostart` entries that `ubuntu-mate-desktop` and its recommends install on Ubuntu 26.04, from the archive's Contents. `mate-hud`, `plank` and the indicator applets are the likely ones.
- For each, decide by the ADR's rule: skipped under Caminito, moved to XWayland, or left alone because it works on Wayland. Record it in this ticket.
- Add the decisions to the series' `0003` (skipped) and `0004` (on XWayland) in `packaging/patches/mate-session-manager/`. They're gated on Caminito, as the rest is.
- Build `mate-session-manager-caminito` on Arch with the new series. An id Arch doesn't install changes nothing there.

## Acceptance criteria

- [ ] The list and each decision are in this ticket, with the package each entry comes from.
- [ ] The patched series applies, and its tests, if any, pass. The session runs on Arch with nothing changed, by the developer.
