/*
 * What the Network applet's icon shows (CMN-193), from NetworkManager's primary connection:
 * the icon's names, the lock over it while a VPN is up, and the tooltip. Nothing here talks
 * to NetworkManager, so that the tests can build any status by hand.
 */

#pragma once

#include <glib.h>

typedef enum
{
  /* Nothing up, and nothing coming up. */
  NW_PHASE_NONE,
  /* A connection coming up, with none up yet. */
  NW_PHASE_CONNECTING,
  NW_PHASE_CONNECTED,
} NwPhase;

typedef enum
{
  /* Ethernet, and whatever else carries the primary connection that isn't Wi-Fi. */
  NW_KIND_WIRED,
  NW_KIND_WIFI,
} NwKind;

typedef struct
{
  /* NetworkManager is on the bus. The rest is meaningless without it. */
  gboolean running;
  /* Every radio off: WirelessEnabled and WwanEnabled both false. */
  gboolean airplane;
  NwPhase  phase;
  /* The connection's, connecting or up. */
  NwKind   kind;
  char    *name;
  /* Wi-Fi's signal, 0 to 100, or -1 where the access point isn't known. */
  int      strength;
  /* Connectivity is portal or limited: the network is up, the internet isn't. */
  gboolean limited;
  /* The VPN that's up, NULL with none. */
  char    *vpn;
} NwStatus;

void     nw_status_clear (NwStatus *status);
gboolean nw_status_equal (const NwStatus *a, const NwStatus *b);
/* Copies src into dest, which holds nothing to free. */
void     nw_status_copy  (NwStatus *dest, const NwStatus *src);

/* The themed names for the icon, MATE's first, then Adwaita's symbolic ones, then Breeze's
   where it has neither. NULL-terminated, in static storage. */
const char *const *nw_state_icon_names (const NwStatus *status);
/* The names of the lock drawn over the icon while a VPN is up. */
const char *const *nw_state_lock_names (void);
/* "Home · 72% · no internet\nVPN: Work", "Not connected", "Airplane mode"… */
char              *nw_state_tooltip    (const NwStatus *status);
