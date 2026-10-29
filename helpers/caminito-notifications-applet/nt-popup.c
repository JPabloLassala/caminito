/*
 * The Notification center (CMN-260). See nt-popup.h.
 *
 * Each update rebuilds the list: the history holds at most 200 entries, and it changes a
 * few times a minute at most, so keeping rows in step with ids would cost more than it
 * saves.
 */

#include "nt-popup.h"

/* The popup's width, and the tallest the list grows before it scrolls. */
#define WIDTH        380
#define LIST_HEIGHT  480
#define APP_ICON_SIZE 16
#define IMAGE_SIZE   48
/* The body's lines before it's cut short. */
#define BODY_LINES   4

/* An age shown, which the minute's tick brings up to date. */
typedef struct
{
  GtkWidget *label;
  gint64     time;
} Age;

struct _NtPopup
{
  CaminitoAppletPopup *applet_popup;
  NtPopupActions       actions;
  gpointer             data;
  GtkWidget           *stack;
  GtkWidget           *scrolled;
  GtkWidget           *list;
  GtkWidget           *clear_all;
  GtkWidget           *do_not_disturb;
  GtkWidget           *until;
  GtkWidget           *durations;
  /* Set while the switch is set to the daemon's state, which calls nothing back. */
  gboolean             updating;
  GArray              *ages;
  guint                tick;
};

static void
set_ages (NtPopup *popup, gint64 now)
{
  for (guint i = 0; i < popup->ages->len; i++)
    {
      Age *age = &g_array_index (popup->ages, Age, i);
      g_autofree char *text = nt_state_age (age->time, now);

      gtk_label_set_text (GTK_LABEL (age->label), text);
    }
}

static gint64
now (void)
{
  return g_get_real_time () / G_USEC_PER_SEC;
}

static gboolean
on_tick (gpointer data)
{
  set_ages (data, now ());
  return G_SOURCE_CONTINUE;
}

/* The ages are right as the popup opens, and stay right while it shows. */
static void
on_map (GtkWidget *window G_GNUC_UNUSED, gpointer data)
{
  NtPopup *popup = data;

  set_ages (popup, now ());
  if (popup->tick == 0)
    popup->tick = g_timeout_add_seconds (60, on_tick, popup);
}

static void
on_unmap (GtkWidget *window G_GNUC_UNUSED, gpointer data)
{
  NtPopup *popup = data;

  g_clear_handle_id (&popup->tick, g_source_remove);
}

/* A path or a file: URI, as the spec allows for app_icon and image-path. */
static char *
icon_file (const char *icon)
{
  if (g_str_has_prefix (icon, "file://"))
    return g_filename_from_uri (icon, NULL, NULL);
  if (g_path_is_absolute (icon))
    return g_strdup (icon);
  return NULL;
}

/* icon, a file or a theme's name, at most size pixels each way; NULL if there's none. */
static GdkPixbuf *
load_icon (const char *icon, int size)
{
  g_autofree char *file = NULL;
  g_autoptr (GError) error = NULL;
  GdkPixbuf *pixbuf;

  if (icon == NULL || *icon == '\0')
    return NULL;
  file = icon_file (icon);
  if (file != NULL)
    pixbuf = gdk_pixbuf_new_from_file_at_scale (file, size, size, TRUE, &error);
  else
    pixbuf = gtk_icon_theme_load_icon (gtk_icon_theme_get_default (), icon, size,
                                       GTK_ICON_LOOKUP_FORCE_SIZE, &error);
  if (pixbuf == NULL)
    g_debug ("can't load the icon %s: %s", icon, error->message);
  return pixbuf;
}

/* image-data's pixels as (iiibiiay), scaled to IMAGE_SIZE each way at most, keeping their
   shape; NULL if they don't add up. */
static GdkPixbuf *
load_pixels (GVariant *image)
{
  g_autoptr (GVariant) data = NULL;
  g_autoptr (GBytes) bytes = NULL;
  g_autoptr (GdkPixbuf) pixbuf = NULL;
  gint32 width, height, rowstride, bits, channels;
  gboolean alpha;
  double scale;

  g_variant_get (image, "(iiibii@ay)", &width, &height, &rowstride, &alpha, &bits, &channels,
                 &data);
  if (width <= 0 || height <= 0 || bits != 8 || channels != (alpha ? 4 : 3)
      || (gsize) rowstride < (gsize) width * channels
      || g_variant_get_size (data) < (gsize) rowstride * (height - 1) + (gsize) width * channels)
    {
      g_debug ("can't draw a notification's image: its pixels don't add up");
      return NULL;
    }
  bytes = g_variant_get_data_as_bytes (data);
  pixbuf = gdk_pixbuf_new_from_bytes (bytes, GDK_COLORSPACE_RGB, alpha, bits, width, height,
                                      rowstride);
  scale = (double) IMAGE_SIZE / MAX (width, height);
  return gdk_pixbuf_scale_simple (pixbuf, MAX (1, (int) (width * scale)),
                                  MAX (1, (int) (height * scale)), GDK_INTERP_BILINEAR);
}

static GtkWidget *
dim_label (const char *text)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_style_context_add_class (gtk_widget_get_style_context (label), "dim-label");
  gtk_label_set_xalign (GTK_LABEL (label), 0);
  return label;
}

static void
close_and_invoke (NtPopup *popup, guint32 id, const char *key)
{
  /* The callback may bring an update, which destroys the widget key belongs to. */
  g_autofree char *copy = g_strdup (key);

  caminito_applet_popup_hide (popup->applet_popup);
  popup->actions.invoke (id, copy, popup->data);
}

static guint32
id_of (GtkWidget *widget)
{
  return GPOINTER_TO_UINT (g_object_get_data (G_OBJECT (widget), "nt-id"));
}

static void
on_row_activated (GtkListBox *list G_GNUC_UNUSED, GtkListBoxRow *row, gpointer data)
{
  close_and_invoke (data, id_of (GTK_WIDGET (row)), "default");
}

static void
on_action (GtkButton *button, gpointer data)
{
  close_and_invoke (data, id_of (GTK_WIDGET (button)),
                    g_object_get_data (G_OBJECT (button), "nt-key"));
}

static void
on_dismiss (GtkButton *button, gpointer data)
{
  NtPopup *popup = data;

  popup->actions.dismiss (id_of (GTK_WIDGET (button)), popup->data);
}

static void
on_clear_all (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  NtPopup *popup = data;

  popup->actions.clear_all (popup->data);
}

static void
on_do_not_disturb_switched (GObject *button G_GNUC_UNUSED, GParamSpec *pspec G_GNUC_UNUSED,
                            gpointer data)
{
  NtPopup *popup = data;
  gboolean on = gtk_switch_get_active (GTK_SWITCH (popup->do_not_disturb));

  if (!popup->updating)
    popup->actions.set_do_not_disturb (on ? "until-turned-off" : "off", popup->data);
}

static void
on_duration (GtkButton *button, gpointer data)
{
  NtPopup *popup = data;

  popup->actions.set_do_not_disturb (g_object_get_data (G_OBJECT (button), "nt-until"),
                                     popup->data);
}

/* The Do Not Disturb row: its name, and when it ends below it while it's on, beside the
   switch; and below them, the durations, which show while it's off. */
static GtkWidget *
do_not_disturb_row (NtPopup *popup)
{
  static const struct
  {
    const char *label;
    const char *until;
  } durations[] = {
    { "1 hour", "one-hour" },
    { "4 hours", "four-hours" },
    { "Tomorrow morning", "tomorrow-morning" },
  };
  GtkWidget *row = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *top = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *text = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  GtkWidget *name = gtk_label_new ("Do Not Disturb");

  g_object_set (row, "margin", 8, NULL);
  gtk_label_set_xalign (GTK_LABEL (name), 0);
  gtk_box_pack_start (GTK_BOX (text), name, FALSE, FALSE, 0);
  popup->until = dim_label ("");
  gtk_widget_set_no_show_all (popup->until, TRUE);
  gtk_box_pack_start (GTK_BOX (text), popup->until, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (top), text, TRUE, TRUE, 0);
  popup->do_not_disturb = gtk_switch_new ();
  gtk_widget_set_name (popup->do_not_disturb, "do-not-disturb");
  gtk_widget_set_valign (popup->do_not_disturb, GTK_ALIGN_CENTER);
  g_signal_connect (popup->do_not_disturb, "notify::active",
                    G_CALLBACK (on_do_not_disturb_switched), popup);
  gtk_box_pack_end (GTK_BOX (top), popup->do_not_disturb, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (row), top, FALSE, FALSE, 0);

  popup->durations = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  for (guint i = 0; i < G_N_ELEMENTS (durations); i++)
    {
      GtkWidget *button = gtk_button_new_with_label (durations[i].label);

      g_object_set_data (G_OBJECT (button), "nt-until", (gpointer) durations[i].until);
      g_signal_connect (button, "clicked", G_CALLBACK (on_duration), popup);
      gtk_box_pack_start (GTK_BOX (popup->durations), button, FALSE, FALSE, 0);
    }
  gtk_box_pack_start (GTK_BOX (row), popup->durations, FALSE, FALSE, 0);
  return row;
}

static GtkWidget *
app_icon (const NtEntry *entry)
{
  g_autoptr (GdkPixbuf) pixbuf = NULL;
  g_autofree char *file = icon_file (entry->app_icon);
  GtkWidget *icon;

  if (file != NULL)
    pixbuf = load_icon (file, APP_ICON_SIZE);
  if (pixbuf != NULL)
    return gtk_image_new_from_pixbuf (pixbuf);
  icon = gtk_image_new ();
  if (file == NULL && *entry->app_icon != '\0')
    {
      const char *names[] = { entry->app_icon, "application-x-executable", NULL };
      g_autoptr (GIcon) gicon = g_themed_icon_new_from_names ((char **) names, -1);

      gtk_image_set_from_gicon (GTK_IMAGE (icon), gicon, GTK_ICON_SIZE_MENU);
    }
  else
    gtk_image_set_from_icon_name (GTK_IMAGE (icon), "application-x-executable",
                                  GTK_ICON_SIZE_MENU);
  gtk_image_set_pixel_size (GTK_IMAGE (icon), APP_ICON_SIZE);
  return icon;
}

static GtkWidget *
body_label (const char *body)
{
  GtkWidget *label = gtk_label_new (NULL);

  /* The daemon has made the markup safe for Pango; anything it lets by shows as text. */
  if (pango_parse_markup (body, -1, 0, NULL, NULL, NULL, NULL))
    gtk_label_set_markup (GTK_LABEL (label), body);
  else
    gtk_label_set_text (GTK_LABEL (label), body);
  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_line_wrap (GTK_LABEL (label), TRUE);
  gtk_label_set_line_wrap_mode (GTK_LABEL (label), PANGO_WRAP_WORD_CHAR);
  gtk_label_set_ellipsize (GTK_LABEL (label), PANGO_ELLIPSIZE_END);
  gtk_label_set_lines (GTK_LABEL (label), BODY_LINES);
  gtk_label_set_max_width_chars (GTK_LABEL (label), 1);
  gtk_widget_set_hexpand (label, TRUE);
  return label;
}

static GtkWidget *
image_of (const NtEntry *entry)
{
  g_autoptr (GdkPixbuf) pixbuf = NULL;
  GtkWidget *image;

  if (entry->image_pixels != NULL)
    pixbuf = load_pixels (entry->image_pixels);
  else
    pixbuf = load_icon (entry->image_icon, IMAGE_SIZE);
  image = gtk_image_new_from_pixbuf (pixbuf);
  gtk_widget_set_name (image, "image");
  gtk_widget_set_valign (image, GTK_ALIGN_START);
  gtk_widget_set_no_show_all (image, TRUE);
  gtk_widget_set_visible (image, pixbuf != NULL);
  return image;
}

/* An entry's row: its app's icon, then its app's name, its age and the close button over
   the summary, body and actions, then its image. */
static GtkWidget *
entry_row (NtPopup *popup, const NtEntry *entry, gint64 now)
{
  GtkWidget *row = gtk_list_box_row_new ();
  GtkWidget *outer = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *text = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  GtkWidget *top = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  GtkWidget *icon = app_icon (entry);
  GtkWidget *age, *dismiss, *summary;
  g_autofree char *age_text = nt_state_age (entry->time, now);
  g_autoptr (PangoAttrList) bold = pango_attr_list_new ();

  g_object_set_data (G_OBJECT (row), "nt-id", GUINT_TO_POINTER (entry->id));
  gtk_list_box_row_set_activatable (GTK_LIST_BOX_ROW (row), entry->has_default);
  g_object_set (outer, "margin", 8, NULL);

  gtk_widget_set_valign (icon, GTK_ALIGN_START);
  gtk_box_pack_start (GTK_BOX (top), dim_label (entry->app_name), FALSE, FALSE, 0);
  age = dim_label (age_text);
  gtk_box_pack_start (GTK_BOX (top), age, TRUE, TRUE, 0);
  g_array_append_val (popup->ages, ((Age) { age, entry->time }));
  dismiss = gtk_button_new_from_icon_name ("window-close-symbolic", GTK_ICON_SIZE_MENU);
  gtk_widget_set_name (dismiss, "dismiss");
  gtk_widget_set_tooltip_text (dismiss, "Dismiss");
  gtk_button_set_relief (GTK_BUTTON (dismiss), GTK_RELIEF_NONE);
  gtk_widget_set_valign (dismiss, GTK_ALIGN_CENTER);
  g_object_set_data (G_OBJECT (dismiss), "nt-id", GUINT_TO_POINTER (entry->id));
  g_signal_connect (dismiss, "clicked", G_CALLBACK (on_dismiss), popup);
  gtk_box_pack_end (GTK_BOX (top), dismiss, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (text), top, FALSE, FALSE, 0);

  summary = gtk_label_new (entry->summary);
  gtk_widget_set_name (summary, "summary");
  pango_attr_list_insert (bold, pango_attr_weight_new (PANGO_WEIGHT_BOLD));
  gtk_label_set_attributes (GTK_LABEL (summary), bold);
  gtk_label_set_xalign (GTK_LABEL (summary), 0);
  gtk_label_set_line_wrap (GTK_LABEL (summary), TRUE);
  gtk_label_set_max_width_chars (GTK_LABEL (summary), 1);
  gtk_box_pack_start (GTK_BOX (text), summary, FALSE, FALSE, 0);
  if (*entry->body != '\0')
    gtk_box_pack_start (GTK_BOX (text), body_label (entry->body), FALSE, FALSE, 0);

  if (entry->actions->len > 0)
    {
      GtkWidget *buttons = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);

      gtk_widget_set_margin_top (buttons, 4);
      for (guint i = 0; i < entry->actions->len; i++)
        {
          const NtAction *action = g_ptr_array_index (entry->actions, i);
          GtkWidget *button = gtk_button_new_with_label (action->label);

          g_object_set_data (G_OBJECT (button), "nt-id", GUINT_TO_POINTER (entry->id));
          g_object_set_data_full (G_OBJECT (button), "nt-key", g_strdup (action->key), g_free);
          g_signal_connect (button, "clicked", G_CALLBACK (on_action), popup);
          gtk_box_pack_start (GTK_BOX (buttons), button, FALSE, FALSE, 0);
        }
      gtk_box_pack_start (GTK_BOX (text), buttons, FALSE, FALSE, 0);
    }

  gtk_box_pack_start (GTK_BOX (outer), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (outer), text, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (outer), image_of (entry), FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (row), outer);
  gtk_widget_show_all (row);
  return row;
}

/* A window shown again keeps the size it had, and the list's own height is what it needs
   to scroll at all: the list asks for the height of its rows, up to LIST_HEIGHT, and the
   window for its request, which grows or shrinks it to fit. */
static void
fit_list (NtPopup *popup)
{
  int natural = 0;

  gtk_widget_get_preferred_height_for_width (popup->list, WIDTH, NULL, &natural);
  gtk_scrolled_window_set_min_content_height (GTK_SCROLLED_WINDOW (popup->scrolled),
                                              MIN (natural, LIST_HEIGHT));
  gtk_window_resize (GTK_WINDOW (caminito_applet_popup_get_window (popup->applet_popup)), 1, 1);
}

void
nt_popup_update (NtPopup *popup, const GPtrArray *entries, gint64 now)
{
  g_autoptr (GList) rows = gtk_container_get_children (GTK_CONTAINER (popup->list));

  for (GList *row = rows; row != NULL; row = row->next)
    gtk_widget_destroy (row->data);
  g_array_set_size (popup->ages, 0);
  for (guint i = 0; i < entries->len; i++)
    gtk_container_add (GTK_CONTAINER (popup->list),
                       entry_row (popup, g_ptr_array_index (entries, i), now));
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack), entries->len > 0 ? "list" : "empty");
  gtk_widget_set_sensitive (popup->clear_all, entries->len > 0);
  fit_list (popup);
}

void
nt_popup_set_do_not_disturb (NtPopup *popup, gboolean on, guint64 end, gint64 now)
{
  g_autoptr (GTimeZone) zone = g_time_zone_new_local ();
  g_autofree char *until = nt_state_until (end, now, zone);

  popup->updating = TRUE;
  gtk_switch_set_active (GTK_SWITCH (popup->do_not_disturb), on);
  popup->updating = FALSE;
  gtk_label_set_text (GTK_LABEL (popup->until), until);
  gtk_widget_set_visible (popup->until, on);
  gtk_widget_set_visible (popup->durations, !on);
  fit_list (popup);
}

NtPopup *
nt_popup_new (CaminitoAppletPopup *applet_popup, const NtPopupActions *actions, gpointer data)
{
  NtPopup *popup = g_new0 (NtPopup, 1);
  GtkWidget *window = caminito_applet_popup_get_window (applet_popup);
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *empty = dim_label ("No notifications");
  GtkWidget *bottom = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 0);

  popup->applet_popup = applet_popup;
  popup->actions = *actions;
  popup->data = data;
  popup->ages = g_array_new (FALSE, FALSE, sizeof (Age));
  gtk_widget_set_size_request (box, WIDTH, -1);
  gtk_box_pack_start (GTK_BOX (box), do_not_disturb_row (popup), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE,
                      FALSE, 0);

  popup->scrolled = gtk_scrolled_window_new (NULL, NULL);
  popup->list = gtk_list_box_new ();
  gtk_widget_set_name (popup->list, "history");
  gtk_list_box_set_selection_mode (GTK_LIST_BOX (popup->list), GTK_SELECTION_NONE);
  g_signal_connect (popup->list, "row-activated", G_CALLBACK (on_row_activated), popup);
  gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (popup->scrolled), GTK_POLICY_NEVER,
                                  GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_propagate_natural_height (GTK_SCROLLED_WINDOW (popup->scrolled), TRUE);
  gtk_scrolled_window_set_max_content_height (GTK_SCROLLED_WINDOW (popup->scrolled), LIST_HEIGHT);
  gtk_container_add (GTK_CONTAINER (popup->scrolled), popup->list);

  gtk_label_set_xalign (GTK_LABEL (empty), 0.5);
  g_object_set (empty, "margin", 24, NULL);
  popup->stack = gtk_stack_new ();
  gtk_stack_set_vhomogeneous (GTK_STACK (popup->stack), FALSE);
  gtk_stack_add_named (GTK_STACK (popup->stack), empty, "empty");
  gtk_stack_add_named (GTK_STACK (popup->stack), popup->scrolled, "list");
  gtk_box_pack_start (GTK_BOX (box), popup->stack, TRUE, TRUE, 0);

  gtk_box_pack_start (GTK_BOX (box), gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE,
                      FALSE, 0);
  popup->clear_all = gtk_button_new_with_label ("Clear all");
  gtk_widget_set_sensitive (popup->clear_all, FALSE);
  g_signal_connect (popup->clear_all, "clicked", G_CALLBACK (on_clear_all), popup);
  g_object_set (bottom, "margin", 6, NULL);
  gtk_box_pack_end (GTK_BOX (bottom), popup->clear_all, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), bottom, FALSE, FALSE, 0);

  gtk_container_add (GTK_CONTAINER (window), box);
  gtk_widget_show_all (box);
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack), "empty");
  g_signal_connect (window, "map", G_CALLBACK (on_map), popup);
  g_signal_connect (window, "unmap", G_CALLBACK (on_unmap), popup);
  return popup;
}

void
nt_popup_free (NtPopup *popup)
{
  GtkWidget *window = caminito_applet_popup_get_window (popup->applet_popup);

  g_signal_handlers_disconnect_by_data (window, popup);
  g_signal_handlers_disconnect_by_data (popup->list, popup);
  g_signal_handlers_disconnect_by_data (popup->do_not_disturb, popup);
  g_clear_handle_id (&popup->tick, g_source_remove);
  g_array_unref (popup->ages);
  g_free (popup);
}
