# 13. A window's workspace reaches the panel through Xfce's foreign toplevel protocol

Date: 2026-10-05
Status: Accepted

## Context

mate-panel's window list on X11 shows the active workspace's windows, and every workspace's when `display-all-workspaces` is set. On Caminito it showed them all (72, 75): wlr-foreign-toplevel-management and ext-workspace, which the panel reads through libxfce4windowing (06), don't say which workspace a toplevel is on, and the panel's Wayland window list never asked.

libxfce4windowing 4.20 does have a source for it. It binds `xfce_foreign_toplevel_manager_private_v1`, from xfce-wayland-protocols, when the compositor offers it, asks for an Xfce handle beside each wlr handle, and takes `workspace_enter` and `workspace_leave` from that handle, which name the client's own ext-workspace handles. `xfw_window_get_workspace` then answers, and without the global it answers nothing. The protocol's own text says it's private to Xfce and its compositor, and may change incompatibly without notice. It's under the MIT licence.

ADR 0002 puts a standard protocol first, and a protocol private to Caminito last.

## Decision

- **Caminito offers `xfce_foreign_toplevel_manager_private_v1` version 1**, as a privileged global (68), in `protocols::xfce_foreign_toplevel`, from the XML vendored at the commit libxfce4windowing 4.20.7 is built from. In ADR 0002's order it stands where a standard protocol does: it's what the component's existing library already speaks.
- **Only the workspace is sent.** A toplevel's Xfce handle enters the client's handle for the workspace policy has the window on, and leaves it when the window moves. Its changes are ended with the wlr handle's `done`, and a new Xfce handle gets one `done` even when there's nothing to say, since libxfce4windowing waits for it before it shows the window.
- **The rest of the protocol is taken and ignored.** No extra state and no icon is sent. The requests that change a toplevel (shade, sticky, above, below, move, resize, move to an output or a workspace, highlight) do nothing, and an icon's pixels always fail. A window list asks what it needs through wlr-foreign-toplevel.
- **A carried patch makes the window list use it** (`mate-panel-caminito`'s `0003`): a window has a button while it's on the active workspace or on all of them, or when `display-all-workspaces` is set. It isn't gated on Caminito (ADR 0011), because where the compositor doesn't say where a window is, every window has a button as before. The preferences' choice between the current workspace and all of them, which the panel greyed out on Wayland, can be made on any compositor now, and does nothing on one without the protocol.
- **The refresh pass runs it on both sides of ext-workspace's refresh.** A toplevel leaves a workspace that's going while the client still has the workspace's handle, and enters a new one once the client has that one's.

## Consequences

- The window list follows the workspace in a session with no change to libxfce4windowing, and the panel's window menu, which already asks `xfw_window_get_workspace`, starts to sort windows by workspace too.
- Caminito depends on a protocol whose owner promises no stability. If Xfce changes it, a newer libxfce4windowing may stop binding version 1, and the window list goes back to showing every window; an incompatible change under the same version would be a protocol error in the panel. The vendored XML names its commit, and a libxfce4windowing update is when to compare it.
- A sticky window is told to be on the one workspace policy has it on. Sticky takes effect in M8 (17), and the protocol's `sticky` state, which libxfce4windowing reads as pinned, is for then.
- The window list's buttons are added and removed, not hidden, so their order isn't kept across workspaces, and neither is one the user made by dragging.
- Rejected:
  - closing and offering wlr handles again as the workspace changes (no setting could show every workspace, `activate` couldn't reach a window on another workspace (65), and the window menu would lose the other workspaces' windows);
  - KDE's `org_kde_plasma_window_management` or cosmic's toplevel-info (libxfce4windowing reads neither, so the patch would be to a shared Xfce library, which ADR 0002 rejects, or a second toplevel client in the panel);
  - a carried patch that asks Caminito over D-Bus (ADR 0002's third channel, when the first works; and it would have to tie a wlr handle to a window by title or app id);
  - a protocol of Caminito's own (the same code in Caminito, and a far larger patch than the one that uses what the library already has).
