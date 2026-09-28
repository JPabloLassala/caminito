# Decoration strategy

Type: grilling
Status: resolved
Blocked by: 03, 04
Map: [Camino v0.1](../map.md)

## Question

How does Camino draw server-side decorations from Marco themes? Which theme-format versions and features does it support in v0.1, how does it render them (cairo/pango in Rust, or our own), where do the GTK colours come from without a GTK process, and how does it negotiate with clients that insist on client-side decorations (GTK3/4 CSD apps, and X11 clients through XWayland)?

## Answer

Decided in a grilling session on 2026-09-28. Inputs were the [Metacity/Marco theme format research](04-metacity-theme-format.md) (branch `research/metacity-theme-format`), the [Smithay capability audit](03-smithay-capability-audit.md) and what [06](06-v01-protocol-and-integration-set.md) handed on. For the client side, the session also read GTK `gtk-3-24` @28aa479 and `main` @91ecb49 (4.24.1), libadwaita, qtbase `dev`, Firefox, Chromium, Smithay `928d4a9`, mate-themes, Arc, Yaru and xdg-desktop-portal-gtk. The relevant facts are quoted inline.

The two decisions that are hard to reverse, porting the engine and getting colours from a GTK helper, are recorded in [ADR 0003](../../../docs/adr/0003-rust-theme-engine-and-gtk-palette-helper.md).

### Rendering

- **The engine is a Rust port** of Marco's `theme.c` and `theme-parser.c`: the parser, model, expressions, geometry and draw ops. It draws with **cairo-rs + pangocairo**, SVG images use librsvg, and PNG uses gdk-pixbuf or `image`. Cairo and Pango give pixel agreement with Marco, including its half-pixel X emulation and its title layout. Vendoring the C over FFI was rejected because every draw function takes a `GtkStyleContext*`.
- **The engine is a standalone crate, `camino-theme`**, with no Smithay dependency. Its API goes from the theme, palette, frame inputs and a cairo surface to pixels, borders, geometry and button rects. It's tested with **golden images against Marco**: a small reference harness renders the surveyed themes (Menta, BlackMATE, Arc, Mint-Y, Yaru, TraditionalOk, Atlanta and others) through Marco's own C in a fixed set of states, and the Rust output is diffed within a tolerance. The rest of the crate layout stays with the crate/module architecture fog.
- **Coverage: everything Marco draws.** That is tiers 0–2, plus from tier 3 `tint`, `clip`, `tile`, the positional and single button backgrounds, `appmenu` (parsed, and in practice never shown), `gtk:light/dark/mid/text_aa` and every state variant.
  - `gtk_box`, `gtk_arrow` and `gtk_vline` are **approximated**: a filled rect with a 1px bg/dark border, a triangle, and a 1px line. That affects only Atlanta, Esco and HighContrastInverse.
  - `menu_icon`, `<fallback>` and `theme:` images are parsed and ignored.
- **Validation is as strict as Marco's**, so a theme works in Camino exactly when it works in Marco. If the configured theme fails, Camino logs it once and falls back to a **built-in theme compiled into the binary**, so it never runs without frames. Changing the `theme` key reloads live.
- **Frames become pixels as four strips** (top including the titlebar, left, right, bottom). Each strip is drawn on the CPU with cairo into a `MemoryRenderBuffer`, replaying the draw list clipped to that strip. Strips are re-rendered only when an input changes (size, title, focus, state, button hover/press, theme, palette or scale) and cached per window. The rounded-corner shape from `frames.c` is an alpha mask on the top and bottom strips, and the same shape is used for input.
- **Scale:** themes render at the output's scale rounded up to an integer, and the renderer downsamples on fractional outputs. Border sizes are in logical pixels. Exact fractional rendering and windows spanning outputs with different scales go to the HiDPI fog ([13](13-hidpi-and-accessibility-scope.md): strips use the largest rounded-up scale among the outputs the window overlaps, and exact fractional frames are later).
- **Shadows:** Marco's compositor shadow model (Gaussian, with per-window-type and focus parameters from `compositor.c`) is drawn around the visible rect of **SSD frames only**. It follows the corner shape, sits outside the invisible resize border, is cached per size, and is off when `compositing-manager=false`. Toplevel captures leave it out (06).
- **Title:** the font is `org.mate.Marco.general titlebar-font`, or `org.mate.interface font-name` when `titlebar-uses-system-font` is set. It's scaled by `title_scale`, laid out with pangocairo (with Marco's 30px fade and `ellipsize_width`), and uses the antialiasing and hinting settings from `org.mate.font-rendering`. The text is the xdg-toplevel title, or `_NET_WM_NAME`/`WM_NAME` for X11 windows. All of these apply live.
- **Window icon** (for the `icon` op and the menu button), in this order:
  1. xdg-toplevel-icon
  2. `_NET_WM_ICON`
  3. the `.desktop` icon for the `app_id` or `WM_CLASS`, found in the `org.mate.interface icon-theme` theme
  4. `application-x-executable`

  The icon is rendered at mini (16px) or full size times the scale and cached per window. The lookup is shared with wlr-foreign-toplevel and Alt+Tab.
- **Style set per window:**
  - Native windows: modal (gtk_shell1 `set_modal` or xdg-dialog modal) gets `modal_dialog`, a window with a parent gets `dialog`, anything else gets `normal`.
  - XWayland windows: Marco's `_NET_WM_WINDOW_TYPE` mapping.
  - `attached` is used only when the attach-modal-dialogs key is on, for modal dialogs with a parent, as in Marco.
- **Hit-testing follows Marco:**
  - 10px invisible resize borders, and none on the screen-touching sides of maximized or tiled frames
  - Fitts's-law button rects
  - per-button hover and press with click-and-release semantics, and Marco's corner resize zones
  - cursors from `org.mate.peripherals-mouse cursor-theme`/`cursor-size`
  - `action-*-titlebar` on SSD frames (05)

### Colours

- **`camino-gtk-palette`** is a small C GTK3 program that Camino starts as a Wayland client of itself. It creates no windows. For both the default and the "dark" variant of the GTK theme, it reads `color` and `background-color` of `window.background` in every state Marco maps, plus the `@define-color` table through `gtk_style_context_lookup_color`, and writes the palette to stdout. It watches `org.mate.interface gtk-theme` and re-emits when that changes.
  - Why a helper: `GtkStyleContext` can't be created without a display (`gtkstylecontext.c:345-348`). Menta's defines are mostly `shade()`/`mix()` expressions (109 of 168), and GTK's `shade` scales saturation as well as lightness. Adwaita, Arc and Yaru ship their CSS inside gresources. Only GTK gives the exact answer.
  - Camino keeps an **on-disk cache** of the last palette per theme and variant, so frames drawn before the helper replies are already correct. With no cache, it uses a neutral built-in palette and re-renders when the helper reports. If the helper crashes it's restarted with backoff, and the last palette stays in use.
- **Dark variant:** Wayland has no per-window hint (`gtk-shell.xml` has none), and GTK3 on Wayland doesn't read the portal colour scheme. Native windows always get the default variant. XWayland windows with `_GTK_THEME_VARIANT=dark` get the dark one, as under Marco. A per-app override is **later**.

### Negotiation with clients

GTK3 and GTK4 bind only the KDE protocol and decide once for the whole display: SSD only if `default_mode=Server`. They ignore the compositor's per-surface reply (`gdkwindow-wayland.c:2494-2502`), and `gtk_window_set_titlebar` always forces CSD. So the compositor **cannot force SSD on GTK**. Qt 6 and Chromium's "system title bar" mode use xdg-decoration and accept the configured mode.

- **Policy:** KDE `default_mode=Server`. xdg-decoration **grants the mode the client asks for**, with server-side when it has no preference. In practice:
  - **SSD windows:** every MATE app (no MATE app uses a header bar: the only `set_titlebar` callers are an atril dialog that removes its titlebar and mcc's system-info, which turns header bars off on Wayland), plain GTK3/4 apps, Qt apps, and Firefox with tabs-in-titlebar off.
  - **CSD windows:** header-bar and libadwaita apps, Firefox with tabs in the titlebar (its default), Chromium's custom frame, and any client that binds neither protocol.
- **CSD windows:**
  - Camino draws no shadow and no invisible resize border. The client resizes itself through `xdg_toplevel.resize`.
  - Snapping, edge resistance, tiling, placement and the work area use `set_window_geometry` for native windows and `_GTK_FRAME_EXTENTS` for X11 windows.
  - Camino sends xdg-shell `tiled_*` states on maximise and edge-tile, so GTK drops its rounded corners and shadows.
  - **Shade stays disabled** (confirming 05): Camino doesn't know where the client's titlebar is.
  - `show_window_menu` opens Camino's window menu (05).
- **XWayland:** Marco's rule. Decorate unless the Motif hints ask for no decorations, and never decorate override-redirect, desktop, dock or splash windows. Motif "border only" gets the `border` style set.
  - **Naming trap:** Smithay's `X11Surface::is_decorated()` returns **true when the client wants no frame** (`surface.rs:1293-1300`).
  - Smithay already advertises `_GTK_FRAME_EXTENTS` and owns `_NET_WM_CM_S0`, so X11 header-bar apps get full CSD with shadows. That's kept.
  - `_NET_FRAME_EXTENTS` for SSD X11 windows goes over the Camino-side X connection (06).

### Handed on

- **HiDPI fog:** exact fractional rendering of frames and windows spanning outputs with different scales ([13](13-hidpi-and-accessibility-scope.md): largest rounded-up overlapped scale; exact frames later).
- **Crate/module architecture fog:** where `camino-theme` and `camino-gtk-palette` sit alongside the rest of the code, and the golden-image harness in CI (development loop fog). ([14](14-development-and-testing-loop.md): committed goldens from a libmarco-private harness, [ADR 0009](../../../docs/adr/0009-committed-marco-goldens-from-libmarco-private.md); the set adds ClearlooksRe, eOS and HighContrast, with Arc, Mint-Y and Yaru vendored.)
- **[08](08-session-launch-model.md):** Camino starts and supervises `camino-gtk-palette`. It needs the Wayland socket up and `org.mate.interface` readable.
- **[09](09-gsettings-mapping.md):** the decoration keys Camino reads (`theme`, `titlebar-font`, `titlebar-uses-system-font`, `button-layout`, `action-*-titlebar`, `org.mate.interface gtk-theme`/`font-name`/`icon-theme`, `org.mate.font-rendering`, the cursor keys), and a later per-app dark-variant key in `org.mate.Camino`.
- **Packaging:** runtime dependencies on cairo, pango, librsvg and GTK3 (for the helper), and the built-in fallback theme.
