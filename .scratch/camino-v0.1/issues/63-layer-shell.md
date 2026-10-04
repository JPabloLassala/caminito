# Layer-shell

Type: implementation
Status: open
Milestone: M5
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

wlr-layer-shell, version 4 or newer ([06](06-v01-protocol-and-integration-set.md)), through Smithay's `layer_map_for_output` in a new `layer` module ([15](15-crate-and-module-architecture.md)), and the scene's four layer-shell layers filled (`scene.rs` has them empty). Its users are mate-panel, caja's desktop, msd's background and notification bubbles: the Layer surfaces of CONTEXT.md.

- **Mapping:** a layer surface gets its output (the one it names, else the one under the pointer, else the first), is arranged by its anchors, margins and size, and is configured. It's rearranged when its state changes and when its output's mode, scale or place changes. When its output goes, it's closed.
- **Scene (15):** overlay above override-redirect windows; top above the workspace's windows; bottom and background below them. Layer surfaces are on every workspace. Their popups are drawn with them.
- **The desktop above the wallpaper (06):** within the background layer of one output, the surface with namespace `desktop` (caja) is drawn above `mate-background` (msd), whatever order they mapped in.
- **Fullscreen windows** cover the top layer on their output, as Marco puts a fullscreen window above docks; the overlay layer stays above.
- **Pointer:** layer surfaces take pointer focus by the scene's order, with their input regions.
- **Keyboard:**
  - `exclusive` on the top or overlay layer takes the keyboard while mapped, and it returns to policy's focused window after;
  - `on_demand` (v4, which caja needs) takes it on a click, and a click on a window gives it back;
  - `none` never takes it.

  `SeatHandler::KeyboardFocus` gains a layer-surface target. Policy's focused window doesn't change while a layer surface has the keyboard, so the frame stays drawn focused, as under Marco when the panel's menu is open.
- **Exclusive zones** are arranged by Smithay here, so layer surfaces avoid each other. Windows honouring them is [64](64-work-areas-from-struts.md).
- **Preferred scale** as for other surfaces (52).
- **Test client:** layer-shell in `camino-test-client`.
- **Debug dump:** the layer surfaces of each output, with layer, namespace, geometry, exclusive zone and keyboard interactivity.

## Acceptance criteria

- [ ] Headless snapshot tests, written before the code:
  - a top-anchored bar of height 28 is configured to the output's width, on the output it names;
  - two exclusive surfaces on one edge sit one after the other;
  - an output's mode change reconfigures its layer surfaces, and unplugging it closes them;
  - the scene's order with a window and one surface in each layer, and a fullscreen window above the top layer;
  - `desktop` above `mate-background` in both mapping orders;
  - an `exclusive` overlay surface takes the keyboard and gives it back on unmap; an `on_demand` one takes it on click only; a `none` one never does;
  - a layer surface's popup maps and takes a grab.
- [ ] `a_client_lists_the_globals` gains `zwlr_layer_shell_v1`.
- [ ] Nested, `mate-panel` started by hand shows on the top edge and its menus open, and a `gtk-layer-shell` demo's keyboard modes behave as above.

## Notes

- **The security-context filter** on the global is [68](68-data-control-and-privileged-globals.md).
- **Override-redirect windows and the overlay layer:** 36 put the override-redirect layer under the overlay one, and its test (`tests/override_redirect.rs`) notes the layers are empty until now.
- **The popup grab without a serial** for Alt+F1 and Alt+F2 is M6 (06).
- **`switch-panels` and `cycle-panels`** are M8, and only work if the panel takes `on_demand` focus (05, 06).
