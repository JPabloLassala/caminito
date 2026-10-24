/*
 * Which desktop entry a window's app id names (CMN-219), without a panel: the Dock applet
 * makes one Dock item of every window that resolves to one entry.
 */

#pragma once

#include <glib.h>

typedef struct _DkCatalog DkCatalog;

DkCatalog *dk_catalog_new       (void);
void       dk_catalog_free      (DkCatalog *catalog);
/* Adds the desktop entry with the id "firefox.desktop" and its StartupWMClass, which may be
   NULL. The first entry added with an id is the one kept: a user's entry shadows the
   system's. */
void       dk_catalog_add       (DkCatalog  *catalog,
                                 const char *id,
                                 const char *startup_wm_class);
/* Every desktop entry the system knows, shown in menus or not. */
DkCatalog *dk_catalog_new_installed (void);

/* The id of the desktop entry app_id names, or NULL if it names none: first the entry whose
   id is the app id (with or without ".desktop", in any case), then the one whose
   StartupWMClass is (in any case), which is how an X11 window's WM_CLASS finds its entry.
   The string is the catalog's. */
const char *dk_catalog_resolve  (const DkCatalog *catalog,
                                 const char      *app_id);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (DkCatalog, dk_catalog_free)
