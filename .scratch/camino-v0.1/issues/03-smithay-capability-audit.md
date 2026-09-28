# Smithay capability audit

Type: research
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

What does current Smithay provide for a Marco-class stacking compositor, and what would Camino have to build itself? Check XWayland and its X11 window manager support, xdg-shell, xdg-decoration (forcing server-side), wlr-layer-shell, wlr-foreign-toplevel and ext-foreign-toplevel-list, ext-workspace, ext-session-lock, wlr-output-management, idle-inhibit and idle-notify, primary selection and data-control, fractional scale, input-method and text-input, the renderer options (GLES, pixman) and damage tracking, the backends (DRM/KMS, libinput, winit for running nested), and session management (libseat). Look at how niri and cosmic-comp build on Smithay for reference. Output: a protocol and subsystem table (provided, partial or missing), plus notes on the maturity and API stability risks.
