# Whether keybinding labels and recent files can be had

Type: research
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../map.md)

## Question

Two things were given from memory and never run. Check both against VS Code 1.140 with a throwaway extension.

- **Keybinding labels:** can an extension read the default keybindings, for instance by opening `vscode://defaultsettings/keybindings.json`? Where is the user's `keybindings.json`, per profile, and can it be read and watched? By what rule does VS Code pick the one label its own menu shows for a command with several bindings? How are the labels written for dbusmenu's `shortcut` property?
- **Recent files:** does `_workbench.getRecentlyOpened` answer an extension, with what shape, and is there an event when the list changes? Which public commands open a recent folder, workspace or file, and clear the list?

## Answer

Full findings are on branch `research/vscode-keybinding-labels-and-recent-files` (commit ba89a4b), in `research/vscode-keybinding-labels-and-recent-files.md`. Run with a throwaway extension in VS Code 1.140.0 under Xvfb, with a temporary profile and Xvfb's keyboard layout, and read from the source at the 1.140.0 tag. Both can be had.

- **The default keybindings can be read.** `workspace.fs.readFile` on `vscode://defaultsettings/keybindings.json` gave 1200 entries of `{key, command, when?, args?}`, extensions' bindings among them, with keys already resolved for the keyboard layout. It's an internal, with no event when it changes.
- **The user's file** is `<user data>/User/keybindings.json`, or `<user data>/User/profiles/<id>/keybindings.json` in another profile. It can be read and watched (a change came within about 80 ms, twice).
- **VS Code doesn't tell an extension its window's profile.** `context.globalStorageUri` is always the default profile's. `<user data>/User/globalStorage/storage.json` gave the right profile for a folder window, and it's private state. `workbench.action.openGlobalKeybindingsFile` shows the file's place but opens a tab.
- **The label VS Code's own menu shows** (read, not run): of a command's bindings in resolver order, the only one; else the last whose `when` holds in the window's root context; else the last. The user's removals apply first. An extension can't evaluate the `when`.
- **dbusmenu's `shortcut`** is `aas`: an array for each key press, modifiers (`Control`, `Alt`, `Shift`, `Super`) and then an exact GDK key name (`comma`, `Page_Up`, `Return`). A name GTK can't parse shows no shortcut. The research has the table for every key in the defaults. The applet (upstream 25.04 read, not the patched package) shows only the first press of a chord, and GTK writes "Shift+Ctrl+P".
- **`_workbench.getRecentlyOpened` answers,** with the whole list of folders, workspaces and files, the most recent first. Cutting it to ten and ten and labelling it is the extension's to do. No event tells of a change. VS Code's own menubar refreshes it when the window is focused.
- **Opening and clearing:** a folder or workspace with `vscode.openFolder` (public, read not run). A file with `_files.windowOpen` (internal), which moves it up the list as the menu does; the public `vscode.open` opens it and leaves the list as it was. `workbench.action.clearRecentFiles` clears, behind VS Code's own confirmation. More… is `workbench.action.openRecent`.
- **Not checked:** any fork, and the user's own keyboard layout.

### Handed on

- **[08](08-what-the-menu-shows-beyond-structure.md):** how the profile is found (read `storage.json`, the default profile only, or the command that opens a tab); the label rule ("the last binding with no `when`, else the last" against a `when` evaluator); a chord (no shortcut, in the label's text, or a change to the patched applet); whether "Shift+Ctrl+P" will do; the two underscore commands; when to refresh (window focus, `AboutToShow`, a change of extensions, the user's file).
- **The spec:** an `--extensionTestsPath` run differs from real use: confirmation dialogs confirm themselves, and `_files.windowOpen` opens a new window.
