# Crate and module architecture

Type: grilling
Status: open
Blocked by: 10, 11, 12, 13, 14
Map: [Camino v0.1](../map.md)

## Question

How is Camino's code split up? Decide:

- the crates, for example the compositor binary, the theme engine (07), the `camino-gtk-palette` helper (07), a policy crate with no Smithay dependency, the protocol implementations Camino builds (06), the D-Bus services (`org.mate.Camino`, the `org.mate.ScreenSaver` shim, `GrabAccelerators`) and the settings layer (09);
- the core state and event-loop shape (calloop), and where the scene graph (03) lives;
- how window-management policy sees windows independently of whether they are xdg or X11;
- the ownership boundaries that make 14's test seams work.

Look at the shapes of niri, cosmic-comp and anvil (03), and decide which to follow and where to depart from them.
