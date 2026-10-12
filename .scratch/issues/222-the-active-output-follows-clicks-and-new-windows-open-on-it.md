# The Active output follows clicks, and new windows open on it

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 231
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The rest of "The Active output" in [the spec](../per-output-workspaces/spec.md).

- **A click on anything on another output makes it the Active output**: a window, a panel or any other layer surface, caja's desktop or the wallpaper. Moving the pointer there changes nothing.
- **A new window opens on the Active output's active workspace**, in place of the focused window's output or else the Primary output. The `pointer` and `manual` placement modes keep the pointer's output. The Primary output is used only while a session has no Active output yet.
- **`rename-workspace`** reads the Active output's active workspace.

## Acceptance criteria

- [ ] Headless tests with two outputs: a click on the other output's layer surface, and on its bare background, changes the Active output, and the next switch binding acts there; pointer motion alone doesn't.
- [ ] A window mapped after a switch to an empty workspace on the second output opens there, on that workspace, with nothing focused before it.
- [ ] A window mapped under `pointer` placement opens on the pointer's output.
- [ ] With the key off, a new window opens where it does today.
- [ ] The Window switcher's popup opens on the pointer's output still, with the Active output the other one.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: with the second monitor on an empty workspace, open a terminal with its binding and see it there; click the first monitor's panel and open one from the menu and see it on the first.
