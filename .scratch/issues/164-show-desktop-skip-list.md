# `show-desktop-skip-list`

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Windows listed in `show-desktop-skip-list` stay on screen when the desktop is shown ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)). [94](94-show-desktop.md) built Show Desktop, shared by Ctrl+Alt+D and the panel's button with Marco's per-workspace semantics, and left the key to M8.

- **Matching:** an entry matches a native window's app id, or an XWayland window's `WM_CLASS`, as 05 decided.
- **Whichever way the desktop is shown:** the binding, the panel's button through `org.mate.Caminito`'s `ShowingDesktop`, and any other path 94 has.
- **A listed window** keeps its place and its focus while the others are hidden, and isn't restored with them, as it wasn't hidden.
- **A change of the key** applies at the next Show Desktop.

## Acceptance criteria

- [x] Policy tests, written before the code: a listed and an unlisted window shown and restored; an empty list; an entry that matches no window; the list changing while the desktop is shown.
- [x] Headless tests through the fixture, on xdg and XWayland windows: a native window listed by app id and an X11 one by `WM_CLASS` stay through Ctrl+Alt+D and through `ShowingDesktop` set over D-Bus.
- [x] `show-desktop-skip-list` leaves `UNSUPPORTED` if 147 put it there; its row in `docs/parity.md` says honoured, with the tests above.
- [x] The checklist has an item, tagged M8: a terminal's app id and `xterm`'s class in the list, then the panel's Show Desktop and Ctrl+Alt+D. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the list's syntax (its separator, whitespace, case), whether Marco matches the class's instance or class part, or the title, and whether a listed window's transients stay with it.

**For the developer:** none.

## Done (2026-10-10)

- **Tested first:** policy's `tests/show_desktop.rs` cases (listed and unlisted shown and restored, an empty list, an entry matching nothing, the list changing while the desktop shows), then `ShowDesktopSkipList` and `Policy` to pass them; then the translate test, then the fixture's tests on both kinds of window.
- **Syntax, from Marco's prefs.c:** `show_desktop_skip_list_handler` splits on `,` and `g_strstrip`s each entry; `meta_prefs_is_in_skip_list` compares with `g_ascii_strcasecmp`. Caminito does the same, and drops empty entries.
- **What is matched:** Marco passes `w->res_class` (screen.c's `queue_windows_showing` and `meta_screen_show_desktop`), `WM_CLASS`'s class part, never the instance or the title. Caminito matches a native window's app id and an XWayland window's class, which `X11Surface::app_id` already gives.
- **Transients (a choice):** Marco matches each window by its own class, so a dialog stays only if its toolkit gives it the app's class, which toolkits do. Caminito also keeps a window whose transient ancestor is listed, so a listed app's dialog never vanishes from under it.
- **Focus:** a seat on a listed window keeps it when the desktop is shown, where Marco would focus the desktop window; the ticket asks for it. A listed window mapped while the desktop shows doesn't end it, and one already there isn't minimised when another window ends it.
- **A change waits:** each workspace showing its desktop keeps the list it was shown with, so a change of the key applies at the next Show Desktop.
- **Tests run** (under `dbus-run-session`, filtered): `cargo test -p caminito-policy --lib show_desktop` (31 passed, before and after merging the integration branch) and `cargo test -p caminito --lib -- tests::show_desktop settings::translate` (155 passed before merging the integration branch, 160 after; the four `::x11` variants, the two new ones among them, fail at "Xwayland starts", which is the environment). The full suite wasn't run, as asked for this run.
- **The X11 variants, run on 2026-10-11:** in the OrbStack machine `camino` (Arch, aarch64), where Xwayland starts, on master at `9ffebec`: the whole of `caminito` as `tools/check` runs it (`XDG_DATA_HOME=$HOME/.local/share dbus-run-session -- cargo test -p caminito`), 1445 passed, 8 failed, 5 ignored, and `cargo test -p caminito-policy --lib` (399 passed). `tests::show_desktop`'s fourteen tests passed, its four `::x11` variants included (`a_listed_window_stays_through_ctrl_alt_d` and `…_through_the_property` among them), and `settings::translate`. None of the 8 failures is this ticket's; they're listed in [148](148-keybindings-from-settings.md)'s Done section. The rest of `tools/check` wasn't run.
