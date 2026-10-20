/*
 * The Thermal applet's source (267). See th-source.h.
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <sensors/error.h>
#include <sensors/sensors.h>

#include "th-model.h"
#include "th-source.h"

#define HWMON "/sys/class/hwmon"

static guint users;
static gboolean initialised;
/* The entries of /sys/class/hwmon when libsensors last found the chips. */
static char *scanned;

static char *
list_hwmon (void)
{
  g_autoptr (GDir) dir = g_dir_open (HWMON, 0, NULL);
  g_autoptr (GPtrArray) names = g_ptr_array_new_with_free_func (g_free);
  const char *name;

  if (dir == NULL)
    return g_strdup ("");
  while ((name = g_dir_read_name (dir)) != NULL)
    g_ptr_array_add (names, g_strdup (name));
  g_ptr_array_sort_values (names, (GCompareFunc) g_strcmp0);
  g_ptr_array_add (names, NULL);
  return g_strjoinv (" ", (char **) names->pdata);
}

static void
init (void)
{
  FILE *empty;
  int error;

  error = sensors_init (NULL);
  if (error == 0)
    {
      initialised = TRUE;
      return;
    }
  /* A broken sensors3.conf mustn't leave the applet with nothing: the chips without it. */
  g_warning ("can't read libsensors' configuration, so its labels won't apply: %s",
             sensors_strerror (error));
  empty = fopen ("/dev/null", "r");
  error = empty != NULL ? sensors_init (empty) : -1;
  if (empty != NULL)
    fclose (empty);
  if (error == 0)
    initialised = TRUE;
  else
    g_warning ("can't start libsensors: %s",
               error > 0 ? sensors_strerror (error) : "can't open /dev/null");
}

static void
rescan_if_changed (void)
{
  g_autofree char *now = list_hwmon ();

  if (initialised && g_strcmp0 (now, scanned) == 0)
    return;
  if (initialised)
    sensors_cleanup ();
  initialised = FALSE;
  g_free (scanned);
  scanned = g_steal_pointer (&now);
  init ();
}

void
th_source_acquire (void)
{
  users++;
}

void
th_source_release (void)
{
  g_return_if_fail (users > 0);
  if (--users > 0)
    return;
  if (initialised)
    sensors_cleanup ();
  initialised = FALSE;
  g_clear_pointer (&scanned, g_free);
}

static char *
read_model (const char *path)
{
  g_autofree char *model = NULL;

  if (!g_file_get_contents (path, &model, NULL, NULL))
    return NULL;
  g_strstrip (model);
  if (*model == '\0')
    return NULL;
  return g_steal_pointer (&model);
}

/* A drive's model, from its device: a SATA drive's (drivetemp) is the SCSI device's own, and
   an NVMe drive's is in its controller's directory, as nvme/nvme0/, under the PCI device. */
static char *
device_model (const sensors_chip_name *chip)
{
  g_autofree char *device = NULL;
  g_autofree char *own = NULL;
  g_autofree char *controllers = NULL;
  g_autoptr (GDir) dir = NULL;
  const char *name;
  char *model;

  if (chip->path == NULL)
    return NULL;
  device = g_build_filename (chip->path, "device", NULL);
  own = g_build_filename (device, "model", NULL);
  model = read_model (own);
  if (model != NULL)
    return model;
  controllers = g_build_filename (device, "nvme", NULL);
  dir = g_dir_open (controllers, 0, NULL);
  while (dir != NULL && (name = g_dir_read_name (dir)) != NULL)
    {
      g_autofree char *path = g_build_filename (controllers, name, "model", NULL);

      model = read_model (path);
      if (model != NULL)
        return model;
    }
  return NULL;
}

static double
value_of (const sensors_chip_name *chip, const sensors_feature *feature,
          sensors_subfeature_type type, gboolean *ok)
{
  const sensors_subfeature *sub = sensors_get_subfeature (chip, feature, type);
  double value;

  if (sub == NULL || !(sub->flags & SENSORS_MODE_R)
      || sensors_get_value (chip, sub->number, &value) != 0)
    {
      if (ok != NULL)
        *ok = FALSE;
      return NAN;
    }
  if (ok != NULL)
    *ok = TRUE;
  return value;
}

GPtrArray *
th_source_read (void)
{
  GPtrArray *raws = g_ptr_array_new_with_free_func ((GDestroyNotify) th_raw_free);
  const sensors_chip_name *chip;
  int chip_nr = 0;

  rescan_if_changed ();
  if (!initialised)
    return raws;
  while ((chip = sensors_get_detected_chips (NULL, &chip_nr)) != NULL)
    {
      const sensors_feature *feature;
      int feature_nr = 0;
      char chip_id[256];
      g_autofree char *model = NULL;

      if (chip->prefix == NULL || th_chip_is_left_out (chip->prefix))
        continue;
      if (sensors_snprintf_chip_name (chip_id, sizeof chip_id, chip) < 0)
        continue;
      if (th_chip_group (chip->prefix) == TH_GROUP_STORAGE)
        model = device_model (chip);
      while ((feature = sensors_get_features (chip, &feature_nr)) != NULL)
        {
          gboolean is_fan = feature->type == SENSORS_FEATURE_FAN;
          g_autofree char *label = NULL;
          gboolean ok;
          double value;
          double limit;

          if (feature->type != SENSORS_FEATURE_TEMP && !is_fan)
            continue;
          value = value_of (chip, feature,
                            is_fan ? SENSORS_SUBFEATURE_FAN_INPUT
                                   : SENSORS_SUBFEATURE_TEMP_INPUT,
                            &ok);
          limit = value_of (chip, feature,
                            is_fan ? SENSORS_SUBFEATURE_FAN_MAX : SENSORS_SUBFEATURE_TEMP_CRIT,
                            NULL);
          /* With no label of the chip's or the configuration's, libsensors gives the
             feature's name. */
          label = sensors_get_label (chip, feature);
          if (label != NULL && strcmp (label, feature->name) == 0)
            g_clear_pointer (&label, free);
          g_ptr_array_add (raws, th_raw_new (chip->prefix, chip_id, feature->name, label,
                                             model, is_fan, ok, ok ? value : 0, limit));
        }
    }
  return raws;
}
