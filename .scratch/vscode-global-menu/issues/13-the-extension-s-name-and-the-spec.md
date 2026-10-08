# The extension's name, and the spec

Type: grilling
Status: resolved
Blocked by:
Map: [VS Code's global menu](../map.md)

## Question

The map's last fog, and then its destination.

- **The extension's name in the glossary.** "Caminito integration" was the working name. Does it do anything besides the menu, now or as a place for later things? The name decides the extension's id, its directory in this repository, the name of its own setting from ADR 0021, and the bus name that makes the offer once ([12](12-when-a-claim-is-made.md)). The package is already `caminito-vscode-git` (ADR 0022) and needn't match.
- **The spec's outline:** its sections, and how the work is cut between the extension and Caminito, and into tickets that can each be run.
- **Where the spec lives,** and whether the map is closed when it's written.

## The session ends by writing the spec

It collects what the tickets handed on to "the spec". The larger items:

- the claim's rules as [12](12-when-a-claim-is-made.md) leaves them, with ADR 0018 amended;
- the menu table and its canary (ADR 0019), and what the menu shows (ADR 0020);
- the offer's wording and its two buttons, and the extension's own setting (ADR 0021);
- the packaging (ADR 0022): the package's directory, the hook and the install script, the `optdepends` line in `caminito-git`, the line for linking a tarball editor by hand;
- how the extension copes with an editor older than its `engines.vscode` floor, which a built-in doesn't check;
- the settings documented with ADR 0017's.

And what a ticket of the spec has to run, since nobody has:

- a link in an editor's real built-in directory, whether a user can disable a built-in, and what an open window shows after an update changes the files under it ([11](11-how-the-package-installs-the-extension.md));
- a fork, a remote window, a menu of more than one submenu, and the panel's stall while a real menu's submenus are preloaded ([10](10-thin-end-to-end-slice.md)).

## Answer

Decided on 2026-10-08. The spec is [spec.md](../spec.md), and the map is closed. The developer took the recommended answer to all nine questions, in two rounds. Nothing was run. One glossary term was added, and [ADR 0021](../../../docs/adr/0021-vs-code-s-in-window-menubar-is-the-user-s-to-hide.md) and [ADR 0022](../../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md) were amended with the names and with the rule for an old editor.

### The name

- **The extension does the menu and nothing else.** No second feature is known, and Alt+F reaching the panel would still be the menu. Its term in [CONTEXT.md](../../../CONTEXT.md) is **global menu extension**, beside the global menu applet. "Caminito integration" is avoided.
- **Its directory is `caminito-vscode/`,** named as the package is.
- **Its id is `caminito.global-menu`,** shown as "Caminito Global Menu".
- **Its setting is `caminito.globalMenu.offerToHideMenuBar`.**
- **The bus name that makes the offer once is `org.mate.Caminito.GlobalMenuExtension.pid<pid>`,** with the main process's pid. An element of a bus name can't start with a digit.
- **The package installs it at `/usr/share/caminito-vscode/global-menu`,** linked into each editor as `caminito.global-menu`. The package's wider name stays, and it could carry a second extension.

### The spec

- **It's `spec.md` beside the map.** Its tickets will be filed in `.scratch/global-menu/issues/` from 115, so that a ticket's number stays the only one in a commit's subject. Filing them is a later run and not this ticket's.
- **Its shape is the repository's spec template, split by side:** the problem, the solution, a short list of user stories, the implementation decisions under Caminito, the extension, packaging and documentation, the testing decisions, what has to be run, the cut and what's out of scope. Each decision is a gist with a link to its ADR.
- **The offer reads** "The panel now shows this window's menus. Hide the menu bar in the window? Pressing Alt shows it again." It names no editor, so a fork reads right.
- **An editor older than the floor is served with what it has.** The floor is the oldest release with every stable API the extension uses. Below it the extension looks for each API, leaves out what's missing, logs one line and never throws into the host. Doing nothing there was rejected: a lagging fork would lose the whole menu over one API.
- **The extension is TypeScript, bundled with esbuild, tested with node's runner at one seam:** its core against a fake of the `vscode` API and a real private bus with a stand-in for `org.mate.Caminito`. No editor is started in CI.
- **The cut is ten slices:** Caminito's menu claim; the extension serving one fixed menu; the package, early, so that an editor's real built-in directory is run soon; the table; checkmarks and updates; keybinding labels; Open Recent; the offer; the canary; and a verification by hand in a real Session.

### The map

- **It's closed.** What still has to be run is listed in the spec under the slice that runs it, and a result that contradicts an ADR reopens that ADR from the slice's ticket.
