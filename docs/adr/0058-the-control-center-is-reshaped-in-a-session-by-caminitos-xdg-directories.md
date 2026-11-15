# 58. The Control Center is reshaped in a Session by Caminito's XDG directories, not by a patch

Date: 2026-11-15
Status: Accepted

## Context

Many of the Control Center's entries are MATE's X11 capplets. Some are replaced by Caminito programs (caminito-displays, caminito-notifications, caminito-locker), and some don't fully do their job in a Session and have no replacement yet (the Unported entries). The user wants the replaced ones gone and the Unported ones gathered in a category of their own, "Not yet ported", as a list of what is left to port. X11 MATE and the Wayfire session must still see the stock Control Center.

A Session already puts `/usr/share/caminito/xdg` first in `XDG_DATA_DIRS`, so a desktop entry there with a stock entry's id shadows it in a Session only. That's how caminito-displays replaces the Display capplet. `matecc.menu` reads `<DefaultMergeDirs/>`, which means any `menus/matecc-merged/*.menu` under `XDG_CONFIG_DIRS`.

## Decision

Everything is done through Caminito's XDG directories, with no carried patch to mate-control-center:

- **Replaced entry:** a same-id copy with `NoDisplay=true`, shipped with the replacement and not before it.
- **Unported entry:** a same-id copy whose `Categories=` is `X-Caminito-Unported` alone. It drops `Settings`, so it leaves the panel's Preferences menu too.
- **The category:** a Session also puts a Caminito config directory first in `XDG_CONFIG_DIRS`, and that directory holds a `menus/matecc-merged/` file that adds "Not yet ported", taking the entries with `X-Caminito-Unported`.
- **The Applications menu:** without `Settings`, an Unported entry would fall into its Other, which takes every unallocated entry that isn't `Settings`, `Core` or `Screensaver`. A merged file's `<Exclude>` can't stop that, because it comes where `<DefaultMergeDirs/>` is, before Other's `<Include>`. So `menus/mate-applications-merged/` holds a `<Deleted/>` menu that takes them: allocated, they are passed over by Other, and libmate-menu shows neither the menu nor them.
- **Caminito's own programs** go into MATE's categories by subject.
- **Entries from other desktops** (KDE's, Manjaro's, duplicates such as pavucontrol) aren't touched: hiding other distributions' packages is the user's choice, made in `~/.local/share/applications`.

The alternative was a carried patch to `matecc.menu` (ADR 0011), which would need gating on Caminito and keeping for every distribution (ADR 0030). The XDG directories are scoped to a Session by construction.

## Consequences

- A Session now sets `XDG_CONFIG_DIRS`, which reaches every program it starts. Only `matecc-merged` is placed there, but other `menus/` or autostart directories under it would take effect too. ADR 0014's autostart skips still go through mate-session's patch.
- Porting an entry means deleting its copy in Caminito's directory. Replacing it means changing the copy to `NoDisplay=true`.
- A copy pins the stock entry's other fields (`Name`, `Exec`, `Icon`) to the version it was copied from, so an upstream rename has to be followed by hand.
