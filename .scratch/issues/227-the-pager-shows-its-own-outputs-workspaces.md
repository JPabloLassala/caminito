# The pager shows its own output's workspaces

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
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
- [ ] `mate-panel-caminito` builds with the patch on Arch and the Debian package takes the same patch.
- [ ] [checklist.md](../../docs/testing/checklist.md) has items for the above.

## For the implementing agent to settle

Recorded in the Done section:

- how the pager learns its panel's output and finds that output's group through libxfce4windowing;
- whether the patch is gated on Caminito (ADR 0011), or needs no gate because one group gives the old result.
