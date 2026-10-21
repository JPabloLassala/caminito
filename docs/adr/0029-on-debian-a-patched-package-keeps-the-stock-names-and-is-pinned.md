# 29. On Debian, a patched package keeps the stock names and is pinned

Date: 2026-10-11
Status: Accepted. Qualifies ADR 0011 for Debian and Ubuntu. Qualified on 2026-10-12 by tickets 196 and 195, and still to be confirmed by the developer. The pin as first written, `version *caminito*`, held nothing: an installed `.deb` stayed at priority 100, and an `apt upgrade` took a newer `mate-panel` from an Ubuntu-origin repository. 196 took this for apt giving a `version` pin only to versions from a source, and added a second stanza for the same binaries, `Pin: release o=Ubuntu` at priority 99, which keeps the installed package on an upgrade and still installs Ubuntu's when it's asked for by version. 195 found the cause was the glob: apt matches no version to `*caminito*`, and the regex `version /caminito/` gives 1001 to a `.deb` installed from a file too. Each pin file has both stanzas, the first with the regex. Qualified on 2026-10-15 by CMN-170 and CMN-172: on Fedora, packaged only for the developer's machine, a patched package also keeps the stock names, at a `0.caminito<n>` Release, and `tools/bootstrap-fedora` holds it with `dnf versionlock`, not with a pin. Qualified on 2026-10-21 by CMN-125: vala-panel-appmenu's and appmenu-gtk-module's baseline is Ubuntu's own upstream version. There, `<baseline>-0caminito<n>` sorts below Ubuntu's (`25.04` < `25.04+dfsg`, `0caminito1` < `1build1`). So the two packages have epoch 1, `1:25.04-0caminito1`, which is above every stock version, and Caminito names that version.

## Context

ADR 0011 makes each patched package a `<pkgname>-caminito` package with `provides` and `conflicts`, so that pacman never swaps stock and patched on its own, and so that `caminito-git`, depending on the real names, is never satisfied by a stock package.

On Ubuntu 26.04, the first Debian target (`debian-packaging`), Debian splits each MATE component into several binary packages (`mate-panel`, `mate-panel-common`, `libmate-panel-applet-4-1`, `gir1.2-matepanelapplet-4.0`, …). Other packages depend on them by name, often with a version. Every patched baseline is newer than Ubuntu's stock version, which is 1.26, or 1.27.1 for mate-panel. So is marco 1.29.1, a backported package (CONTEXT.md).

## Decision

- **The stock names, a `caminito` version.** A patched or backported package keeps the source and binary names of Ubuntu's, and is versioned `<baseline>-0caminito<n>`. Because the baseline is newer, apt takes it as an upgrade, and the packages that depend on the stock names are satisfied.
- **Caminito depends on the baselines' versions** (`mate-panel (>= <baseline>)`), which no stock version reaches. This is ADR 0011's rule that a stock package never satisfies Caminito.
- **Each package pins its own binaries.** It installs `/etc/apt/preferences.d/caminito-<source package>`, which pins its binaries at `version /caminito/` with priority 1001, and Ubuntu's versions of them at 99. A newer MATE from Ubuntu then doesn't replace it on an upgrade. The pin is a conffile: purging the package removes it, and once the package is removed it matches no version apt can install, so it holds nothing back.

## Consequences

- The pins do on apt what ADR 0011's missing `replaces` does on pacman: nothing switches between stock and patched by itself.
- When Ubuntu's version passes a baseline, the pin still holds the patched package, and moving the baseline is a manual rebase, as on Arch.
- Rejected: `-caminito` names with `Provides`, `Conflicts` and `Replaces`, as on Arch. That needs a renamed twin of every split binary package, and a versioned `Provides` for each to satisfy versioned dependencies on the stock names.
- The pin was first written `version *caminito*`, which apt 3.2 on Ubuntu 26.04 matched to no version, though it matches `1.29*`. Ticket 195 found it and made it the regex, and 200 found it too, independently.
