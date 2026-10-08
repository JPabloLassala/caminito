# 19. VS Code's menu table is a snapshot of its own native menubar

Date: 2026-10-08
Status: Accepted

## Context

The extension that serves VS Code's menubar as the global menu (ADR 0018) can't read that menubar at run time, so it carries a table of it. The table can be written by hand, extracted from VS Code's source, or taken from a running VS Code.

Extraction gives every item with its `when` and `toggled` expressions, but it needs the source tree and the type checker, and hand-written cases for what the source registers by code: the View menu's view entries, the Help entries gated on `product.json`, the per-profile submenus. The extension can't evaluate a `when` clause wherever it came from.

With a native title bar, VS Code sends its evaluated menubar to its main process, which writes it as `lastKnownMenubarData` in the profile's `storage.json`. A run under Xvfb with a throwaway profile showed it there about five seconds after start and settled two seconds later: the eight menus in order, command ids, labels with mnemonics, separators and submenus, the parts registered by code included.

The menubar moves little: nothing was added or removed between 1.130 and 1.140, and one or two visible items change in a quarter.

## Decision

- **The table is a snapshot.** A developer command starts the reference build under Xvfb with a native title bar and a throwaway profile, waits until `lastKnownMenubarData` stops growing, and writes the table from it. It takes the snapshot with a folder open and with a multi-root workspace open, and merges the two.
- **The command strips what belongs to the run:** `enabled`, `checked`, the entries of Open Recent (they come from the machine, not the profile) and the profile's name in a label.
- **What a snapshot can't hold is annotated by hand,** in a file keyed by command id: the checkmarks that mirror a setting, the items gated on a `product.json` key, and the items that follow the workspace state. The command fails when an annotation names a command the snapshot doesn't have.
- **The reference build is Microsoft's,** at the version the `visual-studio-code-bin` package is at. It sets every `product.json` entry, so the forks are subsets of it.
- **The table is committed,** in the extension's directory, with the version and commit it was made from. Nothing makes it while a package builds or on the user's machine.
- **One table serves every version and fork.** At start the extension drops each item whose command `commands.getCommands()` doesn't list, then each submenu left empty, and collapses the separators. It gates product-dependent items on the running editor's own `product.json`, never on its name.
- **A newer VS Code is served silently.** The extension writes one line to its log with both versions and tells the user nothing.
- **A weekly canary notices a release.** It downloads the newest stable build, takes the snapshot and compares its structure with the committed table. A difference fails the job and opens or updates an issue with the diff. The table is remade by hand; nothing commits by itself.
- **Titles are English.**

## Consequences

- An item added upstream is missing from the panel's menu until the table is remade. An item removed upstream, or missing in a fork, is dropped.
- An item hidden in both snapshot states is not in the table. The ones known are outside an ordinary editing state: the Zen-mode tabs submenu, Join and Split in Group, the merge-conflict entry.
- An item a fork adds is never shown. Cursor is untested and gets what the tolerance rule leaves. Forks have no canary.
- With a language pack installed, the panel's menu is English and the rest of VS Code isn't. A language pack keys its translations by source module and string key, so the table can gain those two columns later; that was read from the source and not run.
- The table doesn't list the user's profiles under New Window with Profile, shows "Export Profile..." without the profile's name, and has no update item.
- The agent sessions window gets the ordinary menu. An extension can't tell it's in one, and some items may do nothing there.
- `lastKnownMenubarData` is internal. If VS Code stops writing it, the committed table stays and is edited by hand until another route exists.
- Electron under Xvfb in the workflow's container may need its sandbox turned off. If it can't run there, the same command is run locally when the package updates.
- Rejected:
  - Type-checked extraction from the source (a second tool and its special cases, for expressions the extension can't evaluate; the thirty or so items they decide need a hand-made mapping either way).
  - Extraction with the snapshot as its check (both tools for the same table).
  - A table written by hand (245 items, and the snapshot gives them in order for nothing).
  - A table for each VS Code version (a table a month, and unknown versions still need a rule).
  - No menu outside a tested range of versions (the whole menu lost over one item).
  - A table for each fork (the cost the forks were admitted without; Cursor's source can't be read).
  - Making the table on the user's machine (a hidden VS Code started from a hook or from the extension).
  - A notification when VS Code is newer than the table (monthly, for a menu that's almost always right).
  - Detecting the sessions window through a proposed API (`--enable-proposed-api` on every machine for one window type).
