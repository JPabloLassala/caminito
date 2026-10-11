# "Not responding" and Force Quit

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
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

## Done (2026-10-11)

- **The tests were first.** `caminito/src/tests/not_responding.rs` was written before the code. As the code was written before the first run, the tests were then run with the ping turned off: 23 of them failed. With the ping on they pass.
- **The developer's answers, given before the build.** Pings go only with a request to close, as in Marco. Force Quit kills the pid, so every window and connection of that process goes.
- **The timeout, from Marco at `8522ed1`.** Five seconds, `PING_TIMEOUT_DELAY` (`core/display.c:4369`). It waits on Caminito's clock, as a holder of `deadlines`.
- **The dialog's arguments** (`core/delete.c:75-123`, `core/util.c:535-630`). `zenity --question --title Marco --text <markup> --ok-label _Wait --cancel-label "_Force Quit"`, with the text `<big><b><tt>TITLE</tt> is not responding.</b></big>`, an empty line, and `<i>You may choose to wait a short while for it to continue or force the application to quit entirely.</i>`. `--display` and `--class` are left out, as in 167. The dialog names the window's title, not the application: the ticket's second question said "which application", and Marco's says the title.
- **The title is escaped for zenity's markup, which Marco's is not.** As 167's command is. For the developer to confirm.
- **`WINDOWID` is not set.** Marco sets it to make the dialog transient for the window. zenity 4 is a Wayland client on Caminito, and the variable names an X window.
- **What Marco does when the dialog is closed, and what Caminito does: for the developer to confirm.** Marco kills on status 1 only (`dialog_exited`, `core/delete.c:61-73`), and its comment says that status 1 "means the user pressed Force Quit". The real zenity 4.2.2 was run in the OrbStack machine on Xvfb: Escape exits with status 1 too, so in Marco Escape kills the application. Caminito gives zenity `ZENITY_ESC=2`, which was seen to change Escape's status to 2, so Escape is *Wait* and only the button kills. A close request (Alt+F4 under Marco on Xvfb, and `WM_DELETE_WINDOW`) did not close zenity 4's question at all. The developer was told of this choice during the build; Marco's exact behaviour is the removal of one `.env` line.
- **An answer to the ping takes the dialog away, as the ticket says.** Marco's `delete_ping_reply_func` does nothing, so its dialog stays until the window goes.
- **A close while the dialog shows.** No second dialog. After five more seconds with no answer the dialog gets the focus, as Marco's `meta_window_present_delete_dialog` does. Caminito finds the dialog as the window of zenity's process.
- **A close while a ping has no answer yet keeps that ping.** Smithay sends no second ping to a client that owes an answer, so the five seconds start again and the old ping's answer counts. Marco sends a second ping.
- **Where the ping starts.** `Mapped::close` notes the request and the refresh pass sends the ping. Thus Alt+F4, the Frame's button, the window menu and the window list are all covered through policy's one `close`, with no change to `caminito-policy`.
- **An XWayland window without `_NET_WM_PING`** is taken as one that answered, as in Marco (`core/display.c:4441-4447`): no dialog.
- **Force Quit** (`meta_window_kill`, `core/delete.c:162-200`). `SIGKILL` to the pid: the Wayland client's from its socket's credentials, or `_NET_WM_PID` only if `WM_CLIENT_MACHINE` is this machine, as Marco checks, so a client over `ssh -X` gets no local process killed. An XWayland window's client is always cut off with `XKillClient` too, as in Marco, not only with no pid as the ticket says.
- **"Caminito's own clients" is read as Caminito's own process: for the developer to confirm.** The pid of Caminito and the pid of Xwayland are never signalled. A program that Caminito spawns, such as zenity or the locker, is a process of its own and can be killed if its window does not respond. With a pid that is spared, an XWayland window's client is still cut off.
- **The text is Marco's English**, as 167's. Caminito sets no locale.
- **The review found a fault, now corrected.** A read error on the dialog's stdout was taken for its end, and the wait for zenity could then stop the loop while the dialog still showed.
- **What was run.** In the OrbStack machine: `tests::not_responding` (25 tests, the shared ones on both kinds of window) with `tests::dialogs`, several times; then `parity`, `bindings`, `window_menu`, the two foreign-toplevel modules, `window_actions`, `frames`, `deadlines`, `xdg_shell`, `xwayland`, `accelerators`, `accessx`, `commands` and `debug`; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. The real zenity was shown with the dialog's arguments on Xvfb, and it draws the text and the two buttons.
- **`xwayland_crashing_again_and_again_as_it_starts…` failed beside other X11 modules**, with these tests and also with `window_properties` and `frames` alone. It passed alone three times. It is the test known to fail now and then.
- **What was not run.** The full test suite, at the developer's request. The dialog in a Session with a stopped program: that is the checklist's item *"Not responding" and Force Quit*, for 173's closing run.
