# SSD frames

Type: implementation
Status: resolved
Milestone: M2
Blocked by: 27, 28, 29
Map: [Caminito v0.1](../map.md)

## What to build

Caminito draws Marco frames around SSD windows ([07](07-decoration-strategy.md)), with live settings. This replaces M1's client-side decoration stand-in (17). Interaction with the frame is [31](31-frame-interaction.md)'s.

- **Negotiation (07):** KDE server-decoration's `default_mode` is `Server`. xdg-decoration grants the mode the client asks for, and server side when it has no preference. So MATE, plain GTK and Qt apps are SSD, and header-bar and libadwaita apps stay CSD. CSD windows get no frame.
- **The `decoration` module (15):**
  - per window, four strips as `MemoryRenderBuffer`s drawn by `caminito-theme`;
  - cached, and re-rendered only when an input changes: size, title, focus, state, button hover or press, theme, palette, fonts or scale;
  - the `scene` draws them with the window in layer 5.
- **Style set per window (07):** `modal_dialog` for xdg-dialog modal windows (gtk_shell1's `set_modal` joins in M5), `dialog` for a window with a parent, `normal` otherwise, and `attached` for modal dialogs with a parent when `attach-modal-dialogs` is on.
- **Frame extents:** `Mapped`'s `frame_extents` comes from `caminito-theme`'s geometry, so placement, maximise and the work area place the whole frame, and the client is configured to the size inside it.
- **Titles:** the xdg-toplevel title, in `titlebar-font` or `font-name` when `titlebar-uses-system-font` is set, with `org.mate.font-rendering`'s options.
- **Window icons (07):** xdg-toplevel-icon first, then the `.desktop` icon for the `app_id` in the `icon-theme` theme, then `application-x-executable`. Rendered at mini or full size times the scale and cached per window. The lookup lives where wlr-foreign-toplevel (M5) and Alt+Tab (M8) can share it. `_NET_WM_ICON` joins it in M3.
- **Palette:** from [28](28-palette-helper.md)'s helper, cache first. Native windows always use the default variant (07).
- **Scale (13):** strips are rendered at the largest rounded-up scale of the outputs the window overlaps, and re-rendered when that set changes.
- **Live settings:** `theme`, the fonts, `button-layout`, `gtk-theme` and `icon-theme` apply to open windows. A theme that fails to load logs one line and uses the built-in fallback theme ([24](24-theme-parser-and-model.md)).

## Acceptance criteria

- [x] Headless tests, written before the code: decoration-mode configure sequences for each kind of client request (replacing M1's client-side snapshots), frame extents in placement and maximise, strips re-rendered only when an input changes, and the strip scale for a window on 1.0, 1.5 and 2.0 outputs and across two of them.
- [x] A headless render of a framed window matches `caminito-theme`'s strips at the window's position.
- [x] Nested: a GTK3 app gets a Menta frame, and a GTK4 header-bar app stays CSD.
- [x] Nested: changing `theme`, `titlebar-font` or `button-layout` with `gsettings` applies live, and setting a broken theme falls back to the built-in one with a single log line.
