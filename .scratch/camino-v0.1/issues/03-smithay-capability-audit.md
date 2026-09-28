# Smithay capability audit

Type: research
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

What does current Smithay provide for a Marco-class stacking compositor, and what would Camino have to build itself? Check XWayland and its X11 window manager support, xdg-shell, xdg-decoration (forcing server-side), wlr-layer-shell, wlr-foreign-toplevel and ext-foreign-toplevel-list, ext-workspace, ext-session-lock, wlr-output-management, idle-inhibit and idle-notify, primary selection and data-control, fractional scale, input-method and text-input, the renderer options (GLES, pixman) and damage tracking, the backends (DRM/KMS, libinput, winit for running nested), and session management (libseat). Look at how niri and cosmic-comp build on Smithay for reference. Output: a protocol and subsystem table (provided, partial or missing), plus notes on the maturity and API stability risks.

## Answer

Full findings are on branch `research/smithay-capability-audit` (commit 40cf2ae), in `research/smithay-capability-audit.md`.

- **Pin a git revision of Smithay master, not the 0.7.0 release** (June 2025, 452 commits behind). Most of the X11 window-manager features Marco needs exist only on master. Expect breaking changes on every bump.
- **Smithay provides:** xdg-shell, forced server-side decorations (xdg and KDE protocols), layer-shell, session-lock, idle-inhibit/notify, primary selection and data-control, fractional scale, input-method/text-input, GLES and pixman renderers, damage tracking, DRM/KMS, libinput, winit and libseat.
- **Camino must write:** wlr-foreign-toplevel-management and ext-workspace-v1 (mate-panel's window list and workspace switcher go through libxfce4windowing, which binds these), plus wlr-output-management. niri's and cosmic-comp's versions are GPL-3, so they're design references only.
- **Camino owns the scene graph** (Smithay only has `desktop::Space`), all window-management policy and decoration drawing. The X11 window manager lacks the virtual-desktop, work-area and frame-extents properties. The closest reference shapes are cosmic-comp and anvil.
- **Open items handed on:** whether GTK3 honours KDE server-decoration (goes to Decoration strategy), and where to add the missing X11 desktop properties (goes to the XWayland fog).
