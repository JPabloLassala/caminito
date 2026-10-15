/*
 * The applets' panel icon (ADR 0035): a themed icon with fallbacks, sized to the panel.
 * Icon themes disagree on names, so each applet asks for several, its own names in the
 * order it prefers them.
 */

#pragma once

#include <mate-panel-applet.h>

/* A GtkImage that follows the applet's size. Add it to the applet yourself. */
GtkWidget *caminito_applet_icon_new   (MatePanelApplet   *applet);
/* Shows the first of names the icon theme has: GTK looks for every name in the current
   theme before the themes it inherits from. names is NULL-terminated. */
void       caminito_applet_icon_set   (GtkWidget         *icon,
                                       const char *const *names);
/* The icon's size in pixels for a panel of the given size. */
int        caminito_applet_icon_pixel_size (guint panel_size);
