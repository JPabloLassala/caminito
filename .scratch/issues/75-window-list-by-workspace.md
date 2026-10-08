# The window list shows every workspace's windows

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: 65, 66, 70
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

mate-panel's window list shows the windows of the active workspace only, as it does on X11. Today it shows every workspace's windows: neither foreign-toplevel nor ext-workspace says which workspace a toplevel is on, so libxfce4windowing can't tell the panel ([66](66-ext-workspace.md)).

[72](72-session-file-and-first-login.md) saw it in its run on Xvfb: with two windows on workspace 1 and the switcher on workspace 2, the window list on workspace 2 still had both buttons. The switcher itself followed the workspace.

- **What to decide first:** where the workspace of a toplevel comes from. Candidates: a protocol that has it (KDE's `org_kde_plasma_window_management`, which libxfce4windowing doesn't read; a later version of ext-foreign-toplevel or cosmic's toplevel-info), Caminito closing and offering toplevel handles as the workspace changes, so each panel sees the active workspace's windows only, or a carried patch to `mate-panel-caminito`.
- **The applet's `display-all-workspaces` setting** must still be able to show them all.

## Acceptance criteria

- [ ] In a session, a window on workspace 1 has no button in the window list on workspace 2, and has it again on workspace 1.
- [ ] With the applet set to show all workspaces, every window has a button.

## Notes

- **The milestone is a proposal.** 72 was to decide between M5 and M6. M6 is proposed because the session logs in, the list works and loses no window, and M5's exit ([17](17-milestone-slicing.md)) asks only that the window list is shown. The developer hasn't confirmed it.
- **Resolved on 2026-10-05 with both criteria unticked.** The code, the carried patch, the ADR and the checklist item are in. Neither criterion was seen: the machine this was done in has no libxfce4windowing, so `mate-panel-caminito` wasn't built, and nothing was installed there to run a session. The checklist's *The window list shows the active workspace's windows* is both criteria, tagged M6 as the ticket is; it moves with the ticket if the developer says M5.
- **Decided first: Xfce's own protocol,** which isn't among the ticket's candidates ([ADR 0013](../../docs/adr/0013-window-workspaces-through-xfce-foreign-toplevel-protocol.md)). libxfce4windowing 4.20.7 binds `xfce_foreign_toplevel_manager_private_v1` when a compositor has it, and takes each window's workspace from there, so `xfw_window_get_workspace` answers with no change to the library. The ticket's three candidates are rejected in the ADR.
- **Caminito's side** is `protocols::xfce_foreign_toplevel`, written from the XML, which is vendored in `caminito/resources/` at the commit libxfce4windowing 4.20.7 is built from. It sends the workspace only:
  - a toplevel's Xfce handle enters the client's ext-workspace handle for the window's workspace, and leaves it when the window moves, each ended with the wlr handle's `done`;
  - a new Xfce handle gets a `done` even with nothing to say, since libxfce4windowing shows a window only after it;
  - it's refreshed before ext-workspace's refresh and again after it, so a toplevel leaves a workspace that's going while the client still has its handle, and enters a new one once the client has that;
  - no extra state or icon is sent, the requests that change a toplevel are ignored, and an icon's pixels fail;
  - the global is privileged, as the other panel protocols are (68).
- **The panel's side** is `mate-panel-caminito`'s `0003`, at `pkgrel` 2. The window list never asked for a window's workspace, so the protocol alone changes nothing. With the patch a window has a button while it's on the active workspace or pinned, or when `display-all-workspaces` is set, and the list follows each window's workspace and the group's active workspace. It isn't gated (ADR 0011). The choice in the preferences, which the panel greyed out on Wayland, can be made again, on any compositor; without the protocol it does nothing.
- **Written before the code:** the test client's Xfce module and the tests, seen to fail for want of the global. Two tests were sharpened afterwards so that each of the two refresh calls fails a test when it's taken out.
- **Checked on the developer's machine (x86_64):** the tests in `tests/xfce_foreign_toplevel.rs` and the full suite on a private bus (`dbus-run-session`); without one, the twelve tests that own Caminito's D-Bus names fail there, as on `master`. For the patch: `gcc -fsyntax-only -Wall` on `wayland-backend.c` and `window-list.c` against libxfce4windowing 4.20.7's headers, with no warning from the patch; the series applies to `58ac6c1` with no offset; `namcap PKGBUILD` prints nothing and `makepkg --verifysource` passes the sums.
- **The review found a bug in the patch, fixed before the merge:** the list asked each workspace whether it's active, and Caminito sends the workspaces' states in order, so on the way from workspace 2 to 1 the group said workspace 1 was active while 2 still said so too, and 2's buttons stayed. The list now asks the group which workspace is active. No test covers it: the panel's logic has none here, and the checklist's step 2 goes both ways for it.
- **Known and left:**
  - **Sticky isn't sent.** A sticky window is on the one workspace policy has it on until M8 (17), when the protocol's `sticky` state, which the patch already reads as pinned, can go out.
  - **The order of the buttons isn't kept across workspaces,** nor one made by dragging: a button is destroyed and made again.
  - **`move_to_workspace` is ignored** with the protocol's other requests. Nothing in the panel sends it.
  - **The protocol is private to Xfce** and may change (ADR 0013). A libxfce4windowing update is when to compare the vendored XML.
  - **The panel's window menu** already asks `xfw_window_get_workspace`, so it should sort by workspace now. Not seen.
- **Review left as is:** the tests move a window with `Policy::move_to_workspace`, since no binding or request does it yet, and read policy's state as 65's tests do; the workspace id is still a `String` (66); the Xfce module sends the wlr handle's `done` itself, as the protocol has it.
