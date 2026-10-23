/*
 * What the Network applet's icon shows (CMN-193). See nw-state.h.
 */

#include "nw-state.h"

void
nw_status_clear (NwStatus *status)
{
  g_clear_pointer (&status->name, g_free);
  g_clear_pointer (&status->vpn, g_free);
  g_clear_pointer (&status->address, g_free);
  g_clear_pointer (&status->wifi_network, g_free);
  g_clear_pointer (&status->last_vpn, g_free);
}

gboolean
nw_status_equal (const NwStatus *a, const NwStatus *b)
{
  return a->running == b->running && a->airplane == b->airplane && a->phase == b->phase
         && a->kind == b->kind && a->strength == b->strength && a->limited == b->limited
         && g_strcmp0 (a->name, b->name) == 0 && g_strcmp0 (a->vpn, b->vpn) == 0
         && g_strcmp0 (a->address, b->address) == 0 && a->speed == b->speed
         && a->wifi_enabled == b->wifi_enabled && a->wifi_blocked == b->wifi_blocked
         && g_strcmp0 (a->wifi_network, b->wifi_network) == 0 && a->wired == b->wired
         && g_strcmp0 (a->last_vpn, b->last_vpn) == 0;
}

/* The signal in GNOME Shell's four steps: weak, ok, good and excellent. */
static int
step_of (int strength)
{
  if (strength > 80)
    return 3;
  if (strength > 55)
    return 2;
  if (strength > 30)
    return 1;
  return 0;
}

const char *const *
nw_state_icon_names (const NwStatus *status)
{
  /* MATE's themes have the plain names where they have any, and the symbolic ones as
     Adwaita does. Breeze has the signal's names, but not -no-route or a wired -acquiring:
     its own come last. GTK takes the first name the current theme has before looking in
     the themes it inherits, so they don't lose to hicolor's or Adwaita's. */
  static const char *const offline[] = { "network-offline", "network-offline-symbolic", NULL };
  static const char *const airplane[] = {
    "airplane-mode", "airplane-mode-symbolic", "network-flightmode-on", NULL,
  };
  static const char *const wired[] = { "network-wired", "network-wired-symbolic", NULL };
  static const char *const wired_no_route[] = {
    "network-wired-no-route", "network-wired-no-route-symbolic",
    "network-wired-activated-limited", NULL,
  };
  static const char *const wired_acquiring[] = {
    "network-wired-acquiring", "network-wired-acquiring-symbolic", "network-wired", NULL,
  };
  static const char *const wifi_acquiring[] = {
    "network-wireless-acquiring", "network-wireless-acquiring-symbolic", NULL,
  };
  static const char *const wifi[4][3] = {
    { "network-wireless-signal-weak", "network-wireless-signal-weak-symbolic", NULL },
    { "network-wireless-signal-ok", "network-wireless-signal-ok-symbolic", NULL },
    { "network-wireless-signal-good", "network-wireless-signal-good-symbolic", NULL },
    { "network-wireless-signal-excellent", "network-wireless-signal-excellent-symbolic", NULL },
  };
  static const char *const wifi_no_route[4][4] = {
    { "network-wireless-no-route", "network-wireless-no-route-symbolic",
      "network-wireless-20-limited", NULL },
    { "network-wireless-no-route", "network-wireless-no-route-symbolic",
      "network-wireless-40-limited", NULL },
    { "network-wireless-no-route", "network-wireless-no-route-symbolic",
      "network-wireless-80-limited", NULL },
    { "network-wireless-no-route", "network-wireless-no-route-symbolic",
      "network-wireless-100-limited", NULL },
  };
  int step;

  if (!status->running)
    return offline;
  switch (status->phase)
    {
    case NW_PHASE_CONNECTING:
      return status->kind == NW_KIND_WIFI ? wifi_acquiring : wired_acquiring;
    case NW_PHASE_CONNECTED:
      if (status->kind == NW_KIND_WIRED)
        return status->limited ? wired_no_route : wired;
      /* An access point not known yet shows full bars rather than a weak signal. */
      step = status->strength < 0 ? 3 : step_of (status->strength);
      return status->limited ? wifi_no_route[step] : wifi[step];
    case NW_PHASE_NONE:
    default:
      return status->airplane ? airplane : offline;
    }
}

const char *const *
nw_state_lock_names (void)
{
  /* A lock glyph, drawn small in the icon's corner: MATE's, then Adwaita's, which Breeze has
     too. Not nm-applet's nm-vpn-active-lock, which is an emblem at the icon's own size with
     the lock already in its corner. */
  static const char *const lock[] = {
    "changes-prevent", "changes-prevent-symbolic", "object-locked", NULL,
  };

  return lock;
}

char *
nw_state_tooltip (const NwStatus *status)
{
  GString *tooltip;

  if (!status->running)
    return g_strdup ("NetworkManager isn't running");
  switch (status->phase)
    {
    case NW_PHASE_CONNECTING:
      tooltip = g_string_new (NULL);
      g_string_printf (tooltip, "Connecting to %s…", status->name);
      break;
    case NW_PHASE_CONNECTED:
      tooltip = g_string_new (status->name);
      if (status->kind == NW_KIND_WIFI && status->strength >= 0)
        g_string_append_printf (tooltip, " · %d%%", status->strength);
      if (status->limited)
        g_string_append (tooltip, " · no internet");
      break;
    case NW_PHASE_NONE:
    default:
      tooltip = g_string_new (status->airplane ? "Airplane mode" : "Not connected");
      break;
    }
  if (status->vpn != NULL)
    g_string_append_printf (tooltip, "\nVPN: %s", status->vpn);
  return g_string_free (tooltip, FALSE);
}

NwTileState
nw_state_tile (const NwStatus *status, NwTile tile)
{
  NwTileState state = { .sensitive = TRUE };

  switch (tile)
    {
    case NW_TILE_WIFI:
      state.on = status->wifi_enabled && !status->wifi_blocked;
      state.sensitive = !status->airplane && !status->wifi_blocked;
      if (!state.on)
        state.text = "Off";
      else
        state.text = status->wifi_network != NULL ? status->wifi_network : "On";
      break;
    case NW_TILE_WIRED:
      state.on = status->wired == NW_WIRED_CONNECTING || status->wired == NW_WIRED_CONNECTED;
      state.sensitive = status->wired != NW_WIRED_ABSENT && status->wired != NW_WIRED_UNPLUGGED;
      switch (status->wired)
        {
        case NW_WIRED_CONNECTED:
          state.text = "Connected";
          break;
        case NW_WIRED_CONNECTING:
          state.text = "Connecting…";
          break;
        case NW_WIRED_DISCONNECTED:
          state.text = "Disconnected";
          break;
        case NW_WIRED_UNPLUGGED:
        case NW_WIRED_ABSENT:
        default:
          state.text = "Unplugged";
          break;
        }
      break;
    case NW_TILE_VPN:
      state.on = status->vpn != NULL;
      state.sensitive = status->vpn != NULL || status->last_vpn != NULL;
      state.text = status->vpn != NULL ? status->vpn : "Off";
      break;
    case NW_TILE_AIRPLANE:
    default:
      state.on = status->airplane;
      state.text = status->airplane ? "On" : "Off";
      break;
    }
  return state;
}

char *
nw_state_card_title (const NwStatus *status)
{
  switch (status->phase)
    {
    case NW_PHASE_CONNECTING:
      return g_strdup_printf ("Connecting to %s…", status->name);
    case NW_PHASE_CONNECTED:
      return g_strdup (status->name);
    case NW_PHASE_NONE:
    default:
      return g_strdup ("Not connected");
    }
}

char *
nw_state_card_detail (const NwStatus *status)
{
  GString *detail;

  if (status->phase != NW_PHASE_CONNECTED)
    return NULL;
  detail = g_string_new ("Connected");
  if (status->limited)
    g_string_append (detail, " · No internet");
  /* A Wi-Fi link's rate changes all the time: the Details tab has it (CMN-201). */
  if (status->kind == NW_KIND_WIRED && status->speed >= 1000)
    g_string_append_printf (detail, " · %g Gb/s", status->speed / 1000.0);
  else if (status->kind == NW_KIND_WIRED && status->speed > 0)
    g_string_append_printf (detail, " · %u Mb/s", status->speed);
  return g_string_free (detail, FALSE);
}
