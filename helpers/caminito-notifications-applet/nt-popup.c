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
/* A group's latest entries, which show until it's opened. */
#define GROUP_SHOWN  3

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
  GArray              *ages;
  guint                tick;
  /* What the last update gave, for a group opened or closed to be drawn again. */
  GPtrArray           *entries;
  char               **ungrouped;
  gint64               now;
  /* The keys of the apps whose groups are open. */
  GHashTable          *opened;
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
on_clear_app (GtkButton *button, gpointer data)
{
  NtPopup *popup = data;
  /* The callback may bring an update, which destroys the button app belongs to. */
  g_autofree char *app = g_strdup (g_object_get_data (G_OBJECT (button), "nt-app"));

  popup->actions.clear_app (app, popup->data);
}

static void rebuild (NtPopup *popup);

static void
on_more (GtkButton *button, gpointer data)
{
  NtPopup *popup = data;
  const char *app = g_object_get_data (G_OBJECT (button), "nt-app");

  if (!g_hash_table_remove (popup->opened, app))
    g_hash_table_add (popup->opened, g_strdup (app));
  rebuild (popup);
}

static void
on_clear_all (GtkButton *button G_GNUC_UNUSED, gpointer data)
{
  NtPopup *popup = data;

  popup->actions.clear_all (popup->data);
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
   the summary, body and actions, then its image. In a group, whose header has the app, its
   age and the close button alone are over the rest. */
static GtkWidget *
entry_row (NtPopup *popup, const NtEntry *entry, gboolean in_group, gint64 now)
{
  GtkWidget *row = gtk_list_box_row_new ();
  GtkWidget *outer = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *text = gtk_box_new (GTK_ORIENTATION_VERTICAL, 2);
  GtkWidget *top = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);
  GtkWidget *age, *dismiss, *summary;
  g_autofree char *age_text = nt_state_age (entry->time, now);
  g_autoptr (PangoAttrList) bold = pango_attr_list_new ();

  g_object_set_data (G_OBJECT (row), "nt-id", GUINT_TO_POINTER (entry->id));
  gtk_list_box_row_set_activatable (GTK_LIST_BOX_ROW (row), entry->has_default);
  g_object_set (outer, "margin", 8, NULL);

  if (!in_group)
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

  if (!in_group)
    {
      GtkWidget *icon = app_icon (entry);

      gtk_widget_set_valign (icon, GTK_ALIGN_START);
      gtk_box_pack_start (GTK_BOX (outer), icon, FALSE, FALSE, 0);
    }
  gtk_box_pack_start (GTK_BOX (outer), text, TRUE, TRUE, 0);
  gtk_box_pack_start (GTK_BOX (outer), image_of (entry), FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (row), outer);
  gtk_widget_show_all (row);
  return row;
}

/* A group's header: its app's icon and name, its count, and its clear button. */
static GtkWidget *
group_row (NtPopup *popup, const NtGroup *group)
{
  const NtEntry *newest = g_ptr_array_index (group->entries, 0);
  GtkWidget *row = gtk_list_box_row_new ();
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *name = gtk_label_new (newest->app_name);
  g_autofree char *name_text = g_strdup_printf ("group %s", group->app);
  g_autofree char *count = g_strdup_printf ("%u", group->entries->len);
  g_autoptr (PangoAttrList) bold = pango_attr_list_new ();
  GtkWidget *clear;

  gtk_widget_set_name (row, name_text);
  gtk_list_box_row_set_activatable (GTK_LIST_BOX_ROW (row), FALSE);
  g_object_set (box, "margin-start", 8, "margin-end", 8, "margin-top", 6, NULL);
  gtk_box_pack_start (GTK_BOX (box), app_icon (newest), FALSE, FALSE, 0);
  pango_attr_list_insert (bold, pango_attr_weight_new (PANGO_WEIGHT_BOLD));
  gtk_label_set_attributes (GTK_LABEL (name), bold);
  gtk_label_set_ellipsize (GTK_LABEL (name), PANGO_ELLIPSIZE_END);
  gtk_box_pack_start (GTK_BOX (box), name, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), dim_label (count), TRUE, TRUE, 0);
  clear = gtk_button_new_with_label ("Clear");
  gtk_widget_set_name (clear, "clear");
  gtk_widget_set_tooltip_text (clear, "Clear this app's notifications");
  gtk_button_set_relief (GTK_BUTTON (clear), GTK_RELIEF_NONE);
  g_object_set_data_full (G_OBJECT (clear), "nt-app", g_strdup (group->app), g_free);
  g_signal_connect (clear, "clicked", G_CALLBACK (on_clear_app), popup);
  gtk_box_pack_end (GTK_BOX (box), clear, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (row), box);
  gtk_widget_show_all (row);
  return row;
}

/* The row under a group with more than GROUP_SHOWN entries: "N more", which opens it, or
   "Show less", which closes it again. */
static GtkWidget *
more_row (NtPopup *popup, const NtGroup *group, gboolean opened)
{
  GtkWidget *row = gtk_list_box_row_new ();
  g_autofree char *label = opened ? g_strdup ("Show less")
                                  : g_strdup_printf ("%u more", group->entries->len - GROUP_SHOWN);
  GtkWidget *more = gtk_button_new_with_label (label);

  gtk_list_box_row_set_activatable (GTK_LIST_BOX_ROW (row), FALSE);
  gtk_button_set_relief (GTK_BUTTON (more), GTK_RELIEF_NONE);
  gtk_widget_set_halign (more, GTK_ALIGN_START);
  g_object_set (more, "margin-start", 8, "margin-bottom", 4, NULL);
  g_object_set_data_full (G_OBJECT (more), "nt-app", g_strdup (group->app), g_free);
  g_signal_connect (more, "clicked", G_CALLBACK (on_more), popup);
  gtk_container_add (GTK_CONTAINER (row), more);
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

/* Draws the last update's entries again, as its groups are open now. */
static void
rebuild (NtPopup *popup)
{
  g_autoptr (GList) rows = gtk_container_get_children (GTK_CONTAINER (popup->list));
  g_autoptr (GPtrArray) groups = nt_state_groups (popup->entries,
                                                  (const char *const *) popup->ungrouped);
  g_autoptr (GHashTable) grouped = g_hash_table_new (g_str_hash, g_str_equal);
  GHashTableIter opened;
  gpointer app;

  for (GList *row = rows; row != NULL; row = row->next)
    gtk_widget_destroy (row->data);
  g_array_set_size (popup->ages, 0);
  for (guint i = 0; i < groups->len; i++)
    {
      const NtGroup *group = g_ptr_array_index (groups, i);
      gboolean open = g_hash_table_contains (popup->opened, group->app);
      guint shown = open ? group->entries->len : MIN (group->entries->len, GROUP_SHOWN);

      if (!group->grouped)
        {
          gtk_container_add (GTK_CONTAINER (popup->list),
                             entry_row (popup, g_ptr_array_index (group->entries, 0), FALSE,
                                        popup->now));
          continue;
        }
      g_hash_table_add (grouped, (gpointer) group->app);
      gtk_container_add (GTK_CONTAINER (popup->list), group_row (popup, group));
      for (guint j = 0; j < shown; j++)
        gtk_container_add (GTK_CONTAINER (popup->list),
                           entry_row (popup, g_ptr_array_index (group->entries, j), TRUE,
                                      popup->now));
      if (group->entries->len > GROUP_SHOWN)
        gtk_container_add (GTK_CONTAINER (popup->list), more_row (popup, group, open));
    }
  /* A group that's gone is closed when it comes back. */
  g_hash_table_iter_init (&opened, popup->opened);
  while (g_hash_table_iter_next (&opened, &app, NULL))
    if (!g_hash_table_contains (grouped, app))
      g_hash_table_iter_remove (&opened);
  gtk_stack_set_visible_child_name (GTK_STACK (popup->stack),
                                    popup->entries->len > 0 ? "list" : "empty");
  gtk_widget_set_sensitive (popup->clear_all, popup->entries->len > 0);
  fit_list (popup);
}

void
nt_popup_update (NtPopup           *popup,
                 const GPtrArray   *entries,
                 const char *const *ungrouped,
                 gint64             now)
{
  /* Its own references, as the daemon replaces its entries with each List. */
  g_ptr_array_unref (popup->entries);
  popup->entries = g_ptr_array_ref ((GPtrArray *) entries);
  g_strfreev (popup->ungrouped);
  popup->ungrouped = g_strdupv ((char **) ungrouped);
  popup->now = now;
  rebuild (popup);
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
  popup->entries = g_ptr_array_new ();
  popup->ungrouped = g_new0 (char *, 1);
  popup->opened = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  gtk_widget_set_size_request (box, WIDTH, -1);

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
  g_clear_handle_id (&popup->tick, g_source_remove);
  g_array_unref (popup->ages);
  g_ptr_array_unref (popup->entries);
  g_strfreev (popup->ungrouped);
  g_hash_table_unref (popup->opened);
  g_free (popup);
}
