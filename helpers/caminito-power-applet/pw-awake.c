/*
 * The popup's Stay awake row and what keeps the computer awake (265). See pw-awake.h.
 */

#include "pw-awake.h"

#include <gio/gdesktopappinfo.h>

#define ROW_ICON_SIZE  24
#define LINE_ICON_SIZE 24
#define REASON_CHARS   34

struct _PwAwake
{
  PwAwakeActions  actions;
  gpointer        data;
  GtkWidget      *row;
  GtkWidget      *toggle;
  GtkWidget      *inhibitors;
  /* The lines, in a box of their own so that they can be built again. */
  GtkWidget      *lines;
};

static const char *const lock_icon_names[] = {
  "changes-prevent-symbolic", "system-lock-screen-symbolic", "system-lock-screen", NULL,
};

static GtkWidget *
label_new (const char *text, gboolean dim, gboolean small)
{
  GtkWidget *label = gtk_label_new (text);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  if (dim)
    gtk_style_context_add_class (gtk_widget_get_style_context (label), GTK_STYLE_CLASS_DIM_LABEL);
  if (small)
    {
      g_autofree char *markup = g_markup_printf_escaped ("<small>%s</small>", text);

      gtk_label_set_markup (GTK_LABEL (label), markup);
    }
  return label;
}

static GtkWidget *
icon_new (const char *const *names, int pixel_size)
{
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) names, -1);
  GtkWidget *image = gtk_image_new_from_gicon (icon, GTK_ICON_SIZE_LARGE_TOOLBAR);

  gtk_image_set_pixel_size (GTK_IMAGE (image), pixel_size);
  return image;
}

static void
on_toggled (GObject *toggle, GParamSpec *pspec G_GNUC_UNUSED, gpointer data)
{
  PwAwake *awake = data;

  awake->actions.set_stay_awake (gtk_switch_get_active (GTK_SWITCH (toggle)), awake->data);
}

/* The application's name and icon, from its desktop file where it has one: its app ID
   is the desktop file's name, with or without ".desktop". */
static GDesktopAppInfo *
app_info_for (const char *app_id)
{
  g_autofree char *desktop_id = NULL;

  if (app_id[0] == '\0')
    return NULL;
  desktop_id = g_str_has_suffix (app_id, ".desktop") ? g_strdup (app_id)
                                                     : g_strconcat (app_id, ".desktop", NULL);
  return g_desktop_app_info_new (desktop_id);
}

static GtkWidget *
line_new (const PwInhibitor *inhibitor)
{
  static const char *const application[] = { "application-x-executable", NULL };
  GtkWidget *line = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 10);
  GtkWidget *text = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *name;
  GtkWidget *reason;
  GtkWidget *icon;
  g_autoptr (GDesktopAppInfo) info = NULL;

  if (g_strcmp0 (inhibitor->app_id, PW_STAY_AWAKE_APP_ID) == 0)
    {
      /* The user's own: not an application. */
      name = label_new ("You", FALSE, FALSE);
      icon = icon_new (lock_icon_names, LINE_ICON_SIZE);
    }
  else
    {
      info = app_info_for (inhibitor->app_id);
      if (info != NULL)
        {
          GIcon *gicon = g_app_info_get_icon (G_APP_INFO (info));

          name = label_new (g_app_info_get_name (G_APP_INFO (info)), FALSE, FALSE);
          icon = gicon != NULL ? gtk_image_new_from_gicon (gicon, GTK_ICON_SIZE_LARGE_TOOLBAR)
                               : icon_new (application, LINE_ICON_SIZE);
          gtk_image_set_pixel_size (GTK_IMAGE (icon), LINE_ICON_SIZE);
        }
      else
        {
          name = label_new (inhibitor->app_id[0] != '\0' ? inhibitor->app_id : "An application",
                            FALSE, FALSE);
          icon = icon_new (application, LINE_ICON_SIZE);
        }
    }
  gtk_label_set_ellipsize (GTK_LABEL (name), PANGO_ELLIPSIZE_END);
  gtk_box_pack_start (GTK_BOX (text), name, FALSE, FALSE, 0);
  if (inhibitor->reason[0] != '\0')
    {
      reason = label_new (inhibitor->reason, TRUE, TRUE);
      gtk_label_set_ellipsize (GTK_LABEL (reason), PANGO_ELLIPSIZE_END);
      gtk_label_set_max_width_chars (GTK_LABEL (reason), REASON_CHARS);
      gtk_widget_set_tooltip_text (reason, inhibitor->reason);
      gtk_box_pack_start (GTK_BOX (text), reason, FALSE, FALSE, 0);
    }
  gtk_widget_set_valign (icon, GTK_ALIGN_CENTER);
  gtk_box_pack_start (GTK_BOX (line), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (line), text, TRUE, TRUE, 0);
  return line;
}

void
pw_awake_set_inhibitors (PwAwake *awake, const GPtrArray *inhibitors)
{
  GList *children = gtk_container_get_children (GTK_CONTAINER (awake->lines));

  g_list_free_full (children, (GDestroyNotify) gtk_widget_destroy);
  for (guint i = 0; i < inhibitors->len; i++)
    gtk_box_pack_start (GTK_BOX (awake->lines), line_new (g_ptr_array_index (inhibitors, i)),
                        FALSE, FALSE, 0);
  gtk_widget_show_all (awake->lines);
  gtk_widget_set_visible (awake->inhibitors, inhibitors->len > 0);
}

void
pw_awake_set_stay_awake (PwAwake *awake, gboolean on)
{
  g_signal_handlers_block_by_func (awake->toggle, on_toggled, awake);
  gtk_switch_set_active (GTK_SWITCH (awake->toggle), on);
  g_signal_handlers_unblock_by_func (awake->toggle, on_toggled, awake);
}

GtkWidget *
pw_awake_row (PwAwake *awake)
{
  return awake->row;
}

GtkWidget *
pw_awake_inhibitors (PwAwake *awake)
{
  return awake->inhibitors;
}

PwAwake *
pw_awake_new (const PwAwakeActions *actions, gpointer data)
{
  PwAwake *awake = g_new0 (PwAwake, 1);
  GtkWidget *text = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *icon = icon_new (lock_icon_names, ROW_ICON_SIZE);

  awake->actions = *actions;
  awake->data = data;

  awake->row = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 10);
  awake->toggle = gtk_switch_new ();
  gtk_widget_set_valign (awake->toggle, GTK_ALIGN_CENTER);
  g_signal_connect (awake->toggle, "notify::active", G_CALLBACK (on_toggled), awake);
  gtk_box_pack_start (GTK_BOX (text), label_new ("Stay awake", FALSE, FALSE), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (text), label_new ("Don't sleep or lock the screen", TRUE, TRUE),
                      FALSE, FALSE, 0);
  gtk_widget_set_margin_start (icon, 6);
  gtk_widget_set_valign (icon, GTK_ALIGN_CENTER);
  gtk_box_pack_start (GTK_BOX (awake->row), icon, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (awake->row), text, TRUE, TRUE, 0);
  gtk_box_pack_end (GTK_BOX (awake->row), awake->toggle, FALSE, FALSE, 0);
  gtk_widget_show_all (awake->row);

  awake->inhibitors = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  awake->lines = gtk_box_new (GTK_ORIENTATION_VERTICAL, 6);
  gtk_widget_set_margin_start (awake->lines, 6);
  gtk_box_pack_start (GTK_BOX (awake->inhibitors),
                      gtk_separator_new (GTK_ORIENTATION_HORIZONTAL), FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (awake->inhibitors), awake->lines, FALSE, FALSE, 0);
  /* Shown by pw_awake_set_inhibitors() once there are some, whatever show_all of the
     popup's box does. */
  gtk_widget_show_all (awake->inhibitors);
  gtk_widget_set_no_show_all (awake->inhibitors, TRUE);
  gtk_widget_hide (awake->inhibitors);
  return awake;
}

void
pw_awake_free (PwAwake *awake)
{
  g_free (awake);
}
