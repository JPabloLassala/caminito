/*
 * marco-frame-dump: the reference side of Caminito's theme goldens (14, ADR 0009).
 *
 * Marco's own C renders every case in tests/goldens/cases.json through the installed
 * libmarco-private: meta_theme_load, meta_theme_calc_geometry and meta_theme_draw_frame,
 * set up the way Marco's frames.c sets them up. It is run by tools/regenerate-goldens,
 * never by CI, and is not part of Caminito's build.
 *
 *   marco-frame-dump frames CASES OUT
 *     Writes OUT/<theme>/<case>@<scale>x.png and .json for every theme and case, at the
 *     scale GDK reports (GDK_SCALE).
 *   marco-frame-dump palette CASES OUT NAME...
 *     Writes OUT, the colours the frames' GtkStyleContext gives for the fixed GTK theme:
 *     background and text in every state Marco maps, for the default and dark variants,
 *     and each named colour, null where the theme doesn't define it.
 *
 * It needs a GDK display (theme.c:4173), so it runs under xvfb-run.
 */

#include <stdarg.h>
#include <stdlib.h>
#include <string.h>

#include <gtk/gtk.h>
#include <json-glib/json-glib.h>
#include <marco-private/theme-parser.h>
#include <marco-private/theme.h>

static G_GNUC_NORETURN G_GNUC_PRINTF (1, 2) void
fail (const char *format, ...)
{
  va_list args;

  va_start (args, format);
  fputs ("marco-frame-dump: ", stderr);
  vfprintf (stderr, format, args);
  fputc ('\n', stderr);
  va_end (args);
  exit (1);
}

/* The case list */

static JsonObject *
member_object (JsonObject *object, const char *name)
{
  if (!json_object_has_member (object, name))
    fail ("cases: no \"%s\"", name);
  return json_object_get_object_member (object, name);
}

static const char *
member_string (JsonObject *object, const char *name)
{
  if (!json_object_has_member (object, name))
    fail ("cases: no \"%s\"", name);
  return json_object_get_string_member (object, name);
}

static int
member_int (JsonObject *object, const char *name)
{
  if (!json_object_has_member (object, name))
    fail ("cases: no \"%s\"", name);
  return (int) json_object_get_int_member (object, name);
}

static const struct
{
  const char *name;
  MetaFrameFlags flag;
} frame_flags[] = {
  { "allows_delete", META_FRAME_ALLOWS_DELETE },
  { "allows_menu", META_FRAME_ALLOWS_MENU },
  { "allows_appmenu", META_FRAME_ALLOWS_APPMENU },
  { "allows_minimize", META_FRAME_ALLOWS_MINIMIZE },
  { "allows_maximize", META_FRAME_ALLOWS_MAXIMIZE },
  { "allows_vertical_resize", META_FRAME_ALLOWS_VERTICAL_RESIZE },
  { "allows_horizontal_resize", META_FRAME_ALLOWS_HORIZONTAL_RESIZE },
  { "has_focus", META_FRAME_HAS_FOCUS },
  { "shaded", META_FRAME_SHADED },
  { "stuck", META_FRAME_STUCK },
  { "maximized", META_FRAME_MAXIMIZED },
  { "allows_shade", META_FRAME_ALLOWS_SHADE },
  { "allows_move", META_FRAME_ALLOWS_MOVE },
  { "fullscreen", META_FRAME_FULLSCREEN },
  { "is_flashing", META_FRAME_IS_FLASHING },
  { "above", META_FRAME_ABOVE },
  { "tiled_left", META_FRAME_TILED_LEFT },
  { "tiled_right", META_FRAME_TILED_RIGHT },
};

static MetaFrameFlags
flags_from_array (JsonArray *array)
{
  MetaFrameFlags flags = 0;

  for (guint i = 0; i < json_array_get_length (array); i++)
    {
      const char *name = json_array_get_string_element (array, i);
      gsize f;

      for (f = 0; f < G_N_ELEMENTS (frame_flags); f++)
        if (strcmp (name, frame_flags[f].name) == 0)
          break;
      if (f == G_N_ELEMENTS (frame_flags))
        fail ("cases: unknown frame flag \"%s\"", name);
      flags |= frame_flags[f].flag;
    }

  return flags;
}

static JsonArray *
flags_to_array (MetaFrameFlags flags)
{
  JsonArray *array = json_array_new ();

  for (gsize f = 0; f < G_N_ELEMENTS (frame_flags); f++)
    if (flags & frame_flags[f].flag)
      json_array_add_string_element (array, frame_flags[f].name);

  return array;
}

/* The buttons whose state a case can set. The backgrounds are always normal, as in
 * frames.c's meta_frames_paint_to_drawable. */
static const struct
{
  const char *name;
  MetaButtonType type;
} button_types[] = {
  { "close", META_BUTTON_TYPE_CLOSE },
  { "maximize", META_BUTTON_TYPE_MAXIMIZE },
  { "minimize", META_BUTTON_TYPE_MINIMIZE },
  { "menu", META_BUTTON_TYPE_MENU },
  { "appmenu", META_BUTTON_TYPE_APPMENU },
  { "shade", META_BUTTON_TYPE_SHADE },
  { "above", META_BUTTON_TYPE_ABOVE },
  { "stick", META_BUTTON_TYPE_STICK },
  { "unshade", META_BUTTON_TYPE_UNSHADE },
  { "unabove", META_BUTTON_TYPE_UNABOVE },
  { "unstick", META_BUTTON_TYPE_UNSTICK },
};

static void
button_states_from_object (JsonObject      *object,
                           MetaButtonState  states[META_BUTTON_TYPE_LAST])
{
  GList *names = json_object_get_members (object);

  for (int i = 0; i < META_BUTTON_TYPE_LAST; i++)
    states[i] = META_BUTTON_STATE_NORMAL;

  for (GList *l = names; l != NULL; l = l->next)
    {
      const char *name = l->data;
      const char *state = json_object_get_string_member (object, name);
      gsize b;

      for (b = 0; b < G_N_ELEMENTS (button_types); b++)
        if (strcmp (name, button_types[b].name) == 0)
          break;
      if (b == G_N_ELEMENTS (button_types))
        fail ("cases: unknown button \"%s\"", name);

      if (strcmp (state, "normal") == 0)
        states[button_types[b].type] = META_BUTTON_STATE_NORMAL;
      else if (strcmp (state, "prelight") == 0)
        states[button_types[b].type] = META_BUTTON_STATE_PRELIGHT;
      else if (strcmp (state, "pressed") == 0)
        states[button_types[b].type] = META_BUTTON_STATE_PRESSED;
      else
        fail ("cases: unknown button state \"%s\"", state);
    }

  g_list_free (names);
}

/* Marco's button-layout parser, prefs.c's button_function_from_string,
 * button_opposite_function and button_layout_handler, which libmarco-private doesn't
 * export. The RTL inversion is left out: the goldens are left to right. */

static MetaButtonFunction
button_function_from_string (const char *str)
{
  if (strcmp (str, "menu") == 0)
    return META_BUTTON_FUNCTION_MENU;
  else if (strcmp (str, "appmenu") == 0)
    return META_BUTTON_FUNCTION_APPMENU;
  else if (strcmp (str, "minimize") == 0)
    return META_BUTTON_FUNCTION_MINIMIZE;
  else if (strcmp (str, "maximize") == 0)
    return META_BUTTON_FUNCTION_MAXIMIZE;
  else if (strcmp (str, "close") == 0)
    return META_BUTTON_FUNCTION_CLOSE;
  else if (strcmp (str, "shade") == 0)
    return META_BUTTON_FUNCTION_SHADE;
  else if (strcmp (str, "above") == 0)
    return META_BUTTON_FUNCTION_ABOVE;
  else if (strcmp (str, "stick") == 0)
    return META_BUTTON_FUNCTION_STICK;
  else
    return META_BUTTON_FUNCTION_LAST;
}

static MetaButtonFunction
button_opposite_function (MetaButtonFunction ofwhat)
{
  switch (ofwhat)
    {
    case META_BUTTON_FUNCTION_SHADE:
      return META_BUTTON_FUNCTION_UNSHADE;
    case META_BUTTON_FUNCTION_UNSHADE:
      return META_BUTTON_FUNCTION_SHADE;
    case META_BUTTON_FUNCTION_ABOVE:
      return META_BUTTON_FUNCTION_UNABOVE;
    case META_BUTTON_FUNCTION_UNABOVE:
      return META_BUTTON_FUNCTION_ABOVE;
    case META_BUTTON_FUNCTION_STICK:
      return META_BUTTON_FUNCTION_UNSTICK;
    case META_BUTTON_FUNCTION_UNSTICK:
      return META_BUTTON_FUNCTION_STICK;
    default:
      return META_BUTTON_FUNCTION_LAST;
    }
}

static void
parse_button_side (const char         *side,
                   MetaButtonFunction *buttons,
                   gboolean           *has_spacer)
{
  gboolean used[META_BUTTON_FUNCTION_LAST];
  char **names = g_strsplit (side, ",", -1);
  int i = 0;

  for (int f = 0; f < META_BUTTON_FUNCTION_LAST; f++)
    {
      used[f] = FALSE;
      has_spacer[f] = FALSE;
    }

  for (int b = 0; names[b] != NULL; b++)
    {
      MetaButtonFunction f = button_function_from_string (names[b]);

      if (i > 0 && strcmp ("spacer", names[b]) == 0)
        {
          has_spacer[i - 1] = TRUE;
          f = button_opposite_function (f);
          if (f != META_BUTTON_FUNCTION_LAST)
            has_spacer[i - 2] = TRUE;
        }
      else if (f != META_BUTTON_FUNCTION_LAST && !used[f])
        {
          buttons[i] = f;
          used[f] = TRUE;
          ++i;

          f = button_opposite_function (f);
          if (f != META_BUTTON_FUNCTION_LAST)
            buttons[i++] = f;
        }
    }

  /* Marco writes the terminator past the end of the array when a side names every
   * function; refuse such a layout rather than do the same. */
  if (i >= MAX_BUTTONS_PER_CORNER)
    fail ("cases: button layout side \"%s\" has more buttons than Marco has room for", side);
  buttons[i] = META_BUTTON_FUNCTION_LAST;
  has_spacer[i] = FALSE;

  g_strfreev (names);
}

static void
parse_button_layout (const char       *string,
                     MetaButtonLayout *layout)
{
  char **sides = g_strsplit (string, ":", 2);

  /* Marco keeps the previous layout for a missing side; the goldens have none before. */
  layout->left_buttons[0] = META_BUTTON_FUNCTION_LAST;
  layout->right_buttons[0] = META_BUTTON_FUNCTION_LAST;

  if (sides[0] != NULL)
    {
      parse_button_side (sides[0], layout->left_buttons, layout->left_buttons_has_spacer);
      if (sides[1] != NULL)
        parse_button_side (sides[1], layout->right_buttons, layout->right_buttons_has_spacer);
    }

  g_strfreev (sides);
}

/* GTK setup */

/* Pins GTK's settings to the case list's, then checks that the pango context a frame
 * widget gets has them: a machine whose GTK resolves them differently would otherwise
 * render goldens that nothing on Caminito's side reproduces. */
static GtkWidget *
frame_widget_new (JsonObject *cases)
{
  const char *antialias = member_string (cases, "antialias");
  const char *hint_style = member_string (cases, "hint_style");
  const char *hint_metrics = member_string (cases, "hint_metrics");
  int dpi = member_int (cases, "dpi");
  GtkWidget *widget;
  PangoContext *context;
  const cairo_font_options_t *options;

  if (strcmp (antialias, "gray") != 0)
    fail ("cases: only \"gray\" antialiasing is supported");
  if (strcmp (hint_style, "slight") != 0)
    fail ("cases: only \"slight\" hinting is supported");

  g_object_set (gtk_settings_get_default (),
                "gtk-theme-name", member_string (cases, "gtk_theme"),
                "gtk-application-prefer-dark-theme", FALSE,
                "gtk-font-name", member_string (cases, "font"),
                "gtk-xft-antialias", 1,
                "gtk-xft-hinting", 1,
                "gtk-xft-hintstyle", "hintslight",
                "gtk-xft-rgba", "none",
                "gtk-xft-dpi", dpi * 1024,
                NULL);

  /* MetaFrames is a GtkWindow of type popup; its style contexts take this path. */
  widget = gtk_window_new (GTK_WINDOW_POPUP);

  context = gtk_widget_get_pango_context (widget);
  pango_context_set_language (context,
                              pango_language_from_string (member_string (cases, "language")));

  if (pango_context_get_language (context) !=
      pango_language_from_string (member_string (cases, "language")))
    fail ("GTK's pango language is %s, not %s",
          pango_language_to_string (pango_context_get_language (context)),
          member_string (cases, "language"));

  options = pango_cairo_context_get_font_options (context);
  if (options == NULL)
    fail ("GTK set no font options");
  if (cairo_font_options_get_antialias (options) != CAIRO_ANTIALIAS_GRAY)
    fail ("GTK's antialiasing is %d, not gray", cairo_font_options_get_antialias (options));
  if (cairo_font_options_get_hint_style (options) != CAIRO_HINT_STYLE_SLIGHT)
    fail ("GTK's hint style is %d, not slight", cairo_font_options_get_hint_style (options));
  if (cairo_font_options_get_hint_metrics (options) !=
      (strcmp (hint_metrics, "on") == 0 ? CAIRO_HINT_METRICS_ON : CAIRO_HINT_METRICS_OFF))
    fail ("GTK's hint metrics are %d, not %s",
          cairo_font_options_get_hint_metrics (options), hint_metrics);
  if (pango_cairo_context_get_resolution (context) != dpi)
    fail ("GTK's resolution is %g, not %d", pango_cairo_context_get_resolution (context), dpi);

  return widget;
}

/* frames.c's create_style_context: the frame widget's path, with the GTK theme's
 * variant added at settings priority. */
static GtkStyleContext *
create_style_context (GtkWidget  *widget,
                      const char *variant)
{
  GtkStyleContext *style = gtk_style_context_new ();
  char *theme_name;

  g_object_get (gtk_settings_get_default (), "gtk-theme-name", &theme_name, NULL);

  gtk_style_context_set_path (style, gtk_widget_get_path (widget));
  gtk_style_context_add_provider (style,
                                  GTK_STYLE_PROVIDER (gtk_css_provider_get_named (theme_name, variant)),
                                  GTK_STYLE_PROVIDER_PRIORITY_SETTINGS);

  g_free (theme_name);
  return style;
}

/* Output */

static void
write_json (JsonNode   *root,
            const char *path)
{
  JsonGenerator *generator = json_generator_new ();
  GError *error = NULL;
  char *text;
  char *with_newline;

  json_generator_set_pretty (generator, TRUE);
  json_generator_set_indent (generator, 2);
  json_generator_set_root (generator, root);
  text = json_generator_to_data (generator, NULL);
  with_newline = g_strconcat (text, "\n", NULL);

  if (!g_file_set_contents (path, with_newline, -1, &error))
    fail ("%s", error->message);

  g_free (with_newline);
  g_free (text);
  g_object_unref (generator);
}

static gboolean
rect_is_empty (const GdkRectangle *rect)
{
  return rect->x == 0 && rect->y == 0 && rect->width == 0 && rect->height == 0;
}

static void
add_rect (JsonBuilder        *builder,
          const GdkRectangle *rect)
{
  json_builder_begin_object (builder);
  json_builder_set_member_name (builder, "x");
  json_builder_add_int_value (builder, rect->x);
  json_builder_set_member_name (builder, "y");
  json_builder_add_int_value (builder, rect->y);
  json_builder_set_member_name (builder, "width");
  json_builder_add_int_value (builder, rect->width);
  json_builder_set_member_name (builder, "height");
  json_builder_add_int_value (builder, rect->height);
  json_builder_end_object (builder);
}

static void
add_border (JsonBuilder     *builder,
            const char      *name,
            const GtkBorder *border)
{
  json_builder_set_member_name (builder, name);
  json_builder_begin_object (builder);
  json_builder_set_member_name (builder, "left");
  json_builder_add_int_value (builder, border->left);
  json_builder_set_member_name (builder, "right");
  json_builder_add_int_value (builder, border->right);
  json_builder_set_member_name (builder, "top");
  json_builder_add_int_value (builder, border->top);
  json_builder_set_member_name (builder, "bottom");
  json_builder_add_int_value (builder, border->bottom);
  json_builder_end_object (builder);
}

static void
add_int (JsonBuilder *builder,
         const char  *name,
         gint64       value)
{
  json_builder_set_member_name (builder, name);
  json_builder_add_int_value (builder, value);
}

/* A background rect, left out when Marco left it empty. */
static void
add_background (JsonBuilder        *builder,
                const char         *name,
                const GdkRectangle *rect)
{
  if (rect_is_empty (rect))
    return;
  json_builder_set_member_name (builder, name);
  add_rect (builder, rect);
}

/* The middle backgrounds, up to the last one Marco filled in. */
static void
add_middle_backgrounds (JsonBuilder        *builder,
                        const char         *name,
                        const GdkRectangle *rects)
{
  int n = MAX_MIDDLE_BACKGROUNDS;

  while (n > 0 && rect_is_empty (&rects[n - 1]))
    n--;
  if (n == 0)
    return;

  json_builder_set_member_name (builder, name);
  json_builder_begin_array (builder);
  for (int i = 0; i < n; i++)
    add_rect (builder, &rects[i]);
  json_builder_end_array (builder);
}

static JsonNode *
geometry_to_json (const char              *theme_name,
                  const char              *case_name,
                  int                      scale,
                  MetaFrameType            type,
                  MetaFrameFlags           flags,
                  int                      text_height,
                  const MetaFrameGeometry *fgeom)
{
  JsonBuilder *builder = json_builder_new ();
  const struct
  {
    const char *name;
    const MetaButtonSpace *space;
  } buttons[] = {
    { "close", &fgeom->close_rect },
    { "maximize", &fgeom->max_rect },
    { "minimize", &fgeom->min_rect },
    { "menu", &fgeom->menu_rect },
    { "appmenu", &fgeom->appmenu_rect },
    { "shade", &fgeom->shade_rect },
    { "above", &fgeom->above_rect },
    { "stick", &fgeom->stick_rect },
    { "unshade", &fgeom->unshade_rect },
    { "unabove", &fgeom->unabove_rect },
    { "unstick", &fgeom->unstick_rect },
  };
  JsonNode *root;

  json_builder_begin_object (builder);

  json_builder_set_member_name (builder, "theme");
  json_builder_add_string_value (builder, theme_name);
  json_builder_set_member_name (builder, "case");
  json_builder_add_string_value (builder, case_name);
  add_int (builder, "scale", scale);
  json_builder_set_member_name (builder, "type");
  json_builder_add_string_value (builder, meta_frame_type_to_string (type));
  json_builder_set_member_name (builder, "flags");
  json_builder_add_value (builder, json_node_init_array (json_node_alloc (), flags_to_array (flags)));
  add_int (builder, "text_height", text_height);

  add_int (builder, "width", fgeom->width);
  add_int (builder, "height", fgeom->height);

  json_builder_set_member_name (builder, "borders");
  json_builder_begin_object (builder);
  add_border (builder, "visible", &fgeom->borders.visible);
  add_border (builder, "invisible", &fgeom->borders.invisible);
  add_border (builder, "total", &fgeom->borders.total);
  json_builder_end_object (builder);

  json_builder_set_member_name (builder, "title_rect");
  add_rect (builder, &fgeom->title_rect);

  json_builder_set_member_name (builder, "titlebar_edges");
  json_builder_begin_object (builder);
  add_int (builder, "left", fgeom->left_titlebar_edge);
  add_int (builder, "right", fgeom->right_titlebar_edge);
  add_int (builder, "top", fgeom->top_titlebar_edge);
  add_int (builder, "bottom", fgeom->bottom_titlebar_edge);
  json_builder_end_object (builder);

  /* The buttons Marco placed; one it dropped, or that the layout or flags leave out, has
   * both rects empty. */
  json_builder_set_member_name (builder, "buttons");
  json_builder_begin_object (builder);
  for (gsize i = 0; i < G_N_ELEMENTS (buttons); i++)
    {
      if (rect_is_empty (&buttons[i].space->visible) &&
          rect_is_empty (&buttons[i].space->clickable))
        continue;
      json_builder_set_member_name (builder, buttons[i].name);
      json_builder_begin_object (builder);
      json_builder_set_member_name (builder, "visible");
      add_rect (builder, &buttons[i].space->visible);
      json_builder_set_member_name (builder, "clickable");
      add_rect (builder, &buttons[i].space->clickable);
      json_builder_end_object (builder);
    }
  json_builder_end_object (builder);

  json_builder_set_member_name (builder, "button_backgrounds");
  json_builder_begin_object (builder);
  add_background (builder, "left_left", &fgeom->left_left_background);
  add_middle_backgrounds (builder, "left_middle", fgeom->left_middle_backgrounds);
  add_background (builder, "left_right", &fgeom->left_right_background);
  add_background (builder, "left_single", &fgeom->left_single_background);
  add_background (builder, "right_left", &fgeom->right_left_background);
  add_middle_backgrounds (builder, "right_middle", fgeom->right_middle_backgrounds);
  add_background (builder, "right_right", &fgeom->right_right_background);
  add_background (builder, "right_single", &fgeom->right_single_background);
  json_builder_end_object (builder);

  json_builder_set_member_name (builder, "corner_radii");
  json_builder_begin_object (builder);
  add_int (builder, "top_left", fgeom->top_left_corner_rounded_radius);
  add_int (builder, "top_right", fgeom->top_right_corner_rounded_radius);
  add_int (builder, "bottom_left", fgeom->bottom_left_corner_rounded_radius);
  add_int (builder, "bottom_right", fgeom->bottom_right_corner_rounded_radius);
  json_builder_end_object (builder);

  json_builder_end_object (builder);

  root = json_builder_get_root (builder);
  g_object_unref (builder);
  return root;
}

/* frames */

static GdkPixbuf *
load_icon (const char *cases_dir,
           const char *relative)
{
  char *path = g_build_filename (cases_dir, relative, NULL);
  GError *error = NULL;
  GdkPixbuf *pixbuf = gdk_pixbuf_new_from_file (path, &error);

  if (pixbuf == NULL)
    fail ("%s", error->message);
  g_free (path);
  return pixbuf;
}

static void
dump_case (MetaTheme       *theme,
           const char      *theme_name,
           GtkWidget       *widget,
           GtkStyleContext *style,
           JsonObject      *cases,
           JsonObject      *base,
           JsonObject      *case_object,
           int              scale,
           GdkPixbuf       *mini_icon,
           GdkPixbuf       *icon,
           const char      *out_dir)
{
  const char *name = member_string (case_object, "name");
  const char *type_name = json_object_get_string_member_with_default (case_object, "type", member_string (base, "type"));
  const char *layout_string = json_object_get_string_member_with_default (case_object, "button_layout", member_string (base, "button_layout"));
  int client_width = member_int (cases, "client_width");
  int client_height = member_int (cases, "client_height");
  MetaFrameType type = meta_frame_type_from_string (type_name);
  MetaFrameFlags flags = flags_from_array (json_object_get_array_member (base, "flags"));
  MetaButtonState button_states[META_BUTTON_TYPE_LAST];
  MetaButtonLayout button_layout;
  MetaFrameGeometry fgeom;
  PangoFontDescription *override;
  PangoFontDescription *font_desc;
  PangoLayout *title_layout;
  int text_height;
  cairo_surface_t *surface;
  cairo_t *cr;
  JsonNode *geometry;
  char *stem;
  char *path;

  if (type == META_FRAME_TYPE_LAST)
    fail ("cases: unknown frame type \"%s\"", type_name);
  if (json_object_has_member (case_object, "add_flags"))
    flags |= flags_from_array (json_object_get_array_member (case_object, "add_flags"));
  if (json_object_has_member (case_object, "remove_flags"))
    flags &= ~flags_from_array (json_object_get_array_member (case_object, "remove_flags"));
  button_states_from_object (json_object_has_member (case_object, "button_states")
                               ? json_object_get_object_member (case_object, "button_states")
                               : member_object (base, "button_states"),
                             button_states);
  parse_button_layout (layout_string, &button_layout);

  /* frames.c's meta_frames_ensure_layout, with the title font given explicitly where
   * Marco reads the titlebar-font key. */
  title_layout = gtk_widget_create_pango_layout (widget, member_string (cases, "title"));
  pango_layout_set_ellipsize (title_layout, PANGO_ELLIPSIZE_END);
  pango_layout_set_auto_dir (title_layout, FALSE);
  pango_layout_set_single_paragraph_mode (title_layout, TRUE);

  override = pango_font_description_from_string (member_string (cases, "font"));
  font_desc = meta_gtk_widget_get_font_desc (widget,
                                             meta_theme_get_title_scale (theme, type, flags),
                                             override);
  text_height = meta_pango_font_desc_get_text_height (font_desc,
                                                      gtk_widget_get_pango_context (widget));
  pango_layout_set_font_description (title_layout, font_desc);

  meta_theme_calc_geometry (theme, type, text_height, flags,
                            client_width, client_height,
                            &button_layout, &fgeom);

  /* A GTK window at scale 2 draws on a surface with a device scale of 2. */
  surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
                                        fgeom.width * scale, fgeom.height * scale);
  cairo_surface_set_device_scale (surface, scale, scale);
  cr = cairo_create (surface);
  meta_theme_draw_frame (theme, style, cr, type, flags,
                         client_width, client_height,
                         title_layout, text_height,
                         &button_layout, button_states,
                         mini_icon, icon);
  cairo_destroy (cr);

  stem = g_strdup_printf ("%s/%s@%dx", out_dir, name, scale);
  path = g_strconcat (stem, ".png", NULL);
  if (cairo_surface_write_to_png (surface, path) != CAIRO_STATUS_SUCCESS)
    fail ("couldn't write %s", path);
  g_free (path);

  path = g_strconcat (stem, ".json", NULL);
  geometry = geometry_to_json (theme_name, name, scale, type, flags, text_height, &fgeom);
  write_json (geometry, path);
  json_node_unref (geometry);
  g_free (path);
  g_free (stem);

  cairo_surface_destroy (surface);
  pango_font_description_free (font_desc);
  pango_font_description_free (override);
  g_object_unref (title_layout);
}

static void
dump_frames (JsonObject *cases,
             const char *cases_dir,
             const char *out_dir)
{
  GtkWidget *widget = frame_widget_new (cases);
  JsonArray *themes = json_object_get_array_member (cases, "themes");
  JsonArray *scales = json_object_get_array_member (cases, "scales");
  JsonArray *list = json_object_get_array_member (cases, "cases");
  JsonObject *base = member_object (cases, "base");
  GdkPixbuf *mini_icon = load_icon (cases_dir, member_string (cases, "mini_icon"));
  GdkPixbuf *icon = load_icon (cases_dir, member_string (cases, "icon"));
  /* Marco loads theme images at this scale too (theme.c:5663). */
  int scale = gdk_window_get_scale_factor (gdk_get_default_root_window ());
  gboolean listed = FALSE;

  for (guint i = 0; i < json_array_get_length (scales); i++)
    listed |= json_array_get_int_element (scales, i) == scale;
  if (!listed)
    fail ("GDK's scale is %d, which the case list doesn't have", scale);

  for (guint t = 0; t < json_array_get_length (themes); t++)
    {
      const char *theme_name = json_array_get_string_element (themes, t);
      GError *error = NULL;
      MetaTheme *theme = meta_theme_load (theme_name, &error);
      GtkStyleContext *style;
      char *theme_dir;

      if (theme == NULL)
        fail ("%s: %s", theme_name, error ? error->message : "not found");

      theme_dir = g_build_filename (out_dir, theme_name, NULL);
      if (g_mkdir_with_parents (theme_dir, 0755) != 0)
        fail ("couldn't create %s", theme_dir);

      /* One context per theme, as a frame keeps one across draws. */
      style = create_style_context (widget, NULL);
      for (guint c = 0; c < json_array_get_length (list); c++)
        dump_case (theme, theme_name, widget, style, cases, base,
                   json_array_get_object_element (list, c),
                   scale, mini_icon, icon, theme_dir);

      g_object_unref (style);
      g_free (theme_dir);
      meta_theme_free (theme);
    }

  g_object_unref (icon);
  g_object_unref (mini_icon);
  gtk_widget_destroy (widget);
}

/* palette */

/* The states in caminito-theme's GtkState::ALL order. */
static const char *const states[] = {
  "normal", "prelight", "active", "selected",
  "insensitive", "inconsistent", "focused", "backdrop",
};

static void
add_colour (JsonBuilder   *builder,
            const GdkRGBA *rgba)
{
  json_builder_begin_array (builder);
  json_builder_add_double_value (builder, rgba->red);
  json_builder_add_double_value (builder, rgba->green);
  json_builder_add_double_value (builder, rgba->blue);
  json_builder_add_double_value (builder, rgba->alpha);
  json_builder_end_array (builder);
}

/* A colour as Marco renders the spec against the frames' style context, so the
 * background class and the transparent-background fallback are Marco's own. */
static void
add_spec_colour (JsonBuilder     *builder,
                 GtkStyleContext *style,
                 const char      *spec_string)
{
  GError *error = NULL;
  MetaColorSpec *spec = meta_color_spec_new_from_string (spec_string, &error);
  GdkRGBA rgba;

  if (spec == NULL)
    fail ("%s: %s", spec_string, error->message);
  meta_color_spec_render (spec, style, &rgba);
  add_colour (builder, &rgba);
  meta_color_spec_free (spec);
}

static void
add_variant (JsonBuilder *builder,
             GtkWidget   *widget,
             const char  *variant,
             char       **names)
{
  GtkStyleContext *style = create_style_context (widget, variant);

  json_builder_begin_object (builder);

  json_builder_set_member_name (builder, "states");
  json_builder_begin_object (builder);
  for (gsize s = 0; s < G_N_ELEMENTS (states); s++)
    {
      char *upper = g_ascii_strup (states[s], -1);
      char *bg = g_strdup_printf ("gtk:bg[%s]", upper);
      char *fg = g_strdup_printf ("gtk:fg[%s]", upper);

      json_builder_set_member_name (builder, states[s]);
      json_builder_begin_object (builder);
      json_builder_set_member_name (builder, "background");
      add_spec_colour (builder, style, bg);
      json_builder_set_member_name (builder, "text");
      add_spec_colour (builder, style, fg);
      json_builder_end_object (builder);

      g_free (fg);
      g_free (bg);
      g_free (upper);
    }
  json_builder_end_object (builder);

  /* gtk:custom's lookup (theme.c's meta_set_custom_color_from_style). */
  json_builder_set_member_name (builder, "named");
  json_builder_begin_object (builder);
  for (int n = 0; names[n] != NULL; n++)
    {
      GdkRGBA rgba;

      json_builder_set_member_name (builder, names[n]);
      if (gtk_style_context_lookup_color (style, names[n], &rgba))
        add_colour (builder, &rgba);
      else
        json_builder_add_null_value (builder);
    }
  json_builder_end_object (builder);

  json_builder_end_object (builder);
  g_object_unref (style);
}

static int
compare_strings (const void *a,
                 const void *b)
{
  return strcmp (*(char *const *) a, *(char *const *) b);
}

static void
dump_palette (JsonObject *cases,
              const char *out_path,
              char      **names)
{
  GtkWidget *widget = frame_widget_new (cases);
  JsonBuilder *builder = json_builder_new ();
  JsonNode *root;

  qsort (names, g_strv_length (names), sizeof (char *), compare_strings);

  json_builder_begin_object (builder);
  json_builder_set_member_name (builder, "gtk_theme");
  json_builder_add_string_value (builder, member_string (cases, "gtk_theme"));
  json_builder_set_member_name (builder, "variants");
  json_builder_begin_object (builder);
  json_builder_set_member_name (builder, "default");
  add_variant (builder, widget, NULL, names);
  json_builder_set_member_name (builder, "dark");
  add_variant (builder, widget, "dark", names);
  json_builder_end_object (builder);
  json_builder_end_object (builder);

  root = json_builder_get_root (builder);
  write_json (root, out_path);
  json_node_unref (root);
  g_object_unref (builder);
  gtk_widget_destroy (widget);
}

int
main (int argc, char **argv)
{
  JsonParser *parser;
  GError *error = NULL;
  JsonObject *cases;
  char *cases_dir;

  gtk_init (&argc, &argv);

  if (argc < 4 ||
      (strcmp (argv[1], "frames") != 0 && strcmp (argv[1], "palette") != 0) ||
      (strcmp (argv[1], "frames") == 0 && argc != 4))
    {
      fputs ("usage: marco-frame-dump frames CASES OUT\n"
             "       marco-frame-dump palette CASES OUT NAME...\n", stderr);
      return 2;
    }

  parser = json_parser_new ();
  if (!json_parser_load_from_file (parser, argv[2], &error))
    fail ("%s: %s", argv[2], error->message);
  cases = json_node_get_object (json_parser_get_root (parser));
  cases_dir = g_path_get_dirname (argv[2]);

  if (strcmp (argv[1], "frames") == 0)
    dump_frames (cases, cases_dir, argv[3]);
  else
    dump_palette (cases, argv[3], argv + 4);

  g_free (cases_dir);
  g_object_unref (parser);
  return 0;
}
