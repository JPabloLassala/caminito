# How the package installs the extension

Type: grilling
Status: resolved
Blocked by:
Map: [VS Code's global menu](../map.md)

## Question

The research ([05](05-installing-an-extension-from-a-package.md)) found one system-wide place, each editor's built-in extensions directory, and one way for a user, `--install-extension`.

- The built-in directory, or an install for each user at session start? The first changes the map's settled "installed from a `.vsix`" to "installed unpacked", and writes into other packages' trees.
- If the built-in directory: symlinks the package owns (and directories for editors that aren't installed), an alpm hook that links and unlinks as editors come and go, or a split package for each editor.
- Which editors' paths the packaging carries, and whether a `.vsix` also ships for an editor it doesn't reach (a Flatpak, a tarball).
- The package's name, and whether `caminito` lists it as an optional dependency.

## Answer

Decided on 2026-10-08 and recorded as [ADR 0022](../../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md). The developer took the recommended answer to all six questions, in two rounds. Nothing was run, and no glossary term changed.

- **The built-in directory.** The extension is installed unpacked, once, in a directory of the package's own, and each editor's built-in extensions directory gets a symlink to it. This changes the map's settled "installed from a `.vsix`", which the developer agreed to. From [10](10-thin-end-to-end-slice.md): only a built-in reaches the agents window, which has no menubar of its own.
- **An alpm hook makes the links,** where an editor's directory exists, when an editor is installed or upgraded, and unlinks before an editor is removed. The package's install script does the same for the package's own install, upgrade and removal. The links are files no package owns.
- **Five paths,** in one list: those of `visual-studio-code-bin`, `code`, `vscodium-bin`, `vscodium` and `cursor-bin`, as [05](05-installing-an-extension-from-a-package.md) found them. None has been run: ticket 10 ran VS Code 1.140 with the extension among its built-ins through `--builtin-extensions-dir`.
- **No `.vsix` ships.** A tarball editor is linked by hand to the package's directory. A Flatpak isn't reached, and ADR 0018 had already left it out.
- **The package is `caminito-vscode-git`,** with a PKGBUILD of its own. The name doesn't wait on the extension's name in the glossary.
- **`caminito-git` lists it as an optional dependency,** next to `appmenu-gtk-module-caminito`. Nothing depends on it.

### Handed on

- **The spec:** the package's directory for the unpacked extension (`/usr/share/caminito-vscode/` was the example); the hook's triggers and script, with the unlinking before an editor's removal; the install script; the line of documentation for linking a tarball editor by hand; the `optdepends` line in `caminito-git`.
- **The spec, for the extension:** it has to cope by itself with an editor older than its `engines.vscode` floor, since a built-in skips that check. And ADR 0021's offer to hide the menubar needs a rule for the agents window, which has none and which the extension now reaches.
- **The spec's first packaging ticket** runs what ticket 05 asked of ticket 10 and ticket 10 didn't report: a link in `/usr/share/code/resources/app/extensions/` itself, whether a user can disable a built-in, and what an open window shows after the files change under it.
