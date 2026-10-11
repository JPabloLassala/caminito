# `cycle-*`, and panels that are Layer surfaces

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: 148, 108
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The switching actions [108](108-window-switcher.md) left: those with no popup, and the panels that are Layer surfaces ([05](05-v01-marco-feature-cut.md)).

- **`cycle-windows` (Alt+Escape), `cycle-group` (Alt+F6) and `cycle-panels` (Ctrl+Alt+Escape),** and their `-backward` twins: each press activates the next window of the same list as its `switch-*` action, at once, with no Window switcher, as Marco's do.
- **Panels that are Layer surfaces.** Today `switch-panels` lists only docks and the desktop that are XWayland windows, since a Wayland panel is a Layer surface and not a window to Policy. `switch-panels` and `cycle-panels` now also reach the Layer surfaces that accept keyboard focus (on-demand or exclusive interactivity), and give one the keyboard when it's chosen, as 05 asked.
- **In the switcher:** a Layer surface in `switch-panels`' grid has a thumbnail as a window does, and its name from its namespace.

## Acceptance criteria

- [x] Policy tests, written before the code: each `cycle-*` action's next window, both ways, and the list it walks; the panels' list with Layer surfaces in it.
- [x] Headless tests through the fixture, on xdg and XWayland windows, and with a layer-shell client: each `cycle-*` action from its default binding (Alt+Escape, Alt+F6 and Ctrl+Alt+Escape), and the `-backward` twins from Shift and from a binding set in the test; `switch-panels` listing a Layer surface with on-demand interactivity and not one with none; committing to it giving it the keyboard.
- [x] The six `cycle-*` actions' rows in `docs/parity.md` say honoured, and `switch-panels`' row no longer says Layer surfaces aren't listed, with the tests above.
- [x] The checklist has an item, tagged M8: Ctrl+Alt+Tab and Ctrl+Alt+Esc reaching mate-panel and the desktop, and Alt+Escape and Alt+F6 among a native and an X11 window. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: whether `cycle-*` walks the most recently used order, as `switch-*` does, or the stacking order; whether a cycled window is raised at once; and what Marco's `cycle-panels` does with a single panel.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **How a Layer surface is focused for the panel:** the keyboard is given to it, and mate-panel, a GTK3 client on gtk-layer-shell, must then let a key move between its applets. Whether it does, and whether caja's desktop takes the keyboard, is found here.
- **Which cooperation from mate-panel or caja is a known gap:** what doesn't work without a carried patch, written in the ticket and in parity's row, rather than patched here.

## Notes

- **`alt-tab-minimized-placement`** (Marco master's, default `end`) was given to this ticket by [147](147-the-parity-table.md), as no M8 ticket named it: read where the schema has it, it places minimised windows in the lists `switch-*` and `cycle-*` share, as [165](165-focus-stealing-prevention-for-native-windows.md) does `alt-tab-urgent-placement`. Its row says honoured with a Policy test of its values, and it leaves `UNSUPPORTED`.

## Done (2026-10-11)

- **Tested first, in part.** The first Policy test (`cycle-windows` selects the next window with no popup and raises it at once) was written before the code and didn't build without it. The other Policy tests were written after the code they cover, which had gone ahead of them. To show that they hold it, six behaviours were broken on purpose (the raise, the minimised window's show, the three placements, the Layer surfaces in the list, the keyboard's place): 12 tests failed, and passed again with the code put back. The headless tests were written after the compositor's code, except the one for the bug the review found, which failed first.
- **`cycle-*` walks the most recently used order (Marco's source).** `handle_cycle` and `handle_switch` both call `do_choose_window`, which takes its window from `meta_display_get_tab_next` over the same tab list (core/keybindings.c:3034-3155, core/display.c:4742-4862). A cycle is Marco's Alt+Tab grab with the popup hidden. So here: `Action::Cycle` opens policy's switcher with `popup` off, and the compositor times no popup for it.
- **A cycled window is raised at once, and focused when the modifier goes up (Marco's source).** `do_choose_window` raises the first selection, and `process_tab_grab` raises each next one over the stacking the grab began with, whatever `alt-tab-raise-windows` says (keybindings.c:3117-3122, 2301-2327). The focus moves on the release (2085-2113). So "activates at once" in *What to build* is: raised at once, activated on the release. A quick Alt+Escape is one step.
- **A minimised window shows while it's selected,** and is minimised again when the selection leaves it or the cycle is cancelled, as Marco's `tab_unminimized` does. The ticket didn't ask for it by name. Marco does the same for Alt+Tab with `alt-tab-raise-windows`; 108's Window switcher doesn't, and this ticket left that as it was.
- **One panel alone (Marco's source):** `meta_display_get_tab_next` finds no next one, `meta_display_get_tab_current` gives the panel itself, and the grab holds it until the release activates it (keybindings.c:3053-3063). Here it's activated at once with no cycle, as 108 does with one window, which comes to the same.
- **What ends a cycle:** letting go of the modifier commits. A `switch-*` binding, a `cycle-*` binding of another list (Alt+Escape during Alt+F6, and the other way) and any press of the pointer cancel, with the focus and the stacking as they were. Marco ends its grab on those too, and on any other key; here another key does nothing, as in the Window switcher. The arrows, Tab's bindings apart, and Return still act as in the Window switcher, on a grid that doesn't show: Marco's arrows work in a cycle too.
- **A binding's own key steps the way its action goes now,** in the Window switcher too: the compositor looks the press up in the binding table. Before, `switch-windows-backward` on a key of its own opened backward and then stepped forward. A `cycle-*` binding cancels the open Window switcher, which Escape did already with the default bindings.
- **Policy lists Layer surfaces by a name the compositor gives them** (`LayerSurfaceId`), and manages none. The compositor tells it in the refresh pass which accept the keyboard and which has it; policy keeps them most recently used first, and names what a commit chose (`take_chosen`), so that the compositor gives a Layer surface the keyboard, or takes it from one for a window. The Layer surfaces come first in the panels' list when one has the keyboard, and after the XWayland docks and desktop otherwise: policy has no order between the two kinds.
- **Listed: mapped Layer surfaces with `on_demand` or `exclusive` interactivity.** One on the top layer under a fullscreen window is listed and can't hold the keyboard, by 63's rule, so choosing it does nothing. No test covers an `exclusive` one in the list.
- **The debug dump's `switcher` gained `cycle`, `layers` and `selected_layer`,** and `selected` is null when a Layer surface is selected. The headless tests read them.
- **`alt-tab-minimized-placement`** is read where the schema has it: `end`, `mru` and `hidden`, as master's `meta_display_get_tab_list` (display.c:4801-4809). It left `UNSUPPORTED`.
- **For the developer to confirm — how a Layer surface is focused:** it gets the keyboard as a clicked `on_demand` one does (63), and the focused window keeps policy's focus and its Frame drawn focused. caja's desktop asks for `on_demand` and is listed, as `.scratch/issues/163-checks/layer-keyboard.sh` showed in a nested run in the OrbStack machine (`set_keyboard_interactivity(2)`). Whether caja then moves between its icons by the keys wasn't seen: a nested run there takes no key chord. It's the checklist item's step 5 and 6.
- **For the developer to confirm — the known gap is mate-panel:** its two panels ask for no keyboard (`set_keyboard_interactivity(0)` in the same run; `wayland-backend.c` at the pinned commit sets none), so Ctrl+Alt+Tab and Ctrl+Alt+Escape don't list them. A carried patch that sets `on_demand` on the panel's Layer surfaces would list them; whether a key then moves between the applets is unknown. It isn't patched here. Parity's `switch-panels` row and the checklist item say so, and the checklist item expects the desktop reached and the panels not.
- **Not run:** the full test suite, at the developer's word during the work, nor `tools/check`'s other steps. Run: `cargo fmt --check`, clippy on `caminito` and `caminito-policy`, all of `caminito-policy`'s tests, and of `caminito`'s the switcher, bindings, settings, parity, keybinding manifests, layer-shell, debug, focus modes, popups, Workspace popup, keyboard grab and input tests, in the OrbStack machine. Two lock tests failed once in such a run (`a_locker_that_is_killed_leaves_the_session_locked`, `a_second_clients_lock_is_refused_once_the_locker_is_gone`): they fail now and then on master too, 2 of 8 runs there. The checklist item wasn't run: it's 173's.
- **Merged with 165's `alt-tab-urgent-placement`,** which landed meanwhile on the same sort: `alt-tab-minimized-placement` places the minimised windows first, and the urgent ones go first among the rest, as 165 had it with the minimised last. 165's parity row says so now. After the merge the Alt+F6 test gives its second window the focus with a click, as 165 made the Alt+grave test do. The same tests ran again, with 165's and 160's.
