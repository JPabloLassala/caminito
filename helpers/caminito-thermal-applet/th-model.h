/*
 * What the Thermal applet shows (267), from what its source read, apart from libsensors so
 * that the tests can build any computer: each reading's group and name, which readings are
 * left out, the bands, the panel sensors, the panel's and the tooltip's text, and the
 * popup's tiles and bars (268).
 */

#pragma once

#include <glib.h>

typedef enum
{
  TH_GROUP_CPU,
  TH_GROUP_GPU,
  TH_GROUP_STORAGE,
  TH_GROUP_WIRELESS,
  TH_GROUP_BOARD,
  TH_GROUP_OTHER,
} ThGroup;

typedef enum
{
  TH_BAND_NORMAL,
  TH_BAND_WARM,
  TH_BAND_HOT,
} ThBand;

/* The bands' thresholds in °C until the preferences say otherwise (269). */
#define TH_WARM_DEFAULT 75.0
#define TH_HOT_DEFAULT  90.0

/* One temperature or fan as the source read it this time. */
typedef struct
{
  /* The chip's name, as "coretemp" or "nvme", and one that's its own among the chips, as
     libsensors' "coretemp-isa-0000". */
  char    *chip;
  char    *chip_id;
  /* The feature, as "temp1" or "fan2". */
  char    *feature;
  /* The chip's label for it, NULL with none. */
  char    *label;
  /* The device's own name where it has one, an NVMe drive's model; NULL otherwise. */
  char    *device;
  gboolean is_fan;
  /* FALSE when it couldn't be read. */
  gboolean ok;
  /* °C, or RPM for a fan. */
  double   value;
  /* A temperature's crit, or a fan's max; NAN with none. */
  double   limit;
} ThRaw;

ThRaw *th_raw_new  (const char *chip,
                    const char *chip_id,
                    const char *feature,
                    const char *label,
                    const char *device,
                    gboolean    is_fan,
                    gboolean    ok,
                    double      value,
                    double      limit);
void   th_raw_free (ThRaw *raw);

/* A reading the applet shows. */
typedef struct
{
  /* chip_id/feature: the same across reads, and across rescans of the chips. */
  char    *id;
  char    *name;
  ThGroup  group;
  gboolean is_fan;
  double   value;
  double   limit;
} ThReading;

typedef struct _ThModel ThModel;

ThModel *th_model_new    (void);
void     th_model_free   (ThModel *model);
/* What the source read this time, a GPtrArray of ThRaw. A reading missing from it, as a
   chip's whose module was unloaded, is no longer shown. */
void     th_model_update (ThModel *model, GPtrArray *raws);

/* The temperatures shown: read this time, and not 0 every time since the first. In the
   order the source gave them. The model keeps them. */
GPtrArray *th_model_temperatures (ThModel *model);
/* The fans read this time, stopped ones too. */
GPtrArray *th_model_fans         (ThModel *model);
/* The panel sensors, one or two, or none while there's no temperature: the pins' readings,
   or the defaults while there's no pin or a pin's chip is gone, the CPU's reading and the
   hottest GPU reading when there's a GPU. */
GPtrArray *th_model_panel_sensors (ThModel *model);

/* The panel sensors' pins (269), by reading id, oldest first, as the applet's preferences
   keep them; NULL or empty for the defaults. Kept while their chips are gone, until a pin is
   clicked, which starts from the panel sensors shown. */
void               th_model_set_pinned (ThModel *model, const char *const *ids);
/* NULL-terminated; empty for the defaults. */
const char *const *th_model_pinned     (ThModel *model);
/* A click on a temperature's pin: an unpinned one joins the panel sensors, dropping the
   older when there are two; a pinned one leaves them, unless it's the only one. FALSE when
   nothing changed. */
gboolean           th_model_toggle_pin (ThModel *model, const char *id);
/* °F in the panel, the tooltip and the popup; °C by default. */
void               th_model_set_fahrenheit (ThModel *model, gboolean fahrenheit);
/* Where warm and hot start, in °C, for every reading at once. */
void               th_model_set_thresholds (ThModel *model, double warm, double hot);

/* Whether a chip of that name is left out: batteries, AC and USB-C. */
gboolean th_chip_is_left_out (const char *chip);
ThGroup  th_chip_group       (const char *chip);
ThBand   th_band             (double value, double crit, double warm, double hot);

/* In °C or °F. "55°C", "55°C / 81°C" with each in its band's colour as Pango markup, "–"
   with none. */
char *th_model_panel_temperatures (ThModel *model);
/* The fastest fan's "2865 RPM", "Fans Off" when every fan reads 0, NULL with no fan. */
char *th_model_panel_fans         (ThModel *model);
/* The first panel sensor as a bare number in its band's colour, "55°", for a vertical
   panel; "–" with none. */
char *th_model_panel_bare         (ThModel *model);
/* "CPU Package 55 °C · Fan 2865 RPM": every panel sensor and every fan. */
char *th_model_tooltip            (ThModel *model);

/* The popup's (268). */

/* The reading's band at the model's thresholds. */
ThBand     th_model_band            (ThModel *model, const ThReading *reading);
gboolean   th_model_is_panel_sensor (ThModel *model, const ThReading *reading);
/* The group's hottest temperature, NULL with none; count gets how many the group has. */
ThReading *th_model_hottest         (ThModel *model, ThGroup group, guint *count);
/* The fastest fan, stopped or not; NULL with no fan. */
ThReading *th_model_fastest_fan     (ThModel *model);
/* What a fan's bar is against: its max, or with none the fastest it has spun once it has
   been read more than 10 times; NAN for no bar yet. */
double     th_model_fan_scale       (ThModel *model, const ThReading *fan);
/* A temperature's bar, on the one scale of every reading: empty at 20 °C, full at 100 °C. */
double     th_temperature_fraction  (double celsius);
/* "55 °C", or "131 °F". */
char      *th_model_temperature_text (ThModel *model, double celsius);
/* "2865 RPM", or "Stopped". */
char      *th_fan_text              (double rpm);
