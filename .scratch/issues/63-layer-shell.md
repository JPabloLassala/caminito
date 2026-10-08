# Layer-shell

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M5
Blocked by:
Map: [Caminito v0.1](../caminito-v0.1/map.md)

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
- **Test client:** layer-shell in `caminito-test-client`.
- **Debug dump:** the layer surfaces of each output, with layer, namespace, geometry, exclusive zone and keyboard interactivity.

## Acceptance criteria

- [x] Headless snapshot tests, written before the code:
  - a top-anchored bar of height 28 is configured to the output's width, on the output it names;
  - two exclusive surfaces on one edge sit one after the other;
  - an output's mode change reconfigures its layer surfaces, and unplugging it closes them;
  - the scene's order with a window and one surface in each layer, and a fullscreen window above the top layer;
  - `desktop` above `mate-background` in both mapping orders;
  - an `exclusive` overlay surface takes the keyboard and gives it back on unmap; an `on_demand` one takes it on click only; a `none` one never does;
  - a layer surface's popup maps and takes a grab.
- [x] `a_client_lists_the_globals` gains `zwlr_layer_shell_v1`.
- [ ] Nested, `mate-panel` started by hand shows on the top edge and its menus open, and a `gtk-layer-shell` demo's keyboard modes behave as above.

## Notes

- **The security-context filter** on the global is [68](68-data-control-and-privileged-globals.md).
- **Override-redirect windows and the overlay layer:** 36 put the override-redirect layer under the overlay one, and its test (`tests/override_redirect.rs`) notes the layers are empty until now.
- **The popup grab without a serial** for Alt+F1 and Alt+F2 is M6 (06).
- **`switch-panels` and `cycle-panels`** are M8, and only work if the panel takes `on_demand` focus (05, 06).

## Resolution

Layer surfaces are in the `layer` module, on Smithay's `layer_map_for_output`, and the scene's four layer-shell layers are filled. The tests are in `caminito/src/tests/layer_shell.rs`.

- **The scene has one more layer.** Policy already puts a focused fullscreen window and its transients in Marco's fullscreen layer, at the top of the stacking. The scene draws those windows as `FullscreenWindows`, above the top layer, and the other windows below it, so a fullscreen window that loses the focus goes back under the panel, as in Marco. Pointer focus follows the same order through `scene::LAYERS`.
- **The keyboard.** `KeyboardFocus::Layer` is the layer-surface target. An `exclusive` surface of the overlay or top layer has the keyboard while it has content. An `on_demand` one has it from a press on it until a press on a window or a change of policy's focus. The protocol leaves `exclusive` on the bottom and background layers to the compositor, and Caminito treats it there as `on_demand`. A top-layer surface that a fullscreen window covers doesn't hold the keyboard. A popup grab's root is the layer surface, and Smithay hands the keyboard to the root when the grab ends; a surface with no keyboard interactivity hears no enter or leave of that.
- **The wallpaper.** `mate-background` is drawn behind every other surface of the background layer, which puts `desktop` above it in both mapping orders.
- **An output that goes** has its layer surfaces closed, also when output management disables it. A surface that names an output Caminito doesn't show is closed at once.
- **Frame callbacks, presentation feedback and the fifo and commit-timing barriers** go to an output's layer surfaces when it's drawn.
- **Debug dump:** each output has `layer_surfaces`, in the order they were made.

Known, and left as they are:

- A popup is drawn in its surface's layer, as this ticket says. So caja's desktop menu is under any window that overlaps it. sway and niri draw such popups above the windows. This needs a decision before 72. Decided on 2026-10-05, after 76 saw it in the session: above the windows, in [80](80-layer-popups-above-windows.md).
- A commit that unmaps a layer surface gets a configure at once, where the protocol expects it after the next initial commit. niri does the same, and mapping again works.
- A window's popup grab takes the keyboard from an `exclusive` surface until the grab ends.
- After a client closes the menu of a surface with no keyboard interactivity, the ended grab stays until the next input event, so that one key event is dropped.

The third criterion is checked in part. Nested in the guest's Plasma session, mate-panel 1.28.7 with `GDK_BACKEND=wayland` mapped its two panels as layer surfaces, and the dump showed `top panel` at 0,0 1280x27 with zone 27 and at 0,702 1280x24 with zone 24, both with keyboard `none`. That its menus open, and a `gtk-layer-shell` demo's keyboard modes, need eyes and hands; the guest has no demo installed. They're left to [72](72-session-file-and-first-login.md).
