/*
 * The readings' groups, names and bands, what's left out, the panel sensors, and the panel's
 * and the tooltip's text (267), from computers built by hand.
 */

#include <math.h>

#include "th-model.h"

static void
temp (GPtrArray *raws, const char *chip, const char *chip_id, const char *feature,
      const char *label, double value, double crit)
{
  g_ptr_array_add (raws, th_raw_new (chip, chip_id, feature, label, NULL, FALSE, TRUE, value,
                                     crit));
}

static void
fan (GPtrArray *raws, const char *chip, const char *chip_id, const char *feature,
     const char *label, double rpm, double max)
{
  g_ptr_array_add (raws, th_raw_new (chip, chip_id, feature, label, NULL, TRUE, TRUE, rpm,
                                     max));
}

static GPtrArray *
raws_new (void)
{
  return g_ptr_array_new_with_free_func ((GDestroyNotify) th_raw_free);
}

/* The developer's ThinkPad: coretemp, the embedded controller with one fan with no max and
   readings that are always 0, ACPI, Wi-Fi, an NVMe drive, and the battery chips. */
static GPtrArray *
thinkpad (double package, double fan_rpm)
{
  GPtrArray *raws = raws_new ();

  temp (raws, "coretemp", "coretemp-isa-0000", "temp1", "Package id 0", package, 100);
  temp (raws, "coretemp", "coretemp-isa-0000", "temp2", "Core 0", package - 2, 100);
  temp (raws, "coretemp", "coretemp-isa-0000", "temp3", "Core 1", package - 3, 100);
  temp (raws, "thinkpad", "thinkpad-isa-0000", "temp1", "CPU", 52, NAN);
  /* N/A in sensors: it can't be read. */
  g_ptr_array_add (raws, th_raw_new ("thinkpad", "thinkpad-isa-0000", "temp2", "GPU", NULL,
                                     FALSE, FALSE, 0, NAN));
  for (int i = 3; i <= 8; i++)
    {
      g_autofree char *feature = g_strdup_printf ("temp%d", i);

      temp (raws, "thinkpad", "thinkpad-isa-0000", feature, NULL, 0, NAN);
    }
  fan (raws, "thinkpad", "thinkpad-isa-0000", "fan1", NULL, fan_rpm, NAN);
  temp (raws, "acpitz", "acpitz-acpi-0", "temp1", NULL, 50, 128);
  temp (raws, "iwlwifi_1", "iwlwifi_1-virtual-0", "temp1", NULL, 41, NAN);
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0400", "temp1", "Composite",
                                     "SAMSUNG MZVL2512HCJQ", FALSE, TRUE, 38, 84.85));
  temp (raws, "BAT0", "BAT0-acpi-0", "temp1", NULL, 30, NAN);
  temp (raws, "ucsi_source_psy_USBC000:001", "ucsi_source_psy_USBC000:001-isa-0000", "temp1",
        NULL, 30, NAN);
  return raws;
}

/* A desktop: an AMD CPU, a Radeon with a stopped fan, two NVMe drives, three fans. */
static GPtrArray *
desktop (double tctl, double junction, double cpu_fan, double chassis_fan, double gpu_fan)
{
  GPtrArray *raws = raws_new ();

  temp (raws, "k10temp", "k10temp-pci-00c3", "temp1", "Tctl", tctl, NAN);
  temp (raws, "k10temp", "k10temp-pci-00c3", "temp3", "Tccd1", tctl - 3, NAN);
  temp (raws, "amdgpu", "amdgpu-pci-0300", "temp1", "edge", junction - 12, 100);
  temp (raws, "amdgpu", "amdgpu-pci-0300", "temp2", "junction", junction, 110);
  temp (raws, "amdgpu", "amdgpu-pci-0300", "temp3", "mem", junction - 4, 105);
  fan (raws, "amdgpu", "amdgpu-pci-0300", "fan1", NULL, gpu_fan, 3300);
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0100", "temp1", "Composite",
                                     "Samsung SSD 990 PRO 2TB", FALSE, TRUE, 41, 84.85));
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0100", "temp2", "Sensor 1",
                                     "Samsung SSD 990 PRO 2TB", FALSE, TRUE, 45, 84.85));
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0200", "temp1", "Composite",
                                     "WD_BLACK SN850X 1000GB", FALSE, TRUE, 46, 84.85));
  temp (raws, "nct6798", "nct6798-isa-0290", "temp1", "SYSTIN", 34, NAN);
  temp (raws, "nct6798", "nct6798-isa-0290", "temp7", "AUXTIN3", 0, NAN);
  fan (raws, "nct6798", "nct6798-isa-0290", "fan1", NULL, chassis_fan, NAN);
  fan (raws, "nct6798", "nct6798-isa-0290", "fan2", NULL, cpu_fan, NAN);
  return raws;
}

static const char *
name_of (GPtrArray *readings, const char *id)
{
  for (guint i = 0; i < readings->len; i++)
    {
      ThReading *reading = readings->pdata[i];

      if (g_strcmp0 (reading->id, id) == 0)
        return reading->name;
    }
  return NULL;
}

static void
test_groups (void)
{
  struct { const char *chip; ThGroup group; } cases[] = {
    { "coretemp", TH_GROUP_CPU },
    { "k10temp", TH_GROUP_CPU },
    { "zenpower", TH_GROUP_CPU },
    { "cpu_thermal", TH_GROUP_CPU },
    { "amdgpu", TH_GROUP_GPU },
    { "nouveau", TH_GROUP_GPU },
    { "radeon", TH_GROUP_GPU },
    { "i915", TH_GROUP_GPU },
    { "xe", TH_GROUP_GPU },
    { "nvme", TH_GROUP_STORAGE },
    { "drivetemp", TH_GROUP_STORAGE },
    { "iwlwifi_1", TH_GROUP_WIRELESS },
    { "ath11k_hwmon", TH_GROUP_WIRELESS },
    { "mt7921_phy0", TH_GROUP_WIRELESS },
    { "acpitz", TH_GROUP_BOARD },
    { "pch_cannonlake", TH_GROUP_BOARD },
    { "thinkpad", TH_GROUP_BOARD },
    { "dell_smm", TH_GROUP_BOARD },
    { "nct6798", TH_GROUP_BOARD },
    { "it8688", TH_GROUP_BOARD },
    { "spd5118", TH_GROUP_OTHER },
    { "xeon", TH_GROUP_OTHER },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    g_assert_cmpint (th_chip_group (cases[i].chip), ==, cases[i].group);
}

static void
test_left_out_chips (void)
{
  g_assert_true (th_chip_is_left_out ("BAT0"));
  g_assert_true (th_chip_is_left_out ("BAT1"));
  g_assert_true (th_chip_is_left_out ("AC"));
  g_assert_true (th_chip_is_left_out ("ucsi_source_psy_USBC000:001"));
  g_assert_false (th_chip_is_left_out ("acpitz"));
  g_assert_false (th_chip_is_left_out ("ACAD"));
}

static void
test_bands (void)
{
  g_assert_cmpint (th_band (74.9, NAN, 75, 90), ==, TH_BAND_NORMAL);
  g_assert_cmpint (th_band (75, NAN, 75, 90), ==, TH_BAND_WARM);
  g_assert_cmpint (th_band (89.9, NAN, 75, 90), ==, TH_BAND_WARM);
  g_assert_cmpint (th_band (90, NAN, 75, 90), ==, TH_BAND_HOT);
  /* coretemp's crit of 100 is above 90 + 5: 90 stays. */
  g_assert_cmpint (th_band (91, 100, 75, 90), ==, TH_BAND_HOT);
  g_assert_cmpint (th_band (89, 100, 75, 90), ==, TH_BAND_WARM);
  /* An NVMe drive's crit of 84.85: hot from 79.85. */
  g_assert_cmpint (th_band (79, 84.85, 75, 90), ==, TH_BAND_WARM);
  g_assert_cmpint (th_band (80, 84.85, 75, 90), ==, TH_BAND_HOT);
}

static void
test_thinkpad (void)
{
  g_autoptr (GPtrArray) raws = thinkpad (55, 2865);
  ThModel *model = th_model_new ();
  GPtrArray *temps;
  GPtrArray *sensors;
  g_autofree char *panel = NULL;
  g_autofree char *fans = NULL;
  g_autofree char *tooltip = NULL;

  th_model_update (model, raws);
  temps = th_model_temperatures (model);
  /* The GPU's that can't be read, six readings at 0 and the two battery chips are left
     out: 3 + 1 + 1 + 1 + 1. */
  g_assert_cmpuint (temps->len, ==, 7);
  g_assert_cmpstr (name_of (temps, "coretemp-isa-0000/temp1"), ==, "CPU Package");
  g_assert_cmpstr (name_of (temps, "coretemp-isa-0000/temp2"), ==, "Core 0");
  g_assert_cmpstr (name_of (temps, "thinkpad-isa-0000/temp1"), ==, "EC: CPU");
  g_assert_null (name_of (temps, "thinkpad-isa-0000/temp2"));
  g_assert_null (name_of (temps, "thinkpad-isa-0000/temp3"));
  g_assert_cmpstr (name_of (temps, "acpitz-acpi-0/temp1"), ==, "ACPI zone");
  g_assert_cmpstr (name_of (temps, "iwlwifi_1-virtual-0/temp1"), ==, "Wi-Fi");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0400/temp1"), ==, "SAMSUNG MZVL2512HCJQ");

  sensors = th_model_panel_sensors (model);
  g_assert_cmpuint (sensors->len, ==, 1);
  g_assert_cmpstr (((ThReading *) sensors->pdata[0])->id, ==, "coretemp-isa-0000/temp1");

  panel = th_model_panel_temperatures (model);
  fans = th_model_panel_fans (model);
  tooltip = th_model_tooltip (model);
  g_assert_cmpstr (panel, ==, "55°C");
  g_assert_cmpstr (fans, ==, "2865 RPM");
  g_assert_cmpstr (tooltip, ==, "CPU Package 55 °C · Fan 2865 RPM");
  th_model_free (model);
}

static void
test_embedded_controller (void)
{
  g_autoptr (GPtrArray) raws = raws_new ();
  ThModel *model = th_model_new ();
  GPtrArray *temps;

  temp (raws, "dell_smm", "dell_smm-isa-0000", "temp1", "CPU", 60, NAN);
  temp (raws, "dell_smm", "dell_smm-isa-0000", "temp2", NULL, 40, NAN);
  temp (raws, "dell_smm", "dell_smm-isa-0000", "temp4", NULL, 35, NAN);
  temp (raws, "thinkpad", "thinkpad-isa-0000", "temp5", NULL, 33, NAN);
  th_model_update (model, raws);
  temps = th_model_temperatures (model);
  g_assert_cmpstr (name_of (temps, "dell_smm-isa-0000/temp1"), ==, "EC: CPU");
  /* Unlabelled, its friendly name, numbered as the feature is when the chip has several. */
  g_assert_cmpstr (name_of (temps, "dell_smm-isa-0000/temp2"), ==, "Embedded controller 2");
  g_assert_cmpstr (name_of (temps, "dell_smm-isa-0000/temp4"), ==, "Embedded controller 4");
  g_assert_cmpstr (name_of (temps, "thinkpad-isa-0000/temp5"), ==, "Embedded controller");
  th_model_free (model);
}

static void
test_desktop (void)
{
  g_autoptr (GPtrArray) raws = desktop (58, 61, 1180, 820, 0);
  ThModel *model = th_model_new ();
  GPtrArray *temps;
  GPtrArray *sensors;
  g_autofree char *panel = NULL;
  g_autofree char *fans = NULL;
  g_autofree char *tooltip = NULL;

  th_model_update (model, raws);
  temps = th_model_temperatures (model);
  g_assert_cmpuint (temps->len, ==, 9);
  g_assert_cmpstr (name_of (temps, "k10temp-pci-00c3/temp1"), ==, "CPU");
  g_assert_cmpstr (name_of (temps, "k10temp-pci-00c3/temp3"), ==, "CCD 1");
  g_assert_cmpstr (name_of (temps, "amdgpu-pci-0300/temp1"), ==, "GPU edge");
  g_assert_cmpstr (name_of (temps, "amdgpu-pci-0300/temp2"), ==, "GPU junction");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0100/temp1"), ==, "Samsung SSD 990 PRO 2TB");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0100/temp2"), ==, "Samsung SSD 990 PRO 2TB Sensor 1");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0200/temp1"), ==, "WD_BLACK SN850X 1000GB");
  g_assert_cmpstr (name_of (temps, "nct6798-isa-0290/temp1"), ==, "SYSTIN");
  g_assert_null (name_of (temps, "nct6798-isa-0290/temp7"));
  g_assert_cmpstr (name_of (th_model_fans (model), "amdgpu-pci-0300/fan1"), ==, "GPU fan");
  g_assert_cmpstr (name_of (th_model_fans (model), "nct6798-isa-0290/fan2"), ==, "Fan 2");

  sensors = th_model_panel_sensors (model);
  g_assert_cmpuint (sensors->len, ==, 2);
  g_assert_cmpstr (((ThReading *) sensors->pdata[0])->id, ==, "k10temp-pci-00c3/temp1");
  g_assert_cmpstr (((ThReading *) sensors->pdata[1])->id, ==, "amdgpu-pci-0300/temp2");

  panel = th_model_panel_temperatures (model);
  fans = th_model_panel_fans (model);
  tooltip = th_model_tooltip (model);
  g_assert_cmpstr (panel, ==, "58°C / 61°C");
  g_assert_cmpstr (fans, ==, "1180 RPM");
  g_assert_cmpstr (tooltip, ==,
                   "CPU 58 °C · GPU junction 61 °C · GPU fan Stopped · Fan 1 820 RPM · Fan 2 1180 RPM");
  th_model_free (model);
}

static void
test_colours (void)
{
  g_autoptr (GPtrArray) raws = desktop (78, 93, 1900, 1100, 2200);
  ThModel *model = th_model_new ();
  g_autofree char *panel = NULL;
  g_autofree char *bare = NULL;

  th_model_update (model, raws);
  panel = th_model_panel_temperatures (model);
  bare = th_model_panel_bare (model);
  g_assert_cmpstr (panel, ==,
                   "<span foreground=\"#ffa630\">78°C</span> / <span foreground=\"#ff5c5c\">93°C</span>");
  g_assert_cmpstr (bare, ==, "<span foreground=\"#ffa630\">78°</span>");
  th_model_free (model);
}

static void
test_fans_off (void)
{
  g_autoptr (GPtrArray) raws = desktop (40, 45, 0, 0, 0);
  ThModel *model = th_model_new ();
  g_autofree char *fans = NULL;

  th_model_update (model, raws);
  fans = th_model_panel_fans (model);
  g_assert_cmpstr (fans, ==, "Fans Off");
  th_model_free (model);
}

static void
test_nothing_read (void)
{
  g_autoptr (GPtrArray) raws = raws_new ();
  ThModel *model = th_model_new ();
  g_autofree char *panel = NULL;
  g_autofree char *fans = NULL;
  g_autofree char *tooltip = NULL;
  g_autofree char *bare = NULL;

  th_model_update (model, raws);
  panel = th_model_panel_temperatures (model);
  bare = th_model_panel_bare (model);
  fans = th_model_panel_fans (model);
  tooltip = th_model_tooltip (model);
  g_assert_cmpstr (panel, ==, "–");
  g_assert_cmpstr (bare, ==, "–");
  g_assert_null (fans);
  g_assert_cmpstr (tooltip, ==, "No temperatures");
  th_model_free (model);
}

/* With neither coretemp's Package nor k10temp's Tctl, the CPU's reading is the hottest. */
static void
test_cpu_is_the_hottest_without_a_package (void)
{
  g_autoptr (GPtrArray) raws = raws_new ();
  ThModel *model = th_model_new ();
  GPtrArray *sensors;

  temp (raws, "cpu_thermal", "cpu_thermal-virtual-0", "temp1", NULL, 48, NAN);
  temp (raws, "acpitz", "acpitz-acpi-0", "temp1", NULL, 52, NAN);
  temp (raws, "acpitz", "acpitz-acpi-0", "temp2", NULL, 30, NAN);
  th_model_update (model, raws);
  sensors = th_model_panel_sensors (model);
  g_assert_cmpuint (sensors->len, ==, 1);
  g_assert_cmpstr (((ThReading *) sensors->pdata[0])->id, ==, "acpitz-acpi-0/temp1");
  g_assert_cmpstr (name_of (th_model_temperatures (model), "acpitz-acpi-0/temp1"), ==,
                   "ACPI zone 1");
  g_assert_cmpstr (name_of (th_model_temperatures (model), "cpu_thermal-virtual-0/temp1"), ==,
                   "CPU");
  th_model_free (model);
}

/* A reading that was 0 at first and then wasn't is shown from then on, even at 0 again. */
static void
test_dead_until_it_moves (void)
{
  ThModel *model = th_model_new ();
  g_autoptr (GPtrArray) first = thinkpad (55, 2865);
  g_autoptr (GPtrArray) second = thinkpad (56, 2900);
  g_autoptr (GPtrArray) third = thinkpad (56, 2900);

  th_model_update (model, first);
  g_assert_null (name_of (th_model_temperatures (model), "thinkpad-isa-0000/temp3"));
  /* thinkpad's temp3. */
  ((ThRaw *) second->pdata[5])->value = 33;
  th_model_update (model, second);
  g_assert_cmpstr (name_of (th_model_temperatures (model), "thinkpad-isa-0000/temp3"), ==,
                   "Embedded controller 3");
  th_model_update (model, third);
  g_assert_nonnull (name_of (th_model_temperatures (model), "thinkpad-isa-0000/temp3"));
  th_model_free (model);
}

/* A chip that goes, as a module unloaded, takes its readings with it; the panel sensor moves
   to what's left, and back when it returns. */
static void
test_chip_disappears (void)
{
  ThModel *model = th_model_new ();
  g_autoptr (GPtrArray) all = desktop (58, 61, 1180, 820, 0);
  g_autoptr (GPtrArray) without_gpu = raws_new ();
  g_autoptr (GPtrArray) unreadable = desktop (58, 61, 1180, 820, 0);
  g_autoptr (GPtrArray) back = desktop (58, 61, 1180, 820, 0);
  GPtrArray *sensors;
  g_autofree char *panel = NULL;

  th_model_update (model, all);
  for (guint i = 0; i < all->len; i++)
    {
      ThRaw *raw = all->pdata[i];

      if (g_strcmp0 (raw->chip, "amdgpu") != 0)
        g_ptr_array_add (without_gpu, th_raw_new (raw->chip, raw->chip_id, raw->feature,
                                                  raw->label, raw->device, raw->is_fan, raw->ok,
                                                  raw->value, raw->limit));
    }
  th_model_update (model, without_gpu);
  sensors = th_model_panel_sensors (model);
  g_assert_cmpuint (sensors->len, ==, 1);
  g_assert_null (name_of (th_model_temperatures (model), "amdgpu-pci-0300/temp2"));
  g_assert_null (name_of (th_model_fans (model), "amdgpu-pci-0300/fan1"));
  panel = th_model_panel_temperatures (model);
  g_assert_cmpstr (panel, ==, "58°C");

  /* A read that fails, as the chip goes between two scans, is the same. */
  for (guint i = 0; i < unreadable->len; i++)
    if (g_strcmp0 (((ThRaw *) unreadable->pdata[i])->chip, "k10temp") == 0)
      ((ThRaw *) unreadable->pdata[i])->ok = FALSE;
  th_model_update (model, unreadable);
  sensors = th_model_panel_sensors (model);
  /* The hottest reading in the CPU's place, which is the hottest GPU reading too: it's
     shown once. */
  g_assert_cmpuint (sensors->len, ==, 1);
  g_assert_cmpstr (((ThReading *) sensors->pdata[0])->id, ==, "amdgpu-pci-0300/temp2");

  th_model_update (model, back);
  sensors = th_model_panel_sensors (model);
  g_assert_cmpuint (sensors->len, ==, 2);
  g_assert_cmpstr (((ThReading *) sensors->pdata[0])->id, ==, "k10temp-pci-00c3/temp1");
  th_model_free (model);
}

/* The hottest GPU reading is chosen once, and kept while it's there, so that the panel
   doesn't jump between two GPU readings that cross. */
static void
test_gpu_sensor_is_kept (void)
{
  ThModel *model = th_model_new ();
  g_autoptr (GPtrArray) first = desktop (58, 61, 1180, 820, 0);
  g_autoptr (GPtrArray) crossed = desktop (58, 61, 1180, 820, 0);
  GPtrArray *sensors;

  th_model_update (model, first);
  /* The memory's above the junction now. */
  ((ThRaw *) crossed->pdata[4])->value = 70;
  th_model_update (model, crossed);
  sensors = th_model_panel_sensors (model);
  g_assert_cmpstr (((ThReading *) sensors->pdata[1])->id, ==, "amdgpu-pci-0300/temp2");
  th_model_free (model);
}

static void
test_one_line_rounds (void)
{
  g_autoptr (GPtrArray) raws = thinkpad (54.5, 2865);
  ThModel *model = th_model_new ();
  g_autofree char *panel = NULL;

  th_model_update (model, raws);
  panel = th_model_panel_temperatures (model);
  /* Half up, not to even. */
  g_assert_cmpstr (panel, ==, "55°C");
  th_model_free (model);
}

/* Two chips with the same friendly name and no labels are numbered by chip. */
static void
test_same_names_are_numbered (void)
{
  g_autoptr (GPtrArray) raws = raws_new ();
  ThModel *model = th_model_new ();
  GPtrArray *temps;

  temp (raws, "coretemp", "coretemp-isa-0000", "temp1", "Package id 0", 50, 100);
  temp (raws, "coretemp", "coretemp-isa-0001", "temp1", "Package id 1", 52, 100);
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0100", "temp1", "Composite", NULL,
                                     FALSE, TRUE, 40, NAN));
  g_ptr_array_add (raws, th_raw_new ("nvme", "nvme-pci-0200", "temp1", "Composite", NULL,
                                     FALSE, TRUE, 41, NAN));
  th_model_update (model, raws);
  temps = th_model_temperatures (model);
  g_assert_cmpstr (name_of (temps, "coretemp-isa-0000/temp1"), ==, "CPU Package");
  g_assert_cmpstr (name_of (temps, "coretemp-isa-0001/temp1"), ==, "CPU Package 1");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0100/temp1"), ==, "SSD 1");
  g_assert_cmpstr (name_of (temps, "nvme-pci-0200/temp1"), ==, "SSD 2");
  th_model_free (model);
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/model/each-chip-has-its-group", test_groups);
  g_test_add_func ("/model/batteries-ac-and-usb-c-are-left-out", test_left_out_chips);
  g_test_add_func ("/model/hot-starts-at-crit-minus-5-when-that-is-lower", test_bands);
  g_test_add_func ("/model/the-thinkpad-shows-its-package-and-its-fan", test_thinkpad);
  g_test_add_func ("/model/the-embedded-controller-is-prefixed", test_embedded_controller);
  g_test_add_func ("/model/the-desktop-shows-its-cpu-and-hottest-gpu-over-its-fastest-fan", test_desktop);
  g_test_add_func ("/model/warm-is-orange-and-hot-is-red", test_colours);
  g_test_add_func ("/model/every-fan-at-0-is-fans-off", test_fans_off);
  g_test_add_func ("/model/with-nothing-read-the-panel-shows-a-dash", test_nothing_read);
  g_test_add_func ("/model/the-cpu-is-the-hottest-without-a-package",
                   test_cpu_is_the_hottest_without_a_package);
  g_test_add_func ("/model/a-dead-reading-shows-once-it-moves", test_dead_until_it_moves);
  g_test_add_func ("/model/a-chip-that-goes-takes-its-readings", test_chip_disappears);
  g_test_add_func ("/model/the-gpu-panel-sensor-is-kept", test_gpu_sensor_is_kept);
  g_test_add_func ("/model/temperatures-round-half-up", test_one_line_rounds);
  g_test_add_func ("/model/the-same-names-are-numbered", test_same_names_are_numbered);
  return g_test_run ();
}
