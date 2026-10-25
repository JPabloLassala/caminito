/*
 * The Dock applet (CMN-219, ADR 0035): an in-process mate-panel applet that is a row of Dock
 * items, one for every desktop entry with a window and every kept one, in a panel of its
 * own. The windows come from libxfce4windowing (dk-dock.c), the kept items from the
 * instance's own relocatable settings (dk-store.c). Left-click launches a program, or
 * activates, or minimises, its window; middle-click launches a new instance; right-click
 * opens the item's menu. The panel's own menu is where the dock has no item. The Trash
 * (CMN-220) ends the row, after a separator.
 */

#include <mate-panel-applet.h>

#include "dk-dock.h"
#include "dk-store.h"

#define FACTORY_ID "CaminitoDockAppletFactory"
#define APPLET_ID  "CaminitoDockApplet"
#define SCHEMA     "org.mate.panel.applet.caminito-dock"

typedef struct
{
  MatePanelApplet *applet;
  GSettings       *settings;
  DkStore         *store;
  DkDock          *dock;
} Applet;

/* The edge of the screen the panel is along: the orientation is the way its popups open,
   away from it. */
static GtkPositionType
edge_of (MatePanelAppletOrient orient)
{
  switch (orient)
    {
    case MATE_PANEL_APPLET_ORIENT_DOWN:
      return GTK_POS_TOP;
    case MATE_PANEL_APPLET_ORIENT_LEFT:
      return GTK_POS_RIGHT;
    case MATE_PANEL_APPLET_ORIENT_RIGHT:
      return GTK_POS_LEFT;
    default:
      return GTK_POS_BOTTOM;
    }
}

static void
update_layout (Applet *applet)
{
  dk_dock_set_layout (applet->dock, mate_panel_applet_get_size (applet->applet),
                      edge_of (mate_panel_applet_get_orient (applet->applet)));
}

/* Both "change-size" and "change-orient" carry the new value before the user data. */
static void
on_layout_changed (MatePanelApplet *panel_applet G_GNUC_UNUSED, guint value G_GNUC_UNUSED,
                   gpointer data)
{
  update_layout (data);
}

static void
applet_free (gpointer data)
{
  Applet *applet = data;

  dk_dock_free (applet->dock);
  dk_store_free (applet->store);
  g_clear_object (&applet->settings);
  g_free (applet);
}

/* The applet's settings at the path mate-panel gives this instance, or NULL where its schema
   isn't installed, which would abort. */
static GSettings *
settings_new (MatePanelApplet *panel_applet)
{
  GSettingsSchemaSource *source = g_settings_schema_source_get_default ();
  g_autoptr (GSettingsSchema) schema = NULL;
  g_autofree char *path = mate_panel_applet_get_preferences_path (panel_applet);
  g_autofree char *slashed = NULL;

  if (source != NULL)
    schema = g_settings_schema_source_lookup (source, SCHEMA, TRUE);
  if (schema == NULL || path == NULL)
    return NULL;
  slashed = g_str_has_suffix (path, "/") ? g_strdup (path) : g_strconcat (path, "/", NULL);
  return g_settings_new_with_path (SCHEMA, slashed);
}

static gboolean
fill (MatePanelApplet *panel_applet)
{
  Applet *applet = g_new0 (Applet, 1);

  applet->applet = panel_applet;
  mate_panel_applet_set_flags (panel_applet, MATE_PANEL_APPLET_EXPAND_MINOR);
  mate_panel_applet_set_background_widget (panel_applet, GTK_WIDGET (panel_applet));
  applet->settings = settings_new (panel_applet);
  if (applet->settings == NULL)
    g_warning ("can't keep the Dock items past the panel's restart: the schema %s isn't "
               "installed", SCHEMA);
  applet->store = dk_store_new (applet->settings);
  applet->dock = dk_dock_new (applet->store);
  g_object_set_data_full (G_OBJECT (panel_applet), "caminito-dock-applet", applet, applet_free);
  gtk_container_add (GTK_CONTAINER (panel_applet), dk_dock_get_widget (applet->dock));
  update_layout (applet);
  g_signal_connect (panel_applet, "change-size", G_CALLBACK (on_layout_changed), applet);
  g_signal_connect (panel_applet, "change-orient", G_CALLBACK (on_layout_changed), applet);
  gtk_widget_show_all (GTK_WIDGET (panel_applet));
  return TRUE;
}

static gboolean
factory (MatePanelApplet *applet, const char *iid, gpointer data G_GNUC_UNUSED)
{
  if (g_strcmp0 (iid, APPLET_ID) != 0)
    return FALSE;
  return fill (applet);
}

MATE_PANEL_APPLET_IN_PROCESS_FACTORY (FACTORY_ID, PANEL_TYPE_APPLET, "Dock", factory, NULL)
