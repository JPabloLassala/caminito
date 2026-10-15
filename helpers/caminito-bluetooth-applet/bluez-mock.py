"""python-dbusmock's bluez5 template with Battery1, which it has none of (259).

    python3 -m dbusmock --system --template bluez-mock.py

AddBattery(device_path, percentage) gives a device Battery1 and announces it as BlueZ
does, through the object manager's InterfacesAdded.
"""

import dbus

from dbusmock import mockobject
from dbusmock.templates.bluez5 import *  # noqa: F401,F403 -- the template's methods
from dbusmock.templates.bluez5 import BLUEZ_MOCK_IFACE, OBJECT_MANAGER_IFACE

BATTERY_IFACE = "org.bluez.Battery1"


@dbus.service.method(BLUEZ_MOCK_IFACE, in_signature="oy", out_signature="")
def AddBattery(_self, device_path, percentage):
    device = mockobject.objects[device_path]
    properties = {
        "Percentage": dbus.Byte(percentage, variant_level=1),
        "Source": dbus.String("mock", variant_level=1),
    }
    device.AddProperties(BATTERY_IFACE, properties)
    mockobject.objects["/"].EmitSignal(
        OBJECT_MANAGER_IFACE,
        "InterfacesAdded",
        "oa{sa{sv}}",
        [dbus.ObjectPath(device_path), {BATTERY_IFACE: properties}],
    )
