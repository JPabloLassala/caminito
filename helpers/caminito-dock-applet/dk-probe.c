/*
 * The Dock end to end (CMN-219): the dock, in an undecorated toplevel of its own, on
 * whatever GDK backend it's run on, as a layer surface on the edge of the screen where the
 * compositor has the layer shell, as a panel is: a click on it doesn't take the focus from
 * the window it's about, as a click on a toplevel does. caminito's dock_applet test runs it under a headless
 * Caminito, on GDK's Wayland backend, and clicks on its items; the check script shows it
 * too. It's built for those, not installed.
 *
 *   dk-probe [--edge=bottom|top|left|right] [--size=PIXELS] [--title=TITLE]
 *
 * The kept items are the settings' at the path /org/mate/panel/objects/probe/prefs/, so a
 * run over a keyfile backend keeps them past its own end.
 *
 * stdout, a line each:
 *   ready                        the window is mapped
 *   window W H                   the window's size, first in the answer to "items"
 *   item KEY windows=N active=A kept=K x=X y=Y w=W h=H
 *                                one for each item, in order, in answer to "items": the
 *                                item's rectangle in the window, and then "items-end"
 *   menu LABEL                   one for each item of the open menu, in answer to "menu",
 *                                and then "menu-end"
 * stdin:
 *   items | menu | popup KEY | choose LABEL | keep KEY | drop KEY X Y | quit
 * "choose" activates the open menu's item with that label.
 */

#include <stdio.h>
#include <string.h>

#ifdef HAVE_GTK_LAYER_SHELL
#include <gtk-layer-shell.h>
#endif

#include "dk-dock.h"
#include "dk-item.h"

#define PATH "/org/mate/panel/objects/probe/prefs/"

static DkDock *dock;
static DkStore *store;
static GtkWidget *window;

static void
print_items (void)
{
  g_autoptr (GPtrArray) keys = dk_dock_get_keys (dock);

  printf ("window %d %d\n", gtk_widget_get_allocated_width (window),
          gtk_widget_get_allocated_height (window));
  for (guint i = 0; i < keys->len; i++)
    {
      GtkWidget *item = dk_dock_get_item (dock, keys->pdata[i]);
      DkItemView *view = dk_item_view_from_widget (item);
      GtkAllocation a;
      int x, y;

      gtk_widget_get_allocation (item, &a);
      gtk_widget_translate_coordinates (item, window, 0, 0, &x, &y);
      printf ("item %s windows=%u active=%d kept=%d x=%d y=%d w=%d h=%d\n",
              (char *) keys->pdata[i], dk_item_view_get_windows (view)->len,
              dk_item_view_is_active (view), dk_store_is_kept (store, keys->pdata[i]), x, y,
              a.width, a.height);
    }
  printf ("items-end\n");
}

static gboolean
answer_items (gpointer data G_GNUC_UNUSED)
{
  print_items ();
  return G_SOURCE_REMOVE;
}

static void
print_menu (void)
{
  GtkWidget *menu = dk_dock_get_menu (dock);

  if (menu != NULL)
    {
      g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (menu));

      for (GList *l = children; l != NULL; l = l->next)
        if (GTK_IS_SEPARATOR_MENU_ITEM (l->data))
          printf ("menu ---\n");
        else
          printf ("menu %s\n", gtk_menu_item_get_label (GTK_MENU_ITEM (l->data)));
    }
  printf ("menu-end\n");
}

static void
run_command (char *line)
{
  g_auto (GStrv) words = NULL;

  words = g_strsplit (g_strstrip (line), " ", 4);
  if (g_strcmp0 (words[0], "quit") == 0)
    gtk_main_quit ();
  else if (g_strcmp0 (words[0], "items") == 0)
    {
      /* What the compositor sent before the test asked is read by the roundtrip, and handled
         by GDK's source, of a higher priority, and the dock's refresh, before the low idle
         answers. */
      gdk_display_sync (gdk_display_get_default ());
      g_idle_add_full (G_PRIORITY_LOW, answer_items, NULL, NULL);
    }
  else if (g_strcmp0 (words[0], "menu") == 0)
    print_menu ();
  else if (g_strcmp0 (words[0], "popup") == 0 && words[1] != NULL)
    {
      GtkWidget *item = dk_dock_get_item (dock, words[1]);

      if (item != NULL)
        dk_dock_popup_menu (dock, item, NULL);
    }
  else if (g_strcmp0 (words[0], "keep") == 0 && words[1] != NULL)
    dk_store_keep (store, words[1]);
  else if (g_strcmp0 (words[0], "choose") == 0 && dk_dock_get_menu (dock) != NULL)
    {
      GtkWidget *menu = dk_dock_get_menu (dock);
      g_autoptr (GList) children = gtk_container_get_children (GTK_CONTAINER (menu));
      /* The label is the rest of the line. */
      const char *label = strchr (g_strstrip (line), ' ');

      for (GList *l = children; label != NULL && l != NULL; l = l->next)
        if (!GTK_IS_SEPARATOR_MENU_ITEM (l->data)
            && g_strcmp0 (gtk_menu_item_get_label (l->data), label + 1) == 0)
          {
            gtk_menu_item_activate (l->data);
            gtk_menu_popdown (GTK_MENU (menu));
            break;
          }
    }
  else if (g_strcmp0 (words[0], "drop") == 0 && words[1] != NULL && words[2] != NULL
           && words[3] != NULL)
    dk_dock_drop_key (dock, words[1], atoi (words[2]), atoi (words[3]));
}

/* Every line the channel has buffered: the watch fires on the descriptor, not on the buffer. */
static gboolean
on_stdin (GIOChannel *channel, GIOCondition condition G_GNUC_UNUSED, gpointer data G_GNUC_UNUSED)
{
  do
    {
      g_autofree char *line = NULL;
      gsize length;
      GIOStatus status = g_io_channel_read_line (channel, &line, &length, NULL, NULL);

      if (status == G_IO_STATUS_EOF || status == G_IO_STATUS_ERROR)
        {
          gtk_main_quit ();
          return G_SOURCE_REMOVE;
        }
      if (line != NULL)
        run_command (line);
    }
  while (g_io_channel_get_buffer_condition (channel) & G_IO_IN);
  return G_SOURCE_CONTINUE;
}

static gboolean
on_mapped (GtkWidget *widget G_GNUC_UNUSED, GdkEvent *event G_GNUC_UNUSED,
           gpointer data G_GNUC_UNUSED)
{
  printf ("ready\n");
  return FALSE;
}

static gboolean
on_timeout (gpointer data G_GNUC_UNUSED)
{
  printf ("timeout\n");
  gtk_main_quit ();
  return G_SOURCE_REMOVE;
}

int
main (int argc, char **argv)
{
  GtkPositionType edge = GTK_POS_BOTTOM;
  const char *title = "dk-probe";
  guint size = 48;
  g_autoptr (GSettings) settings = NULL;
  g_autoptr (GtkCssProvider) css = gtk_css_provider_new ();
  GIOChannel *in;

  setvbuf (stdout, NULL, _IOLBF, 0);
  gtk_init (&argc, &argv);
  for (int i = 1; i < argc; i++)
    {
      if (g_str_has_prefix (argv[i], "--edge="))
        {
          const char *name = argv[i] + strlen ("--edge=");

          edge = g_strcmp0 (name, "top") == 0 ? GTK_POS_TOP
                 : g_strcmp0 (name, "left") == 0 ? GTK_POS_LEFT
                 : g_strcmp0 (name, "right") == 0 ? GTK_POS_RIGHT : GTK_POS_BOTTOM;
        }
      else if (g_str_has_prefix (argv[i], "--size="))
        size = atoi (argv[i] + strlen ("--size="));
      else if (g_str_has_prefix (argv[i], "--title="))
        title = argv[i] + strlen ("--title=");
    }
  g_object_set (gtk_settings_get_default (), "gtk-enable-animations", FALSE, NULL);
  settings = g_settings_new_with_path ("org.mate.panel.applet.caminito-dock", PATH);
  store = dk_store_new (settings);
  dock = dk_dock_new (store);
  dk_dock_set_layout (dock, size, edge);
  /* Its own window isn't a program in the dock. */
  dk_dock_ignore_app_id (dock, g_get_prgname ());

  window = gtk_window_new (GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title (GTK_WINDOW (window), title);
  /* No client-side frame or shadow: the window's geometry is its allocation. */
  gtk_window_set_decorated (GTK_WINDOW (window), FALSE);
#ifdef HAVE_GTK_LAYER_SHELL
  if (gtk_layer_is_supported ())
    {
      gtk_layer_init_for_window (GTK_WINDOW (window));
      gtk_layer_set_layer (GTK_WINDOW (window), GTK_LAYER_SHELL_LAYER_TOP);
      gtk_layer_set_keyboard_mode (GTK_WINDOW (window), GTK_LAYER_SHELL_KEYBOARD_MODE_NONE);
      /* Along the whole edge, as a panel is, and as thick as its items, which it keeps clear
         of the windows. */
      gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_TOP,
                            edge != GTK_POS_BOTTOM);
      gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_BOTTOM,
                            edge != GTK_POS_TOP);
      gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_LEFT,
                            edge != GTK_POS_RIGHT);
      gtk_layer_set_anchor (GTK_WINDOW (window), GTK_LAYER_SHELL_EDGE_RIGHT,
                            edge != GTK_POS_LEFT);
      gtk_layer_set_exclusive_zone (GTK_WINDOW (window), size);
      if (edge == GTK_POS_TOP || edge == GTK_POS_BOTTOM)
        gtk_widget_set_size_request (window, -1, size);
      else
        gtk_widget_set_size_request (window, size, -1);
    }
#endif
  gtk_css_provider_load_from_data (css, "window { background: #303236; color: #eff0f1; }", -1,
                                   NULL);
  gtk_style_context_add_provider_for_screen (gdk_screen_get_default (), GTK_STYLE_PROVIDER (css),
                                             GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
  gtk_container_add (GTK_CONTAINER (window), dk_dock_get_widget (dock));
  g_signal_connect (window, "map-event", G_CALLBACK (on_mapped), NULL);
  g_signal_connect (window, "destroy", G_CALLBACK (gtk_main_quit), NULL);
  gtk_widget_show_all (window);

  in = g_io_channel_unix_new (0);
  g_io_add_watch (in, G_IO_IN | G_IO_HUP | G_IO_ERR, on_stdin, NULL);
  g_timeout_add_seconds (120, on_timeout, NULL);
  gtk_main ();
  dk_dock_free (dock);
  dk_store_free (store);
  return 0;
}
