"""A fake /sys/class/hwmon for CMN-99's check, through umockdev, which libsensors reads.

    umockdev-wrapper python3 bed.py <thinkpad|desktop> <command> [<argument>...]

builds the computer's chips and runs the command in it: the command and its children, which
inherit umockdev's preload, see the fake tree in /sys. A chip is hwmon<N> in the order below,
so the session finds it by its name file; it writes a value with
`echo 92000 > /sys/class/hwmon/hwmon0/temp1_input` and removes a chip as a module unloaded
does with `rm -r` of the device's directory and the class link.
"""

import os
import subprocess
import sys

import gi

gi.require_version("UMockdev", "1.0")
from gi.repository import UMockdev  # noqa: E402


def temps(*readings):
    """(feature, label or None, °C or None for one that can't be read, crit or None)"""
    attrs = []
    for feature, label, value, crit in readings:
        # An empty input file is one that can't be read: libsensors' scan of it fails.
        attrs += [f"{feature}_input", "" if value is None else f"{round(value * 1000)}\n"]
        if label is not None:
            attrs += [f"{feature}_label", f"{label}\n"]
        if crit is not None:
            attrs += [f"{feature}_crit", f"{round(crit * 1000)}\n"]
    return attrs


def fans(*readings):
    """(feature, rpm, max or None)"""
    attrs = []
    for feature, rpm, top in readings:
        attrs += [f"{feature}_input", f"{rpm}\n"]
        if top is not None:
            attrs += [f"{feature}_max", f"{top}\n"]
    return attrs


THINKPAD = [
    ("coretemp", None,
     temps(("temp1", "Package id 0", 55, 100), ("temp2", "Core 0", 53, 100),
           ("temp3", "Core 1", 52, 100))),
    # The embedded controller: one fan with no max, a GPU that can't be read and six
    # readings that are always 0.
    ("thinkpad", None,
     temps(("temp1", "CPU", 52, None), ("temp2", "GPU", None, None),
           *((f"temp{i}", None, 0, None) for i in range(3, 9)))
     + fans(("fan1", 2865, None))),
    ("acpitz", None, temps(("temp1", None, 50, 128))),
    ("iwlwifi_1", None, temps(("temp1", None, 41, None))),
    ("nvme", "SAMSUNG MZVL2512HCJQ", temps(("temp1", "Composite", 38, 84.85))),
    ("BAT0", None, temps(("temp1", None, 30, None))),
]

DESKTOP = [
    ("k10temp", None, temps(("temp1", "Tctl", 58, None), ("temp3", "Tccd1", 55, None))),
    ("amdgpu", None,
     temps(("temp1", "edge", 49, 100), ("temp2", "junction", 61, 110), ("temp3", "mem", 57, 105))
     + fans(("fan1", 0, 3300))),
    ("nvme", "Samsung SSD 990 PRO 2TB",
     temps(("temp1", "Composite", 41, 84.85), ("temp2", "Sensor 1", 45, 84.85))),
    ("nvme", "WD_BLACK SN850X 1000GB", temps(("temp1", "Composite", 46, 84.85))),
    ("nct6798", None,
     temps(("temp1", "SYSTIN", 34, None), ("temp7", "AUXTIN3", 0, None))
     + fans(("fan1", 820, None), ("fan2", 1180, None))),
]


def build(bed, chips):
    root = bed.get_root_dir()
    for n, (name, model, attrs) in enumerate(chips):
        parent = None
        if model is not None:
            # An NVMe drive, as the kernel has it: the hwmon's device is the drive's PCI
            # device, which libsensors names the chip after (nvme-pci-0100), and the model
            # is its nvme controller's, in nvme/nvme<N>/.
            parent = bed.add_device("pci", f"0000:0{n - 1}:00.0", None, [], [])
            os.makedirs(f"{root}{parent}/nvme/nvme{n}")
            with open(f"{root}{parent}/nvme/nvme{n}/model", "w") as f:
                f.write(f"{model}\n")
        path = bed.add_device("hwmon", f"hwmon{n}", parent, ["name", f"{name}\n", *attrs], [])
        if parent is not None:
            os.symlink(os.path.relpath(root + parent, root + path), root + path + "/device")


def main():
    computer, command = sys.argv[1], sys.argv[2:]
    bed = UMockdev.Testbed.new()
    build(bed, {"thinkpad": THINKPAD, "desktop": DESKTOP}[computer])
    sys.exit(subprocess.run(command).returncode)


main()
