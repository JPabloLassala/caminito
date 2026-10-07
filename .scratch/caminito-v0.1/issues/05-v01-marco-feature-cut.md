# v0.1 Marco feature cut

Type: grilling
Status: resolved
Blocked by: 01
Map: [Caminito v0.1](../map.md)

## Question

Using the Marco feature inventory, which Marco behaviours go into Caminito v0.1, which come later, and which never?

## Answer

Decided in a grilling session on 2026-09-28, working from the [Marco feature inventory](01-marco-feature-inventory.md) (branch `research/marco-feature-inventory`). v0.1 aims at **full Marco parity**: every key and action ships except the ones that are X-only or droppable.

### v0.1

- **Every `org.mate.Marco.*` key and all 103 keybinding actions**, except the "never" list below. That covers:
  - focus modes and auto-raise
  - placement modes
  - modifier+drag and snap-move
  - titlebar actions and `button-layout`
  - edge, corner and keyboard tiling, with tile cycling
  - edge resistance
  - the full window menu, including "Move titlebar on-screen"
  - horizontal and vertical maximise, above and sticky as compositor state
  - Alt+Tab with thumbnails and all its options
  - 1–36 workspaces, dynamic workspaces, wrap styles, names, the workspace popup and switch sounds
  - shadows
  - visual and audible bell
  - the minimise animation
  - the zenity helper dialogs (`rename-workspace` keeps Marco's zenity entry dialog)
- **Unsupported keys:** a key Caminito doesn't honour yet is read, ignored and logged once. The documented list of such keys is the roadmap for "later".
- **`compositing-manager=false`** means "effects off": no shadows, icons instead of thumbnails in Alt+Tab, no minimise animation. **`reduced-resources`** only turns off the minimise animation.
- **Focus-stealing prevention (`focus-new-windows`)** is built on xdg-activation tokens:
  - *smart*: focus needs a valid, recent token. A first window of a freshly launched app with no token still gets focus, because MATE launchers don't pass tokens yet.
  - *strict*: focus needs a token tied to user input.
  - A window refused focus becomes urgent. Urgency feeds `alt-tab-urgent-placement` and the window list.
- **Shade** works on windows Caminito decorates. It is disabled (greyed-out menu item, inert binding) for CSD windows; revisit after the decoration strategy ticket.
- **CSD titlebars:** GTK applies its own titlebar click actions. Caminito honours the client's `show_window_menu`, `move` and `resize` requests. The `action-*-titlebar` keys apply to SSD frames only.
- **`switch-panels` / `cycle-panels`:** Caminito implements them and focuses any layer surface that accepts keyboard focus. They work end to end only if mate-panel and caja cooperate; otherwise it's a known gap.
- **"Not responding" dialog and Force Quit:** xdg_wm_base ping; the pid comes from socket credentials, or from `_NET_WM_PID` for XWayland clients.
- **`show-desktop-skip-list`** matches `app_id` for native clients and `WM_CLASS` for XWayland clients.
- **XWayland clients** get position hints, above/below requests and sticky requests. Sticky may slip to later if Smithay's X11 WM makes it expensive.
- **MATE integration:** all of these are required in v0.1. The channel for each is for [06](06-v01-protocol-and-integration-set.md).
  - Alt+F1, the panel main menu.
  - Alt+F2, the run dialog.
  - The pager grid layout, read from an `org.mate.Caminito` key; the name is for [09](09-gsettings-mapping.md).
  - The minimise animation to the window-list button's rectangle.
  - **Show Desktop state shared** between Ctrl+Alt+D and the panel's Show Desktop button, using Marco's per-workspace semantics rather than minimise-all.
  - mate-control-center's **Windows** dialog opens under Caminito.
  - mate-control-center's **Keyboard Shortcuts** dialog lists Caminito's bindings.
- **WM identity:** Caminito presents itself to MATE's settings tools as **Caminito**, not as Marco. It ships its own `50-caminito-*.xml` keybinding manifests, which point at the `org.mate.Marco.*` keys.
- **Screenshots:** `Print`, `<Shift>Print` and `<Alt>Print` all work.
  - The first two work with current mate-screenshot, which on Wayland uses ext-image-copy-capture or wlr-screencopy, plus layer-shell for the area picker.
  - `<Alt>Print` needs mate-screenshot patched to capture the active toplevel. mate-utils master at `9b23fee` refuses `--window` on Wayland.
  - The capture includes Caminito's frame and excludes the shadow.
- **Patches to MATE components** (mate-control-center, mate-screenshot, and possibly libxfce4windowing or mate-panel for Show Desktop) are **carried downstream** in Caminito's packaging. There are no upstream PRs, consistent with upstreaming being out of scope.

### Later

- XWayland: `_NET_WM_WINDOW_OPACITY`, size increments and the resize popup, `_NET_WM_FULLSCREEN_MONITORS`, and X11 startup-notification.
- Session restore, native clients only, once xdg-session-management is stable. No XSMP.

### Never

- `--replace`, `--display`, `--sync`, `--sm-*` and `--no-force-fullscreen`.
- marco-message and the `_MARCO_*` messages. Changing the `theme` key live-reloads the theme instead.
- `application-based`, `disable-workarounds`, `run-command-13`–`32` and `set-spew-mark`.
- Wireframe move/resize, and the empty close and unminimise effect hooks.
- The `_NET_DESKTOP_GEOMETRY` and `_NET_DESKTOP_VIEWPORT` stubs.
- marco-theme-viewer, marco-window-demo and wm-tester.

These keys stay in the schema, because Caminito reads Marco's schema and doesn't own it; Caminito just never implements them.

### Handed on

- **06:** a channel or protocol for:
  - Alt+F1
  - the pager layout
  - Show Desktop sync
  - Caminito's identity for mate-control-center
  - the active window for mate-screenshot
  - bell sources
  - window types and struts
- **07:** shade on CSD windows.
- **09:** the pager-layout key name.
- **Packaging:** the MATE baseline the carried patches apply to. ([16](16-packaging.md): each component's latest 1.29.x tag, or a pinned master commit for mate-panel and mate-session.)
