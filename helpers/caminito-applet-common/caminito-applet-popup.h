/*
 * The applets' popup (ADR 0035): a window placed against the applet, on the side of it
 * away from the panel's edge. Left-click on the applet toggles it, and a click anywhere
 * else, Escape or losing the pointer's grab closes it.
 */

#pragma once

#include <mate-panel-applet.h>

typedef struct _CaminitoAppletPopup CaminitoAppletPopup;

/* A popup for applet, which it lives as long as. Its window is empty: add the applet's
   content to caminito_applet_popup_get_window(). applet is a MatePanelApplet, whose
   orientation places the popup, or a widget in its place, which opens it downwards. */
CaminitoAppletPopup *caminito_applet_popup_new        (GtkWidget *applet);
/* Fixes the way the popup opens, where the applet isn't a MatePanelApplet to ask. */
void                 caminito_applet_popup_set_orient (CaminitoAppletPopup *popup,
                                                       MatePanelAppletOrient orient);
/* The popup's GtkWindow, to add the content to. The popup keeps it. */
GtkWidget           *caminito_applet_popup_get_window (CaminitoAppletPopup *popup);
/* Places it against the applet and shows it under a seat grab; nothing if it can't grab.
   It's placed again when its content changes its size, so that one that opens upwards
   keeps its bottom against the panel. */
void                 caminito_applet_popup_show       (CaminitoAppletPopup *popup);
void                 caminito_applet_popup_hide       (CaminitoAppletPopup *popup);
/* What left-click does: shows it, unless it was closed a moment ago by that same click. */
void                 caminito_applet_popup_toggle     (CaminitoAppletPopup *popup);
gboolean             caminito_applet_popup_is_shown   (CaminitoAppletPopup *popup);

/* Where the popup goes against the applet for a panel at the given edge: the applet's
   point the popup is anchored to, the popup's point at it, and how it may be moved to stay
   on screen. The orientation is the way the popup opens, away from the panel. */
void caminito_applet_popup_anchors (MatePanelAppletOrient orient,
                                    GdkGravity           *rect_anchor,
                                    GdkGravity           *window_anchor,
                                    GdkAnchorHints       *hints);
