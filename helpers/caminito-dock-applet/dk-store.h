/*
 * The kept Dock items (CMN-219), in the instance's own settings: the order the user
 * chose, which outlives the panel.
 */

#pragma once

#include <gio/gio.h>

typedef struct _DkStore DkStore;

typedef void (*DkStoreChanged) (gpointer data);

/* settings is the applet's relocatable settings, or NULL where its schema isn't installed:
   the items are kept for the session alone. The store follows the settings' changes. */
DkStore            *dk_store_new         (GSettings *settings);
void                dk_store_free        (DkStore *store);
void                dk_store_set_changed (DkStore *store, DkStoreChanged changed, gpointer data);
/* The keys, NULL-terminated, in order. */
const char *const  *dk_store_get         (DkStore *store);
gboolean            dk_store_is_kept     (DkStore *store, const char *key);
/* Each returns whether it changed the items, and tells the changed function if so. */
gboolean            dk_store_keep        (DkStore *store, const char *key);
gboolean            dk_store_remove      (DkStore *store, const char *key);
gboolean            dk_store_move        (DkStore *store, const char *key, guint position);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (DkStore, dk_store_free)
