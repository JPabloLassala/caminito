# XWayland integration

Type: grilling
Status: open
Blocked by: 10
Map: [Camino v0.1](../map.md)

## Question

Using the [XWayland integration audit](10-xwayland-integration-audit.md), how do XWayland windows get Marco's behaviour in v0.1? Decide:

- where each missing X11 feature is implemented (Smithay's window-manager connection or Camino's own), and whether any of it is worth sending upstream to Smithay;
- which root properties Camino publishes and when;
- how X work reconnects after an Xwayland restart;
- how XWayland windows map to Camino's window types, workspaces, stacking and focus policy;
- the v0.1 stance on clipboard and DnD bugs;
- how XWayland windows are scaled on HiDPI outputs (coordinate with [13](13-hidpi-and-accessibility-scope.md)).

This ticket takes over what 05, 06 and 08 handed on: position, above and sticky hints, `_NET_WM_PID`, struts, bells, the EWMH desktop atoms, `_NET_WORKAREA` and `_NET_FRAME_EXTENTS`, and reconnecting after a restart.
