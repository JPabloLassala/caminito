# "Not responding" and Force Quit

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 167
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's handling of a window that doesn't close ([05](05-v01-marco-feature-cut.md), Marco's `delete.c`).

- **The ping on close:** when a window is asked to close (Alt+F4, the Frame's button, the window menu), Caminito pings its client: xdg_wm_base's `ping` for a native window, `_NET_WM_PING` for an XWayland window that lists it in `WM_PROTOCOLS`.
- **No answer in time:** Marco's zenity "not responding" dialog shows ([167](167-zenity-dialogs.md)'s helper), offering to wait or to force the application to quit. An answer to the ping, or the window closing, takes the dialog away.
- **Force Quit:** kills the client's process. The pid comes from the Wayland client's socket credentials, or from `_NET_WM_PID` for an XWayland window; with no pid an XWayland client is cut off with `XKillClient`. [95](95-alt-f1-and-alt-f2.md) dropped the panel's `KILL_DIALOG` for this.
- **Caminito's own clients and Xwayland itself** are never killed.

## Acceptance criteria

- [ ] Headless tests through the fixture, written before the code, with the fixture's driven clock and [167](167-zenity-dialogs.md)'s zenity stand-in: a native client that answers the ping, and one that doesn't, getting the dialog; an X11 client that doesn't answer `_NET_WM_PING`; Force Quit killing a child process by its pid; an X11 client with no `_NET_WM_PID` cut off; the dialog taken away when the window closes.
- [ ] The keyless section's "not responding" and Force Quit rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: a stopped program (`kill -STOP`) closed with Alt+F4, natively and under X11, then Force Quit from the dialog. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the timeout, the dialog's arguments, and what Marco does when the dialog itself is closed.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Pinging at other times** (on focus, periodically), as some compositors do to grey a window: proposed, no, only on close, as Marco does.
- **Several clients sharing a pid** (one process with several Wayland connections, or a Flatpak's helper): proposed, Force Quit kills the pid, and the dialog says which application, as Marco does.
