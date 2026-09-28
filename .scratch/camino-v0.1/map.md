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
- [Metacity/Marco theme format](issues/04-metacity-theme-format.md): Marco's format goes up to v3.5 and themes need a lenient parser; colours depend on the GTK3 theme's CSS; feature tiers 0–3 sized from 87 real themes; no non-C renderer exists.
- [v0.1 Marco feature cut](issues/05-v01-marco-feature-cut.md): full Marco parity, meaning every key and action except the X-only and droppable ones; all Marco-to-MATE integrations (Alt+F1/F2, pager layout, shared Show Desktop, both mate-control-center dialogs, window screenshots) are required, via MATE patches carried downstream; Camino identifies itself as Camino.
- [v0.1 protocol and integration set](issues/06-v01-protocol-and-integration-set.md): Smithay's protocols plus four built ones (wlr-foreign-toplevel, read-only ext-workspace, wlr-output-management, a minimal gtk_shell1). MATE integration goes through a standard protocol, else an environment variable, else D-Bus on `org.mate.Camino` ([ADR 0002](../../docs/adr/0002-mate-integration-channels.md)), with carried patches to mate-panel, mcc, msd and mate-screenshot. Camino owns input settings, idle and the `org.mate.ScreenSaver` shim; msd keeps media keys (grabbing through Camino) and display persistence.

## Not yet specified

- **Screen locking.** 06 settled that Camino owns idle and an `org.mate.ScreenSaver` shim that spawns an ext-session-lock locker. Which locker, and its look, is still open.
- **XWayland integration details.** How X11 windows get Marco behaviour through Smithay's X11 window manager, and clipboard and DnD edge cases. msd already bridges XSettings. 06 decided that Camino keeps its own X connection (for XKB bells and struts), which is the natural place for the virtual-desktop, work-area and frame-extents properties Smithay lacks.
- **Input configuration.** 06 settled the owners: Camino applies the peripherals and xkb gsettings, msd keeps media keys through Camino's `GrabAccelerators`, and the capplets run under XWayland. The key-by-key mapping is for 09.
- **Crate and module architecture.** How Camino's code is split up. It depends on the Smithay audit and on the feature and protocol cuts.
- **Development and testing loop.** Running nested (winit backend) vs. on a TTY, headless tests, and CI.
- **HiDPI and fractional scaling, and accessibility** (screen reader, magnifier, and the AccessX keys and typing break that 06 deferred). Both are in scope eventually; how much goes in v0.1 is unknown.
- **Milestone slicing.** Ordering v0.1 into milestones once the feature, protocol and decoration cuts have landed.
- **Packaging.** How "MATE on Camino" gets installed (session file, Arch/AUR first?).

## Out of scope

- Tiling-WM features beyond Marco's own edge snapping.
- Showy effects (wobbly windows and the like).
- Non-Linux platforms.
- Forking or patching GTK.
- Upstreaming Camino to the MATE project. That would be a separate effort after v0.1.
- Integrating into or replacing the Wayfire-based MATE Wayland session.
