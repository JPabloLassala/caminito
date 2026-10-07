# The primary output

Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

Camino has no Primary output: wlr-output-management carries none, and nothing in a Session agrees on which output the panel and new windows prefer. [ADR 0014](../../../docs/adr/0014-camino-keeps-the-layouts-and-camino-displays-edits-them.md) gives it to Camino (decided 2026-10-05).

- **The setting:** one enabled output is the primary output. It is read and set on `org.mate.Camino` (ADR 0002's third channel), with a signal when it changes. The interface's shape is to decide here.
- **What follows it:**
  - mate-panel and caja's desktop go to it. How they learn of it is to decide here: the order of the `wl_output` globals, the RandR primary through GDK, or a carried patch that asks `org.mate.Camino`.
  - A new window with nothing better to go by opens on it.
  - XWayland's RandR primary is it.
- **The stand-in:** when the primary output is unplugged or disabled, the built-in panel stands in, else the leftmost output. The user's choice returns when its monitor does.
- **With no choice made,** the primary output is the built-in panel, else the leftmost output.
- **The rename:** the code's "primary output", the output a surface is mostly on, becomes Main output (CONTEXT.md), in the code and in comments. Old tickets keep their wording.

## Acceptance criteria

- [x] Headless tests, written before the code: the primary output with no choice made; a choice set over D-Bus is read back and signalled; unplugging it puts the stand-in in, and plugging it back returns the choice; a new window opens on it.
- [x] Under XWayland, `xrandr` shows the chosen output as primary, and shows the change. (Seen as RandR's `GetOutputPrimary` on the X11 tier's real Xwayland, which is what `xrandr` prints. `xrandr` itself isn't installed on the developer's machine, and is in the checklist item.)
- [ ] In the session with two monitors, changing the primary output moves the panel and the desktop icons.
- [x] No "primary output" in the code means the output a surface is mostly on.

## Notes

- Keeping the choice across Sessions is [83](83-saved-layouts.md)'s. Here it lasts for the Session.
- The prototype that settled the design is `.scratch/camino-displays-prototype/index.html` on the branch `prototype/camino-displays` (8a4981d), with its verdicts in the commit message. It is a source, not code to keep.

## Resolution

Decided here, on 2026-10-05:

- **The interface** is one property of `org.mate.Camino`, at `/org/mate/Camino`: `PrimaryOutput`, a string, read and written. It is an output's name, as `wl_output`, output management and RandR have it. Reading gives the primary output there is now, the choice or its stand-in, or the empty string with no outputs. Writing takes the name of an enabled output and is refused with `InvalidArgs` for any other. The signal is the standard `PropertiesChanged`, sent for a choice and for a stand-in going in or out. `org.mate.Camino` is now a name Camino owns, with `org.mate.Camino.Debug`.
- **mate-panel and caja learn of it by carried patches that ask `org.mate.Camino`**, each gated on an owner of that name (ADR 0011). The order of the `wl_output` globals would mean taking every other output's global away and making it again to change the primary output, which closes every layer surface on them. RandR's primary reaches only X11 clients, and the panel and caja are Wayland clients in a Session.
  - `mate-panel-camino`'s fourth patch puts the primary output first among the panel's monitors, as the panel does with RandR's primary on X11, and has the panel name its output, which it never did on Wayland. `pkgrel` is 3.
  - `caja-camino` is a new patched package, the sixth (ADR 0011), on Arch's caja 1.28.0. Its patch puts the desktop on the primary output, at its size. `camino-git` depends on it.
  - Both match the property against GDK's connector name for each monitor, and follow the property's signal. gtk-layer-shell maps the surface again on the new output.
- **A monitor is known** by its EDID's make, model and serial, or by its connector when the EDID has no serial (`MonitorIdentity` in `camino/src/output/primary.rs`). Of two monitors with the same EDID, the one at the connector that was chosen is the choice. That is what brings the choice back, and it's [83](83-saved-layouts.md)'s to reuse or change.

What follows it in Camino:

- **Policy** is told the primary output (`Policy::set_primary_output`), and uses it wherever it used the first output: a new window with no focused window to go by, and the windows of an output that's gone. A new window still opens on the focused window's output first.
- **A layer surface that names no output** still goes to the output under the pointer, and to the primary output only without one.
- **XWayland's RandR primary** is set by Camino's X connection, and set again when RandR's outputs change, since Xwayland hears of an output after Camino makes it.
- **The debug dump** has `primary_output`.
- **Healing** ([82](82-layout-healing.md)) holds the primary output still, where it held the first. With no choice made and no built-in panel, that is the output that's leftmost before the layout is healed.

The tests are `camino/src/tests/primary_output.rs` and `camino-policy/src/tests/outputs.rs`. The mate-panel patch was compiled in a built tree and `caja-camino` was built with `makepkg`; neither was run in a Session.

The third criterion needs two monitors and a Session, and is the developer's. It's the checklist's *The primary output moves the panel, the desktop icons and RandR's primary*.
