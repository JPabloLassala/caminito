/*
 * The Thermal applet's source (267) against fake /sys/class/hwmon trees that umockdev makes,
 * which libsensors reads: drives' models, a reading that can't be read, and chips that come
 * and go. Run under umockdev-wrapper.
 */

#include <math.h>
#include <glib/gstdio.h>
#include <umockdev.h>

#include "th-model.h"
#include "th-source.h"

static const ThRaw *
find (GPtrArray *raws, const char *chip, const char *feature)
{
  for (guint i = 0; i < raws->len; i++)
    {
      const ThRaw *raw = raws->pdata[i];

      if (g_strcmp0 (raw->chip, chip) == 0 && g_strcmp0 (raw->feature, feature) == 0)
        return raw;
    }
  return NULL;
}

/* A file under a device that umockdev's attributes can't make, as nvme/nvme0/model. */
static void
write_file (UMockdevTestbed *bed, const char *device, const char *relative, const char *text)
{
  g_autofree char *path = g_build_filename (umockdev_testbed_get_root_dir (bed), device,
                                            relative, NULL);
  g_autofree char *dir = g_path_get_dirname (path);

  g_assert_cmpint (g_mkdir_with_parents (dir, 0755), ==, 0);
  g_assert_true (g_file_set_contents (path, text, -1, NULL));
}

/* The hwmon's device link, as the kernel makes it and umockdev doesn't. */
static void
link_device (UMockdevTestbed *bed, const char *hwmon, const char *device)
{
  const char *root = umockdev_testbed_get_root_dir (bed);
  g_autofree char *link = g_build_filename (root, hwmon, "device", NULL);
  g_autofree char *target = g_build_filename (root, device, NULL);

  g_assert_cmpint (symlink (target, link), ==, 0);
}

static void
test_drive_models (void)
{
  g_autoptr (UMockdevTestbed) bed = umockdev_testbed_new ();
  g_autofree char *pci = NULL;
  g_autofree char *nvme = NULL;
  g_autofree char *scsi = NULL;
  g_autofree char *disk = NULL;
  g_autoptr (GPtrArray) raws = NULL;
  const ThRaw *raw;

  /* An NVMe drive's hwmon is its PCI device's, and the model is its controller's. */
  pci = umockdev_testbed_add_device (bed, "pci", "0000:01:00.0", NULL, NULL, NULL);
  write_file (bed, pci, "nvme/nvme0/model", "Samsung SSD 990 PRO 2TB  \n");
  nvme = umockdev_testbed_add_device (bed, "hwmon", "hwmon0", pci, "name", "nvme\n",
                                      "temp1_input", "41850\n", "temp1_label", "Composite\n",
                                      "temp1_crit", "84850\n", NULL, NULL);
  link_device (bed, nvme, pci);
  /* A SATA drive's (drivetemp) is the SCSI device's own. */
  scsi = umockdev_testbed_add_device (bed, "scsi", "0:0:0:0", NULL, "model", "WDC WD40EFRX\n",
                                      NULL, NULL);
  disk = umockdev_testbed_add_device (bed, "hwmon", "hwmon1", scsi, "name", "drivetemp\n",
                                      "temp1_input", "33000\n", NULL, NULL);
  link_device (bed, disk, scsi);

  th_source_acquire ();
  raws = th_source_read ();
  raw = find (raws, "nvme", "temp1");
  g_assert_nonnull (raw);
  g_assert_cmpstr (raw->chip_id, ==, "nvme-pci-0100");
  g_assert_cmpstr (raw->device, ==, "Samsung SSD 990 PRO 2TB");
  g_assert_cmpstr (raw->label, ==, "Composite");
  g_assert_cmpfloat_with_epsilon (raw->value, 41.85, 0.001);
  g_assert_cmpfloat_with_epsilon (raw->limit, 84.85, 0.001);
  raw = find (raws, "drivetemp", "temp1");
  g_assert_nonnull (raw);
  g_assert_cmpstr (raw->device, ==, "WDC WD40EFRX");
  th_source_release ();
}

static void
test_readings (void)
{
  g_autoptr (UMockdevTestbed) bed = umockdev_testbed_new ();
  g_autoptr (GPtrArray) raws = NULL;
  const ThRaw *raw;

  /* temp2's input is empty, as one that can't be read: libsensors' scan of it fails. */
  umockdev_testbed_add_device (bed, "hwmon", "hwmon0", NULL, "name", "thinkpad\n",
                               "temp1_input", "52000\n", "temp1_label", "CPU\n",
                               "temp2_input", "", "fan1_input", "2865\n", NULL, NULL);
  th_source_acquire ();
  raws = th_source_read ();
  raw = find (raws, "thinkpad", "temp1");
  g_assert_true (raw->ok);
  g_assert_false (raw->is_fan);
  g_assert_cmpstr (raw->label, ==, "CPU");
  g_assert_cmpfloat (raw->value, ==, 52);
  g_assert_true (isnan (raw->limit));
  raw = find (raws, "thinkpad", "temp2");
  g_assert_false (raw->ok);
  /* With no label, libsensors gives the feature's name, which isn't one. */
  raw = find (raws, "thinkpad", "fan1");
  g_assert_true (raw->ok);
  g_assert_true (raw->is_fan);
  g_assert_null (raw->label);
  g_assert_cmpfloat (raw->value, ==, 2865);
  th_source_release ();
}

static void
test_chips_come_and_go (void)
{
  g_autoptr (UMockdevTestbed) bed = umockdev_testbed_new ();
  g_autofree char *gpu = NULL;
  g_autoptr (GPtrArray) before = NULL;
  g_autoptr (GPtrArray) gone = NULL;
  g_autoptr (GPtrArray) back = NULL;

  umockdev_testbed_add_device (bed, "hwmon", "hwmon0", NULL, "name", "k10temp\n",
                               "temp1_input", "58000\n", "temp1_label", "Tctl\n", NULL, NULL);
  gpu = umockdev_testbed_add_device (bed, "hwmon", "hwmon1", NULL, "name", "amdgpu\n",
                                     "temp1_input", "61000\n", NULL, NULL);
  th_source_acquire ();
  before = th_source_read ();
  g_assert_nonnull (find (before, "amdgpu", "temp1"));

  /* Its module unloaded. */
  umockdev_testbed_remove_device (bed, gpu);
  gone = th_source_read ();
  g_assert_null (find (gone, "amdgpu", "temp1"));
  g_assert_nonnull (find (gone, "k10temp", "temp1"));

  /* And loaded again. */
  g_free (gpu);
  gpu = umockdev_testbed_add_device (bed, "hwmon", "hwmon2", NULL, "name", "amdgpu\n",
                                     "temp1_input", "62000\n", NULL, NULL);
  back = th_source_read ();
  g_assert_cmpfloat (find (back, "amdgpu", "temp1")->value, ==, 62);
  th_source_release ();
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/source/drives-are-named-after-their-models", test_drive_models);
  g_test_add_func ("/source/temperatures-and-fans-are-read-with-their-labels", test_readings);
  g_test_add_func ("/source/a-chip-that-goes-or-comes-is-found-again", test_chips_come_and_go);
  return g_test_run ();
}
