# The pager's workspace count and names

Type: implementation
Status: open
Milestone: M6
Blocked by: 96
Map: [Camino v0.1](../map.md)

## What to build

The workspace switcher's preferences set how many workspaces there are and what they're called, as they do under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Workspace count and names* row, [17](17-milestone-slicing.md)).

- **The carried patch:** under Camino, `mate-panel-camino`'s pager preferences write Marco's `num-workspaces` and `workspace-names` directly. On X11 they ask the window manager through libwnck; ext-workspace is read-only, so on Wayland the controls do nothing or are greyed today. Find which first.
- **Camino's side is done:** it applies both keys live since M2 ([29](29-settings-thread.md)), and ext-workspace adds, removes and renames since [66](66-ext-workspace.md). This ticket adds no code to Camino unless the check finds a gap.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] `mate-panel-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: the count goes from 4 to 6 and back to 2 in the preferences, and the pager and Camino follow; a workspace renamed there shows its name in the pager and in `gsettings`. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE the preferences work as before (the gate, ADR 0011).

## Notes

- **Blocked by 96** because both patches change the pager's preferences, and 96's comes first in the series.
