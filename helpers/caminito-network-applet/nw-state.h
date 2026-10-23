/*
 * What the Network applet's icon shows (CMN-193), from NetworkManager's primary connection:
 * the icon's names, the lock over it while a VPN is up, and the tooltip; and what the
 * popup's tiles and card show (CMN-196). Nothing here talks to NetworkManager, so that the
 * tests can build any status by hand.
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

/* The wired device, for the Wired tile. */
typedef enum
{
  /* No ethernet device at all. */
  NW_WIRED_ABSENT,
  /* The cable out. */
  NW_WIRED_UNPLUGGED,
  NW_WIRED_DISCONNECTED,
  NW_WIRED_CONNECTING,
  NW_WIRED_CONNECTED,
} NwWired;

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

  /* The card's: the connection's IPv4 address, NULL without one, and a wired one's link
     speed in Mb/s, 0 where it isn't known. */
  char    *address;
  guint    speed;
  /* WirelessEnabled, and the hardware switch off: WirelessHardwareEnabled false. */
  gboolean wifi_enabled;
  gboolean wifi_blocked;
  /* The Wi-Fi network that's up, NULL with none. */
  char    *wifi_network;
  NwWired  wired;
  /* What the VPN tile turns on: the VPN activated most recently, NULL with none set up. */
  char    *last_vpn;
} NwStatus;

/* The popup's tiles, in their order: two by two, left to right. */
typedef enum
{
  NW_TILE_WIFI,
  NW_TILE_WIRED,
  NW_TILE_VPN,
  NW_TILE_AIRPLANE,
  NW_N_TILES,
} NwTile;

typedef struct
{
  gboolean    on;
  gboolean    sensitive;
  /* The status line, in static storage or the status's. */
  const char *text;
} NwTileState;

void     nw_status_clear (NwStatus *status);
gboolean nw_status_equal (const NwStatus *a, const NwStatus *b);

/* The themed names for the icon, MATE's first, then Adwaita's symbolic ones, then Breeze's
   where it has neither. NULL-terminated, in static storage. */
const char *const *nw_state_icon_names (const NwStatus *status);
/* The names of the lock drawn over the icon while a VPN is up. */
const char *const *nw_state_lock_names (void);
/* "Home · 72% · no internet\nVPN: Work", "Not connected", "Airplane mode"… */
char              *nw_state_tooltip    (const NwStatus *status);

NwTileState        nw_state_tile        (const NwStatus *status, NwTile tile);
/* The card's first line: the connection's name, "Connecting to Home…" or "Not connected". */
char              *nw_state_card_title  (const NwStatus *status);
/* Its second, with a connection up: "Connected · No internet · 1 Gb/s"; NULL without. */
char              *nw_state_card_detail (const NwStatus *status);
