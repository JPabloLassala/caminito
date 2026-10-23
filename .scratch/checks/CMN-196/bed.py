"""Puts python-dbusmock's networkmanager template, already running, in one of CMN-196's states.

    bed.py <state>

wifi-on, wifi-off, cable-in, cable-out, vpn, airplane, limited, connecting, nothing. Each
state is set on a fresh mock: session.sh starts one for each. The template lacks
PrimaryConnection, the Wi-Fi device's ActiveAccessPoint and the IPv4 configuration, which
NetworkManager has, so they're added here. Its WwanEnabled is false, which NetworkManager's
isn't unless the user turned it off, and without it Wi-Fi off would be airplane mode.
"""

import sys

import dbus

NM = "org.freedesktop.NetworkManager"
NM_PATH = "/org/freedesktop/NetworkManager"
MOCK = "org.freedesktop.DBus.Mock"
TEST = "org.mate.Caminito.Test"
ACTIVATING, ACTIVATED = 1, 2
LIMITED = 3
DEVICE_UNAVAILABLE, DEVICE_DISCONNECTED, DEVICE_ACTIVATED = 20, 30, 100

bus = dbus.SystemBus()
mock = dbus.Interface(bus.get_object(NM, NM_PATH), MOCK)
test = dbus.Interface(bus.get_object(NM, NM_PATH), TEST)
settings = dbus.Interface(bus.get_object(NM, NM_PATH + "/Settings"), NM + ".Settings")


def set_property(path, interface, name, value):
    mock.SetProperty(path, interface, name, value)


def add_connection(name, kind, timestamp=0):
    return settings.AddConnection({"connection": {"id": name, "type": kind,
                                                  "timestamp": dbus.UInt64(timestamp)}})


def set_address(active, address):
    # The mock's AddObject over D-Bus doesn't tell the ObjectManager, which libnm follows.
    mock.AddMethod(TEST, "AddIP4Config", "ss", "",
                   "self.AddObject(args[0], 'org.freedesktop.NetworkManager.IP4Config',"
                   " {'AddressData': dbus.Array([{'address': args[1],"
                   " 'prefix': dbus.UInt32(24, variant_level=1)}], signature='a{sv}'),"
                   " 'Gateway': ''}, [])\n"
                   "self.object_manager_emit_added(args[0])")
    path = NM_PATH + "/IP4Config/1"
    test.AddIP4Config(path, address)
    dbus.Interface(bus.get_object(NM, active), MOCK).AddProperty(
        NM + ".Connection.Active", "Ip4Config", dbus.ObjectPath("/"))
    set_property(active, NM + ".Connection.Active", "Ip4Config", dbus.ObjectPath(path))


def ethernet(state, name="Wired connection 1"):
    device = mock.AddEthernetDevice("eth0", "eth0", state)
    connection = add_connection(name, "802-3-ethernet")
    set_property(device, NM + ".Device", "AvailableConnections",
                 dbus.Array([connection], signature="o"))
    return device, connection


def wired(name="Wired connection 1"):
    device, connection = ethernet(DEVICE_ACTIVATED, name)
    set_property(device, NM + ".Device.Wired", "Speed", dbus.UInt32(1000))
    active = mock.AddActiveConnection([device], connection, "/", "wired", ACTIVATED)
    set_property(NM_PATH, NM, "PrimaryConnection", dbus.ObjectPath(active))
    set_address(active, "192.168.1.20")
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
        set_address(active, "192.168.1.34")


mock.AddProperty(NM, "PrimaryConnection", dbus.ObjectPath("/"))
set_property(NM_PATH, NM, "WwanEnabled", True)
# A VPN saved, for the VPN tile to turn on.
add_connection("Work", "vpn", 1000)
state = sys.argv[1]
if state == "wifi-on":
    ethernet(DEVICE_UNAVAILABLE)
    wifi(70)
elif state == "wifi-off":
    ethernet(DEVICE_UNAVAILABLE)
    set_property(NM_PATH, NM, "WirelessEnabled", False)
elif state == "cable-in":
    wired()
elif state == "cable-out":
    ethernet(DEVICE_UNAVAILABLE)
elif state == "vpn":
    base = wired("Office")
    mock.AddActiveConnection(dbus.Array([], signature="s"),
                             add_connection("Work VPN", "vpn", 2000), base, "vpn", ACTIVATED)
elif state == "airplane":
    ethernet(DEVICE_UNAVAILABLE)
    set_property(NM_PATH, NM, "WirelessEnabled", False)
    set_property(NM_PATH, NM, "WwanEnabled", False)
elif state == "limited":
    ethernet(DEVICE_UNAVAILABLE)
    wifi(70)
    set_property(NM_PATH, NM, "Connectivity", dbus.UInt32(LIMITED))
elif state == "connecting":
    ethernet(DEVICE_UNAVAILABLE)
    wifi(70, ACTIVATING)
elif state == "nothing":
    # The cable in, and nothing up on it or on Wi-Fi.
    ethernet(DEVICE_DISCONNECTED)
else:
    sys.exit(f"no state {state}")
