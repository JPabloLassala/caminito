/*
 * caminito-gtk-palette: the GTK colours Marco themes refer to, for Caminito's frames (07,
 * ADR 0003).
 *
 * Caminito starts it as a Wayland client of itself. It creates no windows. For the default
 * and the dark variant of the GTK theme in org.mate.interface gtk-theme, it reads
 * window.background's colours in every state Marco maps, as Marco's frames.c and
 * theme.c read them, and looks up the gtk:custom names Caminito asks for.
 *
 * stdin: lines "names NAME...", the names to look up. Each one replaces the last and
 *        reports a palette. At end of file the helper exits.
 * stdout: the header "caminito-gtk-palette 1", then a palette for each names line and for
 *         each change of gtk-theme after the first names line. The format is
 *         caminito-theme's (src/palette/stream.rs).
 */

#include <stdlib.h>
#include <string.h>

#include <gio/gunixinputstream.h>
#include <gtk/gtk.h>

#define HEADER "caminito-gtk-palette 1"

/* The states in caminito-theme's GtkState::ALL order, with Marco's flags for them
 * (theme.c:6743). */
static const struct
{
  const char   *name;
  GtkStateFlags flags;
} states[] = {
  { "normal", GTK_STATE_FLAG_NORMAL },
  { "prelight", GTK_STATE_FLAG_PRELIGHT },
  { "active", GTK_STATE_FLAG_ACTIVE },
  { "selected", GTK_STATE_FLAG_SELECTED },
  { "insensitive", GTK_STATE_FLAG_INSENSITIVE },
  { "inconsistent", GTK_STATE_FLAG_INCONSISTENT },
  { "focused", GTK_STATE_FLAG_FOCUSED },
  { "backdrop", GTK_STATE_FLAG_BACKDROP },
};

typedef struct
{
  GSettings        *interface;
  GtkWidget        *frame;
  GDataInputStream *input;
  /* The names Caminito asked for, or NULL before it has. */
  char            **names;
} Helper;

static void
write_channels (GString       *out,
                const GdkRGBA *rgba)
{
  const double channels[] = { rgba->red, rgba->green, rgba->blue, rgba->alpha };
  char buffer[G_ASCII_DTOSTR_BUF_SIZE];

  /* g_ascii_dtostr: the C locale's decimal point, and digits that read back exactly. */
  for (gsize i = 0; i < G_N_ELEMENTS (channels); i++)
    {
      g_string_append_c (out, ' ');
      g_string_append (out, g_ascii_dtostr (buffer, sizeof buffer, channels[i]));
    }
}

/* frames.c's create_style_context: the frame widget's path, with the GTK theme's variant
 * added at settings priority. */
static GtkStyleContext *
create_style_context (GtkWidget  *frame,
                      const char *theme,
                      const char *variant)
{
  GtkStyleContext *style = gtk_style_context_new ();

  gtk_style_context_set_path (style, gtk_widget_get_path (frame));
  gtk_style_context_add_provider (style,
                                  GTK_STYLE_PROVIDER (gtk_css_provider_get_named (theme, variant)),
                                  GTK_STYLE_PROVIDER_PRIORITY_SETTINGS);
  return style;
}

/* theme.c's get_background_color_real. */
static void
get_background_color_real (GtkStyleContext *style,
                           GtkStateFlags    state,
                           GdkRGBA         *color)
{
  GdkRGBA *c;

  gtk_style_context_get (style, state, "background-color", &c, NULL);
  *color = *c;
  gdk_rgba_free (c);
}

/* theme.c's get_background_color: a transparent background is replaced by a fresh
 * toplevel window's. */
static void
get_background_color (GtkStyleContext *style,
                      GtkStateFlags    state,
                      GdkRGBA         *color)
{
  const GdkRGBA empty = { 0.0, 0.0, 0.0, 0.0 };

  get_background_color_real (style, state, color);
  if (gdk_rgba_equal (color, &empty))
    {
      GtkWidget *toplevel = gtk_window_new (GTK_WINDOW_TOPLEVEL);

      get_background_color_real (gtk_widget_get_style_context (toplevel), state, color);
      gtk_widget_destroy (toplevel);
    }
}

static void
write_variant (GString    *out,
               Helper     *helper,
               const char *theme,
               const char *variant)
{
  GtkStyleContext *style = create_style_context (helper->frame, theme, variant);

  g_string_append_printf (out, "variant %s\n", variant == NULL ? "default" : variant);

  /* theme.c's meta_set_color_from_style: bg reads background-color and fg reads color,
   * with the background class added. */
  gtk_style_context_add_class (style, GTK_STYLE_CLASS_BACKGROUND);
  for (gsize s = 0; s < G_N_ELEMENTS (states); s++)
    {
      GdkRGBA background, text;

      get_background_color (style, states[s].flags, &background);
      gtk_style_context_get_color (style, states[s].flags, &text);
      g_string_append_printf (out, "state %s", states[s].name);
      write_channels (out, &background);
      write_channels (out, &text);
      g_string_append_c (out, '\n');
    }

  /* theme.c's meta_set_custom_color_from_style. A name the theme doesn't define is left
   * out, and Marco uses the spec's fallback. */
  for (char **name = helper->names; *name != NULL; name++)
    {
      GdkRGBA rgba;

      if (gtk_style_context_lookup_color (style, *name, &rgba))
        {
          g_string_append_printf (out, "named %s", *name);
          write_channels (out, &rgba);
          g_string_append_c (out, '\n');
        }
    }

  g_object_unref (style);
}

static void
report (Helper *helper)
{
  char *theme = g_settings_get_string (helper->interface, "gtk-theme");
  GString *out;

  if (theme[0] == '\0' || strchr (theme, '\n') != NULL)
    {
      g_warning ("gtk-theme %s can't be reported", theme);
      g_free (theme);
      return;
    }

  /* The fallback toplevel in get_background_color reads the settings' theme. */
  g_object_set (gtk_settings_get_default (), "gtk-theme-name", theme, NULL);

  out = g_string_new (NULL);
  g_string_append_printf (out, "palette %s\n", theme);
  write_variant (out, helper, theme, NULL);
  write_variant (out, helper, theme, "dark");
  g_string_append (out, "end\n");

  if (fwrite (out->str, 1, out->len, stdout) != out->len || fflush (stdout) != 0)
    {
      g_printerr ("caminito-gtk-palette: can't write to stdout\n");
      exit (1);
    }

  g_string_free (out, TRUE);
  g_free (theme);
}

static void
gtk_theme_changed (GSettings  *settings G_GNUC_UNUSED,
                   const char *key G_GNUC_UNUSED,
                   Helper     *helper)
{
  if (helper->names != NULL)
    report (helper);
}

static void read_next_line (Helper *helper);

static void
line_read (GObject      *source G_GNUC_UNUSED,
           GAsyncResult *result,
           gpointer      data)
{
  Helper *helper = data;
  GError *error = NULL;
  char *line = g_data_input_stream_read_line_finish (helper->input, result, NULL, &error);

  if (line == NULL)
    {
      if (error != NULL)
        {
          g_printerr ("caminito-gtk-palette: can't read stdin: %s\n", error->message);
          g_error_free (error);
        }
      gtk_main_quit ();
      return;
    }

  if (strcmp (line, "names") == 0 || g_str_has_prefix (line, "names "))
    {
      g_strfreev (helper->names);
      helper->names = g_strsplit (line[5] == '\0' ? "" : line + 6, " ", -1);
      report (helper);
    }
  else
    g_warning ("ignoring a line it doesn't know: %s", line);

  g_free (line);
  read_next_line (helper);
}

static void
read_next_line (Helper *helper)
{
  g_data_input_stream_read_line_async (helper->input, G_PRIORITY_DEFAULT, NULL, line_read,
                                       helper);
}

int
main (int argc, char **argv)
{
  GSettingsSchema *schema;
  GInputStream *stdin_stream;
  Helper helper = { 0 };

  if (!gtk_init_check (&argc, &argv))
    {
      g_printerr ("caminito-gtk-palette: can't open a display\n");
      return 1;
    }

  schema = g_settings_schema_source_lookup (g_settings_schema_source_get_default (),
                                            "org.mate.interface", TRUE);
  if (schema == NULL)
    {
      g_printerr ("caminito-gtk-palette: the org.mate.interface schema isn't installed\n");
      return 1;
    }
  g_settings_schema_unref (schema);

  helper.interface = g_settings_new ("org.mate.interface");
  g_signal_connect (helper.interface, "changed::gtk-theme", G_CALLBACK (gtk_theme_changed),
                    &helper);

  /* MetaFrames is a GtkWindow of type popup; its style contexts take this path. It's
   * never realized, so no surface is made. */
  helper.frame = gtk_window_new (GTK_WINDOW_POPUP);

  stdin_stream = g_unix_input_stream_new (0, FALSE);
  helper.input = g_data_input_stream_new (stdin_stream);
  g_object_unref (stdin_stream);

  fputs (HEADER "\n", stdout);
  fflush (stdout);

  read_next_line (&helper);
  gtk_main ();
  return 0;
}
