# The Window switcher and Show Desktop across outputs

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 230
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"The Window switcher and Show Desktop" in [the spec](../per-output-workspaces/spec.md).

- **`switch-windows`, `switch-group` and `switch-panels`** list the windows showing on every output, most recently used first. The `-all` actions list every workspace of every output, with the showing ones first.
- **Show Desktop** hides the windows of every output's active workspace, and each of those workspaces keeps it for itself as ADR 0025 has it. A sticky window is hidden on the workspace its output shows.
- **`ShowingDesktop`** on `org.mate.Caminito` is true while every output's active workspace shows its desktop, and its change is signalled as now.

## Acceptance criteria

- [x] Policy unit tests with two outputs showing different workspaces: the switcher's list has both outputs' showing windows and none of the hidden ones; the `-all` list has every window.
- [x] Show Desktop hides both outputs' windows; switching one output away and back shows the desktop again there; a window activated on one output ends Show Desktop as it does today.
- [x] `ShowingDesktop` follows the rule above, in a headless test over D-Bus.
- [x] With the key off, each behaves as today.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: Alt+Tab reaches a window on the other monitor's workspace, and the panel's Show Desktop button clears both monitors.

## Done (2026-10-13)

- **Most of it was true already.** Tickets 221 and 230 made the Window switcher's candidates and Show Desktop go by each window's own output. The four lists needed no code. Their tests here passed at once: the lists with two outputs on different workspaces, and Alt+Tab to the other output in a headless test.
- **Tested first, and red:** Show Desktop was kept by workspace number alone, so two outputs' workspaces of one number shared it. With both outputs on workspace 1, a window activated on one output ended Show Desktop on both and minimised the other output's windows. An output switched to a number that another output showed the desktop on showed a desktop it never had.
- **Show Desktop is kept by output and number** with the key on (`DesktopOf` in `caminito-policy/src/policy.rs`), and by number alone with it off, which is the same as before. `Policy::is_showing_desktop` is true while every output's active workspace has an entry.
- **A window activated or mapped on one output ends Show Desktop on that output's workspace alone.** The other output keeps its desktop, with its windows hidden and not minimised, and `ShowingDesktop` becomes false. The next toggle shows the desktop on every output again. With the key off, it ends for the one workspace and minimises the other windows on every output, as before.
- **A window that maps** has no position when Show Desktop ends for it, so the output is the one it opens on (`opening_output`). After placement policy looks again, by the window's real output.
- **The focus follows ticket 231's rule.** With the key on, Show Desktop focuses a desktop window on the seat's Active output only, and its end gives a seat that lost its window the default focus of its Active output, or none. With the key off both take any output's, as before.
- **The key's change and the outputs.** Turned off, the one workspace of a number shows its desktop if the Active output's did. Turned on, every output's does if the one did. An output that goes takes its shown desktops with it. With no output left, the Primary output's are kept for the first output that comes.
- **`switch-windows-all` is not changed, and this differs from the ticket's words.** The ticket says "with the showing ones first". The list has one most-recently-used order over every window, as on master and as Marco has: a hidden window used more recently comes before a showing one. The showing windows are first only among windows that never had the focus. To put all showing windows first is a change with the key off too, and it is the developer's decision.
- **Not changed, from the review:** a second Show Desktop while only some outputs show theirs writes each seat's focus again for every output. That focus is the latest one, which is the one to give back.
- **`ShowingDesktop` over D-Bus** needed no code: the compositor signals the property when `is_showing_desktop` changes. `caminito/src/tests/show_desktop.rs` has a headless test for it with the key on and one with it off.
- **The checklist** has "Alt+Tab and Show Desktop on two monitors". It was not run: it needs two monitors and the panel, and is ticket 229's.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito-policy` and `caminito`; policy's 652 tests; and caminito's headless tests filtered by `show_desktop`, `switcher`, `per_output_workspaces`, `dynamic_workspaces`, `outputs`, `workspace`, `window_actions`, `bindings`, `placement`, `focus`, `dialogs`, `skip`, `xwayland_windows`, `foreign_toplevel` and `tests::debug` (696 passed).
