# Work areas from exclusive zones and struts

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 63
Map: [Camino v0.1](../map.md)

## What to build

Policy's work area made of each output less what's reserved on it: layer-shell exclusive zones ([63](63-layer-shell.md)) and XWayland windows' struts, which [39](39-x-window-properties-and-requests.md) already reads and keeps per window ([06](06-v01-protocol-and-integration-set.md), [11](11-xwayland-integration.md), [17](17-milestone-slicing.md)). Until now the work area is the whole output (38).

- **Policy (15):** each output descriptor gains its reserved edges, and `work_areas()` gives the output less them. Policy doesn't know where they come from.
- **Sources, combined in `camino`:** an output's layer map's non-exclusive zone, and the struts of mapped XWayland windows, in logical coordinates (41), each applied to the output its span is on, as Marco does (`screen.c`, `meta_window_update_struts`). A strut wider than half its output is ignored, as in Marco.
- **What follows the work area,** as in Marco:
  - maximised windows fill it, and are configured again when it changes;
  - placement puts new windows in it;
  - fullscreen ignores it;
  - a window left under a new panel isn't moved.
- **`_NET_WORKAREA` and `_GTK_WORKAREAS_D<n>` (38)** are republished when it changes, now less the struts. 38 noted that GTK3 keeps the last rectangle that meets a monitor, so the order of `_GTK_WORKAREAS` may now matter; check it against Marco's.
- **Debug dump:** each output's work area.

## Acceptance criteria

- [x] Policy unit tests, written before the code: the work area of an output with a top and a left reservation; maximise fills it; placement stays in it; a change reconfigures maximised windows only; fullscreen ignores it. The proptest invariants still hold with reserved edges among the operations.
- [x] Headless tests: a top layer surface with an exclusive zone of 28 shrinks a maximised window's configure by 28 and moves it down; unmapping it gives the space back; two outputs, with the zone on one, leave the other's work area whole.
- [x] X11-tier tests: a window with `_NET_WM_STRUT_PARTIAL` reserves its edge; `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` show the area less a layer surface's zone, at X11 scales 1 and 2.
- [x] Nested, with `mate-panel` running, a maximised pluma and a maximised xterm both stop at the panel.

## Notes

- **MATE's own panel and desktop are layer surfaces** (11), so X11 struts only come from third-party docks.
- **Edge resistance and snapping against the work area** are M8.

## Resolution

Policy's work area is each output less its reserved edges, and `camino` works those out from the layer surfaces' exclusive zones and the XWayland windows' struts. The policy tests are in `camino-policy/src/tests/work_area.rs`, the headless and X11-tier ones in `camino/src/tests/work_area.rs`, and the unit tests of the combining beside it in `camino/src/work_area.rs`.

- **Policy:** `Output` gained `reserved`, a `ReservedEdges`, and `work_areas()` gives the output less them. `set_outputs` is how a change arrives, so maximised windows are laid out again and a floating window stays where it is. Edges that reserve a whole output leave one pixel of it, where Marco widens such an area to 100 pixels (`workspace.c:665-700`). The proptest sequences gained a `Reserve` operation, and two invariants: a work area is in its output and isn't empty, and a fullscreen window covers an output.
- **Combined as Marco does, with rectangles.** The new `work_area` module makes a rectangle of each strut from the edge of the X screen (`window.c:6400-6520`), and a strip of each edge a layer map's non-exclusive zone leaves out. An output's work area is the largest rectangle of it clear of them all, which is what Marco's spanning set and `meta_rectangle_clip_to_region` give (`boxes.c:524-664`, `909-970`; `workspace.c:656-729`). The reserved edges policy is told are what's around that rectangle. So a strut along part of an edge reserves the edge if there's more room below it than beside it, and the room beside it otherwise.
- **No rule for a strut wider than half its output, on purpose.** This ticket asked for one "as in Marco". Marco 1.29's source has none, so Camino has none: such a strut leaves the largest rectangle beside it.
- **Which struts:** those of every managed XWayland window, on any workspace, since the work area is the same on each. Marco counts the windows of a workspace, and a dock is on them all. Override-redirect windows don't count, as in Marco.
- **When:** the refresh pass compares what's reserved now with what policy has, and tells policy if it differs. That covers a layer surface's commit and unmap, a dock's map, unmap and strut change, and the X11 scale, which is now applied first in the pass so the struts are read at it. `lay_out_outputs` arranges the layer surfaces before it tells policy, for the same answer in one step.
- **`_NET_WORKAREA`** is Marco's screen work area: the largest rectangle of the X screen clear of what's reserved on every output. With a bar on the first of two outputs, that's the whole screen below the bar.
- **`_GTK_WORKAREAS_D<n>` and their order (38):** Marco writes one rectangle per monitor, in monitor order (`screen.c:2354-2393`), and so does Camino, in policy's output order. Each rectangle is inside its own output and outputs don't overlap, so each meets one monitor only, and what GTK3 keeps doesn't depend on the order.
- **New toplevels' bounds** (`placement_bounds`) are the work area's size now, not the output's.
- **Debug dump:** each output has `work_area`.

Known, and left as they are:

- Of two free rectangles as large as each other, Marco takes the one its list has first, and Camino the one it finds first. They may differ.
- Policy's reserved edges can't say all that a strut along part of an edge says, so `_NET_WORKAREA` is worked out from the edges, not from the struts.
- A dock's struts count on every workspace until sticky windows (M8).

The nested criterion was checked in the OrbStack machine with `64-nested-check/check.sh`, under `xvfb-run`: Camino's winit window on Xvfb, with stock mate-panel 1.28.7, pluma on Wayland and xterm on Xwayland. The panel's one layer surface was at 0,776 1280x24 with zone 24, and the dump's work area was 0,0 1280x776. The maximised xterm and the maximised pluma both had the geometry 0,25 1280x751 under a frame of 25, so both frames end at 776, and `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` were 0, 0, 1280, 776. With the panel killed, both went to 0,25 1280x775. Nobody looked at the screen; that's 72's.
