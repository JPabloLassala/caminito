# Theme goldens

Marco's own rendering of every frame case, for Camino's theme engine to match (14,
ADR 0009). `tools/regenerate-goldens` writes everything here except `cases.json`,
`icons/` and this file, on an Arch machine with Marco installed; CI only reads them, and
`camino-theme/tests/golden_files.rs` checks that none is missing or left over. Regenerate when Marco is bumped or a case is
added, and commit the result.

## Inputs

**`cases.json`** is the case list both sides read. Its top level pins everything a frame's
pixels depend on besides the theme:

- `themes` and `scales`: every case is rendered for each theme at each scale.
- `gtk_theme`: the GTK theme the colours come from, GTK3's built-in Adwaita.
- `font`: the title font, a pango description. It's the only font `tests/fonts/fonts.conf`
  makes visible.
- `dpi`, `antialias`, `hint_style`, `hint_metrics`: the cairo font options and resolution
  of the title's pango context. `marco-frame-dump` fails if GTK gives it anything else.
- `language`: the pango context's language, which the text height's font metrics use.
- `title`: the window title, ASCII.
- `mini_icon`, `icon`: the window icons, relative to this directory. They're 16 and 48
  pixels at every scale, as Marco loads them.
- `client_width`, `client_height`: the client size in logical pixels.

`base` is a focused normal window with Marco's default button layout. Each entry of
`cases` varies it:

- `name` names the case's files;
- `type` replaces the frame type (Marco's names: `normal`, `dialog`, `modal_dialog`,
  `utility`, `border`);
- `add_flags` and `remove_flags` change the base's `flags` (`MetaFrameFlags`, lower case
  without the `META_FRAME_` prefix);
- `button_layout` replaces the layout, a string in Marco's `button-layout` format;
- `button_states` replaces the button states, a map from button (`close`, `maximize`,
  `minimize`, `menu`, `appmenu`, `shade`, `above`, `stick`, `unshade`, `unabove`,
  `unstick`) to `normal`, `prelight` or `pressed`. Unlisted buttons, and the button
  backgrounds, are normal.

## Outputs

**`frames/<theme>/<case>@<scale>x.png`** is the whole frame as `meta_theme_draw_frame`
draws it: `width × height` logical pixels at the scale, on a transparent ARGB32 surface,
client area included, before `frames.c` cuts out the client or rounds the corners.

**`frames/<theme>/<case>@<scale>x.json`** is `meta_theme_calc_geometry`'s result for the
same case, in logical pixels:

- `type`, `flags`: the case's resolved inputs;
- `text_height`: the title's text height Marco computed from the font's metrics, which
  geometry takes as input;
- `width`, `height`: the whole frame;
- `borders`: `visible`, `invisible` and `total`, each with `left`, `right`, `top` and
  `bottom`;
- `title_rect` and `titlebar_edges`;
- `buttons`: each button Marco placed, with its `visible` and `clickable` rects. A button
  the layout or flags leave out, or that didn't fit, is absent;
- `button_backgrounds`: the non-empty ones, with the middle ones as a list up to the last
  non-empty one;
- `corner_radii`: the rounded-corner radii `frames.c` shapes the frame with.

**`palette.json`** is what the frames' `GtkStyleContext` gives for the GTK theme, in its
`default` and `dark` variants: `states` has `background` and `text` in every state Marco
maps, as `gtk:bg[...]` and `gtk:fg[...]` render them, and `named` has every name the golden
themes' `gtk:custom` colours use, `null` where the GTK theme doesn't define it. Colours are
`[red, green, blue, alpha]`, each a double from 0 to 1.

**`metadata.json`** records what rendered the goldens: Marco's version and the commit of
its tag, the mate-themes version, the GTK theme, and the Arch packages the rendering
depends on.

## How Marco is run

`marco-frame-dump` sets Marco up as `frames.c` does: its style context has the path of a
popup `GtkWindow` (which `MetaFrames` is) and the GTK theme's variant at settings priority,
and the title layout is `meta_frames_ensure_layout`'s, with the font passed explicitly
where Marco reads `titlebar-font`. Themes are loaded from `tests/themes/`, so both sides
read the same files; the script checks that the stock ones there are what the installed
Marco and mate-themes ship. Each scale is a separate run under `xvfb-run` with `GDK_SCALE`
set, since Marco reads the scale from the root window.
