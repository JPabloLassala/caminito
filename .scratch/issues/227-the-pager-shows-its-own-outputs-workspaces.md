# The pager shows its own output's workspaces

Epic: per-output-workspaces
Type: implementation
Status: open: the code is in, and the four checks in a Session with two monitors are the developer's, in 229
Blocked by: 226
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"The panel" in [the spec](../per-output-workspaces/spec.md): a carried patch to mate-panel's Wayland pager (`applets/wncklet/wayland-workspace.c`), kept in `packaging/patches/mate-panel/` once for every distribution (ADR 0030).

- **The pager lists the workspaces of the group its panel's output is in**, in place of every group's workspaces flat. It follows the panel when the panel moves to another output (ticket 209's Display row), and the group when outputs come and go.
- **A click switches that output's workspace**, through the workspace's `activate`.
- Its preferences (patches `0006`, `0007`) keep writing the one count, the names and the `workspace-layout-*` keys.
- **The window list** shows the windows showing on any output, and every workspace of every output with `display-all-workspaces`. If ticket 226 found patch `0003` short of that, it's fixed here.
- With one group holding every output, which is the key off and any other compositor, the pager shows what it shows today.

## Acceptance criteria

- [ ] In a Session with two monitors and a panel with a pager on each: each pager has N buttons, shows its own monitor's active workspace, and switches only its own monitor.
- [ ] A pager on a panel moved to the other monitor shows that monitor's workspaces without a restart.
- [ ] With one panel on the Primary output, its window list has a button for a window showing on the other monitor, and none for a window on a hidden workspace of either.
- [ ] With the key off, the pager and the window list are as today.
- [x] `mate-panel-caminito` builds with the patch on Arch and the Debian package takes the same patch.
- [x] [checklist.md](../../docs/testing/checklist.md) has items for the above.

## For the implementing agent to settle

Recorded in the Done section:

- how the pager learns its panel's output and finds that output's group through libxfce4windowing;
- whether the patch is gated on Caminito (ADR 0011), or needs no gate because one group gives the old result.

## Done (2026-10-13)

- **How the pager learns its panel's output.** The pager's toplevel is its panel's layer surface, and `gtk_layer_get_monitor` gives the output the panel asked for. That is known before the compositor says where the surface is, which `gdk_display_get_monitor_at_window` needs. The pager then takes the group that has a monitor whose `xfw_monitor_get_gdk_monitor` is that output, and lists that group's workspaces for its buttons, its clicks, the scroll wheel and the count and names in its preferences. The applet library now names gtk-layer-shell in its build, which the panel it is loaded into links already.
- **No gate of its own.** The panel names its output only under Caminito (patch `0004`). Where it names none, or no group has the output, the pager lists every workspace, as before. With one group for every output the group's workspaces are every workspace. A compositor that isn't Caminito and has a group for each output keeps the flat list: the pager could guess the output there, and the guess wasn't worth it unchecked.
- **When the buttons are built again.** On the grid's `map`, since gtk-layer-shell maps the panel's surface again for a new output; on `hierarchy-changed`, for a pager moved to another panel; and on a group's `monitors-changed`. The first two go through the idle rebuild that patch `0007` added. That the surface is mapped again was read in gtk-layer-shell's header, and not seen: it is the second criterion's check.
- **The window list is unchanged.** Ticket 226's Session check wasn't run before this ticket, so nothing says patch `0003` is short, and its code compares each window with its own group's active workspace, which is what the spec asks. If 229 finds it short, the fix is a ticket of its own.
- **Tested first: nothing.** There is no seam: the code is C in mate-panel, and Caminito's headless tests have no panel. The check is `.scratch/issues/227-checks/nested-pager.sh`, written after the patch.
- **What ran.** In the OrbStack machine: `makepkg` of `mate-panel-caminito` at `pkgrel` 12 with the twelve patches (on aarch64, with `--ignorearch`), and `nested-pager.sh` with that package's panel, mounted over the installed one in a mount namespace, since pacman would not replace the machine's old `mate-panel-camino` under `camino-git`. With the key on and off: four buttons, a click on the third makes Caminito's active workspace 3, a fifth workspace gets a fifth button, and gdb shows `libwnck-applet.so` in `xfw_workspace_group_list_workspaces`, which only the patched pager calls. No Rust changed, so no cargo test ran.
- **Not run: two outputs.** The nested backend has one output, so the first four criteria are for a Session, in 229. The checklist's item is "Each monitor's workspace switcher is its own, and the window list shows both monitors".
- **Not run: the Debian build.** `tools/debian-build` copies every patch of `packaging/patches/mate-panel/` into `debian/patches/` and its `series`, so the package takes the patch with no other change. Its changelog has a `0caminito2` entry, as Arch has a new `pkgrel`.
- **Found: two pressed buttons.** After a switch to a lower workspace the pager shows the old and the new button pressed. Upstream's code does this with or without patch `0012`, by the source; the run without the patch didn't finish. It is ticket [240](240-the-pager-shows-two-pressed-buttons-after-a-switch-down.md), which now blocks 229.
- **No code review.** The OrbStack machine stopped answering after the nested runs, so a finding could not have been compiled. The patch was read once more by its author, and nothing in it changed after the build and the check.
