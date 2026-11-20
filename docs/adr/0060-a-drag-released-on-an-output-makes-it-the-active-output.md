# 60. A drag released on an output makes it the Active output

Date: 2026-11-20
Status: Accepted. Qualifies [ADR 0032](0032-each-output-has-its-own-workspaces.md).

## Context

A tab dragged out of Firefox and dropped on another output opens its new window on the output Firefox was on. Firefox on Wayland uses no `xdg-toplevel-drag`: a drop on nothing reaches it as a cancelled drag, and it opens a new toplevel with no position, and since 2026-09 with no idea where the drop was. Caminito puts that window where any new window goes. A drag is pointer motion and a release, not a press, so ADR 0032's Active output never moves to the output it ends on.

ADR 0032 also has two rules for the output a new window opens on. With Per-output workspaces it is the Active output. With the key off, Spanning workspaces, it is the focused window's, which is Marco's. After a drop, focus is still on the window the drag came from, so with the key off no change to the Active output would bring the tab's window over.

Other compositors differ. KWin, Mutter, Hyprland and COSMIC open a new window on the pointer's output, because their active output follows the pointer. niri and Sway leave it on the focused output, as Caminito did. None special-cases a window opened after a drag.

## Decision

- **A drag released on another output makes that output the seat's Active output**, whatever it's dropped on: a window, a panel, caja's desktop, the wallpaper or nothing. It is what a press on that output's panel or wallpaper does, and the focus stays where it is. It applies to every client, Wayland and X11.
- **A drag cancelled, or given up by its Wayland client before the release, changes nothing.** GTK gives a drag up at an Escape by destroying its source, and Caminito asks at the drop whether the source is alive.
- **A new window opens on the Active output with the key off too**, as with it on. Marco's focused window's output is no longer used.
- **The window is placed on that output as any new window is**: first fit or cascade, centred with `center-new-windows`. Caminito doesn't tie the window to the drag, so it isn't put under the pointer.

## Consequences

- A tab dropped on another output opens its window there, with no rule for Firefox.
- With the key off, a new window opened after a press on another output's panel or wallpaper opens there, and not on the focused window's output as in Marco. ADR 0032's "with the key off Caminito is as before" holds for every other behaviour, but not this one.
- Any drag released on another output moves the Active output, including a file dropped on a window there. A click there does the same.
- An X client's drag cancelled with Escape still moves the Active output at the release: Smithay's XDND bridge keeps no source that Caminito can ask about.
- The window doesn't follow the pointer during the drag. That needs `xdg-toplevel-drag-v1`, which Firefox and Smithay don't support yet.
- Rejected:
  - the pointer's output for every new window (it breaks ADR 0032's rule that moving the pointer alone doesn't change the Active output);
  - remembering the drop and putting the drag source client's next window there for a second or so (a guess at timing, and it misses a window a client opens later);
  - keeping Marco's rule with the key off (a tab dragged to another output would still open on the first, and so would an app started from the other output's panel).
