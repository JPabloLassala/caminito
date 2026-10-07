# MATE components on Wayland audit

Type: research
Status: resolved
Blocked by:
Map: [Caminito v0.1](../map.md)

## Question

Which MATE components (1.28 and later, plus current git) already run on Wayland, and which Wayland protocols does each one expect from the compositor? Cover mate-panel and its applets (the window list, workspace switcher and notification area in particular), mate-session, mate-settings-daemon, caja's desktop and background, mate-notification-daemon, mate-screensaver, mate-display-properties, mate-control-center's window/keyboard/mouse capplets, and mate-polkit. Also: how does the existing Wayfire-based MATE session launch and wire things together? Output: a component × protocol table showing working, partial or X11-only, with a source link for each.

## Answer

Full findings are on branch `research/mate-wayland-audit` (commit fc2ecf9), in `research/mate-wayland-audit.md`. They're based on reading source for 10 MATE repos plus libxfce4windowing at master as of 2026-09-28. Nothing was run.

- **Protocols:** MATE uses only wlroots/ext protocols, none private to MATE. `zwlr_layer_shell_v1` v4 is needed by the panel, caja's desktop, the msd wallpaper and notifications. `zwlr_foreign_toplevel_management_v1` is for the window list and `ext_workspace_v1` for the workspace switcher. `zwlr_output_management_v1` is used by msd display settings and `ext_data_control_v1` by the msd clipboard.
- **Timing:** most of this is 2026 git work (1.29 or unreleased), not 1.28. That covers the Wayland workspace switcher, native msd, the Wayland display capplet and panel autohide.
- **Gaps:** mate-screensaver is X11-only. On Wayland, msd disables its mouse, keyboard, keybindings and media-keys plugins, so Caminito must apply input settings and media keys itself. The Windows, Keyboard and Mouse capplets are X11-only, and the Keyboard Shortcuts dialog hides Marco's own bindings. Alt+F1/F2 exists only as the Marco X11 message: Alt+F2 has a workaround (`mate-panel --run-dialog`), Alt+F1 has none.
- **mate-session was never ported.** The Wayfire session uses a respawn script instead. mate-session needs a display to start, sets `DISPLAY` to the Wayland socket (which breaks XWayland apps), detects idle only through X11, and launches Marco unless told otherwise. The realistic order is Caminito → XWayland → mate-session as a child with `DISPLAY` pointing at XWayland.
- **Helpful:** msd already bridges XSettings to XWayland and has a wlr-output-management backend. Caja's desktop covers one monitor only.
