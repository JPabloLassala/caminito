# Decoration strategy

Type: grilling
Status: open
Blocked by: 03, 04
Map: [Camino v0.1](../map.md)

## Question

How does Camino draw server-side decorations from Marco themes? Which theme-format versions and features does it support in v0.1, how does it render them (cairo/pango in Rust, or our own), where do the GTK colours come from without a GTK process, and how does it negotiate with clients that insist on client-side decorations (GTK3/4 CSD apps, and X11 clients through XWayland)?
