/*
 * The Power popup's power profile section (264). See pw-profiles-section.h.
 *
 * The three tiles are made once and shown or hidden by what the daemon lists, so that a
 * change from elsewhere moves the pressed tile in place. A click asks the daemon and the
 * tiles go back to what the daemon has, until it answers with the change.
 */

#include "pw-profiles-section.h"

#include <gio/gdesktopappinfo.h>

#include "pw-ppd.h"

#define TILE_ICON_SIZE 24
#define POPUP_WIDTH    320

typedef struct
{
  PwPpd     *ppd;
  GtkWidget *section;
  GtkWidget *tiles[PW_PROFILE_COUNT];
  GtkWidget *degraded;
  GtkWidget *holds;
  /* What the tiles show now, so that a click can go back to it. */
  PwProfiles *shown;
  gboolean    syncing;
} Section;

static void
add_styles (void)
{
  static gboolean added;
  g_autoptr (GtkCssProvider) provider = NULL;

  if (added)
    return;
  added = TRUE;
  provider = gtk_css_provider_new ();
  /* The warning orange of the prototype's Popup C: the theme has no colour for it that
     every theme names alike. */
  gtk_css_provider_load_from_data (provider,
                                   ".caminito-power-warning { color: #c64600; }"
                                   /* Three tiles with "Performance" under one of them must fit
                                      the popup's width, which a theme's button padding doesn't
                                      allow. */
                                   ".caminito-power-tile { padding: 6px 2px; }",
                                   -1, NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (),
                                             GTK_STYLE_PROVIDER (provider),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/* A window doesn't shrink when its content does: asking for the least brings it back to
   what the content needs, at the width it has. */
static void
fit_window (Section *section)
{
  GtkWidget *window = gtk_widget_get_toplevel (section->section);

  if (GTK_IS_WINDOW (window) && gtk_widget_get_mapped (window))
    gtk_window_resize (GTK_WINDOW (window), POPUP_WIDTH, 1);
}

/* An application's name from its desktop entry, which the daemon's ID names. */
static char *
app_name (const char *application_id, gpointer data G_GNUC_UNUSED)
{
  g_autofree char *id = g_str_has_suffix (application_id, ".desktop")
                          ? g_strdup (application_id)
                          : g_strconcat (application_id, ".desktop", NULL);
  g_autoptr (GDesktopAppInfo) info = g_desktop_app_info_new (id);

  if (info == NULL)
    return NULL;
  return g_strdup (g_app_info_get_name (G_APP_INFO (info)));
}

static void
set_note (GtkWidget *label, const char *text)
{
  gtk_label_set_text (GTK_LABEL (label), text != NULL ? text : "");
  gtk_widget_set_visible (label, text != NULL);
}

static void
show (Section *section, const PwProfiles *profiles)
{
  g_autofree char *degraded = NULL;
  g_autofree char *holds = NULL;

  if (profiles == NULL || !pw_profiles_any (profiles))
    {
      gtk_widget_set_visible (section->section, FALSE);
      fit_window (section);
      return;
    }
  section->syncing = TRUE;
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    {
      gtk_widget_set_visible (section->tiles[i], profiles->listed[i]);
      gtk_toggle_button_set_active (GTK_TOGGLE_BUTTON (section->tiles[i]),
                                    profiles->active == (PwProfile) i);
    }
  section->syncing = FALSE;
  degraded = pw_profiles_degraded_text (profiles);
  holds = pw_profiles_holds_text (profiles, app_name, NULL);
  set_note (section->degraded, degraded);
  set_note (section->holds, holds);
  gtk_widget_set_visible (section->section, TRUE);
  fit_window (section);
}

static void
on_changed (PwPpd *ppd, gpointer data)
{
  Section *section = data;

  pw_profiles_free (section->shown);
  section->shown = pw_ppd_profiles (ppd);
  show (section, section->shown);
}

static void
on_tile_toggled (GtkToggleButton *tile, gpointer data)
{
  Section *section = data;

  if (section->syncing || section->shown == NULL)
    return;
  /* Whichever tile was pressed, the row is what the daemon has until it says otherwise. */
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    if (GTK_WIDGET (tile) == section->tiles[i] && gtk_toggle_button_get_active (tile)
        && section->shown->active != (PwProfile) i)
      pw_ppd_set_active (section->ppd, (PwProfile) i);
  show (section, section->shown);
}

static void
section_free (GtkWidget *widget G_GNUC_UNUSED, gpointer data)
{
  Section *section = data;

  pw_ppd_free (section->ppd);
  pw_profiles_free (section->shown);
  g_free (section);
}

static GtkWidget *
note_new (gboolean warning)
{
  GtkWidget *label = gtk_label_new (NULL);

  gtk_label_set_xalign (GTK_LABEL (label), 0);
  gtk_label_set_line_wrap (GTK_LABEL (label), TRUE);
  gtk_label_set_max_width_chars (GTK_LABEL (label), 40);
  gtk_widget_set_no_show_all (label, TRUE);
  if (warning)
    gtk_style_context_add_class (gtk_widget_get_style_context (label),
                                 "caminito-power-warning");
  else
    gtk_style_context_add_class (gtk_widget_get_style_context (label), GTK_STYLE_CLASS_DIM_LABEL);
  return label;
}

static GtkWidget *
tile_new (PwProfile profile)
{
  GtkWidget *tile = gtk_toggle_button_new ();
  GtkWidget *box = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  g_autoptr (GIcon) icon = g_themed_icon_new_from_names ((char **) pw_profile_icon_names (profile),
                                                         -1);
  GtkWidget *image = gtk_image_new_from_gicon (icon, GTK_ICON_SIZE_LARGE_TOOLBAR);
  GtkWidget *label = gtk_label_new (pw_profile_label (profile));

  gtk_image_set_pixel_size (GTK_IMAGE (image), TILE_ICON_SIZE);
  gtk_box_pack_start (GTK_BOX (box), image, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (box), label, FALSE, FALSE, 0);
  gtk_container_add (GTK_CONTAINER (tile), box);
  gtk_style_context_add_class (gtk_widget_get_style_context (tile), "caminito-power-tile");
  gtk_widget_set_no_show_all (tile, TRUE);
  gtk_widget_show_all (box);
  return tile;
}

GtkWidget *
pw_profiles_section_new (void)
{
  Section *section = g_new0 (Section, 1);
  GtkWidget *tiles = gtk_box_new (GTK_ORIENTATION_HORIZONTAL, 6);

  add_styles ();
  section->section = gtk_box_new (GTK_ORIENTATION_VERTICAL, 4);
  gtk_box_set_homogeneous (GTK_BOX (tiles), TRUE);
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    {
      section->tiles[i] = tile_new ((PwProfile) i);
      g_signal_connect (section->tiles[i], "toggled", G_CALLBACK (on_tile_toggled), section);
      gtk_box_pack_start (GTK_BOX (tiles), section->tiles[i], TRUE, TRUE, 0);
    }
  section->degraded = note_new (TRUE);
  section->holds = note_new (FALSE);
  gtk_box_pack_start (GTK_BOX (section->section), tiles, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (section->section), section->degraded, FALSE, FALSE, 0);
  gtk_box_pack_start (GTK_BOX (section->section), section->holds, FALSE, FALSE, 0);
  gtk_widget_show (tiles);
  /* Hidden until the daemon answers, and for good where there's none. */
  gtk_widget_set_no_show_all (section->section, TRUE);
  g_signal_connect (section->section, "destroy", G_CALLBACK (section_free), section);
  section->ppd = pw_ppd_new (on_changed, section);
  return section->section;
}
