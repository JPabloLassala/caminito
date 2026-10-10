# Marco's zenity dialogs: a failed command, and `rename-workspace`

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 149
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The helper dialogs Marco shows with zenity ([05](05-v01-marco-feature-cut.md)), with the same arguments, so that they look and read as Marco's.

- **A failed command:** a `run-command-*` binding whose command is empty, or fails to start, shows Marco's error dialog with its message, in place of today's log line alone ([149](149-window-and-workspace-actions.md)).
- **`rename-workspace`:** shows Marco's zenity entry dialog with the active workspace's name, and writes the answer to `org.mate.Marco.workspace-names name-N`, which the pager then shows ([97](97-pager-count-and-names.md)). Cancelling leaves the name.
- **zenity** becomes a dependency of `caminito-git`, packaged by [173](173-packaging-and-closing-m8.md).
- **The dialogs are clients like any other:** a zenity window takes the focus as a new window does, and nothing waits for it.

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code, with a stand-in for zenity on `PATH` that records its arguments and answers: an empty command and a missing program each run it with Marco's arguments; `rename-workspace` writes the answer to the right `name-N`, and nothing on cancel.
- [ ] `rename-workspace`'s row in `docs/parity.md` says honoured, and the keyless section's command error dialog too, with the tests above.
- [ ] The checklist has an item, tagged M8: `run-command-1` bound with no command, and `rename-workspace` bound and used, with the pager showing the new name. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: zenity's exact arguments for each dialog (title, text, icon, the entry's initial text), the messages and their translations, and which failures Marco reports.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **zenity missing:** proposed, the failure is logged once and nothing shows, as Marco does if it does the same.
