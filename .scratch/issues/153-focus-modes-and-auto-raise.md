# Focus modes and auto-raise

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's three focus modes and auto-raise, for toplevels ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)). Today focus follows the click only.

- **`focus-mode`:**
  - `click`: as today;
  - `sloppy`: the pointer entering a window focuses it, and leaving it for the desktop keeps the focus where it was;
  - `mouse`: as `sloppy`, but leaving for the desktop takes the focus away.
- **`raise-on-click`:** off, a click focuses a window without raising it, except on its Frame, as in Marco.
- **`auto-raise` and `auto-raise-delay`:** with a pointer focus mode, a window focused by the pointer is raised after the delay, if the pointer is still in it.
- **The pointer's enter and leave** reach Policy as new events, for xdg and XWayland windows alike, from the same place the pointer's focus is decided now. Policy decides; the compositor crate only reports.
- **Layer surfaces** keep taking the keyboard by their own keyboard interactivity, whatever the mode ([09](09-gsettings-mapping.md)).
- **The Window switcher and the workspace popup** hold the pointer focus modes off while they're open.

## Acceptance criteria

- [x] Policy tests, written before the code: each mode with an enter, a leave to another window and a leave to the desktop; `raise-on-click` off on the body and on the Frame; auto-raise before and after the delay, and cancelled by a leave; a mode change applying at once.
- [x] Headless tests through the fixture, on xdg and XWayland windows: sloppy and mouse focus following the pointer; auto-raise after the delay with the fixture's driven clock; a layer surface with on-demand interactivity not focused by an enter; the switcher open, and no focus change under it.
- [x] `focus-mode`, `raise-on-click`, `auto-raise` and `auto-raise-delay` leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: each mode with a native and an X11 window and the panel, and auto-raise at its default delay. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Who owns the auto-raise timer.** Policy is free of time but for the `now` its calls take (15). Proposed: Policy keeps the deadline and says when it next needs to run, and one loop timer runs it, as the `idle` module's `IdleState` does ([128](128-idle-and-blanking.md)).

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **Enters that aren't the user's:** whether an enter caused by a window restacking, mapping or moving under a still pointer, or by a workspace switch, changes the focus in Marco.
- **The desktop window** (caja's, a Layer surface here) under `mouse`: what counts as leaving to the desktop.
- **`raise-on-click`** with a click on a transient or a modal dialog's parent.

## Done (2026-10-10)

- **Tests first, at the ticket's two seams:** `caminito-policy/src/tests/pointer_focus.rs` began with sloppy's enter, written against the new API before it existed; the rest of `pointer.rs` then went in at once, and the remaining policy tests (each mode, `raise-on-click`, auto-raise, a mode change, the switcher, a still pointer's crossings) were written after that first slice and passed on it. In review, Marco's grab-op code showed that its Frame raises only when a press ends as a click, not at the press as the first commit had it; the second commit does that, with its policy and headless tests (below). The proptest gained the crossings, `elapse`, `titlebar_released` and the focus settings as operations. The headless tests are `caminito/src/tests/focus_modes.rs`, on both kinds where the behaviour is shared; translation tests cover the four keys.
- **One event for enter and leave:** `Policy::pointer_entered(seat, PointerTarget, Crossing, now)`. A crossing leaves what the pointer was over and enters `Window(id)` (a body or a Frame), `Desktop` (nothing, or a Layer surface below the windows that isn't a popup) or `Other` (the panel or another Layer surface above the windows, an override-redirect window, a popup, the Window switcher, the lock). The compositor reports it from `pointer_motion`, where the pointer's focus is decided, whenever no grab has the pointer; policy drops a report of what it already has, so the per-motion cost is a map lookup.
- **Who owns the auto-raise timer (for the developer to confirm):** the proposal. Policy keeps the deadline and says when it next needs to run (`deadline()`, `elapse(now)`, as `IdleState` has them), and `caminito/src/policy_timer.rs` runs it with one loop timer set from the refresh pass. The fixture's `advance_clock` runs it too. At the deadline the window is raised only if the pointer is still in it and the focus is still where the enter put it, as `window_raise_with_delay_callback` checks (display.c:1312-1361); a leave drops it at once.
- **Enters that aren't the user's (from Marco's source):** Marco acts on the `EnterNotify` X sends whatever caused it (display.c:2093-2183), but while its `mouse_mode` is off, after the keyboard moved the focus (the tab popup, keybindings.c:2107, 3069, 3100), its focus sentinel drops the enters that showing windows make (window.c:1682-1698). So a `Crossing::Scene`, the pointer still while a window maps, goes, moves or restacks under it, or the workspace changes, counts only while policy's `mouse_mode` is on; the Window switcher's activation turns it off and the next crossing of the pointer's motion turns it on. Marco's default focus in `mouse_mode` after a workspace switch, the window under the pointer (workspace.c:963-1020), comes out of the same rule, through the refresh pass's crossing. A move or resize holds the modes off as well as the switcher, as Marco's grab ops take the crossings.
- **The desktop under `mouse` (from Marco's source):** Marco unfocuses when the pointer enters the desktop window, not on a leave (display.c:2155-2172). Here the desktop is nothing, a Layer surface in the bottom or background layer (caja's desktop, the wallpaper), or a managed window of type desktop; its popups, like Marco's override-redirect menus, change nothing. A dock and the panel never take the focus and never take it away, under either mode. Marco also raises an X11 dock on an enter and lowers it on a leave; that isn't carried, since the panel is a Layer surface.
- **`raise-on-click` (from Marco's source):** off, a click on a window's body focuses it, or its modal dialog, and raises nothing, the dialog or its parent alike, as `meta_window_focus` hands the focus to the modal transient without raising (window.c:4381-4411). On the Frame, Marco doesn't raise at the press either: a move or resize raises the window at its end if the pointer went less than 8 pixels (display.c:3857-3870, 3995-3997). So a titlebar click raises once released (`titlebar_released`), a move or resize that hardly moved raises (`end_grab`, whether it began on the Frame or with Alt), and a drag doesn't. A frame button's click doesn't raise.
- **Layer surfaces:** an enter never gives one the keyboard; an `exclusive` one keeps it whatever the mode. An `on_demand` one that was clicked keeps it until policy's focus changes, as before, so under a pointer mode, moving onto a window takes it back, as Marco's sloppy focus takes it from a panel.
- **The workspace popup** isn't in yet (152): `pointer_modes_held` is where it is to hold the modes off too.
- **Tests run:** `cargo test -p caminito-policy` (265 passed, `pointer_focus` and the proptest included); `cargo test -p caminito --lib tests::focus_modes` and `settings::translate` (every xdg test and translation test passed); and, once, after merging 154 and 157 and before the instruction to run only the ticket's tests, the workspace's suite under `dbus-run-session`: all passed but 254 of `caminito`'s tests, 253 at "Xwayland starts" and `root_properties::display_is_set_only_once_the_root_properties_and_xsettings_exist` at "DISPLAY is set within 5 s", both because Xwayland doesn't start in this container. So the `::x11` halves of `focus_modes` weren't seen to pass. Clippy on the workspace is clean with the three pre-existing `manual_range_contains` lints in caminito-theme allowed. The checklist item, *The focus modes and auto-raise*, is 173's closing run.
