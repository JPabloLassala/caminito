# Screenshots: Print, Shift+Print and Alt+Print

Type: implementation
Status: open
Milestone: M6
Blocked by: 103
Map: [Camino v0.1](../map.md)

## What to build

`Print` takes a screenshot of the screen, `<Shift>Print` of an area, and `<Alt>Print` of the active window with its Frame, as under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Active window for mate-screenshot* row, [17](17-milestone-slicing.md)'s exit).

- **The bindings:** find first who binds each key on X11 MATE (Marco's screenshot commands, or msd), and bind them the same way: Marco's join the hard-coded set that stays until M8, and running the command from `org.mate.Marco.keybinding-commands`; msd's come with [99](99-msd-media-keys-and-osd.md).
- **D-Bus:** `GetActiveToplevel() → s` on `org.mate.Camino`. It returns the ext-foreign-toplevel-list identifier of the most recently focused toplevel that doesn't belong to the caller's pid, or an empty string when there's none. [65](65-foreign-toplevel.md) kept the identifier lookups for this.
- **The package:** `mate-utils-camino`, the fifth Patched package of [16](16-packaging.md), with its PKGBUILD, series, CI job and place in the weekly job. `camino-git` depends on it by its real name.
- **The carried patch:** when `org.mate.Camino` has an owner, mate-screenshot's `--window` asks for the active toplevel and captures it through the ext toplevel image-capture source ([103](103-image-capture.md)). With no owner it keeps its refusal of `--window` on Wayland (ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] Headless tests with a D-Bus client, written before the code: `GetActiveToplevel` returns the focused window's identifier; with the caller's own window focused it returns the one focused before; with no other window it returns the empty string.
- [x] A headless test that each of the three keys runs its command.
- [ ] `mate-utils-camino` builds, and its `makepkg` and `namcap` job passes in CI. It builds where the ticket was done; the job hasn't run (see the notes).
- [ ] The checklist has the item, and it was run in a Session: `Print` saves the whole screen, `<Shift>Print` an area, and `<Alt>Print` the window that had the focus, with its Frame, when another window covers part of it. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md). The item is there; it wasn't run (see the notes).

## Notes

- **The area screenshot** is mate-screenshot's own selection over an output capture. If it doesn't work on Wayland at 16's baseline, file a ticket; this one doesn't write it.
- **mate-screenshot's dialog** is a window of the caller's pid, which is why the method skips the caller's.

## What was found

- **Marco binds all three keys on X11 MATE,** none is msd's: `run-command-screenshot` (`Print`), `run-command-window-screenshot` (`<Alt>Print`) and `run-command-area-screenshot` (`<Shift>Print`) in `org.mate.Marco.global-keybindings`, each running its command of `org.mate.Marco.keybinding-commands` through `handle_run_command` (`core/keybindings.c`, `include/all-keybindings.h:195-197` at Marco `8522ed1`). msd's media-keys plugin has no screenshot key.
- **The area binding is newer than Arch's Marco.** Marco 1.29.0, which Arch has and the test schemas are vendored from, has no `run-command-area-screenshot` and no `command-area-screenshot`, so on Arch's X11 MATE nothing binds `<Shift>Print` today. Marco's master has both, with `<Shift>Print` and `mate-screenshot --area`.
- **mate-utils 1.29.1's mate-screenshot** captures outputs through ext-image-copy-capture, with wlr-screencopy as its fallback, picks an area on a layer surface, and refuses `--window` on Wayland. It already builds the ext-foreign-toplevel-list client code, which ext-image-capture-source's toplevel manager refers to.
- **Wayfire 0.11,** the one other compositor at hand, has ext-image-copy-capture, the output source and ext-foreign-toplevel-list, and no toplevel source.

## What was done

- **The three keys** join the hard-coded set (`input/bindings.rs`), which now holds a `Binding`: something policy does, or a `Command` to run. `Camino::run_command` (`commands.rs`) runs the command line the settings have for it, split as a shell splits words, through GLib's `g_spawn_async`, as Marco does: the program is a grandchild, so Camino has nothing to reap. It gets Camino's environment under `Camino::environment`, and its `PATH` is the one searched.
- **The commands** are read from `org.mate.Marco.keybinding-commands`, a third required schema of Marco's: `command-screenshot`, `command-window-screenshot`, and `command-area-screenshot` where the schema has it, else `mate-screenshot --area`, Marco master's default.
- **`GetActiveToplevel`** is on `org.mate.Camino`. The caller's process is asked of the bus (`GetConnectionUnixProcessID`). `Policy::recently_focused` gives the windows on screen from the focused one back, and the first that isn't of the caller's process is named by `Camino::toplevel_identifier`, which loses its `expect(dead_code)`.
- **`mate-utils-camino`** is Arch's PKGBUILD at the 1.29.1 tag with `--enable-wayland`, gtk-layer-shell, wayland and wayland-protocols, and one carried patch. It's in the packaging job after `caja-camino`, in the weekly job's matrix, in `tools/bootstrap` and the README, and `camino-git` depends on it.
- **The carried patch** (`0001-screenshot-Capture-the-active-window-on-Wayland-with.patch`): while `org.mate.Camino` has an owner, `--window` and *Grab the current window* are allowed on Wayland. At the capture, mate-screenshot calls `GetActiveToplevel`, finds the toplevel with that identifier in ext-foreign-toplevel-list, and captures it through `ext_foreign_toplevel_image_capture_source_manager_v1` with the session code it has for outputs.
- **The checklist item** is *Print, Shift+Print and Alt+Print*, under a new *Screenshots* heading.

## Notes from the work (2026-10-06)

- **Done with 103 still open.** Camino has no image capture yet, so nothing here could capture a window, or a screen, on Camino. What 103 doesn't block was built and tested: the keys, `GetActiveToplevel`, the package and the patch's gate. The patch's capture path compiles and follows the protocol as written, and has never run against a compositor that has the toplevel source. The Session check is 106's, after 103.
- **What was run of the patch,** on a headless Wayfire with a private bus, with the patched mate-screenshot built from the series: with no owner of `org.mate.Camino`, `mate-screenshot --window` prints `Error: window capture is not supported on Wayland.` and exits 1, as stock; with a stand-in owner, it calls `GetActiveToplevel`, finds no toplevel source, warns, and takes the whole screen.
- **The package was built with `makepkg --nodeps`,** since inkscape, one of Arch's makedepends, isn't installed there; configure only uses it to redraw logview's icons. `namcap PKGBUILD` is clean. `namcap` on the package has one error where it was run, `Dependency mate-utils detected and not included`, for `libmatedict.so.6`: the package's own library, which namcap gives to the stock `mate-utils` installed there. The CI job installs the package before it checks it, as it does `caja-camino` with its own library, so it shouldn't show there; that's the criterion left unticked.
- **Written before the code:** the D-Bus tests, the policy test and the tests of the keys were each written before what they test, and the first run of each was with the code in place, so none was seen to fail.
- **Decided here, beyond what the ticket says:**
  - **The window named is one on screen:** on the active workspace, and neither it nor a parent minimised, since Marco's active window is. A desktop or a dock isn't named, as Alt+Tab passes them over, nor a window that never had the focus. With none left it's the empty string, and mate-screenshot takes the whole screen, as it does on X11 with no current window.
  - **A caller the bus can't give a process for** has none of the windows taken for its own.
  - **An XWayland window's process is its `_NET_WM_PID`,** as policy already has it, so one without the property is never the caller's.
  - **`<Shift>Print` is bound though Arch's Marco doesn't bind it,** since the ticket and 17's exit ask for the three.
  - **An empty or unusable command line is logged,** where Marco shows a dialog. The dialog comes with the bindings, in M8.
  - **A window capture keeps the buffer's own pixels.** mate-screenshot scales an output's capture down to GDK's logical size; the window's is saved as Camino renders it, so at a scale of 2 it has twice the pixels of the same window cut from a screen capture.
  - **A stopped capture session fails the capture** in mate-screenshot, for outputs too. Upstream only warns, and would wait for ever for the frame of a window closed after it was named.
  - **The border options stay hidden on Wayland.** Camino's capture always has the Frame, so `--include-border` and `--remove-border` do nothing there, and still warn.
- **The fixture's `connect_as_process`** connects a client that Camino takes for another process's, through a `cfg(test)` field of `ClientState`: every client of a fixture is the test's own process, which is the D-Bus caller's too. Another session, working in the main checkout at the same time, was adding one of the same name; whichever lands second takes the other's.
- **For 103:** `Camino::window_for_toplevel_identifier` is still unused, and the checklist item's fourth step is the one that needs its "whatever covers it".
