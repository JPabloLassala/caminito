# Policy owns a window's functions, and its setters refuse what a window can't do

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

Marco works out what a window can do in one place, `recalc_window_features` (`window.c`): whether it has the minimise, maximise, resize, fullscreen and shade functions. Caminito has two ports of it, and they don't agree:

- `Policy::functions` (`caminito-policy/src/policy/window_actions.rs`), which the keybindings' `window_action` reads. It calls Desktop, Dock and Splashscreen windows fixed, and it has no shade function: `set_shaded` checks only `ssd && !fullscreen`.
- `frame_style` (`caminito/src/decoration/style.rs`), which the Frame's flags come from. It calls an attached modal dialog fixed, and it has the shade function.

Each input path uses a different copy, or none:

| Path | Gate today |
| --- | --- |
| Keybinding | `Policy::functions`, and for `toggle-shaded` a check of the Frame's flags in the compositor (`input/mod.rs`, `focused_window_shades`) |
| Titlebar action | the Frame's flags (`input/frame.rs`, `titlebar_action`) |
| Frame button | none; the theme draws no button the flags don't allow |
| Client request (xdg, foreign toplevel, X11 `_NET_WM_STATE`), and a Pending window's states at map | none |

Marco gates all of them by the functions (`window.c:5205-5254` for client messages, `:880-890` at map). Each port has its own unit tests, and nothing tests that they agree. [161](161-window-menu.md)'s window menu greys its items by the functions, and would be a fourth reader.

This was decided in an architecture review on 2026-10-11.

## What to build

- **One derivation, in `caminito-policy`.** A pure function from a window's facts (type, size hints, whether it has a parent, whether it's an SSD window, whether it's fullscreen) and the settings to its functions: minimise, maximise, resize, fullscreen, shade, and whether it has a border only. It's the one port of `recalc_window_features`, and it follows Marco but for fullscreen, below. `Policy::functions(id)` is that function with the managed window's facts, and it's public.
- **"Border only" is Policy's.** `attach-modal-dialogs` goes into Policy's `Config`, from `settings/translate.rs`. A modal dialog with a parent, with the key on, has a border only, and so no shade, resize or maximise function.
- **Fullscreen differs from Marco, on purpose.** A normal window or a dialog always has the fullscreen function, whether or not it can resize. Marco refuses a fixed-size window unless its minimum size is the screen's, which an X11 game arranges and a Wayland client can't. `docs/parity.md` says so.
- **The setters are the gate.** `set_maximized`, `set_maximized_horizontally`, `set_maximized_vertically`, `set_fullscreen`, `set_shaded` and `minimize` do nothing when they'd put a window into a state it has no function for. Leaving a state is always allowed. `window_action` keeps no guard of its own that the setters now have. A Pending window that asks for a state it has no function for maps without it.
- **The Frame doesn't derive.** Policy sends the functions and "border only" to the window with the `FrameState`, and the Frame draws with what Policy last sent, as [ADR 0012](../../docs/adr/0012-policy-asks-frame-extents-for-its-own-state.md) has it for the state. `frame_style` only maps them to the frame type and the `FrameFlags`. "Border only" changes the Frame extents, so it has to be there when Policy asks for them in a layout.
- **A Pending window's extents** (`Decorations::extents`, before Policy has the window): the compositor calls the pure function with the Pending window's facts. It's the one place the compositor gives the facts itself, and it stays until Policy knows Pending windows.
- **The compositor's gates go:** the `ToggleShaded` arm in `input/mod.rs`, `focused_window_shades`, and the `if flags.allows_*` guards in `titlebar_action`. `titlebar_action` and the frame buttons keep their own `match`, since the titlebar's `Lower` and the maximise button's mouse buttons aren't the keys', and they read `shaded`, `above` and `stuck` from Policy, not from the flags.
- **ADR 0012 gets one line:** the flags' `allows_*` inputs and "border only" come from Policy, with the state.

No new term in `CONTEXT.md`. "A window's functions" is Marco's word for it.

## What the user sees change

- A client's request to maximise or minimise a window that has no such function is refused, where it's honoured today.
- The fullscreen key works on a fixed-size normal window or dialog, where it does nothing today.
- `toggle-shaded`, the titlebar's shade actions and a client's shade request do nothing on an attached modal dialog, on every path.

## Acceptance criteria

- [ ] Policy tests, written before the code: a table of the functions for each window type, with and without size hints that fix its size, fullscreen, a CSD window, and a modal dialog with a parent with `attach-modal-dialogs` on and off.
- [ ] Policy tests, written before the code: each setter refuses a window without the function, and always lets a window leave the state.
- [ ] `decoration/style/tests.rs` keeps only the mapping from the functions to the flags and the frame type; `frame_style` has no `grows` and reads no size hints for the `allows_*` flags.
- [ ] Headless tests through the fixture, written before the code: a client's maximise request on a fixed-size window is refused, on xdg and XWayland windows; a Pending window that asks for it maps unmaximised; `toggle-shaded` does nothing on a CSD window and on an attached modal dialog; a fixed-size window that asks for fullscreen gets it.
- [ ] `grep -rn "focused_window_shades\|flags.allows_" caminito/src/input` finds nothing.
- [ ] `docs/parity.md` records the fullscreen difference, and `attach-modal-dialogs`' row names Policy's tests.
- [ ] ADR 0012 has its line.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

From Marco's source (`recalc_window_features`, `window.c`), recorded in the Done section:

- **Which window types are fixed,** and which have no minimise, maximise or fullscreen function. The two ports disagree today, and Marco decides.
- **Motif hints:** if Caminito reads an XWayland window's `_MOTIF_WM_HINTS` functions, whether they change the functions as Marco's `mwm_has_*_func` do. If it doesn't read them, say so and leave it.
- **How the functions travel with the `FrameState`:** in it, or beside it. The Frame's table of extents is keyed by the state, so the key shouldn't grow by more than "border only" needs.
