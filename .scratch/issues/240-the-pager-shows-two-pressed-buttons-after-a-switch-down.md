# The workspace switcher shows two pressed buttons after a switch to a lower workspace

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What happens

Found on 2026-10-13 by ticket [227](227-the-pager-shows-its-own-outputs-workspaces.md)'s nested check (`.scratch/issues/227-checks/nested-pager.sh`), which reads the switcher's buttons through AT-SPI. With the third workspace active, a click on the first button switches Caminito to the first workspace, and the switcher then shows the first and the third button pressed. A switch to a higher workspace shows one. It is the same with `workspaces-per-output` on and off, so it isn't per-output workspaces' doing, but ticket 229's check of "each pager shows its own monitor's active workspace" would meet it.

The cause, from the source and not from a debugger: mate-panel's Wayland pager (`applets/wncklet/wayland-workspace.c`, `rebuild_ui`) presses a button when its workspace's own state has `XFW_WORKSPACE_STATE_ACTIVE`. Caminito sends the workspaces' states in order. libxfce4windowing 4.20.7 (`xfw-workspace-wayland.c`) tells the group of its new active workspace as that workspace's state arrives, and the pager builds its buttons then. A workspace with a higher number that was active still says it is at that moment, and nothing tells the pager when it stops: the group has another active workspace by then, so it signals nothing. Patch `0003` met the same thing in the window list, and asks the group.

## What to build

A carried patch to mate-panel, an upstream fix with no gate (ADR 0011), kept in `packaging/patches/mate-panel/` (ADR 0030):

- `rebuild_ui` and `wayland_workspace_get_active_index` take a workspace as active when its group's active workspace is that workspace, as `tasklist_shows_window` of patch `0003` does, and fall back to the workspace's own state where it has no group.

## Acceptance criteria

- [ ] `nested-pager.sh` shows one starred button after each click, up and down, with the key on and off.
- [ ] `mate-panel-caminito` builds with the patch, at the next `pkgrel`.
- [ ] The scroll wheel over the switcher moves from the workspace that is active, after a switch down.

## For the implementing agent

The check was not run against the panel without patch `0012`: the OrbStack machine stopped answering before that run. The code that decides the pressed button is upstream's and patch `0012` doesn't touch it.
