# Map: Camino v0.1

Label: wayfinder:map

## Destination

An architecture spec and milestone plan for **Camino v0.1**, a Rust/Smithay Wayland compositor that runs a usable MATE session with Marco's look and behaviour. The foundation, the Marco features in v0.1, the Wayland protocols and MATE integration, the decoration approach and the milestones are all decided, so implementation can start the next day.

## Notes

- **Domain:** the vocabulary is in [CONTEXT.md](../../CONTEXT.md). Camino is a *compositor*, never a "window manager".
- **Skills:** every grilling session calls `grilling` and `domain-modeling`, and updates CONTEXT.md and `docs/adr/` inline.
- **Plan, don't do:** tickets produce decisions, not code. A prototype ticket may produce throwaway code as an asset.
- **Tracker:** local markdown. Tickets are in `issues/`, blocking is the `Blocked by:` line, and research findings go on `research/<name>` branches.
- **Settled while charting** (these are fixed; don't reopen them in tickets):
  - Rust on Smithay: [ADR 0001](../../docs/adr/0001-rust-and-smithay.md).
  - XWayland is required in v0.1.
  - Server-side decorations are drawn from Marco/Metacity themes. The level of theme-format support is still open.
  - Configuration is read from the existing `org.mate.Marco.*` gsettings, and Wayland-only keys go in a new `org.mate.Camino` schema.
  - v0.1 session scope: mate-panel (including the window list and workspace switcher applets), mate-session running in the session (who launches whom is for the Session launch model ticket; the MATE Wayland audit shows mate-session can't launch a compositor), mate-settings-daemon, and caja's desktop and background.
  - It ships as its own session entry, "MATE on Camino", separate from the Wayfire-based MATE Wayland session.
  - Personal project, GPL-2.0-or-later, following MATE conventions where that's cheap.

## Decisions so far

<!-- one line per closed ticket -->

- [Smithay capability audit](issues/03-smithay-capability-audit.md): pin Smithay master; it covers the core, but Camino writes foreign-toplevel, ext-workspace and output-management plus its own scene graph and window-management policy.
- [Marco feature inventory](issues/01-marco-feature-inventory.md): 194 gsettings keys and 103 keybinding actions; the core behaviour carries over as compositor-internal, and what breaks is Marco's X11 conversations with mate-panel, the pager and mate-control-center.
- [MATE components on Wayland audit](issues/02-mate-wayland-audit.md): MATE needs only wlr/ext protocols (layer-shell, foreign-toplevel, ext-workspace, output-management, data-control), mostly from 2026 git rather than 1.28. mate-session isn't ported and must run as a child of Camino, and msd leaves input settings and media keys to the compositor.

## Not yet specified

- **Replacing Marco's X11 conversations with MATE programs.** Alt+F1/F2 (`_MATE_PANEL_ACTION`), the pager's grid layout (`_NET_DESKTOP_LAYOUT`), Show Desktop (`_NET_SHOWING_DESKTOP`), and mate-control-center's WM detection and keybinding dialog. Each one needs a Wayland-side channel (D-Bus? a protocol? a patch to the MATE component?). This probably sharpens once the v0.1 protocol and integration set is decided.

- **Screen locking.** mate-screensaver is X11-only (confirmed by the MATE audit), and mate-session's idle detection is X11-only too. The likely route is ext-session-lock plus some locker, but it depends on what the MATE Wayland audit finds.
- **Display configuration.** Per the MATE audit, msd already has a wlr-output-management backend and a Wayland display capplet exists in git, so the question is mostly whether Camino or msd owns and persists the output layout.
- **XWayland integration details.** How X11 windows get Marco behaviour through Smithay's X11 window manager, XSettings bridging from mate-settings-daemon, and clipboard and DnD edge cases. Smithay's X11 window manager lacks the virtual-desktop, work-area and frame-extents properties (see the Smithay capability audit).
- **Input configuration and media keys.** msd turns off its mouse, keyboard, keybindings and media-keys plugins on Wayland, so Camino must map MATE's mouse, touchpad and keyboard gsettings to libinput and xkb, and handle media keys. The X11-only Mouse and Keyboard capplets are a related gap.
- **Crate and module architecture.** How Camino's code is split up. It depends on the Smithay audit and on the feature and protocol cuts.
- **Development and testing loop.** Running nested (winit backend) vs. on a TTY, headless tests, and CI.
- **HiDPI and fractional scaling, and accessibility** (screen reader, magnifier). Both are in scope eventually; how much goes in v0.1 is unknown.
- **Milestone slicing.** Ordering v0.1 into milestones once the feature, protocol and decoration cuts have landed.
- **Packaging.** How "MATE on Camino" gets installed (session file, Arch/AUR first?).

## Out of scope

- Tiling-WM features beyond Marco's own edge snapping.
- Showy effects (wobbly windows and the like).
- Non-Linux platforms.
- Forking or patching GTK.
- Upstreaming Camino to the MATE project. That would be a separate effort after v0.1.
- Integrating into or replacing the Wayfire-based MATE Wayland session.
