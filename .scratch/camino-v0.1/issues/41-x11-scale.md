# X11 scale

Type: implementation
Status: open
Milestone: M3
Blocked by: 39, 40
Map: [Camino v0.1](../map.md)

## What to build

One global X11 scale ([ADR 0008](../../../docs/adr/0008-fractional-default-scale-integer-x11-scale.md), [11](11-xwayland-integration.md), [13](13-hidpi-and-accessibility-scope.md)).

- **The value (ADR 0008):** the largest rounded-up output scale, computed in the `output` module as a pure function of the outputs (15).
- **Applied** through Smithay's `set_client_scale` on the Xwayland client.
- **XSETTINGS (13):** `Xft/DPI`, `Gdk/UnscaledDPI`, `Gdk/WindowScalingFactor` and `Gtk/CursorThemeSize` from msd's formulas (`msd-xsettings-manager.c:356-374`, `:411-421`), with `org.mate.font-rendering dpi`, or 96 when it's ≤ 0, added to [40](40-xsettings.md)'s table.
- **When it changes** (only when outputs or their scales change, 13): re-send XSETTINGS, reload the X cursor at the new size, republish the work area and every window's frame extents in the new client space, and reconfigure every XWayland window (11).
- **Coordinates:** every value `xconn` writes is in X client space (logical × scale), and the struts and input regions read from X are divided by it ([38](38-x-connection-and-root-properties.md), [39](39-x-window-properties-and-requests.md)).
- **Debug dump:** the X11 scale.

## Acceptance criteria

- [ ] Unit tests of the scale for 1.0, 1.25, 1.5, 2.0 and mixed output sets, and of the DPI formulas for `dpi` of 0, 96 and 120 at scales 1 and 2, written before the code.
- [ ] X11-tier tests on headless outputs at 1.0 and then 2.0: `Gdk/WindowScalingFactor` and `Xft/DPI` change, `_NET_WORKAREA` and `_NET_FRAME_EXTENTS` are in X client space, and an open X window is reconfigured to the same logical size.
- [ ] Nested at a winit scale of 2, xterm and an X11 gtk3-demo render sharp at scale 2 with correct frames, and the dump shows an X11 scale of 2.
