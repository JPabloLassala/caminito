# Saved layouts

Type: implementation
Status: resolved
Milestone: M6 (proposed)
Blocked by: 81, 82
Map: [Camino v0.1](../map.md)

## What to build

Camino keeps the Saved layouts itself and restores them, in place of msd's wlrandr plugin and `monitors-wayland.xml` ([ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md), which supersedes that part of [71](71-mate-settings-daemon-camino.md)).

- **What is saved:** per set of connected monitors, each output's enabled state, mode, place and output scale, and the primary output ([81](81-primary-output.md)).
- **How a monitor is known:** by its EDID make, model and serial, or by its connector when it reports none. Two alike with no serial are told apart by connector.
- **Restoring:** at startup, before the first frame, and on hot-plug when the new set has a saved layout. A saved layout that no longer fits (a mode that's gone) is applied as far as it goes and healed ([82](82-layout-healing.md)).
- **Saving:** only when asked on `org.mate.Camino`. A configuration applied over wlr-output-management lasts for the Session and isn't saved.
- **Set, not default:** a restored scale and place count as set (58), so `window-scaling-factor` doesn't override them (ADR 0008).
- **msd:** a carried patch turns the wlrandr plugin off under Camino. `monitors-wayland.xml` isn't imported.
- **The file:** its place and its form are to decide here. Under `$XDG_CONFIG_HOME/camino/` is the proposal.

## Acceptance criteria

- [x] Headless tests, written before the code: a layout saved for two monitors is restored when Camino starts with those two; a different set gets the defaults; plugging the second monitor back restores the layout and the primary output; an applied configuration that wasn't saved is gone after a restart; a saved mode that the monitor no longer has leaves a contiguous layout.
- [ ] On the tty with two monitors, a layout saved in one Session is there at the next login from the first frame.
- [ ] msd's wlrandr plugin isn't loaded in the session, and 73's checklist item on `monitors-wayland.xml` is rewritten.
- [x] ADR 0008's and CONTEXT.md's "msd's saved layout" read "Camino's saved layout".

## Notes

- 60's hot-plug check with a real monitor is still open, and this ticket's hot-plug restore needs the same hardware.
- **The hot-plug restore was seen on hardware (2026-10-06),** in [60](60-tty-checks-left-over.md)'s run: each monitor plugged back came up at its saved mode, place and scale, and the LG as the primary output. The desktop icons didn't follow it back ([89](89-desktop-icons-stay-when-the-primary-comes-back.md)), which was a fault in `caja-camino`'s patch, fixed in its `pkgrel` 2.

## Resolution

The code is `camino/src/output/saved.rs`, with its tests in `camino/src/tests/saved_layouts.rs` and, for monitors with EDID serials, `camino/src/output/saved/tests.rs`.

Decided here, on 2026-10-05:

- **The file** is `$XDG_CONFIG_HOME/camino/layouts.json`, or `~/.config/camino/layouts.json` when that variable is unset, empty or relative. It is JSON, written whole beside itself and renamed over. It has a `version`, 1, and `layouts`. A layout is its `monitors`. A monitor has its `connector`, its `edid` (`make`, `model`, `serial`) when the EDID has a serial, `shows` (`mode` as `width`, `height` and `refresh` in mHz, `x`, `y`, `scale`) unless it's disabled, and `primary` on the primary output. A file Camino can't read, or of another version, is warned of and counts as empty, and the next layout saved replaces it.
- **Saving** is the method `SaveLayout` of `org.mate.Camino`, with no arguments. It saves the layout as it is, for the monitors connected, the disabled ones among them, in place of the one saved for that set. It fails with no output enabled, and with nowhere to keep the file.
- **The primary output saved** is the one there is: the choice, or its stand-in with no choice made. Restored, it is a choice. So after a layout is saved with no choice made, the built-in panel or the leftmost output of that day stays primary for that set. Whether a stand-in should be saved as no choice is the developer's to say.
- **Which layout is a set's:** the one with exactly the connected monitors, each known as 81 knows it. Of two monitors with one EDID, each is the one at the connector it was saved at, if one is there.
- **When it's restored:** when a monitor was plugged in or unplugged, once the backend has added and removed what its scan found: at the end of `State::new`, and after the tty's udev and seat events. So unplugging restores the layout saved for the monitors left, too. A set with no saved layout stays as it is, the new output at the defaults (51). A restore goes through the same steps as a configuration from output management (58), so the scales count as set. The place has no such mark since 82.
- **Every saved scale counts as set,** the ones that were Camino's default when saved among them: `window-scaling-factor` no longer changes a set that has a saved layout.
- **What no longer fits:** an output whose saved mode it no longer lists keeps the mode it shows, or takes its preferred one if it was disabled, and the saved scales and places are still applied and healed. Healing moves an output that ends up under another off it by the shortest way, which may be above it and not beside it (82). If the backend refuses a mode the monitor lists, the restore is tried once more with every output in the mode it shows. If that fails too, nothing is restored, the primary output's choice among it, and a warning is logged.
- **A layout that enables no monitor** isn't restored, as it would leave nothing to see it on.
- **On the tty** each monitor is still lit in its preferred mode as it's found, and switched to its saved one before any frame is drawn: two modesets where the modes differ.
- **Nested,** Camino keeps the winit output's layout in the same file, as the set of the one output `winit`.
- **msd:** `mate-settings-daemon-camino`'s fifth patch, `0005-wlrandr-Do-not-start-under-Camino.patch`, returns from the wlrandr plugin's `impl_activate` under Camino, as `0003` does for typing-break. `pkgrel` is 2. It applies to the 1.29.1 tree; the package wasn't built again, and wasn't run in a Session.

Not tested: the start-up path of `State::new` itself, since the headless backend has no monitors before the first is plugged in, so a start with two monitors is the second one's plugging in; the second try after a refused mode with a disabled monitor in the layout; and the file at its real place.

The second criterion and the first half of the third need the tty, two monitors and a Session, and are the developer's. They're the checklist's *Camino restores the saved layout from the first frame*, which replaces 73's *msd applies the saved display layout at login*. `73-closing-checks.sh` still writes a `monitors-wayland.xml`, and its layout step no longer holds.
