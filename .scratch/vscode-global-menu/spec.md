# Spec: VS Code's global menu

Written on 2026-10-08 as the destination of [the map](map.md). It gists the decisions and links the ADR that holds each one. Where the two differ, the ADR is right and this file is to be fixed.

Nothing here is built. The only code is the throwaway slice on branch `prototype/vscode-global-menu-slice` (commit 23b9697), which is a source to read and never one to merge.

## Problem

VS Code on native Wayland with its custom title bar builds no native menu, so in a Session with the global menu applet on a panel its menubar stays inside the window while every other app's is in the panel. The only way to the panel today is XWayland with a native title bar, which is drawn at the X11 scale and so is soft on an output with a fractional scale.

## Solution

The **global menu extension**, installed by a package as a built-in of each editor, rebuilds the editor's menubar, serves it as a dbusmenu and makes a **menu claim** for its window. Caminito ties the claim to the window and publishes the menu as it does any other. The user adds nothing to the editor and sets nothing. The extension offers once to hide the menubar in the window.

## User stories

1. As a user of VS Code on native Wayland, I want its menus in the panel when its window has the focus, so that it behaves like my other apps.
2. As a user with several VS Code windows, I want the panel's menu to act on the window I'm in, so that an item never runs somewhere else.
3. As a user, I want every item of the in-window menubar in an ordinary editing state to be in the panel's menu and to run its command.
4. As a user, I want the items' shortcuts shown, my own bindings included, so that the menu still teaches me the keys.
5. As a user, I want Open Recent to list what VS Code's own does.
6. As a user, I want a checkmark only where it's right, so that the menu never shows a state that isn't so.
7. As a user, I want to be asked once whether to hide the menubar in the window, and to get it back with Alt.
8. As a user, I want the menu to come and go with the global menu applet without restarting the editor.
9. As a user of VSCodium, Code - OSS or Cursor, I want the same menu, less whatever my editor lacks.
10. As a user of a window attached to SSH, a container or WSL, I want the menu there too.
11. As a user who installs the package, I want the extension in every editor I have and in every profile, updated and removed by pacman.
12. As a user who runs VS Code outside a Session, I want the extension to do nothing.
13. As the developer, I want to hear within a week that a VS Code release changed its menubar.

## Implementation decisions

### Caminito

[ADR 0018](../../docs/adr/0018-a-menu-claim-ties-a-menu-to-its-process-s-focused-window.md). The prototype's `menu_claim_prototype` module has all of this in throwaway form.

- **`ClaimGlobalMenu(path)` on `org.mate.Caminito`.** The menu's bus name is the caller's unique name. The reply is empty whatever happened.
- **The caller's pid comes from the bus, and its parent from `/proc/<pid>/stat`,** one step up. A focused toplevel of a client with either pid is tied to the caller and the path. With none, the claim is dropped.
- **Ties:** the last claim on a toplevel wins, a caller may hold several, and a tie ends with the toplevel or with the caller's bus name. There's no release.
- **An unclaimed toplevel has no menu of its own,** so it shows its transient parent's or none (ADR 0017). A menu the app set on its own surface wins over a claim, which is kept.
- **A claimed menu is published in `GlobalMenu` as a dbusmenu,** with the application's name from the toplevel. The global menu applet and its patched package don't change.
- **`GlobalMenuOffered`,** a read-only boolean property that tells when it changes, is true while Caminito offers the other three channels, the ten seconds of grace included. A claim made while it's false is dropped, and every tie is forgotten when it turns false.
- **Caminito sends nothing on a focus change.**
- **The wire is general and the promise isn't.** The method is documented as the global menu extension's, and no other client is promised it.
- **The log says which pid matched,** the caller's or its parent's. Ticket 10 had to compare them by hand.
- **Nothing rejects a late claim.** A claim came about 10 ms after the focus change, and a wrong tie lasted only for switches 10 ms apart or less.

### The global menu extension

**Names.** Decided in [ticket 13](issues/13-the-extension-s-name-and-the-spec.md).

- **The glossary term is global menu extension.** The menu is all it does.
- **It lives in `caminito-vscode/`,** at the top level of this repository.
- **Its id is `caminito.global-menu`:** publisher `caminito`, name `global-menu`, shown as "Caminito Global Menu".

**The build.** It's written in TypeScript and bundled with esbuild into plain files, with npm and a committed lockfile. Nothing native is in it.

**The manifest.** From [ticket 04](issues/04-serving-dbusmenu-from-an-extension.md) and [ticket 05](issues/05-installing-an-extension-from-a-package.md).

- `extensionKind: ["ui"]` alone, so it runs on the local side of a remote window.
- `capabilities.untrustedWorkspaces.supported: true`, or it's off in Restricted Mode.
- `activationEvents: ["onStartupFinished"]`.
- `engines.vscode`: the oldest release that has every stable API the extension uses. The first extension ticket looks it up. The oldest base among the five editors is Cursor's, 1.128.

**An editor older than the floor.** [ADR 0022](../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md).

- A built-in is loaded below its floor. The extension looks for each API before using it and leaves out what depends on a missing one.
- It writes one line to its log with the editor's version and the floor.
- Nothing it does throws into the extension host: a failure at activation is logged and the extension stays idle.

**The gate and the claim.** ADR 0018.

- **It serves and claims only on Linux, while `org.mate.Caminito` has an owner and `GlobalMenuOffered` is true.** It watches both, so it follows a restart of Caminito and the applet coming and going with no reload.
- **It claims:**
  - at activation, when `window.state.focused` reads true;
  - when its window gains focus;
  - when `GlobalMenuOffered` turns true, or `org.mate.Caminito` gains an owner with it true, if `focused` is true, whether or not a focus event came before;
  - while `focused` is true, when the active text editor or the active notebook editor changes and when the tab groups change.
- **It never acts on focus lost.**
- **It serves under its connection's unique name.** A reload ends the host and the menu empties until the new host claims.
- **A window whose `focused` is stuck at false after a dropped focus event gets no claim,** from any of these. Nothing covers it.

**Serving the dbusmenu.** Ticket 04.

- **`@jellybrick/dbus-next`,** pinned, bundled. Its `Interface.configureMembers()` registers nothing in 0.11.4, and deleting three own fields in the subclass's constructor works around it. The bug is to be reported to `JellyBrick/node-dbus-next`.
- **The server is `GetLayout`, `Event` and `AboutToShow`,** with the `LayoutUpdated` and `ItemsPropertiesUpdated` signals. `LayoutUpdated`'s revision grows.
- **`Event` is answered before the command runs,** and `AboutToShow` at once with false. The applet's importer waits for both inside mate-panel.
- **An item's properties are sent in alphabetical order.** With `label` before `children-display`, the importer gives a submenu a stray `action`.
- **`GetLayout` returns the whole tree.** The importer fetches it at once.

**The menu table.** [ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md).

- **A developer command makes the table.** It starts Microsoft's build, at `visual-studio-code-bin`'s version, under Xvfb with a native title bar and a throwaway profile, waits until `lastKnownMenubarData` in `storage.json` stops growing, and does so with a folder open and with a multi-root workspace open. It merges the two.
- **It strips** `enabled`, `checked`, Open Recent's entries and the profile's name in a label, and keeps the 108 keybinding labels as a fallback.
- **The table's format is the first table ticket's to set.** It holds what the snapshot does: the menus in order, and for each item its command id, its label with the mnemonic, its separators and its submenus.
- **The table is committed in `caminito-vscode/`,** with the version and commit it was made from.
- **The annotations file is written by hand, keyed by command id,** with three kinds of entry:
  - a setting's checkmark or radio value;
  - a `product.json` key the item is gated on;
  - a workspace state the item follows: no folder, a folder, a multi-root workspace.

  The command fails on an annotation whose command the snapshot lacks.
- **The tolerance rule, at start:** drop each item whose command `commands.getCommands()` doesn't list, then each submenu left empty, then collapse the separators. Gate product-dependent items on the running editor's own `product.json`, read at `vscode.env.appRoot`.
- **A VS Code newer than the table is served silently,** with one log line naming both versions. An older one gets what the tolerance rule leaves.
- **Titles are English.** There's no list of profiles, no profile name in Export Profile, and no update item.
- **The weekly canary** is a job in the weekly workflow. It downloads the newest stable build, takes the snapshot, compares its structure with the committed table, and on a difference fails and opens or updates an issue with the diff. Nothing commits by itself. If Electron can't run under Xvfb in the workflow's container, the command is run locally when the package updates, and the spec is amended to say so.

**What the menu shows.** [ADR 0020](../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md).

- **Checkmarks only where they mirror a setting,** twelve of them, four as radio groups, each read with no resource or language scope. Editor Actions Position's Title Bar follows `workbench.editor.showTabs` as well. An item whose state can't be read is a plain item.
- **Every item is enabled.** Only the items annotated with a workspace state come and go.
- **Other extensions' items aren't shown.** Copy As drops out as an empty submenu, and Share keeps what the table has of VS Code's own.
- **Keybinding labels** are computed from the default keybindings (`vscode://defaultsettings/keybindings.json`) and the user's file: the user's removals first; a binding whose keys a later entry with no `when` gives to another command stops counting; then the only binding, else the last with no `when`, else the last.
- **The fallback** when the defaults can't be read is the snapshot's labels, with the user's own bindings over them.
- **The user's file is found through `storage.json`:** the profile tied to the window's folder or workspace file; for a window with no entry, the profile `window.newWindowProfile` names; else the default profile's. A profile that uses the default's keybindings has the default profile's file, and so has every window if `storage.json` can't be read or isn't shaped as expected. The file is read and watched, never opened in a tab.
- **dbusmenu's `shortcut`** is an array for each press, the modifiers `Control`, `Alt`, `Shift`, `Super` and then a GDK key name. The table from VS Code's key names to GDK's is in the research for [ticket 02](issues/02-keybinding-labels-and-recent-files.md), on branch `research/vscode-keybinding-labels-and-recent-files`. A chord has no shortcut, nor has a key with no GDK name.
- **Open Recent** is Reopen Closed Editor, the first ten folders and workspaces, the first ten files, More… and Clear Recently Opened, from `_workbench.getRecentlyOpened`. A label is the path with the home directory as `~`, a workspace and a remote marked as VS Code marks them, cut in the middle at 120 characters, with each underscore doubled so that it isn't read as a mnemonic.
- **A recent folder or workspace opens with `vscode.openFolder`,** a file with `_files.windowOpen`, and with `vscode.open` if that fails. If the list can't be read, the submenu keeps its fixed items.
- **Changes are pushed.** The extension re-reads on these events and sends an update only if the result differs:
  - focus gained: the recent list and the window's profile;
  - a change of the editor tabs, or the return of a command it ran to open a recent entry or clear the list: the recent list;
  - a change of an annotated setting: its checkmarks;
  - a change of the user's keybindings file: the labels;
  - a change of the installed extensions: the default keybindings and the list of commands;
  - a change of the workspace's folders: the items that follow them.
- **Updates within a short time are sent as one.** A change of structure is a `LayoutUpdated`, and a change of a checkmark or a shortcut alone is an `ItemsPropertiesUpdated`. The importer drops an `ItemsPropertiesUpdated` for 100 ms after a layout reply.
- **There's no timer, and nothing is built on `AboutToShow`.**

**The offer.** [ADR 0021](../../docs/adr/0021-vs-code-s-in-window-menubar-is-the-user-s-to-hide.md).

- **The extension never writes `window.menuBarVisibility` on its own.**
- **When it claims while that setting is `classic` or `visible`, it shows a notification:** "The panel now shows this window's menus. Hide the menu bar in the window? Pressing Alt shows it again." It names no editor.
- **Hide Menu Bar** sets `window.menuBarVisibility` to `toggle` in the user's settings. **Keep Menu Bar** changes nothing.
- **`caminito.globalMenu.offerToHideMenuBar`** is the extension's own setting: a boolean, true by default, application-scoped, with `ignoreSync`. The extension turns it off when it shows the notification, whatever the answer, and makes no offer while it's off.
- **One host offers.** Before offering, a host asks for the bus name `org.mate.Caminito.GlobalMenuExtension.pid<pid>`, with its parent's pid, without queueing. Only the host that gets it offers, and it keeps the name while it lives.
- **No offer comes from the agents window,** which the extension knows by its workspace file, `agent-sessions.code-workspace` in the user's settings directory. Its host serves and claims.
- **View > Appearance > Menu Bar** stays in the menu and runs VS Code's own command. Its checkmark is on when the setting is `classic` or `visible`. Under the custom title bar that command takes `classic` or `visible` to `compact` and anything else to `classic`, never to `toggle`.

### Packaging

ADR 0022.

- **`caminito-vscode-git`,** with a PKGBUILD of its own in `packaging/arch/`. It builds the extension with node and the bundler, replaces nothing and carries no patch.
- **It installs the extension unpacked at `/usr/share/caminito-vscode/global-menu`.** No `.vsix` ships.
- **One list of five built-in directories:**
  - `/usr/share/code/resources/app/extensions` (`visual-studio-code-bin`)
  - `/usr/lib/code/extensions` (`code`)
  - `/opt/vscodium-bin/resources/app/extensions` (`vscodium-bin`)
  - `/usr/share/vscodium/resources/app/extensions` (`vscodium`)
  - `/usr/share/cursor/resources/app/extensions` (`cursor-bin`)
- **One script links and unlinks.** In each directory of the list that exists it makes the link `caminito.global-menu` to the package's directory, or removes it.
- **An alpm hook runs it:** to link after an install or upgrade that touches one of the five directories, and to unlink before a removal that does.
- **The package's install script runs it too:** to link after the package's own install and upgrade, and to unlink before its removal. pacman doesn't own the links.
- **`caminito-git` gains an `optdepends` line** for `caminito-vscode-git`, beside `appmenu-gtk-module-caminito`'s. Nothing depends on it.
- **The packaging workflow builds it** with the other packages.

### Documentation

In the README's section on the global menu, where ADR 0017's settings are.

- **VS Code's paragraph is rewritten.** With `caminito-vscode-git` installed, VS Code, VSCodium, Code - OSS and Cursor on native Wayland with the custom title bar have their menu in the panel, and follow the applet while they run. XWayland with a native title bar stays as the way without the package.
- **The settings:** `window.menuBarVisibility` and what `toggle` does; `caminito.globalMenu.offerToHideMenuBar`, and that turning it on brings the offer back; `settingsSync.ignoredSettings` for a user who wants the menubar hidden on this machine only. And that the Menu Bar item never goes back to `toggle`, so a user who brought the menubar back with it sets the setting by hand.
- **Linking an editor the hook doesn't reach,** a tarball in the user's home say: one `ln -s` line from the package's directory into the editor's `resources/app/extensions`.
- **What it doesn't do:** a Flatpak editor, items greyed out, checkmarks for a state it can't read, a chord's shortcut, other extensions' items, a translated menu, Alt+F opening the panel's menu.

## Testing decisions

A good test here drives the same wire the real peers use and asserts what they'd see. It doesn't reach into a module to check how the menu is kept.

- **Caminito:** headless tests on a bus of the test's own, as `caminito/src/tests/global_menu.rs` has them, with a test client that calls `ClaimGlobalMenu` and reads `GlobalMenu` and `GlobalMenuOffered`. The parent step needs a claim made from a child process of the client's.
- **The extension has one seam.** Its core takes the `vscode` API as a value. A test hands it a fake of the part it uses and runs it on a private `dbus-daemon` with a stand-in for `org.mate.Caminito`. It asserts the dbusmenu a client reads from the bus, the signals sent, the claims made, the name asked for and what was written to the fake's settings.
- **The rules with many cases** (the tolerance rule, the label rule, the key names, a recent entry's label, the `storage.json` lookup) are tested through that seam where it's cheap, and directly, as functions of their inputs, where the cases are many.
- **Node's own test runner.** A job in `ci.yml` type-checks, runs the tests and builds the bundle.
- **No editor is started in CI.** A run under `--extensionTestsPath` isn't real use either: confirmation dialogs confirm themselves there, and `_files.windowOpen` opens a new window.
- **The developer command and the canary** are tested by running them.
- **The packaging script** is tested against directories under a temporary root, and the package with `namcap` as the others are.
- **A real editor is the last ticket's,** by hand, in a Session.

## What has to be run

Nobody has run these. Each is listed under the slice that runs it, and a result that contradicts an ADR reopens that ADR from that slice's ticket.

**With the package** (slice 3):

- A link in an editor's real built-in directory, for each of the five that can be installed.
- Whether a user can disable a built-in extension, and what the documentation then says.
- What an open window shows after an update changes the files under it.

**With the whole table** (slice 4):

- A menu of more than one submenu, and how long the panel stalls while the importer preloads a real menu's submenus.
- How GTK draws the stray `action` of a submenu whose `label` is sent first, to confirm the alphabetical order is what avoids it.
- The snapshot route once more from the command itself, since the earlier run's output wasn't kept.

**With the checkmarks and updates** (slice 5):

- Whether a setting read with no scope is what VS Code's `config.*` key holds.
- How the applet draws a radio item.
- What a `LayoutUpdated` does to a panel menu that's open.
- Whether an `ItemsPropertiesUpdated` sent within 100 ms of a layout reply has to be sent again.

**With the labels** (slice 6):

- Whether the patched applet cuts a chord to its first press as upstream's 25.04 does.
- The `storage.json` lookup for a window on a `.code-workspace` file.
- The user's own keyboard layout.

**With Open Recent** (slice 7):

- Whether a change of tabs comes with every change of the recent list.
- Whether the window regains focus, and so re-reads, when a panel menu closes.

**With the offer** (slice 8):

- Two windows focused 50 ms apart, with the bus name.
- `workspace.workspaceFile` in the agents window.
- That the offer isn't made again in another profile.

**With the canary** (slice 9):

- Whether Electron runs under Xvfb in the weekly workflow's container, with its sandbox on or off.

**In a real Session, by hand** (slice 10):

- Whether a click's focus change is ever dropped, as 3 of 20 were through the nested run's debug call. If one is, ADR 0018's dropped focus event is reopened.
- `focused` while a panel menu has the keyboard.
- A floating editor window, opened and reached from its workbench window, with a text editor and with only a terminal; and whether the claim made as it opens lands on it or on the workbench window.
- A fork: the value of `focused` at activation, and the menu the tolerance rule leaves. Cursor's window has never been checked.
- A remote window.
- A window with a menu on its own surface and a claim.
- The agents window, with more than one item tried.
- An editor older than the floor, if one can be had.
- Whether a window ever closes by itself soon after a reload, as one did once, 0.7 s after, in ticket 10's runs.

## The cut into tickets

Ten slices, filed in `.scratch/global-menu/issues/` from 115 by a later run. Each is run by itself once the ones it's blocked by are in.

| | Slice | Blocked by |
| --- | --- | --- |
| 1 | **Caminito takes a menu claim.** The method, the property, the ties and the read of `/proc`, with headless tests. | |
| 2 | **The extension serves one fixed menu and claims.** `caminito-vscode/`, the manifest, the build, the CI job, the test seam, the gate, the claim rule and the rule for an old editor. | 1 |
| 3 | **The package installs it.** The PKGBUILD, the script, the hook, the install script, the `optdepends` line, the packaging workflow and the README. | 2 |
| 4 | **The menu is VS Code's.** The developer command, the table, the annotations' `product.json` gates, the tolerance rule, and every item running its command. | 2 |
| 5 | **Checkmarks and items that follow the workspace.** The other two kinds of annotation, the events and the coalescing of updates. | 4 |
| 6 | **Keybinding labels.** The defaults, the user's file, the profile lookup, the key names and the fallback. | 4 |
| 7 | **Open Recent.** | 4 |
| 8 | **The offer to hide the menubar.** The notification, the setting, the bus name and the agents window. | 2 |
| 9 | **The weekly canary.** | 4 |
| 10 | **A real Session, by hand.** A verification ticket, with what's left under "What has to be run". | 3, 5, 6, 7, 8 |

Slices 5 to 7 each add their own events to the update path, and whichever comes first builds the coalescing. Slice 3 writes the README for the menu as it will be, and a slice that changes what a user sees corrects it.

## Out of scope

- **The menu Electron registers under a native title bar on native Wayland,** which is ticket 113's open question.
- **Alt+F and F10 opening the panel's menu** (ADR 0017).
- **A carried patch that draws a chord's shortcut** in the applet's translator, a later ticket.
- **A carried patch that makes the importer's calls asynchronous.** It's the fix if slice 4 finds the panel's stall too long.
- **A Flatpak editor.**
- **Other extensions' items, greying out, a translated menu,** and any proposed API.
- **A `.vsix`, the Marketplace and Open VSX.**
- **A canary for a fork.**
- **Any other Electron app.**
- **Activation with `*`.** Ticket 10 didn't run it, and `onStartupFinished` is the decision.

## Further notes

- **The prototype's own comments are older than its findings** in two places: `extension.js` says the value read at activation is true before the real one arrives, and its claim when `GlobalMenuOffered` turns true asks for no earlier focus event. This spec has the rule.
- **The harness nests Caminito with a runtime directory of its own.** Without one, the accessibility bus the private session bus starts takes the desktop's socket path in `/run/user/<uid>/at-spi/`.
- **The prototype's debug calls,** Activate and Key, were what moved the focus and pressed keys. Slice 1 takes them only if its tests need them.
- **Internals the extension leans on:** `lastKnownMenubarData`, `storage.json`, the defaults file, `_workbench.getRecentlyOpened`, `_files.windowOpen`, and the agents window's workspace file. Each has a fallback that keeps the menu with less in it.
