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

/* An xdg-activation token for the app, asked for while the popup still has the keyboard
   and the click's serial; NULL if GDK has none. */
static char *
activation_token (NtPopup *popup)
{
  GtkWidget *window = caminito_applet_popup_get_window (popup->applet_popup);
  g_autoptr (GdkAppLaunchContext) context =
    gdk_display_get_app_launch_context (gtk_widget_get_display (window));

  return g_app_launch_context_get_startup_notify_id (G_APP_LAUNCH_CONTEXT (context), NULL, NULL);
}

static void
close_and_invoke (NtPopup *popup, guint32 id, const char *key)
{
  /* The callback may bring an update, which destroys the widget key belongs to. */
  g_autofree char *copy = g_strdup (key);
  g_autofree char *token = activation_token (popup);

  caminito_applet_popup_hide (popup->applet_popup);
  popup->actions.invoke (id, copy, token, popup->data);
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
on_reply (GtkEntry *field, gpointer data)
{
  NtPopup *popup = data;
  /* The callback may bring an update, which destroys the field. */
  g_autofree char *text = g_strdup (gtk_entry_get_text (field));
  g_autofree char *stripped = g_strstrip (g_strdup (text));

  if (*stripped == '\0')
    return;
  popup->actions.reply (id_of (GTK_WIDGET (field)), text, popup->data);
}

static void
on_reply_icon (GtkEntry            *field,
               GtkEntryIconPosition position G_GNUC_UNUSED,
               GdkEvent            *event G_GNUC_UNUSED,
               gpointer             data)
{
  on_reply (field, data);
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

  /* The center closes with it (CMN-285). */
  caminito_applet_popup_hide (popup->applet_popup);
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

  /* Every row is activatable: without a default action, a click brings the app forward. */
  g_object_set_data (G_OBJECT (row), "nt-id", GUINT_TO_POINTER (entry->id));
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

  if (*entry->reply != '\0')
    {
      GtkWidget *field = gtk_entry_new ();

      gtk_widget_set_name (field, "reply");
      gtk_widget_set_margin_top (field, 4);
      gtk_entry_set_placeholder_text (GTK_ENTRY (field), entry->reply);
      gtk_entry_set_icon_from_icon_name (GTK_ENTRY (field), GTK_ENTRY_ICON_SECONDARY,
                                         "document-send-symbolic");
      gtk_entry_set_icon_tooltip_text (GTK_ENTRY (field), GTK_ENTRY_ICON_SECONDARY, "Send");
      g_object_set_data (G_OBJECT (field), "nt-id", GUINT_TO_POINTER (entry->id));
      g_signal_connect (field, "activate", G_CALLBACK (on_reply), popup);
      g_signal_connect (field, "icon-release", G_CALLBACK (on_reply_icon), popup);
      gtk_box_pack_start (GTK_BOX (text), field, FALSE, FALSE, 0);
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
  g_free (popup);
}
