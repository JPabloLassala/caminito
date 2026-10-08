# The extension's name, and the spec

Type: grilling
Status: open
Blocked by: 12
Map: [VS Code's global menu](../map.md)

## Question

The map's last fog, and then its destination.

- **The extension's name in the glossary.** "Caminito integration" was the working name. Does it do anything besides the menu, now or as a place for later things? The name decides the extension's id, its directory in this repository, and the name of its own setting from ADR 0021. The package is already `caminito-vscode-git` (ADR 0022) and needn't match.
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
