"""Puts python-dbusmock's networkmanager template, already running, in one of CMN-193's states.

    bed.py <state>

wired, wifi-<strength>, connecting, limited, vpn, airplane, nothing. Each state is set on
a fresh mock: session.sh starts one for each. The template lacks PrimaryConnection and the
Wi-Fi device's ActiveAccessPoint, which NetworkManager has, so they're added here.
"""

import sys

import dbus

NM = "org.freedesktop.NetworkManager"
NM_PATH = "/org/freedesktop/NetworkManager"
MOCK = "org.freedesktop.DBus.Mock"
ACTIVATING, ACTIVATED = 1, 2
LIMITED = 3

bus = dbus.SystemBus()
mock = dbus.Interface(bus.get_object(NM, NM_PATH), MOCK)
settings = dbus.Interface(bus.get_object(NM, NM_PATH + "/Settings"), NM + ".Settings")


def set_property(path, interface, name, value):
    mock.SetProperty(path, interface, name, value)


def add_connection(name, kind):
    return settings.AddConnection({"connection": {"id": name, "type": kind}})


def wired(name="Wired connection 1"):
    device = mock.AddEthernetDevice("eth0", "eth0", 100)
    active = mock.AddActiveConnection([device], add_connection(name, "802-3-ethernet"), "/",
                                      "wired", ACTIVATED)
    set_property(NM_PATH, NM, "PrimaryConnection", dbus.ObjectPath(active))
    return active


def wifi(strength, state=ACTIVATED):
    device = mock.AddWiFiDevice("wlan0", "wlan0", 100)
    dbus.Interface(bus.get_object(NM, device), MOCK).AddProperty(
        NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath("/"))
    ap = mock.AddAccessPoint(device, "home", "Home", "00:11:22:33:44:55", 2, 2437, 54000,
                             dbus.Byte(strength), 0x100)
    connection = mock.AddWiFiConnection(device, "home", "Home", "wpa-psk")
    active = mock.AddActiveConnection([device], connection, ap, "wifi", state)
    set_property(device, NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath(ap))
    if state == ACTIVATED:
        set_property(NM_PATH, NM, "PrimaryConnection", dbus.ObjectPath(active))


mock.AddProperty(NM, "PrimaryConnection", dbus.ObjectPath("/"))
state = sys.argv[1]
if state == "wired":
    wired()
elif state.startswith("wifi-"):
    wifi(int(state[len("wifi-"):]))
elif state == "connecting":
    wifi(70, ACTIVATING)
elif state == "limited":
    wifi(70)
    set_property(NM_PATH, NM, "Connectivity", dbus.UInt32(LIMITED))
elif state == "vpn":
    base = wired("Office")
    mock.AddActiveConnection(dbus.Array([], signature="s"), add_connection("Work", "vpn"), base,
                             "vpn", ACTIVATED)
elif state == "airplane":
    # The template's WwanEnabled is already false.
    set_property(NM_PATH, NM, "WirelessEnabled", False)
elif state != "nothing":
    sys.exit(f"no state {state}")
