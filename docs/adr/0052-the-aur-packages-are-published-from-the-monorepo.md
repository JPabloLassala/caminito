# 52. The AUR packages are published from the monorepo

Date: 2026-10-27
Status: Accepted. Qualifies ADR 0030, whose PKGBUILDs listed their patches by a path into `packaging/patches/`.

## Context

Caminito is made public on GitHub as `JPabloLassala/caminito`, and its Arch packages go to the AUR. An AUR package is a git repo of its own on `aur.archlinux.org`, holding only the PKGBUILD's directory: the PKGBUILD, its `.SRCINFO` and small local files. The monorepo holds Caminito, `caminito-vscode`, 13 PKGBUILDs and every carried patch.

Before this, `caminito-git` and `caminito-vscode-git` took their source from the local checkout (`git+file://`), and the patched packages took their patches from `file://…/../../patches/<component>/` (ADR 0030). Neither works in an AUR repo.

## Decision

- **The monorepo isn't split.** The AUR repos are publication targets only: `packaging/arch/<pkg>/` is the source of truth, and a script in `tools/` writes each one's `.SRCINFO` and pushes it to its AUR repo.
- **`caminito-git` and `caminito-vscode-git` clone the GitHub repo.** The patched packages clone their upstream, as before, and never the monorepo.
- **A patched PKGBUILD lists its patches by bare filename.** The series stays once at `packaging/patches/<component>/` (ADR 0030). `tools/bootstrap` copies it beside the PKGBUILD before `makepkg`, git-ignored, as `tools/debian-build` does for Debian, and the publish script copies it into the AUR repo, where it's committed.
- **All 13 packages are published, as `-git` packages** until releases are tagged. Each patched package's `pkgdesc` says it's the MATE component with Caminito's patches.

## Consequences

- The PKGBUILD that's tested is byte for byte the one on the AUR, and an AUR repo builds without the monorepo.
- A plain `makepkg` in `packaging/arch/<pkg>/` no longer finds the patches; the build goes through `tools/bootstrap`.
- `caminito-vscode-git` clones the whole monorepo (about 16 MB) for a small extension. Versioned packages can take a tag's tarball instead.
- Rejected:
  - one repo per package (sync work, with nothing gained for the AUR);
  - keeping the `file://` paths and rewriting them with `sed` on publish (the published PKGBUILD would differ from the tested one);
  - fetching the patches from the GitHub repo at a tag (the AUR package would break when the monorepo moves).
