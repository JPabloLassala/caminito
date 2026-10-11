# The VS Code link script is shared, and knows VSCodium's .deb path

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" (caminito-vscode) in [the spec](../debian-packaging/spec.md).

- Move `packaging/arch/caminito-vscode-git/caminito-vscode-link` to a place both distributions build from, `caminito-vscode/link/caminito-vscode-link`. The PKGBUILD and `caminito-vscode/test/link.test.mts` follow it.
- The script still installs to `/usr/share/libalpm/scripts/` on Arch, as the hooks name it.
- Its list of directories gains `/usr/share/codium/resources/app/extensions`, VSCodium's `.deb` path. The hooks' targets come from that list, so they follow.
- Taking pacman's targets on stdin stays Arch's. The Debian package (204) calls it with no targets, to link into every directory that exists.

## Acceptance criteria

- [ ] The link tests cover all six directories, including VSCodium's `.deb` path, and pass from `tools/check`.
- [ ] The hooks `makepkg` generates list the new target.
- [ ] Called with no targets, the script links into every listed directory that exists and removes dangling links, and a test says so.
