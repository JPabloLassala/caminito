# HiDPI and accessibility scope

Type: grilling
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How much HiDPI and accessibility goes into v0.1?

- **HiDPI:**
  - Fractional scale, with frames at the output scale rounded up (07).
  - Mixed-scale outputs, and windows spanning outputs.
  - The default scale per output (msd persists output config, 06).
  - What `org.mate.interface window-scaling-factor` means under Camino (09 left it unsupported).
  - How XWayland apps are scaled (coordinate with [11](11-xwayland-integration.md)).
- **Accessibility:**
  - Orca: AT-SPI works through the toolkits, but what does the compositor owe it (key event forwarding, focus tracking)?
  - The magnifier.
  - AccessX, meaning sticky, slow, bounce and mouse keys, from `org.mate.accessibility-keyboard` (06 and 09 deferred it).
  - Typing break.

For each item decide v0.1, later, or never, and who implements it (Camino, a MATE component, or another existing tool).
