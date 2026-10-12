# 29. On Debian, a patched package keeps the stock names and is pinned

Date: 2026-10-11
Status: Accepted. Qualifies ADR 0011 for Debian and Ubuntu. Qualified on 2026-10-12 by ticket 196, and still to be confirmed by the developer: apt gives a `version` pin only to a version it finds in a source, and a package installed from a `.deb` file is in dpkg's status alone, at priority 100, so the pin held nothing, and an `apt upgrade` took a newer `mate-panel` from an Ubuntu-origin repository. Each pin file has a second stanza for the same binaries, `Pin: release o=Ubuntu` at priority 99, which keeps the installed package on an upgrade and still installs Ubuntu's when it's asked for by version. The first stanza stays for when the packages come from a repository.

## Context

ADR 0011 makes each patched package a `<pkgname>-caminito` package with `provides` and `conflicts`, so that pacman never swaps stock and patched on its own, and so that `caminito-git`, depending on the real names, is never satisfied by a stock package.

On Ubuntu 26.04, the first Debian target (`debian-packaging`), Debian splits each MATE component into several binary packages (`mate-panel`, `mate-panel-common`, `libmate-panel-applet-4-1`, `gir1.2-matepanelapplet-4.0`, …). Other packages depend on them by name, often with a version. Every patched baseline is newer than Ubuntu's stock version, which is 1.26, or 1.27.1 for mate-panel. So is marco 1.29.1, a backported package (CONTEXT.md).

## Decision

- **The stock names, a `caminito` version.** A patched or backported package keeps the source and binary names of Ubuntu's, and is versioned `<baseline>-0caminito<n>`. Because the baseline is newer, apt takes it as an upgrade, and the packages that depend on the stock names are satisfied.
- **Caminito depends on the baselines' versions** (`mate-panel (>= <baseline>)`), which no stock version reaches. This is ADR 0011's rule that a stock package never satisfies Caminito.
- **Each package pins its own binaries.** It installs `/etc/apt/preferences.d/caminito-<source package>`, which pins its binaries at `version *caminito*` with priority 1001. A newer MATE from Ubuntu then doesn't replace it on an upgrade, and removing the package removes its pin.

## Consequences

- The pins do on apt what ADR 0011's missing `replaces` does on pacman: nothing switches between stock and patched by itself.
- When Ubuntu's version passes a baseline, the pin still holds the patched package, and moving the baseline is a manual rebase, as on Arch.
- Rejected: `-caminito` names with `Provides`, `Conflicts` and `Replaces`, as on Arch. That needs a renamed twin of every split binary package, and a versioned `Provides` for each to satisfy versioned dependencies on the stock names.
