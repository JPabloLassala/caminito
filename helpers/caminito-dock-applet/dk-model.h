/*
 * The Dock's items (CMN-219), from the windows and the kept items, without a panel: one item
 * for every desktop entry, with the windows of every Application whose app id resolves to
 * it, and one for every app id that resolves to none. The kept items come first, in the
 * user's order. What a click on an item does is decided here too.
 */

#pragma once

#include <glib.h>

#include "dk-resolve.h"

/* One toplevel, as the window source reports it. */
typedef struct
{
  /* The source's own window, which the model only hands back. */
  gpointer handle;
  const char *app_id;
  const char *title;
  gboolean minimised;
  gboolean active;
  /* It demands attention (the xfce private protocol's state, which libxfce4windowing calls
     urgent). Caminito clears its own demand when the window is focused, but an X11 client's
     urgency hint stays until the client drops it. */
  gboolean urgent;
  /* When it was opened and when it was last active: a counter, larger for later. */
  guint64 opened;
  guint64 activated;
} DkWindow;

typedef struct
{
  /* The desktop entry's id, "firefox.desktop"; for an item with no entry, "window:" and the
     app id. */
  char *key;
  /* The desktop entry's id, or NULL if the item has none. */
  const char *entry_id;
  gboolean kept;
  /* The item's windows, most recently active first: DkWindow copies, whose strings the
     item owns. */
  GArray *windows;
} DkItem;

typedef enum
{
  DK_CLICK_LAUNCH,
  DK_CLICK_ACTIVATE,
  DK_CLICK_MINIMISE,
} DkClick;

/* The items for kept (the keys of the kept items, in order) and windows. A kept key that is
   a desktop entry's id stays when it has no window. The array is of DkItem, and owns them. */
GArray *dk_items_build (const DkCatalog *catalog,
                        const char *const *kept,
                        const DkWindow *windows,
                        guint            n_windows);
void    dk_item_clear  (DkItem *item);

/* Whether key is a desktop entry's, not the item of an app id that has none. */
gboolean dk_key_is_entry (const char *key);

/* What a click does: launch a program that isn't running; minimise the one window of a
   focused item; activate the most recent window otherwise (the Application overview will
   take the several windows' case). target is that window, or NULL for a launch. */
DkClick dk_item_click  (const DkItem *item, const DkWindow **target);

/* The kept keys, changed. They return TRUE when the list changed. A key is added at the
   end, moved to the position given (counted before it leaves its old one), or removed. */
gboolean dk_kept_add    (GPtrArray *kept, const char *key);
gboolean dk_kept_remove (GPtrArray *kept, const char *key);
gboolean dk_kept_move   (GPtrArray *kept, const char *key, guint position);
