# caminito-vscode, linked by dpkg triggers

Epic: debian-packaging
Type: implementation
Status: ready-for-agent
Milestone: none: after v0.1
Blocked by: 193, 194
Map: [Debian packaging](../debian-packaging/map.md)

## What to build

Under "The packages" in [the spec](../debian-packaging/spec.md), and ADR 0022's qualification.

- `packaging/debian/caminito-vscode/debian/`. It builds the extension with `npm ci` and `npm run build`, runs `npm test`, and installs the manifest and bundle to `/usr/share/caminito-vscode/global-menu` and 193's link script to `/usr/share/caminito-vscode/`.
- `debian/triggers`: `interest-noawait` on each directory in the script's list.
- `postinst` runs the script on `configure` and `triggered`. `prerm` unlinks on `remove`.
- `Architecture: all`. The version is the same scheme as `caminito`'s.

## Acceptance criteria

- [ ] It builds and tests in `tools/debian-build`.
- [ ] In the container, with Microsoft's `code` `.deb` installed after it, the extension's link is in `/usr/share/code/resources/app/extensions`, and it's gone after `apt remove caminito-vscode`.
- [ ] Upgrading `code` keeps the link.
