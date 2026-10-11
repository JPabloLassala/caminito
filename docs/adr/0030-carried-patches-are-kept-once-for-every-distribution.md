# 30. Carried patches are kept once, for every distribution

Date: 2026-10-11
Status: Accepted. Qualifies ADR 0011, whose patches lived beside each PKGBUILD.

## Context

ADR 0011 keeps each patched package's series as `git format-patch` files beside its PKGBUILD, as the only copy. The `debian-packaging` epic builds the same patched components as Debian source packages for Ubuntu 26.04, which take their patches from `debian/patches/` with a `series` file. The carried patches and upstream fixes are Caminito's, not a distribution's. A build patch is a distribution's: it lets a baseline build against that distribution's libraries, and one distribution can need it where another doesn't.

## Decision

- Each component's series lives once, at `packaging/patches/<component>/`, in the order it applies.
- A build patch for one distribution lives in `packaging/patches/<component>/arch/` or `…/debian/`, and applies after the shared series.
- The PKGBUILDs list their patches from there. A Debian source package's `debian/patches/` and `series` are copied from there when it's built, and aren't committed.

## Consequences

- A baseline move is one rebase, exported once, for both distributions.
- Rejected:
  - a copy per distribution, which doubles every rebase and lets the two drift;
  - Debian referencing `packaging/arch/`, which leaves Arch's build patches in Debian's series.
