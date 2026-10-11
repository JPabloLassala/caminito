# Policy owns a window's functions, and its setters refuse what a window can't do

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] Policy tests, written before the code: a table of the functions for each window type, with and without size hints that fix its size, fullscreen, a CSD window, and a modal dialog with a parent with `attach-modal-dialogs` on and off.
- [x] Policy tests, written before the code: each setter refuses a window without the function, and always lets a window leave the state.
- [x] `decoration/style/tests.rs` keeps only the mapping from the functions to the flags and the frame type; `frame_style` has no `grows` and reads no size hints for the `allows_*` flags.
- [x] Headless tests through the fixture, written before the code: a client's maximise request on a fixed-size window is refused, on xdg and XWayland windows; a Pending window that asks for it maps unmaximised; `toggle-shaded` does nothing on a CSD window and on an attached modal dialog; a fixed-size window that asks for fullscreen gets it.
- [x] `grep -rn "focused_window_shades\|flags.allows_" caminito/src/input` finds nothing.
- [x] `docs/parity.md` records the fullscreen difference, and `attach-modal-dialogs`' row names Policy's tests.
- [x] ADR 0012 has its line.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

From Marco's source (`recalc_window_features`, `window.c`), recorded in the Done section:

- **Which window types are fixed,** and which have no minimise, maximise or fullscreen function. The two ports disagree today, and Marco decides.
- **Motif hints:** if Caminito reads an XWayland window's `_MOTIF_WM_HINTS` functions, whether they change the functions as Marco's `mwm_has_*_func` do. If it doesn't read them, say so and leave it.
- **How the functions travel with the `FrameState`:** in it, or beside it. The Frame's table of extents is keyed by the state, so the key shouldn't grow by more than "border only" needs.

## Done (2026-10-11)

- **Tested first:** Policy's tests (`caminito-policy/src/tests/functions.rs`) were written against a stub of `functions` and ungated setters, and 15 of 17 failed before the code. The headless tests (`caminito/src/tests/functions.rs`) were written after Policy's gates, not before. To see them fail, `functions` was loosened to the old behaviour for one run: all 8 failed, and the change was put back.
- **Which window types are fixed (Marco's `recalc_window_features`, read in 1.29.1):** Desktop, Dock and Splashscreen have no resize function, and nor has an attached modal dialog. Only Normal and Dialog have the minimise, maximise and fullscreen functions, so a modal dialog, a utility window, a menu and a toolbar have none of the three. A window that can't resize has no maximise function. A fullscreen window has no shade, resize or maximise function. Shade needs a Frame with a titlebar: not a CSD window, a toolbar, or a border-only Frame. The size is fixed only when the least and greatest sizes are equal both ways.
- **Motif hints:** Caminito reads only the decorations field of `_MOTIF_WM_HINTS` (`xwayland/xconn.rs`), not the functions, and not `MWM_DECOR_BORDER`. They are left as they are: Marco's `mwm_has_*_func` count as on.
- **How the functions travel:** beside the `FrameState`, by a new request, `PolicyWindow::set_functions`, sent before the first geometry and when they change. The `FrameState` got "border only" and also the two ways the window resizes (`resizes_horizontally`, `resizes_vertically`). This is more than the ticket's "not more than border only needs": a Marco theme picks a frame's style by `resize="both"`, `"horizontal"`, `"vertical"` or `"none"`, so the extents depend on them, and `frame_style` may no longer read the size hints. The table of extents went from 24 to 192 entries, and it now lays out one title for each title scale of the theme, not one for each state, so it costs less than before.
- **`Functions` has the resize function each way** (`resize_horizontally`, `resize_vertically`), since the Frame's two resize flags need them. `resize()` is Marco's `has_resize_func`.
- **One-way maximise asks the resize function, not the maximise one.** Marco's `handle_maximize_vertically` and `_horizontally` ask only `has_resize_func`, so a utility window maximises one way by the key. Marco's titlebar asks the maximise function for one way too, so `titlebar_action` keeps a guard on `Policy::functions(window).maximize` for its two one-way actions, and one on "border only" for *Close* (Marco's `META_FRAME_ALLOWS_DELETE`). They read Policy's functions, not the flags. The developer can drop them if one gate is preferred to Marco's detail.
- **Two places give the facts themselves, not one:** a Pending window (`pending_functions`), and an XWayland window that asks for `_NET_FRAME_EXTENTS` before it maps (`xwayland/mod.rs`), which Policy doesn't know either. Both call `functions_before_managed`. The second has no size hints yet, so a fixed-size X window gets the extents of one that resizes until it maps.
- **Found by the headless tests:** the pre-map configure asked a fixed-size window for the maximised size though Policy then refused the state. `configure_pending` now asks for a state only if the window will have the function.
- **Not asked for, and done:** `maximize_if_too_big` needs the maximise function, as Marco's placement does (`place.c`), since it sets the state without the setter. A window's "may move" in a titlebar drag is `!is_fullscreen` from Policy, in place of `flags.allows_move`.
- **Left as in Marco:** a window that loses a function keeps its state (a shaded dialog that becomes attached stays shaded, as Marco's FIXME says), and can always leave it.
- **`attach-modal-dialogs` is copied into each managed window,** so that a window's functions come from its own fields in every layout. `settings_changed` keeps the copies equal.
- **No schema has `attach-modal-dialogs`,** so `set_setting` can't set it. `Fixture::set_attach_modal_dialogs` hands Caminito the settings directly, and `State::settings_changed` is `pub(crate)` for it. `docs/parity.md` has the key in a new row of the keyless section, since the parity test allows no row in `general` for a key the schema lacks.
- **Two older Policy tests changed:** they minimised a modal dialog directly, which has no minimise function. They now minimise it while it's a dialog.
- **What was run, in the OrbStack machine:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p caminito-policy` (501 pass), and 813 of `caminito`'s tests after the merge of `master`: the unit tests of `decoration`, `window`, `input`, `settings` and `deadlines`, and the headless modules `functions`, `decoration`, `frames`, `window_actions`, `xdg_shell`, `xwayland_windows`, `gtk_shell`, `placement`, `tiling`, `window_menu`, `foreign_toplevel`, `window_properties`, `shadows`, `bindings`, `keyboard_grab`, `dialogs`, `grabs`, `settings`, `parity`, `switcher`, `accelerators`, `global_menu`, `night_light` and `workspace_popup`.
- **What was not run:** `tools/check` as a whole: `cargo deny`, the build step, the other crates' tests, the palette helper, the helpers' tests, caminito-locker, the extension, and the rest of `caminito`'s suite.
