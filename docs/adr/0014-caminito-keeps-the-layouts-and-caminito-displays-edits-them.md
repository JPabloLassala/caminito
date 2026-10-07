# 14. Caminito keeps the layouts, and caminito-displays edits them

Date: 2026-10-05
Status: Accepted

## Context

Until now msd's wlrandr plugin kept the user's layout in `monitors-wayland.xml` and applied it over wlr-output-management at login, and mate-control-center's Wayland Display capplet edited it, with a carried patch for 5% scale steps (ADR 0008). That has three costs. The session shows Caminito's default layout until msd starts. One file holds one layout, so a laptop doesn't remember its desk and its projector apart. And nothing has a primary output: wlr-output-management doesn't carry one, so the panel, new windows and X11 apps have no agreed output to prefer.

The capplet's design is also not the one wanted. The aim is Plasma's page: the outputs arranged on a canvas, with the selected one's settings below.

## Decision

- **Caminito saves and restores layouts itself.** A saved layout is kept per set of connected monitors, each known by its EDID make, model and serial, or by its connector when it has none. Caminito restores it at startup and on hot-plug, and when a configuration enables a disabled monitor and asks no mode, place or scale of it, as `wlr-randr --on` does (ticket 90). msd's `monitors-wayland.xml` isn't imported.
- **Caminito owns the primary output** and offers it on `org.mate.Caminito` (ADR 0002's third channel). It decides where the panel and caja's desktop go, where new windows open, and the RandR primary under XWayland. (Ticket 81: it is the `PrimaryOutput` property, an output's name, and mate-panel and caja follow it through carried patches.)
- **A layout is saved only when asked over D-Bus**, not whenever a configuration is applied. A configuration from `wlr-randr` lasts for the Session, or until the saved layout is next restored.
- **Every layout is contiguous.** Caminito heals a layout on hot-plug and when a saved one no longer fits.
- **caminito-displays replaces the Display capplet.** It is a Rust program in this repo, in GTK4 without libadwaita. It applies configurations over wlr-output-management, and uses `org.mate.Caminito` for the primary output and for saving, after the user keeps a change.
- **caminito-displays offers scales from 100% to 300% in 25% steps**, plus the current one when it is off that grid. Caminito's default rule (ADR 0008) is unchanged.
- **msd's wlrandr plugin is off under Caminito**, by a carried patch, and the carried mate-control-center patch for 5% steps is dropped.

## Consequences

- The first frame of a Session already has the user's layout.
- A layout that blacks out a monitor is never on disk before the user has kept it.
- Caminito gains a D-Bus interface for layouts, a file format of its own, and the healing of layouts.
- ADR 0008's third point, the carried mate-control-center patch, is superseded. Its "msd's saved layout overrides it" now reads "Caminito's saved layout".
- The code's "primary output", the output a surface is mostly on, is renamed main output.
- GTK4 follows MATE's GTK theme less closely than a GTK3 program would.
- Rejected:
  - caminito-displays writing msd's file, with msd still applying it (the late layout, one file, and nowhere for the primary output);
  - GTK3 in Rust (its bindings are archived);
  - a larger carried patch on the capplet, in C (the Plasma design fights the capplet's layout);
  - saving on every applied configuration (a bad layout is on disk during the countdown);
  - rounding Caminito's default scale to 25% (ADR 0008's reasons for KWin's rule stand).
