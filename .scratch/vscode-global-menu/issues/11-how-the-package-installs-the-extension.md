# How the package installs the extension

Type: grilling
Status: open
Blocked by:
Map: [VS Code's global menu](../map.md)

## Question

The research ([05](05-installing-an-extension-from-a-package.md)) found one system-wide place, each editor's built-in extensions directory, and one way for a user, `--install-extension`.

- The built-in directory, or an install for each user at session start? The first changes the map's settled "installed from a `.vsix`" to "installed unpacked", and writes into other packages' trees.
- If the built-in directory: symlinks the package owns (and directories for editors that aren't installed), an alpm hook that links and unlinks as editors come and go, or a split package for each editor.
- Which editors' paths the packaging carries, and whether a `.vsix` also ships for an editor it doesn't reach (a Flatpak, a tarball).
- The package's name, and whether `caminito` lists it as an optional dependency.
