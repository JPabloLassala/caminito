# 20. VS Code's menu shows only the state its extension can read

Date: 2026-10-08
Status: Accepted

## Context

The extension that serves VS Code's menubar as the global menu (ADR 0018) builds it from a committed table (ADR 0019). The table has the structure. A checkmark, a keybinding label, the entries of Open Recent and whether an item is there at all depend on the running window.

An extension can read settings, the workspace's folders, the default keybindings (a virtual file, internal), the user's `keybindings.json` and the recently opened list (an internal command). It can't read a context key or evaluate a `when` clause, VS Code doesn't tell it its window's profile, and neither the default keybindings nor the recent list has a change event.

The global menu applet's importer fetches the whole tree at once and calls the server synchronously from inside mate-panel, so nothing can be built when a menu is about to show. It draws only the first press of a chord, and GTK writes the modifiers its own way ("Shift+Ctrl+P").

The decision on a chord was reversed on 2026-10-09. A chord at first had no shortcut, since the applet would have drawn its first press alone. A nested run on VS Code 1.140 ([ticket 120](../../.scratch/issues/120-keybinding-labels.md)) showed it: served both presses of Open Folder's, the panel drew "Ctrl+K" where the window's own menubar has "Ctrl+K Ctrl+O". Seeing the items with no shortcut at all, the developer asked for the chord in the panel, as the first version's bar has it. It takes the carried patch this ADR had left to a later ticket.

## Decision

- **A checkmark is shown only where it mirrors a setting.** The annotations file lists them: Auto Save, Column Selection Mode, Status Bar, Minimap, Breadcrumbs, Sticky Scroll, Render Control Characters and Menu Bar as checks, and Render Whitespace, Activity Bar Position, Tab Bar and Editor Actions Position as radio groups. The value is the window's, read with no resource or language scope. Editor Actions Position's Title Bar follows `workbench.editor.showTabs` as well, which is read too.
- **An item whose state can't be read has no checkmark, checked or not.** Full Screen, Zen Mode, Centered Layout, the side bars, Panel, Title Bar, Word Wrap, Panel Position and Alignment are plain items that run their command.
- **Every item is enabled.** An item that passed ADR 0019's drops is shown whatever the editor's state, except the ones the annotations tie to the workspace state (no folder, a folder, a multi-root workspace), which follow the workspace's folders and file.
- **Other extensions' items aren't shown.** Copy As, which only extensions fill, is dropped as an empty submenu, and Share keeps what the table has of VS Code's own. No proposed API is enabled for them.
- **Keybinding labels are computed at run time** from the default keybindings and the user's file. The user's removals apply first, and a binding whose keys a later entry with no `when` gives to another command stops counting. Of what's left for a command the label is the only binding, else the last with no `when`, else the last.
- **The snapshot's labels are the fallback.** If the default keybindings can't be read, the table's 108 labels are used. A command the user's file names then takes the last binding that file gives it, or no shortcut if the file only removes one.
- **The user's file is found through VS Code's `storage.json`:** the profile tied to the window's folder or workspace file, and the default profile's file if that profile uses the default's keybindings. A window with no entry there, an empty one among them, uses the profile `window.newWindowProfile` names, or the default profile. If `storage.json` can't be read or isn't shaped as expected, it's the default profile's. The file is only read. The command that opens the keybindings file in a tab is never run.
- **A chord's shortcut is every press of it.** The extension serves each press in dbusmenu's `shortcut`, and a carried patch to the applet's translator draws them one after another, as VS Code's own menubar writes them ("Ctrl+K Ctrl+O"), in the place of a one-press shortcut. An applet without the patch draws the first press alone, so the patch ships before, or with, the extension that serves chords. A key with no GDK name, such as a bare scan code, has no shortcut, and nor has a chord with such a press. GTK's order of modifiers is left as it is.
- **Open Recent is laid out as VS Code's own:** Reopen Closed Editor, the first ten folders and workspaces, the first ten files, More… and Clear Recently Opened. A label is the path with the home directory as `~`, a workspace and a remote marked as VS Code marks them, cut in the middle at 120 characters.
- **A recent folder or workspace opens with `vscode.openFolder`,** a file with the internal `_files.windowOpen` so that it moves up the list, and with `vscode.open` if that command fails. If the list can't be read, the submenu keeps its fixed items.
- **Changes are pushed, and nothing is built on `AboutToShow`,** which is answered at once. There is no timer. The extension re-reads on these events and sends an update only if the result differs:
  - the window gaining focus: the recent list and the window's profile;
  - a change of the editor tabs, or the return of a command the extension ran to open a recent entry or clear the list: the recent list;
  - a change of an annotated setting: its checkmarks;
  - a change of the user's keybindings file: the labels;
  - a change of the installed extensions: the default keybindings and the list of commands;
  - a change of the workspace's folders: the items that follow them.
- **Updates within a short time are sent as one.** A change of structure is a `LayoutUpdated`; a change of a checkmark or a shortcut alone is an `ItemsPropertiesUpdated`.

## Consequences

- An item that toggles something unreadable never shows whether it's on. Showing a state that may be wrong was judged worse than showing none.
- An item may be shown where VS Code's own menubar would hide or grey it. Its command then does nothing or shows VS Code's own message.
- A checkmark can still be wrong outside an ordinary editing state: Tab Bar follows another setting in Zen mode, which can't be read.
- A label can differ from VS Code's own when a command's later binding has a `when` that holds in the window's root context, or when a binding is taken over by an entry with a `when`.
- The items with a chord (Open Folder, Close Folder, Save All, Keyboard Shortcuts and others) show it, which the first version's bar asks for ("keybinding labels are there"). It costs a fourth carried patch to the applet, in its translator, which other applications' chords pass through too.
- With the stock applet, or a patched one older than that patch, a chord shows as its first press, on a dozen items, each wrong.
- `storage.json`, the defaults file, `_workbench.getRecentlyOpened` and `_files.windowOpen` are internals. Each has a fallback that keeps the menu, with less in it.
- An empty window opened in a profile by hand shows another profile's labels, and so does a window whose profile was switched until it's next focused.
- The default keybindings are written for the keyboard layout in use when they're read. A change of layout shows in the labels only at the next change of extensions or reload.
- That a change of tabs comes with every change of the recent list, the applet's limits in the patched package (they were read at upstream's tag), and the `storage.json` lookup for a workspace file haven't been run.
- A file opened in another VS Code window joins the list when this window is next focused. The list isn't re-read while a panel menu is open.
- Ctrl-clicking a recent entry to open it in a new window, as the in-window menu allows, isn't possible from the panel.
- The Menu Bar checkmark mirrors `window.menuBarVisibility`, which the decision on the in-window menubar may change the meaning of.
- Rejected:
  - A checkmark kept by counting the extension's own toggles (it's wrong after the first toggle made from a keybinding or the command palette).
  - Leaving out the items whose state can't be read (Full Screen and Word Wrap would be gone from the menu).
  - Greying out by a hand-written rule for each `when` (the first version's bar doesn't ask for it, and each rule is a guess at a context key).
  - `--enable-proposed-api` for Copy As and Share (a start-up flag on every machine for two submenus that are most often empty).
  - Copy As from the local side's extensions alone, which needs no flag (an item's `when` can't be evaluated, and a remote window's extensions aren't seen).
  - The snapshot's labels alone (they miss the user's bindings, other extensions' and the keyboard layout).
  - The default profile's keybindings only (wrong labels for every window in another profile).
  - `workbench.action.openGlobalKeybindingsFile` to find the profile (it opens a tab and creates the file).
  - A chord cut to its first press (Ctrl+K on a dozen items, each wrong).
  - No shortcut for a chord, the first decision (a dozen items short of the first version's bar, with nothing to say why).
  - A chord written into the item's label (it isn't aligned with the other shortcuts and breaks the label's own text).
  - Rebuilding on `AboutToShow` (the importer has the whole tree already, and a late reply stalls the panel).
  - A timer (the events cover every source but the keyboard layout and another window's recent files, and neither is worth a wake-up in every window).
  - Re-reading the recent list on focus lost (a panel menu opening takes the focus, and the menu would be replaced as it opens).
