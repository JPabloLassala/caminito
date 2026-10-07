# Rename the project to Caminito

Type: implementation
Status: open
Milestone: none: it gates no milestone
Blocked by: 101
Map: [Camino v0.1](../map.md)

## What to build

The project is called **Caminito**, and nothing is called Camino any more. Decided with the developer on 2026-10-07: Caminito is the better name, it is Argentine (the street in La Boca, and the tango), and as the diminutive of *camino*, "way", it keeps the pun on Wayland.

One name, everywhere. The developer first wanted a shorter command, and settled on `caminito` for it too: it is as long as `hyprland`, the display manager types it, and a second name would have to be explained in every doc and log line.

Two commits, in this order, and then the steps outside the tree.

### 1. The rename

A case-preserving replace over every tracked file: `camino` to `caminito`, `Camino` to `Caminito`, `CAMINO` to `CAMINITO`. Those three spellings are the only ones in the tree, and every one of them is this project. No exceptions, no aliases and no compatibility shims: the developer's is the only install. It covers, among the rest:

- **The programs and crates:** `caminito`, `caminito-displays`, `caminito-locker`, `caminito-gtk-palette`, and the six workspace crates with their directories.
- **D-Bus and gsettings:** `org.mate.Caminito` and everything under it, the schema's file, id and path.
- **The gate of the carried patches:** `MATE_WM_NAME=Caminito`.
- **Environment variables:** `CAMINITO_*`.
- **The session and install paths:** the session entry, "MATE on Caminito", `/usr/share/caminito/xdg`.
- **The user's directories:** `caminito` under `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME` and `$XDG_STATE_HOME`. Caminito doesn't look in the old ones.
- **The packages:** `caminito-git`, `caminito-desktop` and every patched `<pkgname>-caminito`, 101's among them. No `replaces=` for the old names.
- **The carried patches:** replace inside each patch file, then name the file as `git format-patch` would name it from its new subject line (the names are cut at 52 characters, so the longer word moves the cut), refresh the checksums and bump each `pkgrel`. The replace touches only subject lines and added lines, so the hunks stay valid; the commits in the MATE checkouts aren't reworded.
- **The docs:** the README, CONTEXT.md, the coding standards, every ADR and the four ADR file names with the name in them, the testing docs, and these tickets and the map with their directory. The records say Caminito as if it always had; git keeps the old name.
- **The test account:** `caminotest` becomes `caminitotest` in the docs, and the account is renamed to match (below).
- **The logo's file:** `caminito.svg`.
- **The repository's URL,** where the workspace manifest and the package name it.

In CONTEXT.md the term is **Caminito**, and "Camino" joins its _Avoid_ list as the former name. Where the name comes from is one line of the README, not the glossary's.

### 2. The logo

The new logo is the house that is in the tree as the project's icon by then (commit it before the rename if it is still an uncommitted change).

- **A square canvas:** it is 458 by 551, and an icon theme wants a square. Widen the `viewBox` to 551 by 551 with the drawing centred and untouched.
- **No content credentials:** the file carries a C2PA manifest whose hash the new canvas breaks. Strip the block.
- **Installed** in the hicolor theme's scalable apps as `caminito.svg`.
- **The session entry** gets `Icon=caminito`.
- **The README** shows it at its old proportions, by its width and height.
- **caminito-displays keeps `preferences-desktop-display`:** it stands in the Display capplet's place, and that is the icon users find it by.

### 3. Outside the tree, the developer's

- Rename the GitHub repository and point `origin` at it.
- Move `~/.config/camino`, `~/.cache/camino` and `~/.local/state/camino` to their new names. The first has the saved layouts.
- Rename the `caminotest` account on the virtual machine (`usermod -l`) before the next hand check.
- Remove the installed packages with the old names, install the new ones, and delete the old built package files from the packaging directories.
- Delete the untracked `casa.svg` at the root, a copy of the logo.
- Rename the checkout's directory, last: it breaks any worktree's link and the paths that tools key their history on.

## Acceptance criteria

- [ ] A case-insensitive search of the tracked files for `camino` not followed by `ito` finds nothing, file names included.
- [ ] The rename commit is the replace and what follows from it mechanically (file names, checksums, `pkgrel`s, the lock file) and nothing else.
- [ ] `cargo test` passes, and CI is green.
- [ ] Every package builds and installs, with every carried patch applying.
- [ ] The logo's file is square, has no `c2pa` metadata, and is installed where `Icon=caminito` finds it.
- [ ] The developer logs in to "MATE on Caminito" from the display manager: the panel, the desktop and the settings daemon behave as under Camino (so the gate, the bus name and the patches agree), the saved layout is restored from its new directory, and the greeter shows the logo if it shows icons.

## Notes

- **Land it on a quiet master.** The replace touches about 400 files, so every open branch conflicts with it. It waits for [101](101-mate-control-center-camino.md), which adds a patched package, and should go in with no other ticket's branch open; a ticket that starts after it is in the new name from its first commit.
- **The `research/` and `prototype/` branches keep the old name.** They are reference material and are never merged.
- **No ADR:** the rename is costly to undo, but nobody will wonder why the project has its name.
- **`ticket-58-output-management` was dropped** on 2026-10-07, locally and on GitHub, as superseded; its one commit was `0cd91ec`.
