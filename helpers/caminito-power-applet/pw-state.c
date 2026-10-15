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

PwDevice *
pw_device_new (void)
{
  return g_new0 (PwDevice, 1);
}

void
pw_device_free (PwDevice *device)
{
  if (device == NULL)
    return;
  g_free (device->path);
  g_free (device->model);
  g_free (device);
}

void
pw_row_free (PwRow *row)
{
  if (row == NULL)
    return;
  g_free (row->path);
  g_free (row->name);
  g_free (row->state);
  g_strfreev (row->icon_names);
  g_free (row->percentage);
  g_free (row->health);
  g_free (row);
}

gboolean
pw_row_equal (const PwRow *a, const PwRow *b)
{
  return g_strcmp0 (a->path, b->path) == 0
         && a->laptop == b->laptop
         && g_strcmp0 (a->name, b->name) == 0
         && g_strcmp0 (a->state, b->state) == 0
         && g_strv_equal ((const char *const *) a->icon_names,
                          (const char *const *) b->icon_names)
         && a->fraction == b->fraction
         && g_strcmp0 (a->percentage, b->percentage) == 0
         && a->low == b->low
         && g_strcmp0 (a->health, b->health) == 0
         && a->limit_supported == b->limit_supported
         && a->limit_enabled == b->limit_enabled
         && a->limit_end == b->limit_end;
}

const char *
pw_state_state_name (PwState state)
{
  switch (state)
    {
    case PW_STATE_CHARGING:
      return "Charging";
    case PW_STATE_DISCHARGING:
      return "Discharging";
    case PW_STATE_EMPTY:
      return "Empty";
    case PW_STATE_FULLY_CHARGED:
      return "Fully charged";
    case PW_STATE_PENDING_CHARGE:
    case PW_STATE_PENDING_DISCHARGE:
      return "Not charging";
    case PW_STATE_UNKNOWN:
    default:
      return "Unknown";
    }
}

const char *
pw_state_kind_name (guint type)
{
  switch (type)
    {
    case PW_TYPE_UPS:
      return "UPS";
    case PW_TYPE_MONITOR:
      return "Monitor";
    case PW_TYPE_MOUSE:
      return "Mouse";
    case PW_TYPE_KEYBOARD:
      return "Keyboard";
    case PW_TYPE_PDA:
      return "PDA";
    case PW_TYPE_PHONE:
      return "Phone";
    case PW_TYPE_MEDIA_PLAYER:
      return "Media player";
    case PW_TYPE_TABLET:
      return "Tablet";
    case PW_TYPE_COMPUTER:
      return "Computer";
    case PW_TYPE_GAMING_INPUT:
      return "Game controller";
    case PW_TYPE_PEN:
      return "Pen";
    case PW_TYPE_TOUCHPAD:
      return "Touchpad";
    case PW_TYPE_HEADSET:
      return "Headset";
    case PW_TYPE_SPEAKERS:
      return "Speakers";
    case PW_TYPE_HEADPHONES:
      return "Headphones";
    case PW_TYPE_OTHER_AUDIO:
      return "Audio device";
    case PW_TYPE_REMOTE_CONTROL:
      return "Remote control";
    case PW_TYPE_CAMERA:
      return "Camera";
    case PW_TYPE_WEARABLE:
      return "Wearable";
    case PW_TYPE_BATTERY:
      return "Battery";
    default:
      return "Device";
    }
}

const char *const *
pw_state_kind_icon_names (guint type)
{
  /* Symbolic first, as the panel's glyphs are, then the colour icon of the same thing.
     A type with no icon of its own gets a battery. */
  static const char *const mouse[] = { "input-mouse-symbolic", "input-mouse", NULL };
  static const char *const keyboard[] = { "input-keyboard-symbolic", "input-keyboard", NULL };
  static const char *const phone[] = { "phone-symbolic", "phone", NULL };
  static const char *const player[] = { "multimedia-player-symbolic", "multimedia-player", NULL };
  static const char *const tablet[] = { "input-tablet-symbolic", "input-tablet", NULL };
  static const char *const computer[] = { "computer-symbolic", "computer", NULL };
  static const char *const gaming[] = { "input-gaming-symbolic", "input-gaming", NULL };
  static const char *const touchpad[] = { "input-touchpad-symbolic", "input-touchpad", NULL };
  static const char *const headset[] = { "audio-headset-symbolic", "audio-headset", NULL };
  static const char *const speakers[] = { "audio-speakers-symbolic", "audio-speakers", NULL };
  static const char *const headphones[] = {
    "audio-headphones-symbolic", "audio-headphones", NULL,
  };
  static const char *const audio[] = { "audio-card-symbolic", "audio-card", NULL };
  static const char *const camera[] = { "camera-photo-symbolic", "camera-photo", NULL };
  static const char *const monitor[] = { "video-display-symbolic", "video-display", NULL };
  static const char *const battery[] = { "battery-symbolic", "battery", NULL };

  switch (type)
    {
    case PW_TYPE_MOUSE:
      return mouse;
    case PW_TYPE_KEYBOARD:
      return keyboard;
    case PW_TYPE_PDA:
    case PW_TYPE_PHONE:
      return phone;
    case PW_TYPE_MEDIA_PLAYER:
      return player;
    case PW_TYPE_TABLET:
    case PW_TYPE_PEN:
      return tablet;
    case PW_TYPE_COMPUTER:
      return computer;
    case PW_TYPE_GAMING_INPUT:
      return gaming;
    case PW_TYPE_TOUCHPAD:
      return touchpad;
    case PW_TYPE_HEADSET:
      return headset;
    case PW_TYPE_SPEAKERS:
      return speakers;
    case PW_TYPE_HEADPHONES:
      return headphones;
    case PW_TYPE_OTHER_AUDIO:
      return audio;
    case PW_TYPE_CAMERA:
      return camera;
    case PW_TYPE_MONITOR:
      return monitor;
    default:
      return battery;
    }
}

#define LOW_PERCENTAGE      20
#define DEFAULT_LIMIT_END   80

static gboolean
is_laptop_battery (const PwDevice *device)
{
  return device->type == PW_TYPE_BATTERY && device->power_supply;
}

/* What the table lists: what's there and has a percentage, but not the mains. */
static gboolean
is_listed (const PwDevice *device)
{
  return device->present && device->type != PW_TYPE_LINE_POWER && device->has_percentage;
}

static int
percent_of (double percentage)
{
  return (int) floor (CLAMP (percentage, 0, 100) + 0.5);
}

static PwRow *
row_new (const PwDevice *device, const char *name, gboolean laptop)
{
  PwRow *row = g_new0 (PwRow, 1);
  int percent = percent_of (device->percentage);

  row->path = g_strdup (device->path);
  row->laptop = laptop;
  row->name = g_strdup (name);
  row->fraction = percent / 100.0;
  row->percentage = g_strdup_printf ("%d%%", percent);
  row->low = percent < LOW_PERCENTAGE;
  if (laptop)
    {
      PwDisplay display = {
        .present = TRUE,
        .state = device->state,
        .percentage = device->percentage,
      };

      row->icon_names = g_strdupv ((char **) pw_state_icon_names (&display));
      row->limit_supported = device->threshold_supported;
      row->limit_enabled = device->threshold_supported && device->threshold_enabled;
      /* A limit of 100% limits nothing: what UPower reports with the limit off. */
      row->limit_end = device->threshold_end > 0 && device->threshold_end < 100
                         ? device->threshold_end
                         : DEFAULT_LIMIT_END;
      if (row->limit_enabled)
        row->state = g_strdup_printf ("%s · limit %u%%", pw_state_state_name (device->state),
                                      row->limit_end);
      if (device->capacity > 0)
        row->health = g_strdup_printf ("Health %d%%", percent_of (device->capacity));
      else
        row->health = g_strdup ("Health unknown");
    }
  else
    row->icon_names = g_strdupv ((char **) pw_state_kind_icon_names (device->type));
  if (row->state == NULL)
    /* A mouse with no state is "Mouse", not "Unknown". */
    row->state = g_strdup (!laptop && device->state == PW_STATE_UNKNOWN
                             ? pw_state_kind_name (device->type)
                             : pw_state_state_name (device->state));
  return row;
}

static int
compare_devices_by_path (gconstpointer a, gconstpointer b)
{
  const PwDevice *x = *(PwDevice *const *) a;
  const PwDevice *y = *(PwDevice *const *) b;

  return g_strcmp0 (x->path, y->path);
}

static const char *
peripheral_name (const PwDevice *device)
{
  return device->model != NULL && device->model[0] != '\0' ? device->model
                                                           : pw_state_kind_name (device->type);
}

static int
compare_peripherals (gconstpointer a, gconstpointer b)
{
  const PwDevice *x = *(PwDevice *const *) a;
  const PwDevice *y = *(PwDevice *const *) b;
  int by_name = g_utf8_collate (peripheral_name (x), peripheral_name (y));

  return by_name != 0 ? by_name : g_strcmp0 (x->path, y->path);
}

GPtrArray *
pw_state_rows (GPtrArray *devices)
{
  GPtrArray *rows = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_row_free);
  g_autoptr (GPtrArray) laptop = g_ptr_array_new ();
  g_autoptr (GPtrArray) peripherals = g_ptr_array_new ();

  for (guint i = 0; i < devices->len; i++)
    {
      PwDevice *device = g_ptr_array_index (devices, i);

      if (!is_listed (device))
        continue;
      g_ptr_array_add (is_laptop_battery (device) ? laptop : peripherals, device);
    }
  g_ptr_array_sort (laptop, compare_devices_by_path);
  g_ptr_array_sort (peripherals, compare_peripherals);
  for (guint i = 0; i < laptop->len; i++)
    {
      /* "Battery" alone, "Battery 1" and "Battery 2" when there's more than one: UPower's
         own model names ("DELL 5XKWK") tell the user nothing. */
      g_autofree char *name = laptop->len == 1 ? g_strdup ("Battery")
                                               : g_strdup_printf ("Battery %u", i + 1);

      g_ptr_array_add (rows, row_new (g_ptr_array_index (laptop, i), name, TRUE));
    }
  for (guint i = 0; i < peripherals->len; i++)
    {
      PwDevice *device = g_ptr_array_index (peripherals, i);

      g_ptr_array_add (rows, row_new (device, peripheral_name (device), FALSE));
    }
  return rows;
}
