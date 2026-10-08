# Policy crate core

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M1
Blocked by: 18
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`caminito-policy`'s model, with M1's share of Marco's behaviour, built test-first against `TestWindow`. This is the seam 14 and 15 designed for, so it needs no Wayland code.

- **`PolicyWindow` (15):**
  - *facts:* a stable id, the window type, the transient parent's id, size hints, the requested position, pid, app id, whether it's SSD, and its frame extents;
  - *requests:* set geometry and states, set activated, close, and set the outputs the window is on.

  Only the facts M1 uses need real values yet, but the trait has its final shape.
- **`Policy<W: PolicyWindow>`:**
  - windows, outputs (id, logical rectangle, scale) and a fixed count of workspaces. This is a stand-in until M2 reads `num-workspaces` (17);
  - per-workspace stacking with Marco's layers;
  - click focus, focus moving to the next window in stacking order when one goes away, and one focus per seat;
  - transients stacked above their parents and following them between workspaces;
  - Marco's *automatic* placement;
  - maximise, minimise and fullscreen;
  - interactive move and resize as operations the caller drives.
- **The event and action API:** events in (map, unmap, commit with new hints, click, output added or removed) and effects applied through `W`. It includes the keybinding action enum, with only the variants M1 binds used so far, and it takes `now` as an argument wherever time matters.
- **`TestWindow`**, which records every call policy makes on it.
- **Tests (14):**
  - example tests for each behaviour above;
  - proptest operation sequences (add, remove and move windows and outputs, focus, workspace switches, transients) that check four invariants after every operation:
    - at most one focused window per seat;
    - transients above their parents;
    - every window on exactly one workspace;
    - stacking is a permutation of the managed windows.

  The pattern follows niri's `check_ops` and `verify_invariants`.

## Acceptance criteria

- [x] Every behaviour above has example tests, written before the code they cover.
- [x] The proptest invariants pass at the default case count, and at `PROPTEST_CASES=200000` in the weekly job.
- [x] Nothing in the crate names a Wayland, X11 or Smithay protocol type, and it builds under 18's dependency check.
