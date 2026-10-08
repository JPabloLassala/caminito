# How a package installs an extension for VS Code and its forks

Type: research
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../map.md)

## Question

The extension ships as a `.vsix` in an Arch package. How does it get into VS Code without the user installing it by hand?

- Is there a system-wide extensions directory that `visual-studio-code-bin`, `code` (Code - OSS), `vscodium` and Cursor read, or a way to add one (built-in extensions under `resources/app/extensions`, `--extensions-dir`, a policy)?
- If not, what do other Arch or distribution packages that carry a VS Code extension do?
- What does `code --install-extension <file>.vsix` need (a running user session, a profile), and how does an update of the package reach an installed copy?
- Can an unpublished extension be installed in each fork, and does any of them warn about it?

## Answer

Full findings are on branch `research/vscode-installing-an-extension-from-a-package` (commit 304ce2d), in `research/vscode-installing-an-extension-from-a-package.md`. Read from the source at tag 1.140.0, the editors' PKGBUILDs and this machine. Nothing was run against an editor.

- **The only system-wide place is each editor's built-in directory,** `<app root>/extensions`. The scanner loads every child directory with a `package.json`, with no list of names and no signature, for every user and profile, and a symlink to a directory counts. Microsoft's own package has a Marketplace extension there as a plain directory. A built-in skips the `engines.vscode` check.
- **Nothing else adds a location.** `--extensions-dir`, `--builtin-extensions-dir` and `VSCODE_EXTENSIONS` each replace one. No policy installs an extension. A directory dropped into the user's extensions directory is removed at the next start, since user extensions come from `extensions.json`.
- **The directories:** `/usr/share/code/resources/app/extensions` (`visual-studio-code-bin`), `/usr/lib/code/extensions` (`code`), `/opt/vscodium-bin/resources/app/extensions` (`vscodium-bin`), `/usr/share/vscodium/resources/app/extensions` (`vscodium`), `/usr/share/cursor/resources/app/extensions` (`cursor-bin`).
- **No AUR package installs an extension into an editor** (114 PKGBUILDs read). Two ship a `.vsix` under `/usr/share` and print the command from `post_install`. `code-marketplace` is the precedent for writing into another editor's tree, with an alpm hook.
- **`code --install-extension <file>.vsix`** needs no running editor and no display, but it runs as the user (the launcher refuses root), into the default profile only, once for each editor. A package's update never reaches that copy and the editor won't update it either; removing the package leaves it.
- **An unpublished extension installs in all four with no warning.** A built-in is never "installed", so none of the checks run. Cursor's window was not checked, only its CLI bundle.
- **The manifest needs** `extensionKind: ["ui"]`, `capabilities.untrustedWorkspaces.supported: true` (or it's off in Restricted Mode), and an `engines.vscode` floor: the oldest base found is Cursor's 1.128.

### Handed on

- **The packaging decision** (in the map's fog, now specifiable): the built-in directory (recommended by the research: nothing for the user to do, updates with pacman) or an install for each user at session start. The built-in directory changes the map's settled "installed from a `.vsix`" to "installed unpacked", which is the developer's to change. Then its shape: symlinks the package owns, an alpm hook that links as editors come and go, or a split package for each editor.
- **[10](10-thin-end-to-end-slice.md):** put a directory, then a symlink, in `/usr/share/code/resources/app/extensions/` and confirm 1.140 loads it, whether it can be disabled, and what a window shows after the files change.
