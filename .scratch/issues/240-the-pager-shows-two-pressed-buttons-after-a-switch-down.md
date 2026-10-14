# The workspace switcher shows two pressed buttons after a switch to a lower workspace

Epic: per-output-workspaces
Type: implementation
Status: resolved
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What happens

Found on 2026-10-13 by ticket [227](227-the-pager-shows-its-own-outputs-workspaces.md)'s nested check (`.scratch/issues/227-checks/nested-pager.sh`), which reads the switcher's buttons through AT-SPI. With the third workspace active, a click on the first button switches Caminito to the first workspace, and the switcher then shows the first and the third button pressed. A switch to a higher workspace shows one. It is the same with `workspaces-per-output` on and off, so it isn't per-output workspaces' doing, but ticket 229's check of "each pager shows its own monitor's active workspace" would meet it.

The cause, from the source and not from a debugger: mate-panel's Wayland pager (`applets/wncklet/wayland-workspace.c`, `rebuild_ui`) presses a button when its workspace's own state has `XFW_WORKSPACE_STATE_ACTIVE`. Caminito sends the workspaces' states in order. libxfce4windowing 4.20.7 (`xfw-workspace-wayland.c`) tells the group of its new active workspace as that workspace's state arrives, and the pager builds its buttons then. A workspace with a higher number that was active still says it is at that moment, and nothing tells the pager when it stops: the group has another active workspace by then, so it signals nothing. Patch `0003` met the same thing in the window list, and asks the group.

## What to build

A carried patch to mate-panel, an upstream fix with no gate (ADR 0011), kept in `packaging/patches/mate-panel/` (ADR 0030):

- `rebuild_ui` and `wayland_workspace_get_active_index` take a workspace as active when its group's active workspace is that workspace, as `tasklist_shows_window` of patch `0003` does, and fall back to the workspace's own state where it has no group.

## Acceptance criteria

- [x] `nested-pager.sh` shows one starred button after each click, up and down, with the key on and off.
- [x] `mate-panel-caminito` builds with the patch, at the next `pkgrel`.
- [x] The scroll wheel over the switcher moves from the workspace that is active, after a switch down.

## For the implementing agent

The check was not run against the panel without patch `0012`: the OrbStack machine stopped answering before that run. The code that decides the pressed button is upstream's and patch `0012` doesn't touch it.

## Done (2026-10-13)

- **Tested first: the check, against the panel without the fix.** `nested-pager.sh` with ticket 227's package (`pkgrel` 12) showed `3*, 2, 1*` after the click on the first button, with the key on and off. That run is the one ticket 227 could not make. There is no seam for a test in the code: the fix is C in mate-panel.
- **Patch `0013`: the group says which workspace is active.** A new `workspace_is_active` asks the workspace's group for its active workspace, and reads the workspace's own state only where it has no group. `rebuild_ui` and `wayland_workspace_get_active_index` use it. No other place in `wayland-workspace.c` reads the state. A group that has no active workspace presses no button, as patch `0003` shows no window there.
- **Found: the wheel did nothing over the Wayland switcher.** The scroll check showed that the panel got the `wl_pointer` axis events and the workspace did not change, with and without patch `0013`. The applet's handler is on the grid, which has no window, and a toggle button does not ask for scroll events, so GDK gave the event to the panel's window. This is upstream's code, and not per-output workspaces' doing.
- **Patch `0014`: the buttons ask for scroll events.** It was not in "What to build". It is here because the third criterion can't pass without it, and it is a patch of its own, so it can be taken out alone. It is an upstream fix with no gate (ADR 0011). It asks for `GDK_SCROLL_MASK` only: the applet's handler returns on a smooth event, so a touchpad's scroll still does nothing, as on upstream.
- **The wheel's start was never wrong.** A trial build with the scroll mask and without patch `0013` went from the first workspace to the second after a switch down, with two buttons pressed. The old workspace's state is wrong only while the buttons are built, and the handler reads it later. Patch `0013` makes the two agree by the same test.
- **What ran.** In the OrbStack machine: `makepkg` of `mate-panel-caminito` at `pkgrel` 13 with the fourteen patches (on aarch64, with `--ignorearch --nocheck`); then, with that package's panel mounted over the installed one (`PKG=`), `nested-pager.sh` (one starred button after each click, up and down, key on and off) and the new `.scratch/issues/240-checks/nested-scroll.sh` (click 3, click 1, one notch down: workspace 2, one starred button). The wheel is `xdotool click 5` on the outer display. Caminito's binary was ticket 227's (`target-226`): no Rust changed, so no cargo build and no cargo test ran.
- **Not run: a real Session, two outputs, the Debian build.** The nested backend has one output; the two monitors are ticket 229's, whose checklist item now asks for `pkgrel` 13, one pressed button for each switcher, and one workspace for each notch. `tools/debian-build` takes every patch of `packaging/patches/mate-panel/`; the changelog has a `0caminito3` entry.
- **The review.** One finding was taken: the Debian changelog called the two upstream fixes carried patches (ADR 0011). Left as they are: `nested-scroll.sh` copies `nested-pager.sh`'s setup, and `workspace_is_active` repeats patch `0003`'s test, since each patch can go upstream alone.
