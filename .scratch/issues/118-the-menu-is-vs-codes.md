# The panel's menu is VS Code's own

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [116](116-the-extension-serves-one-menu-and-claims.md)

## What to build

[ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md), under "The menu table" in [the spec](../vscode-global-menu/spec.md). The panel shows VS Code's eight menus with every item of an ordinary editing state, and each item runs its command.

- **The developer command** starts Microsoft's build under Xvfb with a native title bar and a throwaway profile, waits for `lastKnownMenubarData` to settle, with a folder open and with a multi-root workspace open, merges the two and strips what belongs to the run. It keeps the snapshot's keybinding labels for [120](120-keybinding-labels.md).
- **The table** is committed with the version and commit it was made from. Its format is this ticket's to set.
- **The annotations file,** written by hand and keyed by command id, with the `product.json` gates. The command fails on an annotation whose command the snapshot lacks.
- **The tolerance rule at start:** drop each item whose command isn't listed, then each submenu left empty, then collapse the separators; gate product-dependent items on the running editor's own `product.json`.
- **One log line** names the table's version and the editor's when they differ.
- **Open Recent keeps its fixed items only,** and there are no checkmarks or shortcuts yet.

## Acceptance criteria

- [x] The command writes the committed table, and running it again changes nothing.
- [x] Through the seam: with every command listed, a client reads the table's menus in order with their mnemonics, separators and submenus.
- [x] Through the seam: an item whose command is missing is gone, a submenu left empty is gone, and no two separators are together or at an end.
- [x] Through the seam: an item gated on a `product.json` key is there only when the fake's `product.json` has it.
- [x] Through the seam: an `Event` on any item runs its command, with its arguments if the table has them.
- [ ] In a nested run with VS Code 1.140, the panel shows the eight menus and an item of each runs.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] A menu of more than one submenu, and how long the panel stalls while the importer preloads the real menu's submenus. If it's too long, say so here: the fix is a carried patch that makes the importer's calls asynchronous, a ticket of its own.
- [ ] How GTK draws the stray `action` of a submenu whose `label` is sent first, to confirm that alphabetical order is what avoids it.
- [x] The snapshot route from the command itself. The earlier run's output wasn't kept.

## Notes

- Titles are English. There's no list of profiles, no profile name in Export Profile, and no update item.
- Copy As drops out as an empty submenu, and Share keeps what the table has.

## Comments

**2026-10-08, the table built.** `npm run snapshot` in `caminito-vscode/` is the developer command (`tools/snapshot.mts`, with what it makes of the data in `tools/menubar.mts`). The table is `menu/table.json`, the annotations `menu/annotations.json`, and the rule that fits the table to the running editor is `fit` in `src/menu-table.mts`. The core reads the commands and `product.json` once at start, before it serves.

- **The table's format** is in `src/menu-table.mts`: `version` and `commit`; `menus`, each `{ label, submenu, items }` with `submenu` VS Code's id for the menu; an item `{ label, command }`, with `args` when the table has them (the snapshot never writes any); a separator `{ separator: true }`; and `keybindings`, the snapshot's 108 by command id as VS Code has them (`label`, `userSettingsLabel`, `isNative`), for [120](120-keybinding-labels.md). Labels are dbusmenu's: VS Code's `&&` is `_` and an underscore is doubled. A top-level menu's mnemonic is its first letter, as VS Code's eight have it.
- **The annotations** are `{ "<command>": { "product": "<key>" } }`, nine of them, from the source of 1.140's Help menu: Show Release Notes, Keyboard Shortcuts Reference, Video Tutorials, Tips and Tricks, Documentation, Join Us on YouTube, Search Feature Requests, View License and Privacy Statement. An item is shown when the key's value is truthy, as VS Code's own `AVAILABLE` checks are. They're joined to the table at start rather than written into it, so the command checks them and a test holds the committed pair to it.
- **The merge** keeps the longest run of items common to both states, in order, and puts what only one has where it has it, the folder's first. An item is the same in both by its command and label, so Close Folder and Close Workspace are both in the table, with the one command `workbench.action.closeFolder`: that's the only difference between the two states in 1.140. Which of the two follows which state is [119](119-checkmarks-and-workspace-items.md)'s, and its annotation will need the label as well as the command id.
- **Stripped:** `enabled`, `checked`, Open Recent's entries (anything with a `uri`), each profile's entry (`workbench.action.openProfile.*` and `workbench.profiles.actions.profileEntry.*`), ` (Default)` in a label (Share > Export Profile), and then each submenu left empty and the separators.

**The snapshot route, from the command.** Microsoft's 1.140.0 (`code_1.140.0-1790759618_amd64.deb` from packages.microsoft.com, commit `07f806f9`) under Xvfb in an Ubuntu 24.04 container, as root, with `--no-sandbox`: `lastKnownMenubarData` appeared about 5 s after start and settled at about 8 s, as before. The command took 16 s for both states, and a second run wrote the same bytes. It passes `--no-sandbox` always, so the canary ([123](123-the-weekly-canary.md)) needn't find out whether the sandbox works in its container. The table is made from 1.140.0 because that's the version this ticket names; packages.microsoft.com already has 1.141.0, and `visual-studio-code-bin`'s version couldn't be read from here (the AUR is out of the session's reach).

**The real editor, headless.** The built extension in that 1.140.0, with the custom title bar, under Xvfb on a private bus with the tests' stand-in for `org.mate.Caminito`: it claimed 4.2 s after start, a client read the eight menus, and a click on one item of each ran it (Auto Save, Find in Files, Select All, Command Palette, Go to File, Toggle Breakpoint, New Terminal, Show All Commands, each in the renderer's trace log, and Auto Save in `settings.json`); File > Exit closed the editor. This isn't the nested run, which needs a Wayland desktop and the applet, so the last criterion stays open.

**The rule at start drops two items it shouldn't, which qualifies [ADR 0019](../../docs/adr/0019-vs-code-s-menu-table-is-a-snapshot-of-its-own-native-menubar.md)'s "at start".** At `onStartupFinished`, 1.140 lists 2917 commands; 5 s later it lists 3274. Two of the late ones are the menubar's: Backup and Sync Settings (`workbench.userDataSync.actions.turnOn`) and Report Issue (`workbench.action.openIssueReporter`), so the served menu lacks them in every window. Found with a probe extension that called `commands.getCommands()` at activation, at 5 s and at 15 s. A fix needs the commands read again later and a `LayoutUpdated` when the menu changes, which is [119](119-checkmarks-and-workspace-items.md)'s update path; focus gained is the obvious event, since the spec has no timer. **Decided by the developer on 2026-10-08: the two items can stay missing,** so nothing re-reads the commands for them, and ADR 0019 isn't reopened, only qualified. 119's update path may bring them back as a side effect; it doesn't have to.

**Labels that belong to the run and aren't stripped,** found in review. Three items' labels follow a setting of the run, not a menu entry, and the table has them as the run had them: Selection > "Switch to Ctrl+Click for Multi-Cursor" (`editor.multiCursorModifier`), View > Appearance > "Move Primary Side Bar Right" (the side bar's position) and View > Appearance > "Custom Title Bar" (the native title bar the run forces). Each runs the right toggle, but its label is wrong for a user whose setting differs. The ADR's strip list doesn't name them and 119's three kinds of annotation have none for a label. Until 119, a multi-root window and a folder window also both show Close Folder and Close Workspace.

**Not run here:** the nested run with the panel, and so the panel's stall while the importer preloads the submenus and how GTK draws a stray `action`. They need a Wayland desktop and the applet's package.

