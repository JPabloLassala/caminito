# caminito-vscode-git installs the global menu extension

Epic: global-menu
Type: implementation
Status: resolved
Milestone: none: after v0.1
Blocked by: [116](116-the-extension-serves-one-menu-and-claims.md)

## What to build

[ADR 0022](../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md), under "Packaging" and "Documentation" in [the spec](../vscode-global-menu/spec.md). A user who installs `caminito-vscode-git` has the global menu extension as a built-in of every editor of the five that's installed, in every profile, and pacman updates and removes it.

- **The package:** a PKGBUILD of its own that builds the extension with node and the bundler and installs it unpacked at `/usr/share/caminito-vscode/global-menu`. No `.vsix`.
- **One script and one list of the five built-in directories.** In each that exists the script makes or removes the link `caminito.global-menu`.
- **An alpm hook** links after an install or upgrade that touches one of the five directories and unlinks before a removal that does. **The install script** does the same for the package's own install, upgrade and removal.
- **`caminito-git`** gains the `optdepends` line. **The packaging workflow** builds the package.
- **The README:** VS Code's paragraph rewritten, the three settings, the line for linking an editor the hook doesn't reach, and what the extension doesn't do, each as the spec lists it.

## Acceptance criteria

- [x] The script, run against directories under a temporary root, links only where a directory exists, is harmless when run twice, and removes only its own link.
- [x] `namcap` passes, and the packaging workflow builds the package.
- [x] Installing the package with `visual-studio-code-bin` installed puts the link in its built-in directory, and VS Code loads the extension.
- [x] Installing an editor after the package links it, and removing an editor or the package leaves no link and no directory behind.
- [x] The README says what a user reads on screen, and its XWayland route stays as the way without the package.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [x] A link in an editor's real built-in directory, for each of the five that can be installed. Seen for `visual-studio-code-bin` alone, which jpablo accepted (2026-10-09).
- [x] Whether a user can disable a built-in extension. The README then says so, or says there's no way.
- [ ] What an open window shows after an update changes the files under it. Not seen here, and now [124](124-vs-code-in-a-real-session.md)'s.

## Notes

- The README is written for the menu as it will be. A later ticket that changes what a user sees corrects it.

## Done (2026-10-09)

- **The script was tested first,** in `caminito-vscode/test/link.test.mts`, against the five directories under a temporary root (`CAMINITO_VSCODE_ROOT`): it links only where a directory exists, a second run changes nothing and warns of nothing, it leaves a user's own link or a directory at the path alone, and the removal hook's `--targets` unlinks only the removed editor's directory. It runs in `ci.yml`'s extension job and in the package's `check()`.
- **The one list is the script's.** `caminito-vscode-link directories` prints it, and the PKGBUILD writes both hooks' `Target` lines from it, so the five paths are written once.
- **The removal hook unlinks only what it's removing,** from its `NeedsTargets` paths, so removing one editor leaves the others linked. An editor upgrade that drops a file fires it too, and the link hook links again after the transaction.
- **The package ships `package.json` and `dist/extension.js` alone.** The menu table is bundled, and the source map is left out.
- **`depends=(bash coreutils gawk)`** for the script, which namcap may call partly unneeded.
- **The packaging workflow also runs on a change under `caminito-vscode/`,** since the package builds it.
- **Not run here** (the container isn't Arch): namcap, makepkg, a pacman transaction, and any editor. Whether a built-in can be disabled is still open, so the README says nothing of it.

## Run on Arch (2026-10-09)

On jpablo's machine, at `79fd9fe`, with `visual-studio-code-bin` 1.140.0 and none of the other four editors.

- **namcap and makepkg, locally.** `namcap PKGBUILD` prints nothing. `makepkg` builds `caminito-vscode-git-r408.79fd9fe-1-any.pkg.tar.zst` and its `check()` passes 68 tests. namcap on the package prints two warnings and no error: `coreutils` and `gawk` "may not be needed". The workflow's run on master at the same commit (37940721930) passed: it built the package, the 68 tests passed, and namcap in the bare container printed no error and two more warnings, that `sh` is "an uninstalled dependency" of the link script and that `bash` "may not be needed".
- **Installing the package** made `/usr/share/code/resources/app/extensions/caminito.global-menu`, which no package owns. A VS Code window on a fresh `--user-data-dir` logged `ExtensionService#_doActivateExtension caminito.global-menu, startup: false, activationEvent: 'onStartupFinished'`.
- **The menu itself wasn't seen.** The extension logged `can't read GlobalMenuOffered: Unknown property 'GlobalMenuOffered'`: the running compositor is `caminito-git` r333, from before 115. That's the gate working, and it's 116's to see with a current `caminito-git`.
- **An editor installed after the package,** with a throwaway package that owns one file under `/opt/vscodium-bin/resources/app/extensions` standing in for `vscodium-bin` (`code` conflicts with `visual-studio-code-bin`, and the other three are in the AUR). Installing it ran the link hook and made the link. Removing it ran the unlink hook, pacman removed `/opt/vscodium-bin` whole, and VS Code's link stayed. With both installed, removing `caminito-vscode-git` took both links and `/usr/share/caminito-vscode`, and installing it again made both.
- **A built-in can be disabled.** With `[{"id":"caminito.global-menu"}]` under `extensionsIdentifiers/disabled` in the profile's `state.vscdb`, which is where the Extensions view's *Disable* is kept, the window didn't activate it, and neither did one started with `--disable-extension caminito.global-menu`. Nobody clicked *Disable* in the view. The README now says both.
- **An update under an open window:** reinstalling the same package under an open window left it running with nothing new in its logs. That's the same files and no menu on screen, so the item went to [124](124-vs-code-in-a-real-session.md), which runs with the menu on screen.
- **The link in a real built-in directory** is seen for `visual-studio-code-bin` alone. jpablo accepted that for the ticket, so `code`'s, `vscodium-bin`'s, `vscodium`'s and `cursor-bin`'s paths are as their packages list them and nobody has seen a link in one.
- **Closed on jpablo's word (2026-10-09),** with every acceptance criterion met.
