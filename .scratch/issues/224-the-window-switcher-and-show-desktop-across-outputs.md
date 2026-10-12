# The Window switcher and Show Desktop across outputs

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"The Window switcher and Show Desktop" in [the spec](../per-output-workspaces/spec.md).

- **`switch-windows`, `switch-group` and `switch-panels`** list the windows showing on every output, most recently used first. The `-all` actions list every workspace of every output, with the showing ones first.
- **Show Desktop** hides the windows of every output's active workspace, and each of those workspaces keeps it for itself as ADR 0025 has it. A sticky window is hidden on the workspace its output shows.
- **`ShowingDesktop`** on `org.mate.Caminito` is true while every output's active workspace shows its desktop, and its change is signalled as now.

## Acceptance criteria

- [ ] Policy unit tests with two outputs showing different workspaces: the switcher's list has both outputs' showing windows and none of the hidden ones; the `-all` list has every window.
- [ ] Show Desktop hides both outputs' windows; switching one output away and back shows the desktop again there; a window activated on one output ends Show Desktop as it does today.
- [ ] `ShowingDesktop` follows the rule above, in a headless test over D-Bus.
- [ ] With the key off, each behaves as today.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: Alt+Tab reaches a window on the other monitor's workspace, and the panel's Show Desktop button clears both monitors.
