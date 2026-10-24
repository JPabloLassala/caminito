# 47. Caminito hides and slides an Intellihide panel, which only asks for it

Date: 2026-10-23
Status: Accepted

## Context

The Dock applet sits in a panel of its own, and that panel should get out of the way when a window overlaps it and come back when none does. This is Intellihide. mate-panel's own autohide is different: it hides whenever the pointer leaves, its Wayland version is unreleased 1.29 work, and the panel can't know when a window overlaps it, because foreign-toplevel carries no geometry.

## Decision

- **A carried mate-panel patch adds _Intellihide_ to a panel's Properties, beside Autohide.** With it on, the panel reserves no space, and it tells Caminito, through a small private protocol on its layer surface, that it wants intellihide.
- **The protocol is `caminito_intellihide_manager_v1`** (`caminito/resources/caminito-intellihide-v1.xml`, CMN-224). Its one request, `get_intellihide(id, layer_surface)`, makes a `caminito_intellihide_v1` for a `zwlr_layer_surface_v1`, and the object's existence is the request: destroying it, or the layer surface, ends it, which is how the panel turns the option off while it runs. A second object for one layer surface is a protocol error (`already_constructed`). **It has no events**, since the panel is never told that it is hidden, and it changes nothing of the layer surface: the panel keeps its own layer, anchors and margins, and reserves no exclusive zone. The edge it hides off is the one the surface is anchored to alone, with or without both ends of the edge; a surface anchored to a corner or to opposite edges is never hidden. The global is offered to the clients that may make layer surfaces.
- **Caminito does the rest.** It knows every window's geometry, so it decides when a window on the panel's output's active workspace overlaps the panel, and when the pointer has rested at that edge long enough to reveal it. It slides the layer surface out and back itself, with the popup slide (ADR 0037): the same motion, 200 ms, going the whole way out of the output, and off with the Popup slide's setting, where the panel goes at once.
- **The panel is never told it is hidden.** It draws as usual, and its input region goes with the surface.

## Consequences

- Hiding is compositor policy, so it can be tested in `caminito-policy` like the work area, without a panel. A window counts where it is asked to be, so the panel starts to go as a window starts to grow over it. The rest of the pointer at the edge is 250 ms, and a pointer on the panel, or in the gap between a floating panel and the edge, keeps it shown; so does an open popup of the panel, a menu or a tooltip.
- A hidden panel is drawn and found by the pointer where it has slid to, out of the output, so it takes no input and nothing of it is under the pointer at the edge.
- Without Caminito, the patch's option does nothing and the panel behaves as an ordinary one. It is gated like every carried patch (ADR 0011).
- Rejected: a panel that hides itself. It would need every window's geometry from a new protocol, and it would race the compositor on every move and resize.
