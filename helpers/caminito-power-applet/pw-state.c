/*
 * What the Power applet shows (262). See pw-state.h.
 */

#include "pw-state.h"

#include <math.h>

PwIconKind
pw_state_icon_kind (const PwDisplay *display)
{
  if (!display->present)
    return PW_ICON_PLUG;
  switch (display->state)
    {
    case PW_STATE_CHARGING:
      return PW_ICON_CHARGING;
    case PW_STATE_FULLY_CHARGED:
      return PW_ICON_FULL;
    default:
      return PW_ICON_BATTERY;
    }
}

/* The percentage in steps of ten, the steps Adwaita and Breeze draw: 0 to 10. */
static int
level_of (double percentage)
{
  return (int) floor (CLAMP (percentage, 0, 100) / 10 + 0.5);
}

const char *const *
pw_state_icon_names (const PwDisplay *display)
{
  /* Static tables, one for each name an icon theme draws: they're built where they're
     asked for and kept: a panel asks for 33 of them at most. */
  static char *names[PW_ICON_PLUG][11][4];
  static const char *const plug[] = { "ac-adapter", "ac-adapter-symbolic", NULL };
  PwIconKind kind = pw_state_icon_kind (display);
  int level = level_of (display->percentage);
  int percentage = level * 10;
  const char *mate_name;
  char **names_of_level;

  if (kind == PW_ICON_PLUG)
    return plug;

  /* MATE's own themes have five levels, with "-charging" and "-charged" variants, and
     Adwaita the level in tens, symbolic. GTK takes the first name the current theme has
     before looking in the themes it inherits, so Breeze's come last without losing to
     hicolor's or Adwaita's. No name is a catch-all, as battery-missing-symbolic would be:
     GTK takes the last symbolic name of a list over the ones before it. MATE's five names
     follow the same steps of ten as Adwaita's glyphs, so that every theme shows the same
     level. */
  if (level >= 8)
    mate_name = "battery-full";
  else if (level >= 5)
    mate_name = "battery-good";
  else if (level >= 2)
    mate_name = "battery-low";
  else if (level >= 1)
    mate_name = "battery-caution";
  else
    mate_name = "battery-empty";

  names_of_level = names[kind][level];
  if (names_of_level[0] == NULL)
    {
      switch (kind)
        {
        case PW_ICON_CHARGING:
          names_of_level[0] = g_strdup_printf ("%s-charging", mate_name);
          names_of_level[1] = g_strdup_printf ("battery-level-%d-charging-symbolic", percentage);
          names_of_level[2] = g_strdup_printf ("battery-%03d-charging", percentage);
          break;
        case PW_ICON_FULL:
          names_of_level[0] = g_strdup ("battery-full-charged");
          names_of_level[1] = g_strdup ("battery-level-100-charged-symbolic");
          names_of_level[2] = g_strdup ("battery-100");
          break;
        case PW_ICON_BATTERY:
        case PW_ICON_PLUG:
        default:
          names_of_level[0] = g_strdup (mate_name);
          names_of_level[1] = g_strdup_printf ("battery-level-%d-symbolic", percentage);
          names_of_level[2] = g_strdup_printf ("battery-%03d", percentage);
          break;
        }
    }
  return (const char *const *) names_of_level;
}

char *
pw_state_percentage (const PwDisplay *display)
{
  if (!display->present)
    return NULL;
  return g_strdup_printf ("%d%%", (int) floor (CLAMP (display->percentage, 0, 100) + 0.5));
}

char *
pw_state_duration (gint64 seconds)
{
  gint64 minutes = MAX ((seconds + 30) / 60, 1);
  gint64 hours = minutes / 60;

  minutes %= 60;
  if (hours == 0)
    return g_strdup_printf ("%" G_GINT64_FORMAT " min", minutes);
  if (minutes == 0)
    return g_strdup_printf ("%" G_GINT64_FORMAT " h", hours);
  return g_strdup_printf ("%" G_GINT64_FORMAT " h %" G_GINT64_FORMAT " min", hours, minutes);
}

char *
pw_state_tooltip (const PwDisplay *display)
{
  g_autofree char *percentage = pw_state_percentage (display);
  g_autofree char *duration = NULL;

  if (percentage == NULL)
    return g_strdup ("Plugged in");
  switch (display->state)
    {
    case PW_STATE_CHARGING:
      if (display->time_to_full > 0)
        {
          duration = pw_state_duration (display->time_to_full);
          return g_strdup_printf ("%s\nFull in %s", percentage, duration);
        }
      return g_strdup_printf ("%s\nCharging", percentage);
    case PW_STATE_DISCHARGING:
      if (display->time_to_empty > 0)
        {
          duration = pw_state_duration (display->time_to_empty);
          return g_strdup_printf ("%s\n%s left", percentage, duration);
        }
      return g_strdup_printf ("%s\nOn battery", percentage);
    case PW_STATE_FULLY_CHARGED:
    case PW_STATE_PENDING_CHARGE:
      return g_strdup_printf ("%s\nPlugged in", percentage);
    case PW_STATE_EMPTY:
      return g_strdup_printf ("%s\nEmpty", percentage);
    case PW_STATE_PENDING_DISCHARGE:
    case PW_STATE_UNKNOWN:
    default:
      return g_strdup (percentage);
    }
}
