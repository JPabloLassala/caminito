# Checkmarks and the items that follow the workspace

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [118](118-the-menu-is-vs-codes.md)

## What to build

[ADR 0020](../../../docs/adr/0020-vs-code-s-menu-shows-only-the-state-its-extension-can-read.md), under "What the menu shows" in [the spec](../../vscode-global-menu/spec.md). The panel's menu shows a checkmark where it mirrors a setting, and the items tied to the workspace state come and go with the workspace's folders. Both follow a change without the menu being reopened.

- **The annotations' other two kinds:** a setting's checkmark or radio value, and a workspace state (no folder, a folder, a multi-root workspace).
- **Twelve checkmarks, four of them radio groups,** each read with no resource or language scope. Editor Actions Position's Title Bar follows `workbench.editor.showTabs` as well. Menu Bar is on when `window.menuBarVisibility` is `classic` or `visible`.
- **An item whose state can't be read is a plain item,** and every item is enabled.
- **Pushed updates:** a change of an annotated setting, of the workspace's folders, and of the installed extensions (the list of commands). An update is sent only if something differs.
- **Coalescing:** updates within a short time are sent as one. A change of structure is a `LayoutUpdated`, and a change of a checkmark alone is an `ItemsPropertiesUpdated`. If [120](120-keybinding-labels.md) or [121](121-open-recent.md) was built first, the coalescing is there already.

## Acceptance criteria

- [ ] Through the seam: each of the twelve reads its setting, and a radio group has one item on.
- [ ] Through the seam: a change of an annotated setting sends one `ItemsPropertiesUpdated`, and a change that leaves the result as it was sends nothing.
- [ ] Through the seam: adding a second folder shows the multi-root items with one `LayoutUpdated`, whose revision grows.
- [ ] Through the seam: several changes at once are sent as one update.
- [ ] Through the seam: installing an extension that adds a command the table names brings its item back.
- [ ] Nothing is built on `AboutToShow` and there's no timer.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Whether a setting read with no scope is what VS Code's `config.*` key holds.
- [ ] How the applet draws a radio item.
- [ ] What a `LayoutUpdated` does to a panel menu that's open.
- [ ] Whether an `ItemsPropertiesUpdated` sent within 100 ms of a layout reply has to be sent again. The importer drops them for that long.
