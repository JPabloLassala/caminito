# The window list shows every workspace's windows

Type: implementation
Status: open
Milestone: M6
Blocked by: 65, 66, 70
Map: [Camino v0.1](../map.md)

## What to build

mate-panel's window list shows the windows of the active workspace only, as it does on X11. Today it shows every workspace's windows: neither foreign-toplevel nor ext-workspace says which workspace a toplevel is on, so libxfce4windowing can't tell the panel ([66](66-ext-workspace.md)).

[72](72-session-file-and-first-login.md) saw it in its nested run: with two windows on workspace 1 and the switcher on workspace 2, the window list on workspace 2 still had both buttons. The switcher itself followed the workspace.

- **What to decide first:** where the workspace of a toplevel comes from. Candidates: a protocol that has it (KDE's `org_kde_plasma_window_management`, which libxfce4windowing doesn't read; a later version of ext-foreign-toplevel or cosmic's toplevel-info), Camino closing and offering toplevel handles as the workspace changes, so each panel sees the active workspace's windows only, or a carried patch to `mate-panel-camino`.
- **The applet's `display-all-workspaces` setting** must still be able to show them all.

## Acceptance criteria

- [ ] In a session, a window on workspace 1 has no button in the window list on workspace 2, and has it again on workspace 1.
- [ ] With the applet set to show all workspaces, every window has a button.

## Notes

- **The milestone is a proposal.** 72 was to decide between M5 and M6. M6 is proposed because the session logs in, the list works and loses no window, and M5's exit ([17](17-milestone-slicing.md)) asks only that the window list is shown. The developer hasn't confirmed it.
