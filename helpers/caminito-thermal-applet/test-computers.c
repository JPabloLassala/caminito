/*
 * The computers the Thermal applet's tests build by hand: what libsensors would read of
 * them. See test-computers.h.
 */

#include <math.h>

#include "test-computers.h"

void
temp (GPtrArray *raws, const char *chip, const char *chip_id, const char *feature,
      const char *label, double value, double crit)
{
  g_ptr_array_add (raws, th_raw_new (chip, chip_id, feature, label, NULL, FALSE, TRUE, value,
                                     crit));
}

void
fan (GPtrArray *raws, const char *chip, const char *chip_id, const char *feature,
     const char *label, double rpm, double max)
{
  g_ptr_array_add (raws, th_raw_new (chip, chip_id, feature, label, NULL, TRUE, TRUE, rpm,
                                     max));
}

GPtrArray *
raws_new (void)
{
  return g_ptr_array_new_with_free_func ((GDestroyNotify) th_raw_free);
}

/* The developer's ThinkPad: coretemp, the embedded controller with one fan with no max and
   readings that are always 0, ACPI, Wi-Fi, an NVMe drive, and the battery chips. */
GPtrArray *
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
GPtrArray *
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
