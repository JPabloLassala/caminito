/*
 * NetworkManager as the Network applet reads it (CMN-193), through libnm (ADR 0041): the
 * primary connection, what's coming up, connectivity, the radios and the VPN, as an
 * NwStatus. It follows NetworkManager stopping and starting again.
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
const NwStatus *nw_nm_status (NwNm *nm);
void            nw_nm_free   (NwNm *nm);
