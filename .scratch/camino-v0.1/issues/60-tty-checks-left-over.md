# Tty checks left over from 55

Type: verification
Status: open
Milestone: M4
Blocked by: 55
Map: [Camino v0.1](../map.md)

## What to check

Two of [55](55-the-tty-backend.md)'s criteria were checked only in the UTM virtual machine, which has no real monitors. This ticket runs them on the developer's machine, with its two monitors. It's checking only: anything that fails gets its own ticket.

- **Two monitors (55):** the virtual machine has one connector with a picture. A second one, `Virtual-2`, was forced on through `/sys/class/drm/card0-Virtual-2/status`. Camino lit both, left to right, but the forced connector has no EDID, so its preferred mode, physical size and default scale ([50](50-default-output-scale.md)) weren't exercised, and nobody saw its picture.
- **Hot-plug (55):** the forced connector was turned off and on again, with a udev change event sent by hand. Camino removed and added the output. No real hot-plug event came from hardware, and no window was on the output that went, so windows moving off it wasn't checked.

Setup: on a spare tty as `caminotest`, run bare `camino`. With [56](56-libinput-on-the-tty.md) in, the keyboard and pointer work there, so a window can be dragged to the second monitor. A terminal is started from another VT or over ssh with `WAYLAND_DISPLAY` set to the socket Camino logs. The outputs' places and scales are in the debug interface's `DumpState` (32).

## Acceptance criteria

- [ ] Bare `camino` lights both of the developer's monitors in their preferred modes, left to right, at their default scales, and a terminal started with its `WAYLAND_DISPLAY` shows on them.
- [ ] Unplugging a monitor removes its output, and a window that was on it moves to the other monitor.
- [ ] Re-plugging the monitor adds its output again, in its preferred mode and at its default scale.
- [ ] The results are recorded in this ticket's notes, with a new ticket for each failure.
