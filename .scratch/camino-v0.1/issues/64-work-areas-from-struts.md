# Work areas from exclusive zones and struts

Type: implementation
Status: open
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

- [ ] Policy unit tests, written before the code: the work area of an output with a top and a left reservation; maximise fills it; placement stays in it; a change reconfigures maximised windows only; fullscreen ignores it. The proptest invariants still hold with reserved edges among the operations.
- [ ] Headless tests: a top layer surface with an exclusive zone of 28 shrinks a maximised window's configure by 28 and moves it down; unmapping it gives the space back; two outputs, with the zone on one, leave the other's work area whole.
- [ ] X11-tier tests: a window with `_NET_WM_STRUT_PARTIAL` reserves its edge; `_NET_WORKAREA` and `_GTK_WORKAREAS_D0` show the area less a layer surface's zone, at X11 scales 1 and 2.
- [ ] Nested, with `mate-panel` running, a maximised pluma and a maximised xterm both stop at the panel.

## Notes

- **MATE's own panel and desktop are layer surfaces** (11), so X11 struts only come from third-party docks.
- **Edge resistance and snapping against the work area** are M8.
