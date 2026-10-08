# caminito-vscode-git installs the global menu extension

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [116](116-the-extension-serves-one-menu-and-claims.md)

## What to build

[ADR 0022](../../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md), under "Packaging" and "Documentation" in [the spec](../../vscode-global-menu/spec.md). A user who installs `caminito-vscode-git` has the global menu extension as a built-in of every editor of the five that's installed, in every profile, and pacman updates and removes it.

- **The package:** a PKGBUILD of its own that builds the extension with node and the bundler and installs it unpacked at `/usr/share/caminito-vscode/global-menu`. No `.vsix`.
- **One script and one list of the five built-in directories.** In each that exists the script makes or removes the link `caminito.global-menu`.
- **An alpm hook** links after an install or upgrade that touches one of the five directories and unlinks before a removal that does. **The install script** does the same for the package's own install, upgrade and removal.
- **`caminito-git`** gains the `optdepends` line. **The packaging workflow** builds the package.
- **The README:** VS Code's paragraph rewritten, the three settings, the line for linking an editor the hook doesn't reach, and what the extension doesn't do, each as the spec lists it.

## Acceptance criteria

- [ ] The script, run against directories under a temporary root, links only where a directory exists, is harmless when run twice, and removes only its own link.
- [ ] `namcap` passes, and the packaging workflow builds the package.
- [ ] Installing the package with `visual-studio-code-bin` installed puts the link in its built-in directory, and VS Code loads the extension.
- [ ] Installing an editor after the package links it, and removing an editor or the package leaves no link and no directory behind.
- [ ] The README says what a user reads on screen, and its XWayland route stays as the way without the package.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] A link in an editor's real built-in directory, for each of the five that can be installed.
- [ ] Whether a user can disable a built-in extension. The README then says so, or says there's no way.
- [ ] What an open window shows after an update changes the files under it.

## Notes

- The README is written for the menu as it will be. A later ticket that changes what a user sees corrects it.
