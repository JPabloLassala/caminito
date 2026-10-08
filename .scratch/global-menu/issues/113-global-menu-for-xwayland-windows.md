# The global menu of an XWayland window shows in the panel

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [112](112-global-menu-for-qt-apps-and-firefox.md)

## What to build

An X11 client calls `com.canonical.AppMenu.Registrar`'s `RegisterWindow` with its window id and a dbusmenu path. It's the only way VS Code has a global menu (ADR 0017).

- **Caminito serves the registrar's interface** on the name it already owns: `RegisterWindow`, `UnregisterWindow` and `GetMenuForWindow`. It keeps the menu with the XWayland window of that id, and drops it when the window or the caller's connection goes.
- **A registration whose id is no XWayland window's is kept apart and never shown.** An Electron app on native Wayland makes one.
- **Check Electron on native Wayland.** With the appmenu global offered, Chromium reports that global menus are supported, and Electron then registers an id that isn't an X11 window's. Run VS Code and one other Electron app that way, with `window.titleBarStyle` set to `native`, and see whether the menu is lost. If it is, take it back to the developer before this ships.
- **Document VS Code:** `--ozone-platform=x11` in `~/.config/code-flags.conf` and `window.titleBarStyle: "native"`, and that it's then drawn at the X11 scale (ADR 0008).

## Acceptance criteria

- [ ] In a Session with the applet on the panel, VS Code set up as documented shows its menu in the panel, and the items work.
- [ ] An Electron app on native Wayland keeps a usable menu, or the finding is with the developer.
- [ ] A headless test with the X11 test client: a registered window's menu reaches the published property when it has the focus, and goes when the window does.

## Notes

- Not checked: whether `window.menuBarVisibility: hidden` stops VS Code's export.
- Electron was read at v43.7.3, the one VS Code 1.140 has.
