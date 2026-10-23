/*
 * NetworkManager as the Network applet reads it (CMN-193), through libnm (ADR 0041): the
 * primary connection, what's coming up, connectivity, the radios and the VPN, as an
 * NwStatus, with each connection up as the Details tab shows it (CMN-201). It follows
 * NetworkManager stopping and starting again. It also does what the popup's tiles and card
 * ask (CMN-196).
 */

#pragma once

#include "nw-state.h"

typedef struct _NwNm NwNm;

/* Called whenever the status changes, and not otherwise. */
typedef void (*NwNmChanged) (NwNm *nm, gpointer data);

/* Starts reading NetworkManager. Until libnm answers, the status is that it isn't
   running. */
NwNm           *nw_nm_new    (NwNmChanged changed, gpointer data);
/* The status now, which nm keeps. */
const NwStatus *nw_nm_status     (NwNm *nm);
/* A tile's click: Wi-Fi's WirelessEnabled; the wired device connected or not; the VPN used
   last up or down; airplane mode, every radio off, then back as they were before it. The
   status follows once NetworkManager has done it, and doesn't if it refuses. */
void            nw_nm_set_tile   (NwNm *nm, NwTile tile, gboolean on);
/* The card's Disconnect: the device of the connection it shows goes down, as nmcli device
   disconnect does, so that NetworkManager doesn't bring up another network on it. */
void            nw_nm_disconnect (NwNm *nm);
void            nw_nm_free       (NwNm *nm);
