# X11 scale

Type: implementation
Status: resolved
Milestone: M3
Blocked by: 39, 40
Map: [Caminito v0.1](../map.md)

## What to build

One global X11 scale ([ADR 0008](../../../docs/adr/0008-fractional-default-scale-integer-x11-scale.md), [11](11-xwayland-integration.md), [13](13-hidpi-and-accessibility-scope.md)).

- **The value (ADR 0008):** the largest rounded-up output scale, computed in the `output` module as a pure function of the outputs (15).
- **Applied** through Smithay's `set_client_scale` on the Xwayland client.
- **XSETTINGS (13):** `Xft/DPI`, `Gdk/UnscaledDPI`, `Gdk/WindowScalingFactor` and `Gtk/CursorThemeSize` from msd's formulas (`msd-xsettings-manager.c:356-374`, `:411-421`), with `org.mate.font-rendering dpi`, or 96 when it's ≤ 0. [40](40-xsettings.md)'s `xsettings::table` already reads `dpi` and runs msd's formulas with a `scale` of 1; 41 makes that the X11 scale, and the refresh pass re-sends whatever it changes.
- **When it changes** (only when outputs or their scales change, 13): re-send XSETTINGS, reload the X cursor at the new size, republish the work area and every window's frame extents in the new client space, and reconfigure every XWayland window (11).
- **Coordinates:** every value `xconn` writes is in X client space (logical × scale), and the struts and input regions read from X are divided by it ([38](38-x-connection-and-root-properties.md), [39](39-x-window-properties-and-requests.md)).
- **Debug dump:** the X11 scale.

## Acceptance criteria

- [x] Unit tests of the scale for 1.0, 1.25, 1.5, 2.0 and mixed output sets, and of the DPI formulas for `dpi` of 0, 96 and 120 at scales 1 and 2, written before the code.
- [x] X11-tier tests on headless outputs at 1.0 and then 2.0: `Gdk/WindowScalingFactor` and `Xft/DPI` change, `_NET_WORKAREA` and `_NET_FRAME_EXTENTS` are in X client space, and an open X window is reconfigured to the same logical size.
- [x] Nested at a winit scale of 2, xterm and an X11 gtk3-demo render sharp at scale 2 with correct frames, and the dump shows an X11 scale of 2.

## Notes

- **The value** is `output::x11_scale`, a pure function of the outputs' scales: the largest rounded up, or 1 with none. `Caminito::x11_scale` runs it on the outputs there are, and the debug dump shows it as `x11_scale`.
- **Applying it:** `xwayland::scale` gives it to Xwayland's client with `set_client_scale` at spawn, before Xwayland binds the outputs, and again whenever the refresh pass finds it changed. The refresh pass works it out first, since everything after is at the scale. Smithay sends xdg-output's client-space sizes, which Xwayland's screen is made from, only when an output's state changes (`output/xdg.rs:115-140`), so a change nudges every output with an empty `change_current_state`, as cosmic-comp does. Without it the X screen stayed at the logical size (the X11-tier test caught it).
- **Coordinates:** `X11Scale` does the conversions, and `xconn` stays scale-free: the root properties and window properties are built in client space, so the refresh pass rewrites them when the scale changes, because they differ. Written in client space: `_NET_WORKAREA`, `_GTK_WORKAREAS_D<n>`, `_NET_FRAME_EXTENTS` (also the answer to `_NET_REQUEST_FRAME_EXTENTS`). Read from it: struts (width rounded up, span by floor), input regions (the logical rectangle that covers each) and `_NET_MOVERESIZE_WINDOW` (rounded as Smithay rounds a `ConfigureRequest`). Smithay converts what goes through `X11Wm` itself: configures, `ConfigureRequest`s, size hints and `_GTK_FRAME_EXTENTS`.
- **When it changes:** every pending and managed XWayland window is configured again with `configure(None)`, which Smithay sends as the same logical rectangle in the new client space. Their struts and input regions are read again, and managed windows' facts too, since Smithay's size hints are logical. XSETTINGS, the work area and frame extents follow in the same pass.
- **The X cursor:** Caminito now sets the root window's cursor with `X11Wm::set_cursor`, the theme's default cursor at `cursor-size` × the scale, after every `start_wm` before `DISPLAY` and whenever the theme, size or scale changes. Xwayland's own, from `XCURSOR_SIZE`, would be half size at scale 2. Smithay doesn't flush `set_cursor` either, so it's followed by the same empty restack as XSETTINGS (40), now `flush_wm`.
- **Left stale:** Smithay converts `_NET_WM_OPAQUE_REGION` only as it reads it (`surface.rs:1545-1553`), so after a scale change a window's opaque region is at the old scale until its client writes it again. It only affects damage and occlusion, not what's drawn. The root cursor has no test: X's `GetCursorImage` needs the pointer over an X window with no cursor of its own.
- **XSETTINGS:** `xsettings::table` takes the scale, and the refresh pass sends what changes: `Gdk/WindowScalingFactor`, `Xft/DPI` and `Gtk/CursorThemeSize` (`Gdk/UnscaledDPI` doesn't change).
- **Tests:** unit tests of the scale and of the client-space conversions, and of the DPI formulas at scales 1 and 2. X11-tier tests (`tests/x11_scale.rs`) take a headless output from 1.0 to 2.0: the XSETTINGS, work area and frame extents double, the X window goes from 300 × 200 to 600 × 400 while policy keeps it at 300 × 200, and the X screen stays 1920 × 1080. Struts at scale 2 are halved, and read again at 1 when the scale changes. The debug dump's x11 snapshot now shows `x11_scale` 2 and the published extents in client space, and its 400 × 300 X window is 200 × 150 logical.
- **The nested check** ran winit on Xvfb at `CAMINITO_WINIT_SCALE=2` in a private session bus and `XDG_CONFIG_HOME`. The 1280 × 800 winit output is 640 × 400 logical, and `xdpyinfo` and `_NET_WORKAREA` showed a 1280 × 800 X screen. gtk3-demo's application demo with `GDK_BACKEND=x11` rendered sharp at 2, with a Menta frame drawn at strip scale 2, `_NET_FRAME_EXTENTS` of 2, 2, 62, 2 for its 1, 1, 31, 1 frame, and `x11_scale` 2 in the dump. xterm isn't installed, so that criterion stays open here and its xterm half is a check in [46](46-nested-x11-checks-left-over.md).
