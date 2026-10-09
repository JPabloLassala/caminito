# Drag a maximized window past the threshold to unmaximize it

Epic: caminito-v0.1
Type: implementation
Status: resolved: the Session check is the developer's
Milestone: M8
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Dragging a maximized window by its titlebar past the existing drag threshold unmaximizes it and keeps moving it under the cursor, as Marco and KDE Plasma both do. Settled by grilling on 2026-10-09.

- **Trigger.** Only a drag that starts on the titlebar (`FrameHold::Title`) counts. A modifier+drag on the window body, or a keyboard-initiated move, never unmaximizes. Applies to windows maximized on both axes and to windows maximized on one axis only (snapped left/right); fullscreen windows are unaffected because they have no frame to grab.
- **Threshold.** Reuses the existing `drag-threshold` setting (`org.mate.peripherals-mouse`, default 8 logical px, already read in `caminito/src/input/grab.rs:145-147` to decide when a titlebar press becomes an ordinary move). No new gsetting or constant.
- **Where it hooks in.** `caminito-policy::Policy::begin_grab` (`policy.rs:953-988`) refuses to start *any* grab, move or resize, on a maximized window (the `!m.maximized() && !m.fullscreen` check at line 964), and that's unchanged: `begin_move`/`begin_resize`/`begin_grab` keep refusing a maximized window exactly as before, which is still right for the xdg_toplevel move request, X11's `_NET_WM_MOVERESIZE`, and Alt+drag on the body — none of those should auto-unmaximize (only a titlebar drag does). Instead, a new `Policy::begin_titlebar_move(seat, id, at_press, pointer)` is the titlebar-drag-only entry point: it unmaximizes first (if needed) and only then calls the ordinary `begin_move`, which now succeeds because the window is no longer maximized. `caminito/src/input/grab.rs`'s `motion()` calls it (via a new private `start_titlebar_move_grab`) only from the `FrameHold::Title` branch, in place of the generic `start_window_grab` it used before. **Resize** stays blocked on a still-maximized window, unchanged.
- **The unmaximize is part of the same motion**, not a separate step: crossing the threshold clears the maximized axis flag(s) through the existing `set_maximized`-style machinery (so restore geometry comes from `Managed::floating`, same as the `Alt+F10` toggle), and the same grab continues as an ordinary move — no re-grab.
- **Half-maximized windows.** Today `begin_grab` already lets these move freely, and `grab_motion`'s `pin_maximized_axes` (`policy.rs:189-208`, `882-918`) keeps the maximized axis's size pinned for the whole drag. Past the threshold, that pin breaks too: a snapped window fully unmaximizes on the dragged side, not just the free axis. One rule for full and half maximize.
- **Reposition.** The window's size comes from `Managed::floating`. Its position is chosen so the pointer keeps the same relative % position across the titlebar it had at the moment the threshold was crossed (not a fixed pixel offset), so the window doesn't jump or leave the pointer off the title bar for a much-narrower restored width.

## Acceptance criteria

- [x] Policy tests: `begin_titlebar_move` starts a Move grab on a maximized window and on a half-maximized window from a titlebar hold, unmaximizing it first; a plain `begin_move`/`begin_resize` still refuses a maximized window exactly as before (`maximised_and_fullscreen_windows_cant_be_moved_or_resized`, unchanged).
- [x] Policy tests: crossing the threshold on a fully maximized window clears both axis flags, restores `floating`'s size, and repositions keeping the pointer's relative % across the titlebar constant — including the non-trivial case where the press and the threshold-crossing point differ, matching the catch-up `grab_motion` call `grab.rs` makes right after (`a_catch_up_after_crossing_the_threshold_keeps_the_pointers_fraction_across_the_titlebar`).
- [x] Policy tests: crossing the threshold on a half-maximized window clears the previously-pinned axis too, not just the already-free one, proven by a further `grab_motion` call that moves it freely on both axes (`dragging_the_titlebar_of_a_half_maximised_window_unmaximises_both_axes`).
- [x] A drag that never crosses the threshold leaves the window maximized, as an ordinary titlebar click does today: unchanged, since `grab.rs`'s threshold check (`caminito/src/input/grab.rs:145-147`) still gates whether `start_titlebar_move_grab` is even called.
- [ ] Checked in a Session: dragging a maximized window down by its titlebar unmaximizes it under the cursor like Plasma; the same works for a window snapped to a screen half; no resize handles appear until after it's unmaximized.

## Notes

- `Managed::floating` is guaranteed populated by the time a window is maximized — every path that sets a maximize/fullscreen flag true (`set_maximized_axes`, `set_fullscreen`, `maximize_if_too_big`) calls `save_floating()` first, including a window a client asks to map already maximized. So there's no "no restore geometry" fallback to design.
- Edge-snap-to-maximize (dragging to a screen edge to maximize, the reverse direction) is out of scope here — it isn't implemented anywhere in Caminito yet and is a separate feature.
- No `CONTEXT.md` or ADR changes: no new ambiguous vocabulary came out of the grilling session, and reusing the existing threshold and `floating` mechanism is additive and easily reversed.

## Resolution

Done on 2026-10-09.

- **`Policy::begin_titlebar_move(seat, id, at_press, pointer)`** (`caminito-policy/src/policy.rs`) is the new titlebar-only entry point. If the window is maximized either way, it calls a private `unmaximize_for_drag(id, at_press, pointer)` first, then the ordinary `begin_move`. `begin_grab` itself is untouched, so every other caller (Alt+drag, the xdg_toplevel move request, X11's `_NET_WM_MOVERESIZE`, X11's keyboard move) keeps refusing a maximized window exactly as before — only a titlebar drag unmaximizes.
- **`unmaximize_for_drag`** computes the pointer's fraction across the still-maximized titlebar from `managed.geometry` and `visible_extents()` — policy's own requested rectangle — rather than `Managed::frame` (which reads the client's last-*committed* size and lags behind an unacknowledged resize, as a Pending-window–style configure would). It then calls `set_maximized(id, false)` (clearing both axes, restoring `floating`'s size and position), and patches `geometry.loc.x` so the pointer keeps the same fraction across the now-narrower titlebar. The vertical position and the gap between the press point and the threshold-crossing point are both left to the ordinary catch-up `begin_move`'s caller already applies (a `grab_motion` call with the current pointer, immediately after the grab starts) — no separate vertical formula was needed.
- **`caminito/src/input/grab.rs`**: a new private `start_titlebar_move_grab` wraps `begin_titlebar_move`; `Grab::motion`'s `FrameHold::Title` branch calls it instead of the generic `start_window_grab`, passing both the press location and the current one. Nothing else about the threshold check changed.
- **Tests:** `caminito-policy/src/tests/interactive.rs` covers the geometry (full and half maximize, the non-trivial catch-up case, and that an ordinary window's `begin_titlebar_move` is identical to `begin_move`). `caminito/src/tests/frames.rs` adds two `on_both_kinds!` headless tests through the real input stack (`Fixture`): dragging a maximized window's titlebar past the threshold unmaximizes it and keeps following the pointer, and the same for a horizontally-maximized (half-maximized) window.
- **Not run:** the Session check (dragging a real maximized/snapped window in MATE on Caminito) is the developer's, per the acceptance criteria above.
