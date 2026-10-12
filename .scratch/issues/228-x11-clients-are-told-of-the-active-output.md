# X11 clients are told of the Active output

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 222
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"X11" in [the spec](../per-output-workspaces/spec.md).

- **`_NET_CURRENT_DESKTOP`** is the Active output's active workspace, written again when the Active output changes as well as when it switches.
- **A window's `_NET_WM_DESKTOP`** is its number within its own output's workspaces, or `0xFFFFFFFF` when sticky.
- **A request to change `_NET_CURRENT_DESKTOP`** switches the Active output. **A request to change a window's `_NET_WM_DESKTOP`** moves it within its own output, or makes it sticky there.
- **Xwayland's stacking** (`refresh_x11_stacking`) keeps the windows that aren't showing below those showing on any output.
- `_NET_NUMBER_OF_DESKTOPS`, `_NET_DESKTOP_NAMES` and the work-area properties are unchanged, since the count, the names and the work areas are one for every output.

## Acceptance criteria

- [ ] Headless tests with Xwayland and two outputs on different workspaces: `_NET_CURRENT_DESKTOP` is the Active output's, and changes when a click makes the other output active with no switch.
- [ ] A `_NET_CURRENT_DESKTOP` request switches the Active output only; a `_NET_WM_DESKTOP` request on a window of the other output moves it there and leaves it on that output.
- [ ] The X stacking order has every showing window above every hidden one.
- [ ] With the key off, each is as today.
