/*
 * What the Network applet's icon shows (CMN-193), from NetworkManager's primary connection:
 * the icon's names, the lock over it while a VPN is up, and the tooltip; what the popup's
 * tiles and card show (CMN-196); the Networks tab's rows, their order, the search and the
 * bar that joins one (CMN-200); and its Details tab (CMN-201). Nothing here talks to
 * NetworkManager, so that the tests can build any status by hand.
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

typedef enum
{
  NW_DETAILS_WIRED,
  NW_DETAILS_WIFI,
  /* A VPN plugin's or WireGuard. */
  NW_DETAILS_VPN,
} NwDetailsKind;

/* An active connection as the Details tab shows it (CMN-201). A value NetworkManager
   hasn't is NULL, 0 or -1, and is left out. */
typedef struct
{
  NwDetailsKind kind;
  char         *name;
  char         *interface;
  /* "192.168.1.34/24", NULL-terminated. */
  char        **ipv4;
  char        **ipv6;
  /* IPv4's, then IPv6's. */
  char        **gateway;
  char        **dns;
  char         *mac;
  /* The link's speed in Mb/s: a wired one's, or Wi-Fi's bitrate. */
  guint         speed;
  /* Wi-Fi's: the access point's frequency in MHz, its security as "WPA2", and its signal,
     0 to 100. */
  guint         frequency;
  char         *security;
  int           strength;
} NwDetails;

typedef struct
{
  /* In static storage. */
  const char *label;
  /* Its lines apart by "\n". */
  char       *value;
} NwDetailRow;

/* A row of the Networks tab. */
typedef enum
{
  NW_NETWORK_WIRED,
  NW_NETWORK_WIFI,
  /* WireGuard too. */
  NW_NETWORK_VPN,
} NwNetworkKind;

/* A Wi-Fi network's, from its access point. */
typedef enum
{
  /* Open, or OWE, which has no password either. */
  NW_SECURITY_NONE,
  NW_SECURITY_WEP,
  /* WPA or WPA3 with a password. */
  NW_SECURITY_PSK,
  /* 802.1X. */
  NW_SECURITY_ENTERPRISE,
} NwSecurity;

typedef struct
{
  /* What a join is asked for by: "ssid:<SSID>" for a Wi-Fi network, saved or not, so that
     it stays the same while a new one is saved and comes up; "uuid:<UUID>" for the rest. */
  char         *key;
  char         *name;
  NwNetworkKind kind;
  NwSecurity    security;
  /* Wi-Fi's signal, 0 to 100, its strongest access point's; -1 for the rest. */
  int           strength;
  /* A connection NetworkManager has for it. */
  gboolean      saved;
  gboolean      active;
  /* Coming up, or asked to and not answered yet. */
  gboolean      connecting;
} NwNetwork;

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
  /* The Details tab's: the NwDetails of each active connection, the one the card shows
     first. NULL or empty with none. */
  GPtrArray *details;

  /* The Networks tab's rows, in nw_networks_sort()'s order: every connection but the
     primary one, which the card has. */
  NwNetwork *networks;
  guint      n_networks;
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

void               nw_details_free      (NwDetails *details);
/* The Details tab's grid for a connection, in its order, of NwDetailRow without what it
   hasn't: interface, IPv4, IPv6, gateway, DNS, MAC address, link speed, and for Wi-Fi the
   band, channel, security and signal. */
GPtrArray         *nw_state_detail_rows (const NwDetails *details);
/* The icon by the connection's name: its kind, and for Wi-Fi its signal. */
const char *const *nw_state_details_icon_names (const NwDetails *details);
/* A copy of network, for nw_network_free(). */
NwNetwork         *nw_network_copy        (const NwNetwork *network);
void               nw_network_free        (NwNetwork *network);
/* Active or coming up first, then VPNs, then saved, then available, each by signal then
   name. */
void               nw_networks_sort       (NwNetwork *networks, guint n_networks);
/* Whether search, in any case, is part of the network's name. An empty or NULL search
   matches every network. */
gboolean           nw_network_matches     (const NwNetwork *network, const char *search);
/* The row's icon: its kind's, and Wi-Fi's by its signal. */
const char *const *nw_network_icon_names  (const NwNetwork *network);

/* What the bar under the list is for the network selected. */
typedef enum
{
  /* Nothing to do: it's up. */
  NW_BAR_NONE,
  /* A new network with a password: a password field and "Join". */
  NW_BAR_PASSWORD,
  /* "Join Cafe", "Connect Work". */
  NW_BAR_JOIN,
  /* A new Enterprise network, which nm-connection-editor sets up: "Join eduroam…". */
  NW_BAR_EDITOR,
  /* A spinner and "Connecting to Cafe…". */
  NW_BAR_CONNECTING,
} NwBar;

/* The bar for network, and in text its button's label or its line, NULL for NW_BAR_NONE. */
NwBar              nw_state_bar           (const NwNetwork *network, char **text);
/* Whether NetworkManager takes password for the new network: WPA's 8 to 63 characters or
   64 hex digits, WEP's key or passphrase. */
gboolean           nw_state_password_valid (const NwNetwork *network, const char *password);
