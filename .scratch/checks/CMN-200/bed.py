"""Puts python-dbusmock's networkmanager template, already running, in one of CMN-200's states.

    bed.py <state>

Each state is on a fresh mock: session.sh starts one for each. Every one has Home up and
primary on Wi-Fi, which the card shows and the list leaves out; a cable in with "Dock"
saved; the VPNs "Work" and "Office VPN" saved; and in range, Library (two access points,
saved), eduroam (Enterprise), Cafe (open), Old (WEP) and a hidden network.

    list         that alone
    connecting   Library coming up too
    wifi-off     Wi-Fi off: the cable and the VPNs only

The template lacks PrimaryConnection and the Wi-Fi device's ActiveAccessPoint, which
NetworkManager has, so they're added here. Its WwanEnabled is false, which NetworkManager's
isn't unless the user turned it off.
"""

import sys

import dbus

NM = "org.freedesktop.NetworkManager"
NM_PATH = "/org/freedesktop/NetworkManager"
MOCK = "org.freedesktop.DBus.Mock"
ACTIVATING, ACTIVATED = 1, 2
DEVICE_DISCONNECTED = 30
OPEN, PSK, ENTERPRISE = 0, 0x100, 0x200

bus = dbus.SystemBus()
mock = dbus.Interface(bus.get_object(NM, NM_PATH), MOCK)
settings = dbus.Interface(bus.get_object(NM, NM_PATH + "/Settings"), NM + ".Settings")


def set_property(path, interface, name, value):
    mock.SetProperty(path, interface, name, value)


def add_connection(name, kind, timestamp=0):
    return settings.AddConnection({"connection": {"id": name, "type": kind,
                                                  "timestamp": dbus.UInt64(timestamp)}})


def ap(device, name, ssid, strength, security):
    return mock.AddAccessPoint(device, name, ssid, "00:11:22:33:44:%02x" % strength, 2, 2437,
                               54000, dbus.Byte(strength), security)


mock.AddProperty(NM, "PrimaryConnection", dbus.ObjectPath("/"))
set_property(NM_PATH, NM, "WwanEnabled", True)
add_connection("Work", "vpn", 1000)
add_connection("Office VPN", "vpn", 2000)

ethernet = mock.AddEthernetDevice("eth0", "eth0", DEVICE_DISCONNECTED)
set_property(ethernet, NM + ".Device", "AvailableConnections",
             dbus.Array([add_connection("Dock", "802-3-ethernet")], signature="o"))

wifi = mock.AddWiFiDevice("wlan0", "wlan0", 100)
dbus.Interface(bus.get_object(NM, wifi), MOCK).AddProperty(
    NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath("/"))
home = ap(wifi, "home", "Home", 70, PSK)
active = mock.AddActiveConnection([wifi], mock.AddWiFiConnection(wifi, "home", "Home", "wpa-psk"),
                                  home, "wifi", ACTIVATED)
set_property(wifi, NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath(home))
set_property(NM_PATH, NM, "PrimaryConnection", dbus.ObjectPath(active))
library = ap(wifi, "library", "Library", 80, PSK)
ap(wifi, "library2", "Library", 50, PSK)
saved = mock.AddWiFiConnection(wifi, "library", "Library", "wpa-psk")
ap(wifi, "eduroam", "eduroam", 60, ENTERPRISE)
ap(wifi, "cafe", "Cafe", 40, OPEN)
# WEP: the privacy flag with neither WPA nor RSN, which the template's own can't make.
set_property(ap(wifi, "old", "Old", 20, OPEN), NM + ".AccessPoint", "Flags", dbus.UInt32(1))
ap(wifi, "hidden", "", 90, PSK)

state = sys.argv[1]
if state == "connecting":
    mock.AddActiveConnection(dbus.Array([], signature="s"), saved, library, "library",
                             ACTIVATING)
elif state == "wifi-off":
    set_property(NM_PATH, NM, "WirelessEnabled", False)
elif state != "list":
    sys.exit(f"no state {state}")
