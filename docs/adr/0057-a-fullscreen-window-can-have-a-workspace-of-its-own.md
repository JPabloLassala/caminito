# 57. A fullscreen window can have a workspace of its own

Date: 2026-10-31
Status: Accepted

## Context

The user wants a fullscreen window on a workspace of its own, which they can switch away from and back to, behind a setting. With the setting off, a fullscreen window should stay on its workspace and cover the panel.

ADR 0032 has the count, the names and the Workspace grid the same on every output, and `_NET_WM_DESKTOP` a window's number within its output's set. Two models fit the request:

- **an ordinary number**: the window moves to the next workspace that's empty on its output, and the count grows on every output if none is. The pager, ext-workspace, X11 and the bindings work unchanged, but the other outputs gain an empty workspace for a while, and the numbers change under the user.
- **a workspace outside the grid**, on one output, as a macOS Space is. The numbers and the grid stay as they are, but ADR 0032's invariant gets an exception, and the pager and X11 need an answer for a workspace with no number.

Marco also keeps a fullscreen window above the docks only while it or a transient of it has focus, on any output. With a game on one output, a click on another output brings the panel back over the game.

## Decision

- **A Fullscreen workspace is outside the grid and has no number.** It's on one output and holds one fullscreen window and its transients, as long as the window is fullscreen. The numbered workspaces and their bindings don't change. The key is `org.mate.Caminito`'s `fullscreen-workspaces`, off by default, and it applies to every fullscreen window, because games can't be told apart reliably.
- **It comes right after the window's Home workspace.** From the Home workspace, the workspace to the right is the Fullscreen workspace, the newest first when there are several, and then the grid's next. From a Fullscreen workspace, up and down act as from its Home workspace.
- **It holds nothing else.** Sticky windows don't show on it, nothing can be dragged to it, and a window opened from it goes to the Home workspace, to which the output slides.
- **A focused window's output slides to it.** A window that goes fullscreen without focus gets one, and the output stays where it is.
- **Leaving fullscreen or closing removes it.** If the output was showing it, the output slides to the Home workspace, with the window focused there if it's still open. A fullscreen window doesn't keep its Home workspace alive under `dynamic-workspaces`; if the Home workspace has gone, the window takes the last workspace.
- **Moving the window to a workspace changes its Home workspace.** It stays on its Fullscreen workspace.
- **Turning the key on gives each fullscreen window a Fullscreen workspace, and no output switches. Turning it off returns them to their Home workspaces, still fullscreen.**
- **An unplugged output's Fullscreen workspaces go to the Primary output** with their windows, each with the same-numbered Home workspace, as ADR 0032 has it for other windows. They don't return with the output.
- **X11 clients are told of the Home workspace.** `_NET_NUMBER_OF_DESKTOPS` doesn't count Fullscreen workspaces. The window's `_NET_WM_DESKTOP` is its Home workspace's number, and `_NET_CURRENT_DESKTOP` is that number too while the Active output shows the Fullscreen workspace, so a game still finds itself on the current desktop.
- **ext-workspace offers it in its output's group, without coordinates.** A carried patch shows it in the pager as a small cell after the grid. The Overview's bar and the Workspace popup show it, and activating its window from the Window switcher slides to it.
- **Under Spanning workspaces only its own output shows it.** The other outputs keep showing the Home workspace's number, and a switch to a numbered workspace moves every output, away from the Fullscreen workspace too. This is an exception to one active workspace for every output.
- **A fullscreen window is in the fullscreen layer while it or a transient of it was the last window focused on its output**, with the key on or off. Focus on another output doesn't lower it, and a window focused on its own output does, which brings the panel back. It stays lowered until the fullscreen window has focus again. The hot corners use the same test. It replaces Marco's test of focus anywhere (`get_standalone_layer`, stack.c:272-315).

## Consequences

- ADR 0032's invariant holds for the numbered workspaces only. Whatever walks an output's workspaces has to decide what it does with a Fullscreen workspace, and every later workspace feature is checked with the key on as well, under each of ADR 0032's two behaviours.
- Caminito's stacking differs from Marco's with two or more outputs, and the parity tests have to expect that.
- An X11 client can't tell a Fullscreen workspace from its Home workspace, so a pager that reads X11's properties doesn't show it.
- On any one output, a window's stacking now depends on that output's focus history as well as on the current focus.
