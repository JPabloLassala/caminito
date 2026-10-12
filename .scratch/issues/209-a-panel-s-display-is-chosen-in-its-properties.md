# A panel can't be put on an output that isn't the primary one

Epic: caminito-v0.1
Type: implementation
Status: open: the patch and the package are in, and the checklist item, which needs a Session with two monitors and `mate-panel-caminito` at `pkgrel` 11 installed, is the developer's
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The developer wants panels on the outputs that aren't the primary one (2026-10-12). In a Session nothing in the panel's interface can put one there: every panel is on the primary output.

Under X11, a panel is moved to another monitor by dragging it there.

## Acceptance criteria

- [x] The cause is written down in this ticket's notes.
- [x] A panel whose `monitor` key is 1 is on the output that isn't primary, and the work area of each output follows it. (Seen in the developer's Session: see the notes.)
- [x] A panel's properties choose its display, under Caminito and with more than one monitor.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item for it.
- [ ] In a Session with two monitors, the checklist item passes.

## Notes

- **The cause is mate-panel's, and it's only the interface.** `panel_toplevel_begin_grab_op` and the motion handler return at once on Wayland, with upstream's FIXME that dragging "hasn't worked properly in wayland ever", and the properties dialog has no row for the monitor. So nothing writes a panel's `monitor` key.
- **The key itself works under Caminito**, by [81](81-primary-output.md)'s patch, the fourth: the panel's monitors are the primary output and then the others in GDK's order, and the panel names its output to the compositor. Nothing changes in Caminito.
- **Seen in the developer's Session (2026-10-12)**, on `mate-panel-caminito 1.28.7.r111.g58ac6c1-10` and `caminito-git r672.989d558-1`, with HDMI-A-1, the LG, as the primary output and DP-1, the AOC, to its right. An empty panel, `toplevel-0`, at the right edge, had its key written with `dconf write`, 1 and 0 by turns, five times:
  - at 1 its layer surface was on DP-1 at 4455,0 and 25×1080, and DP-1's work area was 1895 wide and HDMI-A-1's 2560;
  - at 0 it was on HDMI-A-1 at 2535,25 and 25×1440, with the work areas the other way round;
  - the panel wrote its own `x` after each move, and nothing else. mate-panel kept running.
- **The fix is mate-panel's eleventh carried patch**, `0011-properties-Choose-the-panel-s-display-under-Caminito.patch`, gated on an owner of `org.mate.Caminito` through the fourth patch's `panel_multimonitor_get_caminito_monitor` (ADR 0011). `pkgrel` is 11.
  - A panel's properties have a *Display* row below *Orientation*, with a list of the panel's monitors. Choosing one writes the `monitor` key.
  - A display is named as caminito-displays names it: a built-in panel as *Built-in display*, another by its make and model, and one that reports neither by its connector. Two that are alike have their connectors too. The first is the primary output, and says `(primary)`.
  - The list is made again when the panel is allocated its size, which is when the monitors change, when the primary output does, and when the panel has moved. It is left alone while its names are the same, so an open list isn't closed by the clock.
  - The row is hidden with one monitor, and without Caminito. A drawer has none.
- **Dragging stays off.** Making it work on Wayland is upstream's to do: the panel can't follow the pointer between outputs as a layer surface.

## Decisions

- **The chooser is in the properties dialog, not a drag** (2026-10-12, the agent's, for the developer to overrule). It's the smaller patch to carry, and it works with the keyboard.
- **The key stays a number in the panel's order.** With two monitors, 0 is the primary output and 1 is the other, whichever monitors they are, so a panel chosen for the other display changes monitor when the primary output is changed. With three or more, the ones that aren't primary are in GDK's order, which is the order their `wl_output` globals were made in, and an unplugged monitor returns at the end of it. A panel that stays with a monitor needs the key to name it, as [83](83-saved-layouts.md) knows a monitor. That is a larger patch, and isn't here.

## Left open

- **The new strings aren't translated:** `_Display:`, `%s (primary)` and `Built-in display`. The carried patches change no `.po` file.
- **The dialog wasn't run.** The patch compiled, and the package was built with `makepkg`. The row and its list were seen by nobody.
- **A third monitor wasn't tried.**
- **The first try lost the panel, and why isn't known.** `toplevel-0` was the developer's own, at the right edge of HDMI-A-1. Three seconds after its key was first written to 1 it was on neither output, and it was then found gone from `toplevel-id-list` with its keys reset, which in mate-panel only *Delete This Panel* does. The agent made it again, empty, with `orientation`, `expand`, `size` and `monitor`, and the five moves above are of that one. Its other keys, if it had any, are lost. Another agent was working in the same Session at the time ([208](208-desktop-menu-on-the-other-outputs.md)).
