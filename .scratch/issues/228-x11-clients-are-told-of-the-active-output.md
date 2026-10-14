# X11 clients are told of the Active output

Epic: per-output-workspaces
Type: implementation
Status: resolved
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

- [x] Headless tests with Xwayland and two outputs on different workspaces: `_NET_CURRENT_DESKTOP` is the Active output's, and changes when a click makes the other output active with no switch.
- [x] A `_NET_CURRENT_DESKTOP` request switches the Active output only; a `_NET_WM_DESKTOP` request on a window of the other output moves it there and leaves it on that output.
- [x] The X stacking order has every showing window above every hidden one.
- [x] With the key off, each is as today.

## Done (2026-10-13)

- **One line of the compositor changed.** `root_properties` reads `Policy::active_output_workspace(SEAT)` in place of `primary_active_workspace`. The refresh pass writes `_NET_CURRENT_DESKTOP` only when its value changes, so a press or a focus that changes the Active output writes it again with no other code.
- **The rest was per output already.** The `_NET_CURRENT_DESKTOP` request runs `Action::SwitchToWorkspace`, which switches the Active output since 231. The `_NET_WM_DESKTOP` request runs `set_sticky` and `move_to_workspace`, which stay within the window's own output since 223. `refresh_x11_stacking` goes by `active_stacking`, which is each window's own output's active workspace since 230. A window's `_NET_WM_DESKTOP` was its number already.
- **Tested first.** The five headless tests in `caminito/src/tests/per_output_x11.rs` were written before the change. The two on `_NET_CURRENT_DESKTOP` failed, and passed after it. The other three passed at once, since they hold what the earlier tickets built. The stacking was then broken on purpose (the windows of the primary output's active workspace on top, as before 230): its test failed, and the code was put back. The `_NET_WM_DESKTOP` test and the key-off test were not seen to fail.
- **"Hidden" in the stacking is "on a workspace its output doesn't show".** A minimised window of a shown workspace keeps its place among the showing ones in X's order, as before this epic and as with the key off. The code review named it, and it was left alone.
- **`primary_active_workspace` stays** for ext-workspace's one group with the key off. `Policy::switch_workspace` has no caller left in the compositor, only tests.
- **No checklist item.** The ticket asks for none, and the Session run with two monitors is 229's.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito` and `caminito-policy`, and caminito's headless tests filtered by `per_output_x11`, `root_properties`, `window_properties`, `xwayland`, `per_output_workspaces` and `ext_workspace` (145 passed). `xwayland_crashing_again_and_again_as_it_starts…` failed one time in that run and passed alone, as it does on `master`. Policy's tests were not run: its change is two doc comments.
