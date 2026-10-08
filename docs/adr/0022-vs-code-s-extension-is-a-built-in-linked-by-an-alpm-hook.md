# 22. VS Code's extension is installed as a built-in, linked into each editor by an alpm hook

Date: 2026-10-08
Status: Accepted

## Context

The extension that serves VS Code's menubar as the global menu (ADR 0018) ships in an Arch package, and a user shouldn't have to install it by hand.

There is one system-wide place an editor loads extensions from: its built-in extensions directory, `<app root>/extensions`. It loads every child directory with a `package.json`, and a symlink to a directory counts. Each editor has its own, inside its own package's tree. The other way is `code --install-extension <file>.vsix`, which runs as the user, into the default profile, once for each editor, and leaves a copy that no package updates or removes.

In the agents window only built-in extensions are started, and that window has no in-window menubar, so the panel's menu is the only one it has.

pacman has no divert mechanism (ADR 0011), and a path can't be a directory in one package and a symlink in another.

The extension's names and its rule for an editor older than its floor were added on 2026-10-08, when the spec was written.

## Decision

- **The extension is a built-in.** The package installs it unpacked, once, in a directory of its own, `/usr/share/caminito-vscode/global-menu`, and each editor's built-in extensions directory gets a symlink to it, named `caminito.global-menu` after the extension's id. No `--install-extension` is run, for any user.
- **An alpm hook makes the links.** It links where an editor's built-in directory exists, when an editor is installed or upgraded, and unlinks before an editor is removed. The package's install script does the same when the package itself is installed, upgraded or removed, since pacman doesn't own the links.
- **The packaging carries five paths,** in one list:
  - `/usr/share/code/resources/app/extensions` (`visual-studio-code-bin`)
  - `/usr/lib/code/extensions` (`code`)
  - `/opt/vscodium-bin/resources/app/extensions` (`vscodium-bin`)
  - `/usr/share/vscodium/resources/app/extensions` (`vscodium`)
  - `/usr/share/cursor/resources/app/extensions` (`cursor-bin`)
- **No `.vsix` ships.** An editor the hook doesn't reach, such as a tarball in the user's home, is linked by hand to the package's directory, and the documentation says how.
- **The package is `caminito-vscode-git`,** with a PKGBUILD of its own in `packaging/arch/`. It isn't a patched package in ADR 0011's sense: it replaces nothing and carries no patch.
- **`caminito-git` lists it as an optional dependency,** as it lists `appmenu-gtk-module-caminito`. Nothing depends on it.
- **An editor older than the extension's floor is served with what it has.** The floor in the manifest is the oldest release that has every stable API the extension uses, and a built-in is loaded below it all the same. The extension looks for each API before it uses it, leaves out what depends on a missing one, writes one line to its log with both versions, and never throws into the extension host.

## Consequences

- The extension is there for every user and profile, in the agents window too, and it's updated and removed with pacman.
- In the agents window the extension claims with no in-window menubar to hide, so ADR 0021's offer needs a rule for that window.
- A built-in skips the `engines.vscode` check, so an editor older than the extension's floor loads it anyway. There it may lack what a missing API gave, a floating editor window's claims for one. The oldest base found among the five editors was Cursor's, 1.128.
- The links are files in other packages' trees that no package owns. `pacman -Qo` names no owner for them. If the package's removal script fails, a link is left dangling, and if the hook fails when an editor is removed, the editor's directory stays with the link in it.
- ADR 0017 left other programs' defaults alone ("other programs' defaults aren't the patched packages' to change"). This package does put a file into another package's tree. It changes none of the editor's files or settings, and it's installed only by a user who asks for it.
- A fork or a repackaging at a sixth path gets nothing until its path is added to the list.
- None of the five paths has been run. VS Code 1.140 was run with the extension among its built-ins, through `--builtin-extensions-dir` and not in `/usr/share/code` (ticket 10). The paths are from reading the PKGBUILDs, and Cursor's window was never checked.
- Not run: a link in an editor's real built-in directory, whether a user can disable a built-in extension, and what an open window shows after an update changes the files under it.
- A Flatpak editor isn't reached. ADR 0018 already leaves it out: its bus caller is `xdg-dbus-proxy`, and the Flathub manifests don't let `org.mate.Caminito` through.
- The package's name is wider than the extension's. The extension is the global menu extension, `caminito.global-menu`, in the repository's `caminito-vscode/` directory, and the menu is all it does. The package could carry a second extension.
- Rejected:
  - An install for each user at session start with `--install-extension` (the default profile only, never the agents window, a reinstall after each update of the package for each editor, and a copy left behind when the package is removed).
  - Symlinks the package owns (the package would create the directory trees of editors that aren't installed, a removed editor's tree would stay with the link in it, and an editor's package that made one of those directories a symlink would conflict with it).
  - A split package for each editor (five packages for five paths, and one more for every fork or repackaging).
  - Also shipping a `.vsix` under `/usr/share` (a copy installed from it is never updated or removed by the package and doesn't reach the agents window; it can be added if somebody asks).
  - Making it a dependency of `caminito-desktop` (a user with no VS Code has no use for it).
  - A second package split out of `caminito-git`'s PKGBUILD (node and the bundler would come into Caminito's build).
  - Doing nothing in an editor older than the floor, with or without telling the user (a fork that lags upstream would lose the whole menu over one missing API, which ADR 0019 rejected for a version outside a tested range).
