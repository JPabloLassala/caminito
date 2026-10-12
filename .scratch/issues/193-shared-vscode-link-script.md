# The VS Code link script is shared, and knows VSCodium's .deb path

Epic: debian-packaging
Type: implementation
Status: resolved
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

- [x] The link tests cover all six directories, including VSCodium's `.deb` path, and pass from `tools/check`.
- [x] The hooks `makepkg` generates list the new target.
- [x] Called with no targets, the script links into every listed directory that exists and removes dangling links, and a test says so.

## Done (2026-10-12)

- **Test first:** `link.test.mts` moved to `../link/caminito-vscode-link` and gained VSCodium's `.deb` path, a test that links it, and three for a run with no targets. Four failed before the script changed.
- **"Removes dangling links" was read as an editor's leftover, not a link to a missing target.** On Debian, removing an editor leaves its directory behind holding only this link, since dpkg won't remove a directory that isn't empty, and then fires the trigger. A link to a missing extension can't happen while `postinst` runs. So `link` now links only into a directory holding something besides the link (an editor's built-ins), and removes this package's link from one that holds nothing else. A user's own link stays. The now-empty directory is left: removing it would be 204's call.
- **This changes Arch's `link` too:** an empty editor directory is no longer linked. The tests' `editors()` puts a built-in in each directory to match. Arch's hooks run after an editor is installed, so its directory is never empty then.
- **The PKGBUILD installs the script from `caminito-vscode/link/`** to `/usr/share/libalpm/scripts/`, and writes the hooks' targets from its list before moving into `packaging/arch/caminito-vscode-git` for the hook templates.
- **Not run:** `makepkg` and `tools/check` as a whole, since this container has no pacman or Meson and no Rust changed. The extension step of `tools/check` was run as written (`npm ci`, `typecheck`, `test`, `build`; 143 tests pass). `package()` was run by hand into a fake `pkgdir`, and both hooks it wrote list `usr/share/codium/resources/app/extensions/*`.
