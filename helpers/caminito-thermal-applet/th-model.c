/*
 * What the Thermal applet shows (267). See th-model.h.
 */

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "th-model.h"

/* How many reads a fan with no max is watched for before its fastest is its bar's scale. */
#define FAN_WATCHED 10

/* The temperature bar's empty and full ends, in °C. */
#define BAR_EMPTY 20.0
#define BAR_FULL  100.0

/* The panel's colours for warm and hot, which read on a dark panel and a light one. */
#define WARM_COLOUR "#ffa630"
#define HOT_COLOUR  "#ff5c5c"

struct _ThModel
{
  GPtrArray  *temperatures;
  GPtrArray  *fans;
  GPtrArray  *panel_sensors;
  /* The ids of the readings that weren't 0 at least once. */
  GHashTable *moved;
  /* A FanWatch for each fan's id, since the applet started. */
  GHashTable *fan_watches;
  /* The panel sensors chosen by default, kept while they're shown. */
  char       *cpu_id;
  char       *gpu_id;
  /* The CPU's Package or Tctl in the last read, NULL with neither. */
  char       *package_id;
  /* The pins' reading ids, oldest first; empty for the defaults. */
  char      **pinned;
  gboolean    fahrenheit;
  /* Where warm and hot start, in °C. */
  double      warm;
  double      hot;
};

typedef struct
{
  guint  reads;
  double fastest;
} FanWatch;

ThRaw *
th_raw_new (const char *chip, const char *chip_id, const char *feature, const char *label,
            const char *device, gboolean is_fan, gboolean ok, double value, double limit)
{
  ThRaw *raw = g_new0 (ThRaw, 1);

  raw->chip = g_strdup (chip);
  raw->chip_id = g_strdup (chip_id);
  raw->feature = g_strdup (feature);
  raw->label = g_strdup (label);
  raw->device = g_strdup (device);
  raw->is_fan = is_fan;
  raw->ok = ok;
  raw->value = value;
  raw->limit = limit;
  return raw;
}

void
th_raw_free (ThRaw *raw)
{
  g_free (raw->chip);
  g_free (raw->chip_id);
  g_free (raw->feature);
  g_free (raw->label);
  g_free (raw->device);
  g_free (raw);
}

static void
reading_free (ThReading *reading)
{
  g_free (reading->id);
  g_free (reading->name);
  g_free (reading);
}

static gboolean
is (const char *chip, const char *name)
{
  return g_strcmp0 (chip, name) == 0;
}

static gboolean
starts (const char *chip, const char *prefix)
{
  return g_str_has_prefix (chip, prefix);
}

gboolean
th_chip_is_left_out (const char *chip)
{
  return starts (chip, "BAT") || is (chip, "AC") || starts (chip, "ucsi_");
}

ThGroup
th_chip_group (const char *chip)
{
  if (is (chip, "coretemp") || is (chip, "k10temp") || is (chip, "zenpower")
      || is (chip, "cpu_thermal"))
    return TH_GROUP_CPU;
  if (is (chip, "amdgpu") || is (chip, "nouveau") || is (chip, "radeon") || is (chip, "i915")
      || is (chip, "xe"))
    return TH_GROUP_GPU;
  if (is (chip, "nvme") || is (chip, "drivetemp"))
    return TH_GROUP_STORAGE;
  if (starts (chip, "iwlwifi") || starts (chip, "ath") || starts (chip, "mt79"))
    return TH_GROUP_WIRELESS;
  /* The motherboard's Super I/O chips, Nuvoton's, ITE's, Winbond's and Fintek's, and the
     makers' own. */
  if (is (chip, "acpitz") || starts (chip, "pch_") || is (chip, "thinkpad")
      || is (chip, "dell_smm") || starts (chip, "nct") || starts (chip, "it8")
      || starts (chip, "w83") || starts (chip, "f71") || starts (chip, "asus")
      || is (chip, "gigabyte_wmi"))
    return TH_GROUP_BOARD;
  return TH_GROUP_OTHER;
}

/* The embedded controller's chips. */
static gboolean
is_ec (const char *chip)
{
  return is (chip, "thinkpad") || is (chip, "dell_smm");
}

/* What a reading with no label is called after its chip. */
static const char *
friendly (const char *chip)
{
  if (is_ec (chip))
    return "Embedded controller";
  if (is (chip, "acpitz"))
    return "ACPI zone";
  if (starts (chip, "pch_"))
    return "Chipset";
  if (is (chip, "nvme"))
    return "SSD";
  if (is (chip, "drivetemp"))
    return "Disk";
  switch (th_chip_group (chip))
    {
    case TH_GROUP_CPU:
      return "CPU";
    case TH_GROUP_GPU:
      return "GPU";
    case TH_GROUP_WIRELESS:
      return "Wi-Fi";
    case TH_GROUP_BOARD:
      return "Motherboard";
    case TH_GROUP_STORAGE:
    case TH_GROUP_OTHER:
    default:
      return chip;
    }
}

/* The number in "temp3". */
static const char *
feature_number (const char *feature)
{
  const char *digits = feature;

  while (*digits != '\0' && !g_ascii_isdigit (*digits))
    digits++;
  return digits;
}

static gboolean
is_package (const ThRaw *raw)
{
  if (is (raw->chip, "coretemp"))
    return g_str_has_prefix (raw->label != NULL ? raw->label : "", "Package id ");
  if (is (raw->chip, "k10temp") || is (raw->chip, "zenpower"))
    return g_strcmp0 (raw->label, "Tctl") == 0;
  return FALSE;
}

/* A reading's name before two of them can be the same. unlabelled is how many readings of
   its kind its chip has with no label. */
static char *
raw_name (const ThRaw *raw, guint unlabelled)
{
  ThGroup group = th_chip_group (raw->chip);
  const char *base = friendly (raw->chip);

  if (group == TH_GROUP_STORAGE && raw->device != NULL)
    base = raw->device;

  if (raw->is_fan)
    {
      if (raw->label != NULL)
        return g_strdup (raw->label);
      if (group == TH_GROUP_GPU)
        return g_strdup ("GPU fan");
      if (unlabelled > 1)
        return g_strdup_printf ("Fan %s", feature_number (raw->feature));
      return g_strdup ("Fan");
    }

  if (raw->label == NULL)
    {
      if (unlabelled > 1)
        return g_strdup_printf ("%s %s", base, feature_number (raw->feature));
      return g_strdup (base);
    }
  if (is (raw->chip, "coretemp") && is_package (raw))
    {
      const char *id = raw->label + strlen ("Package id ");

      return strcmp (id, "0") == 0 ? g_strdup ("CPU Package")
                                   : g_strdup_printf ("CPU Package %s", id);
    }
  if (is (raw->chip, "k10temp") || is (raw->chip, "zenpower"))
    {
      if (strcmp (raw->label, "Tctl") == 0)
        return g_strdup ("CPU");
      if (strcmp (raw->label, "Tdie") == 0)
        return g_strdup ("CPU die");
      if (g_str_has_prefix (raw->label, "Tccd"))
        return g_strdup_printf ("CCD %s", raw->label + strlen ("Tccd"));
    }
  if (is_ec (raw->chip))
    return g_strdup_printf ("EC: %s", raw->label);
  if (group == TH_GROUP_GPU || group == TH_GROUP_STORAGE)
    {
      /* An NVMe drive's Composite is the drive's own. */
      if (strcmp (raw->label, "Composite") == 0)
        return g_strdup (base);
      return g_strdup_printf ("%s %s", base, raw->label);
    }
  return g_strdup (raw->label);
}

/* Numbers the names that two readings or more have, in their order. */
static void
number_same_names (GPtrArray *readings)
{
  g_autoptr (GHashTable) counts = g_hash_table_new (g_str_hash, g_str_equal);
  g_autoptr (GHashTable) seen = g_hash_table_new (g_str_hash, g_str_equal);
  g_autoptr (GPtrArray) old_names = g_ptr_array_new_with_free_func (g_free);

  for (guint i = 0; i < readings->len; i++)
    {
      ThReading *reading = readings->pdata[i];
      guint count = GPOINTER_TO_UINT (g_hash_table_lookup (counts, reading->name));

      g_hash_table_insert (counts, reading->name, GUINT_TO_POINTER (count + 1));
    }
  for (guint i = 0; i < readings->len; i++)
    {
      ThReading *reading = readings->pdata[i];
      guint n;

      if (GPOINTER_TO_UINT (g_hash_table_lookup (counts, reading->name)) < 2)
        continue;
      n = GPOINTER_TO_UINT (g_hash_table_lookup (seen, reading->name)) + 1;
      g_hash_table_insert (seen, reading->name, GUINT_TO_POINTER (n));
      /* The tables' keys are the old names, which stay until they're done with. */
      g_ptr_array_add (old_names, reading->name);
      reading->name = g_strdup_printf ("%s %u", reading->name, n);
    }
}

ThBand
th_band (double value, double crit, double warm, double hot)
{
  if (!isnan (crit) && crit - 5 < hot)
    hot = crit - 5;
  if (value >= hot)
    return TH_BAND_HOT;
  if (value >= warm)
    return TH_BAND_WARM;
  return TH_BAND_NORMAL;
}

ThModel *
th_model_new (void)
{
  ThModel *model = g_new0 (ThModel, 1);

  model->temperatures = g_ptr_array_new_with_free_func ((GDestroyNotify) reading_free);
  model->fans = g_ptr_array_new_with_free_func ((GDestroyNotify) reading_free);
  model->panel_sensors = g_ptr_array_new ();
  model->moved = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
  model->fan_watches = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);
  model->pinned = g_new0 (char *, 1);
  model->warm = TH_WARM_DEFAULT;
  model->hot = TH_HOT_DEFAULT;
  return model;
}

void
th_model_free (ThModel *model)
{
  g_ptr_array_unref (model->panel_sensors);
  g_ptr_array_unref (model->temperatures);
  g_ptr_array_unref (model->fans);
  g_hash_table_unref (model->moved);
  g_hash_table_unref (model->fan_watches);
  g_free (model->cpu_id);
  g_free (model->gpu_id);
  g_free (model->package_id);
  g_strfreev (model->pinned);
  g_free (model);
}

static ThReading *
find (GPtrArray *readings, const char *id)
{
  if (id == NULL)
    return NULL;
  for (guint i = 0; i < readings->len; i++)
    {
      ThReading *reading = readings->pdata[i];

      if (strcmp (reading->id, id) == 0)
        return reading;
    }
  return NULL;
}

static ThReading *
hottest (GPtrArray *readings, gboolean gpu_only)
{
  ThReading *best = NULL;

  for (guint i = 0; i < readings->len; i++)
    {
      ThReading *reading = readings->pdata[i];

      if (gpu_only && reading->group != TH_GROUP_GPU)
        continue;
      if (best == NULL || reading->value > best->value)
        best = reading;
    }
  return best;
}

static void
choose_panel_sensors (ThModel *model)
{
  ThReading *cpu = find (model->temperatures, model->package_id);
  ThReading *gpu;

  /* The Package or Tctl whenever it's there; else the hottest reading, kept while it's
     there so that the panel doesn't jump between two that cross. */
  if (cpu == NULL)
    cpu = find (model->temperatures, model->cpu_id);
  if (cpu == NULL)
    cpu = hottest (model->temperatures, FALSE);
  gpu = find (model->temperatures, model->gpu_id);
  if (gpu == NULL || gpu->group != TH_GROUP_GPU)
    gpu = hottest (model->temperatures, TRUE);

  g_free (model->cpu_id);
  g_free (model->gpu_id);
  model->cpu_id = cpu != NULL ? g_strdup (cpu->id) : NULL;
  model->gpu_id = gpu != NULL ? g_strdup (gpu->id) : NULL;
  g_ptr_array_set_size (model->panel_sensors, 0);
  if (cpu != NULL)
    g_ptr_array_add (model->panel_sensors, cpu);
  if (gpu != NULL && gpu != cpu)
    g_ptr_array_add (model->panel_sensors, gpu);

  /* The pins in the defaults' place, unless a pin's chip is gone: the defaults then, until
     it comes back. */
  if (model->pinned[0] == NULL)
    return;
  for (guint i = 0; model->pinned[i] != NULL; i++)
    if (find (model->temperatures, model->pinned[i]) == NULL)
      return;
  g_ptr_array_set_size (model->panel_sensors, 0);
  for (guint i = 0; model->pinned[i] != NULL; i++)
    g_ptr_array_add (model->panel_sensors, find (model->temperatures, model->pinned[i]));
}

/* The key of a raw's chip and kind, temperatures or fans, to count their unlabelled ones. */
static char *
kind_key (const ThRaw *raw)
{
  return g_strdup_printf ("%s/%c", raw->chip_id, raw->is_fan ? 'f' : 't');
}

static void
watch_fan (ThModel *model, const ThReading *fan)
{
  FanWatch *watch = g_hash_table_lookup (model->fan_watches, fan->id);

  if (watch == NULL)
    {
      watch = g_new0 (FanWatch, 1);
      g_hash_table_insert (model->fan_watches, g_strdup (fan->id), watch);
    }
  watch->reads++;
  watch->fastest = MAX (watch->fastest, fan->value);
}

void
th_model_update (ThModel *model, GPtrArray *raws)
{
  g_autoptr (GHashTable) unlabelled = g_hash_table_new_full (g_str_hash, g_str_equal, g_free,
                                                             NULL);
  g_ptr_array_set_size (model->panel_sensors, 0);
  g_clear_pointer (&model->package_id, g_free);
  g_ptr_array_set_size (model->temperatures, 0);
  g_ptr_array_set_size (model->fans, 0);

  /* How many temperatures and fans each chip has with no label, to number them. */
  for (guint i = 0; i < raws->len; i++)
    {
      ThRaw *raw = raws->pdata[i];
      char *key;

      if (raw->label != NULL)
        continue;
      key = kind_key (raw);
      g_hash_table_insert (unlabelled, key,
                           GUINT_TO_POINTER (GPOINTER_TO_UINT (g_hash_table_lookup (unlabelled,
                                                                                    key))
                                             + 1));
    }

  for (guint i = 0; i < raws->len; i++)
    {
      ThRaw *raw = raws->pdata[i];
      g_autofree char *key = kind_key (raw);
      ThReading *reading;
      char *id;

      if (!raw->ok || th_chip_is_left_out (raw->chip))
        continue;
      id = g_strdup_printf ("%s/%s", raw->chip_id, raw->feature);
      /* A temperature that has been 0 every time since the first isn't one. Fans at 0 are
         stopped, and shown. */
      if (!raw->is_fan && raw->value != 0)
        g_hash_table_add (model->moved, g_strdup (id));
      if (!raw->is_fan && !g_hash_table_contains (model->moved, id))
        {
          g_free (id);
          continue;
        }
      reading = g_new0 (ThReading, 1);
      reading->id = id;
      reading->name = raw_name (raw, GPOINTER_TO_UINT (g_hash_table_lookup (unlabelled, key)));
      reading->group = th_chip_group (raw->chip);
      reading->is_fan = raw->is_fan;
      reading->value = raw->value;
      reading->limit = raw->limit;
      g_ptr_array_add (raw->is_fan ? model->fans : model->temperatures, reading);
      if (raw->is_fan)
        watch_fan (model, reading);
      if (!raw->is_fan && model->package_id == NULL && is_package (raw))
        model->package_id = g_strdup (id);
    }
  number_same_names (model->temperatures);
  number_same_names (model->fans);
  choose_panel_sensors (model);
}

GPtrArray *
th_model_temperatures (ThModel *model)
{
  return model->temperatures;
}

GPtrArray *
th_model_fans (ThModel *model)
{
  return model->fans;
}

GPtrArray *
th_model_panel_sensors (ThModel *model)
{
  return model->panel_sensors;
}

void
th_model_set_pinned (ThModel *model, const char *const *ids)
{
  g_strfreev (model->pinned);
  model->pinned = ids != NULL ? g_strdupv ((char **) ids) : g_new0 (char *, 1);
  choose_panel_sensors (model);
}

const char *const *
th_model_pinned (ThModel *model)
{
  return (const char *const *) model->pinned;
}

gboolean
th_model_toggle_pin (ThModel *model, const char *id)
{
  GPtrArray *pins;
  gboolean was_pinned = FALSE;

  if (find (model->temperatures, id) == NULL)
    return FALSE;
  /* From the panel sensors shown, the defaults or the pins whose chips are there. */
  pins = g_ptr_array_new_with_free_func (g_free);
  for (guint i = 0; i < model->panel_sensors->len; i++)
    {
      const char *shown = ((ThReading *) model->panel_sensors->pdata[i])->id;

      if (strcmp (shown, id) == 0)
        was_pinned = TRUE;
      else
        g_ptr_array_add (pins, g_strdup (shown));
    }
  if (was_pinned && pins->len == 0)
    {
      g_ptr_array_unref (pins);
      return FALSE;
    }
  if (!was_pinned)
    {
      g_ptr_array_add (pins, g_strdup (id));
      if (pins->len > 2)
        g_ptr_array_remove_index (pins, 0);
    }
  g_ptr_array_add (pins, NULL);
  g_strfreev (model->pinned);
  /* The array's own, not freed with it. */
  model->pinned = (char **) g_ptr_array_free (pins, FALSE);
  choose_panel_sensors (model);
  return TRUE;
}

void
th_model_set_fahrenheit (ThModel *model, gboolean fahrenheit)
{
  model->fahrenheit = fahrenheit;
}

void
th_model_set_thresholds (ThModel *model, double warm, double hot)
{
  model->warm = warm;
  model->hot = hot;
}

/* Rounded half up, as people read a thermometer, and not half to even as printf does. */
static long
whole (double value)
{
  return (long) floor (value + 0.5);
}

ThBand
th_model_band (ThModel *model, const ThReading *reading)
{
  return th_band (reading->value, reading->limit, model->warm, model->hot);
}

/* A temperature in the model's units, rounded. */
static long
in_units (ThModel *model, double celsius)
{
  return whole (model->fahrenheit ? celsius * 9 / 5 + 32 : celsius);
}

static const char *
unit (ThModel *model)
{
  return model->fahrenheit ? "°F" : "°C";
}

/* The band's colour in the panel; NULL for normal, which is the theme's text colour. */
static const char *
band_colour (ThBand band)
{
  switch (band)
    {
    case TH_BAND_HOT:
      return HOT_COLOUR;
    case TH_BAND_WARM:
      return WARM_COLOUR;
    case TH_BAND_NORMAL:
    default:
      return NULL;
    }
}

static void
append_coloured (GString *text, ThModel *model, const ThReading *reading, const char *unit)
{
  const char *colour = band_colour (th_model_band (model, reading));

  if (colour != NULL)
    g_string_append_printf (text, "<span foreground=\"%s\">%ld%s</span>", colour,
                            in_units (model, reading->value), unit);
  else
    g_string_append_printf (text, "%ld%s", in_units (model, reading->value), unit);
}

char *
th_model_panel_temperatures (ThModel *model)
{
  GString *text = g_string_new (NULL);

  for (guint i = 0; i < model->panel_sensors->len; i++)
    {
      if (i > 0)
        g_string_append (text, " / ");
      append_coloured (text, model, model->panel_sensors->pdata[i], unit (model));
    }
  if (text->len == 0)
    g_string_append (text, "–");
  return g_string_free (text, FALSE);
}

char *
th_model_panel_bare (ThModel *model)
{
  GString *text = g_string_new (NULL);

  if (model->panel_sensors->len == 0)
    g_string_append (text, "–");
  else
    append_coloured (text, model, model->panel_sensors->pdata[0], "°");
  return g_string_free (text, FALSE);
}

char *
th_model_panel_fans (ThModel *model)
{
  double fastest = 0;

  if (model->fans->len == 0)
    return NULL;
  for (guint i = 0; i < model->fans->len; i++)
    fastest = MAX (fastest, ((ThReading *) model->fans->pdata[i])->value);
  if (whole (fastest) == 0)
    return g_strdup ("Fans Off");
  return g_strdup_printf ("%ld RPM", whole (fastest));
}

char *
th_model_tooltip (ThModel *model)
{
  g_autoptr (GPtrArray) parts = g_ptr_array_new_with_free_func (g_free);

  for (guint i = 0; i < model->panel_sensors->len; i++)
    {
      ThReading *reading = model->panel_sensors->pdata[i];

      g_ptr_array_add (parts, g_strdup_printf ("%s %ld %s", reading->name,
                                               in_units (model, reading->value),
                                               unit (model)));
    }
  for (guint i = 0; i < model->fans->len; i++)
    {
      ThReading *reading = model->fans->pdata[i];

      g_autofree char *speed = th_fan_text (reading->value);

      g_ptr_array_add (parts, g_strdup_printf ("%s %s", reading->name, speed));
    }
  if (parts->len == 0)
    return g_strdup ("No temperatures");
  g_ptr_array_add (parts, NULL);
  return g_strjoinv (" · ", (char **) parts->pdata);
}

gboolean
th_model_is_panel_sensor (ThModel *model, const ThReading *reading)
{
  for (guint i = 0; i < model->panel_sensors->len; i++)
    if (strcmp (((ThReading *) model->panel_sensors->pdata[i])->id, reading->id) == 0)
      return TRUE;
  return FALSE;
}

ThReading *
th_model_hottest (ThModel *model, ThGroup group, guint *count)
{
  ThReading *best = NULL;

  *count = 0;
  for (guint i = 0; i < model->temperatures->len; i++)
    {
      ThReading *reading = model->temperatures->pdata[i];

      if (reading->group != group)
        continue;
      (*count)++;
      if (best == NULL || reading->value > best->value)
        best = reading;
    }
  return best;
}

ThReading *
th_model_fastest_fan (ThModel *model)
{
  ThReading *best = NULL;

  for (guint i = 0; i < model->fans->len; i++)
    {
      ThReading *fan = model->fans->pdata[i];

      if (best == NULL || fan->value > best->value)
        best = fan;
    }
  return best;
}

double
th_model_fan_scale (ThModel *model, const ThReading *fan)
{
  FanWatch *watch;

  if (!isnan (fan->limit) && fan->limit > 0)
    return fan->limit;
  watch = g_hash_table_lookup (model->fan_watches, fan->id);
  /* A fan that has never spun has nothing to be against. */
  if (watch == NULL || watch->reads <= FAN_WATCHED || watch->fastest <= 0)
    return NAN;
  return watch->fastest;
}

double
th_temperature_fraction (double celsius)
{
  return CLAMP ((celsius - BAR_EMPTY) / (BAR_FULL - BAR_EMPTY), 0.0, 1.0);
}

char *
th_model_temperature_text (ThModel *model, double celsius)
{
  return g_strdup_printf ("%ld %s", in_units (model, celsius), unit (model));
}

char *
th_fan_text (double rpm)
{
  if (whole (rpm) == 0)
    return g_strdup ("Stopped");
  return g_strdup_printf ("%ld RPM", whole (rpm));
}
