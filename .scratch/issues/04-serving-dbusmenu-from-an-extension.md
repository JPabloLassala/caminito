# How an extension host can serve a dbusmenu

Epic: vscode-global-menu
Type: research
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../vscode-global-menu/map.md)

## Question

The extension has to serve `com.canonical.dbusmenu` on the session bus from VS Code's extension host, as a `.vsix` with no install step.

- **A JavaScript D-Bus library**, bundled: which are maintained (`dbus-next` and its forks, others), do they need native modules, do they run in the extension host's Node and Electron versions of 1.140, and can they serve the interface with its `a{sv}` and `(ia{sv}av)` types?
- **A helper program** the extension starts and talks to over stdio: what it would cost against the library, and whether Caminito's own packaging could ship it.
- **The local side:** how an extension is made to run on the local machine when the window is attached to SSH, a container or WSL (`extensionKind: ["ui"]`), and what that takes away from it.
- **What vala-panel-appmenu's dbusmenu importer asks of a server:** the methods and signals it uses (`GetLayout`, `AboutToShow`, `Event`, `LayoutUpdated`, `ItemsPropertiesUpdated`), so the smallest server that works is known.
- **Activation:** which activation event starts the extension with every window, and what a window reload does to its bus connection.

## Answer

Full findings are on branch `research/vscode-serving-dbusmenu-from-an-extension` (commit a99397d), in `research/vscode-serving-dbusmenu-from-an-extension.md`. The runs used VS Code 1.140's own binary as Node (`ELECTRON_RUN_AS_NODE=1`, Node 24.21.0, Electron 43.7.3), not a live extension host, which is still untried.

- **A bundled JavaScript library is enough.** Five were run, each as one minified file of 158 to 207 kB with no native module, and each owned a name and served `a{sv}` and `(ia{sv}av)` to `busctl`, `gdbus` and vala-panel-appmenu's own importer (tag 25.04, compiled into a headless driver), which built the right menu and delivered clicks.
- **Recommended: `@jellybrick/dbus-next`** 0.11.4 (released 2026-09-27): maintained, the fewest dependencies, the lowest Node floor (12.20, which matters for the forks). Its `Interface.configureMembers()` registers nothing, and deleting three fields in the subclass's constructor works around it. `dbus-native` 0.16.1 is the fallback (Node 22.12 or later). `dbus-next` 0.10.2 is dead.
- **A helper program** would cost a second program and a protocol, a binary for one architecture and a process for each window, and the `.vsix` would depend on a file outside it. Caminito's packaging could ship one (`helpers/caminito-gtk-palette` is the precedent). Its one gain: it can answer the panel while the extension host is busy.
- **The importer's calls block the panel.** `Event`, `AboutToShow` and a depth-0 `GetLayout` are synchronous with a 25 s timeout, and the applet runs inside mate-panel. A reply held 2 s stalled the importer 2 s. So `Event` must be answered before the command runs, and a busy extension host freezes the panel.
- **The smallest server that works:** `GetLayout`, `Event` and `AboutToShow`, with `LayoutUpdated` (the revision must grow) and `ItemsPropertiesUpdated` (dropped for 100 ms after a layout reply). No properties and no other method are used.
- **The importer fetches the whole tree at once,** so `AboutToShow` is no place to build a submenu late. A change, the recent files among them, is pushed with `LayoutUpdated`.
- **Property order matters:** `label` sent before `children-display` gave submenu items a stray `action`. Alphabetical order avoids it.
- **The manifest:** `extensionKind: ["ui"]` alone (an extension with a `main` and no kind runs on the remote side), `capabilities.untrustedWorkspaces.supported: true`, `activationEvents: ["onStartupFinished"]`. Running on the local side takes away Node access to the remote machine and keeps `executeCommand`, which is all the menu needs.
- **A reload, a restart or a crash of the extension host ends its process,** and its bus names go at once. The importer then empties the menu. The new host has a new unique name, and the two may overlap for a moment (read, not run).

### Handed on

- **[06](06-channel-between-extension-and-caminito.md):** a unique name or a well-known one for each window (a well-known name comes back after a reload with no new registration); whether a frozen panel under a busy extension host is accepted, or answered by a helper, by Caminito serving the dbusmenu itself, or by a carried patch that makes the importer's calls asynchronous.
- **[08](08-what-the-menu-shows-beyond-structure.md):** rebuilding on `AboutToShow` is out; changes are pushed.
- **[10](10-thin-end-to-end-slice.md):** a run in a live extension host, the stall under a busy host, and how GTK draws the stray action.
- **The spec:** the manifest lines, the implementation rules above, and reporting the `configureMembers` bug to `JellyBrick/node-dbus-next`.
