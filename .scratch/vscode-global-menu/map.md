# Map: VS Code's global menu

Label: wayfinder:map

## Destination

A spec, ready to cut into implementation tickets, for showing VS Code's menubar as the **global menu** while VS Code runs as a native Wayland window with its custom title bar: a VS Code extension that serves the menu, and the Caminito side that ties it to the window. The spec may end in "don't build it" if the research shows a part can't be done.

## Notes

- **Domain:** the vocabulary is in [CONTEXT.md](../../CONTEXT.md) (**Global menu**, **Global menu applet**). The broker and its three channels are [ADR 0017](../../docs/adr/0017-caminito-brokers-the-global-menu-for-a-patched-appmenu-applet.md).
- **Skills:** every grilling session calls `grilling` and `domain-modeling`, and updates CONTEXT.md and `docs/adr/` inline.
- **Plan, don't do:** tickets produce decisions, not code. The prototype ticket may produce throwaway code as an asset.
- **Tracker:** local markdown. Tickets are in `issues/`, blocking is the `Blocked by:` line, and research findings go on `research/<name>` branches.
- **Why an extension:** with `window.titleBarStyle` at `custom`, VS Code 1.140 builds no native menu (`window.menuStyle: native` is ignored then), so nothing is exported. An extension can't read the menubar either, so it rebuilds it.
- **Settled while charting** (these are fixed; don't reopen them in tickets):
  - It serves VS Code and the forks that share its extension API (VSCodium, Code - OSS, Cursor), as long as a fork costs no more than not hard-coding names. No other Electron app.
  - The first version's bar: the same top-level menus and items as the in-window menubar in an ordinary editing state, every item runs its command, keybinding labels and recent files are there. Items needn't grey out, context-only items may be always or never there, and a checkmark is needed only where it mirrors a setting.
  - The extension lives in this repository, in its own top-level directory, and an Arch package ships it, installed unpacked (ADR 0022).
  - A window attached to SSH, a container or WSL must work: the extension runs on the local side.

## Decisions so far

<!-- one line per closed ticket -->

- [What an extension can read of VS Code's menubar](issues/01-menubar-sources.md): nothing at run time; the core items (about 245) come from VS Code's source at build time through the type checker, other extensions reach only Copy As and Share, and no context key or `when` clause can be read.
- [How a package installs an extension for VS Code and its forks](issues/05-installing-an-extension-from-a-package.md): the only system-wide place is each editor's built-in extensions directory, which loads any directory or symlink put there (read, not run); `--install-extension` works only as the user and never follows a package's update.
- [How Caminito can tell which window an extension host belongs to](issues/03-window-of-an-extension-host.md): the process is found through the bus caller's pid and its parent, but nothing an extension knows names its window; two windows can be told apart only by the extension claiming focus.
- [Whether keybinding labels and recent files can be had](issues/02-keybinding-labels-and-recent-files.md): both can, through internals (a virtual defaults file and `_workbench.getRecentlyOpened`), run on 1.140; the window's profile isn't told, neither has a change event, and the applet shows only a chord's first press.
- [How an extension host can serve a dbusmenu](issues/04-serving-dbusmenu-from-an-extension.md): a bundled JavaScript library (`@jellybrick/dbus-next`, with one workaround) served the applet's importer in runs outside a live extension host; the importer's calls block the panel, so a busy extension host freezes it.
- [The channel between the extension and Caminito](issues/06-channel-between-extension-and-caminito.md): a menu claim (ADR 0018): the extension calls `ClaimGlobalMenu` on `org.mate.Caminito` when its window gains focus, and Caminito ties the caller to its parent process's focused toplevel; it follows the applet's switch through a `GlobalMenuOffered` property, and a reload empties the menu until the new host claims.
- [Where the menu's structure comes from and how it's kept current](issues/07-source-of-the-menu-structure.md): a committed table (ADR 0019), a snapshot of what VS Code with a native title bar writes as `lastKnownMenubarData`, plus hand-written annotations; one table for every version and fork, with items whose command is missing dropped; a weekly canary snapshots the newest release and opens an issue on a difference; titles are English.
- [What the menu shows beyond structure](issues/08-what-the-menu-shows-beyond-structure.md): only the state the extension can read (ADR 0020): checkmarks where they mirror a setting and none elsewhere, every item enabled, keybinding labels computed from the default keybindings and the user's file with the snapshot's as fallback, no shortcut for a chord, Open Recent as VS Code's own; changes are pushed on events, never built on `AboutToShow` or a timer.
- [What happens to the in-window menubar](issues/09-the-in-window-menubar.md): it stays the user's (ADR 0021): the extension never writes `window.menuBarVisibility` on its own and offers once for a machine to set it to `toggle`, where Alt shows it again; nothing brings it back when the applet goes, and only the panel's menu follows `GlobalMenuOffered`.
- [A thin end-to-end slice](issues/10-thin-end-to-end-slice.md): it works, with two windows, a reload and a restart of the panel, and a claim comes about 10 ms after the focus change; but a window focused when its host starts gets no focus event (its `focused` is already the real value), a floating editor window is claimed only when reached from another window, two hosts can both make the offer within about 300 ms, and the extension reaches the agents window only as a built-in. ADR 0018's claim rule is to reopen.
- [How the package installs the extension](issues/11-how-the-package-installs-the-extension.md): as a built-in (ADR 0022): `caminito-vscode-git` installs it unpacked in a directory of its own, and an alpm hook links it into the built-in extensions directory of each of five editors that's installed; no `.vsix` ships, nothing is installed for a user, and `caminito-git` lists the package as an optional dependency.

## Not yet specified

- **The spec's outline:** its sections, and how the work is cut between the extension and Caminito.
- **The extension's name in the glossary.** "Caminito integration" was the working name. Does it do anything besides the menu?

## Out of scope

- **Showing the menu Electron already registers under a native title bar.** VS Code on native Wayland with `window.titleBarStyle: native` serves a full dbusmenu and registers it under an id that's no X window's. Matching it to the window is a Caminito change that would serve other Electron apps too. The developer keeps the custom title bar, so it stays with [ticket 113](../global-menu/issues/113-global-menu-for-xwayland-windows.md)'s open Electron question.
- **Alt+F and F10 opening the panel's menu.** Left to a later ticket by ADR 0017.
- **Publishing to the Marketplace or Open VSX.**
