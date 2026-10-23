"""Puts python-dbusmock's networkmanager template, already running, in one of CMN-201's states.

    bed.py <state>

wifi, wired-vpn, nothing, wired: each set on a fresh mock, as session.sh starts one for
each. move-address then changes the wired connection's IPv4 address on the mock as it is.
The template lacks PrimaryConnection, the Wi-Fi device's ActiveAccessPoint and the IP
configurations, which NetworkManager has, so they're added here. Its WwanEnabled is false,
which NetworkManager's isn't unless the user turned it off.
"""

import sys

import dbus

NM = "org.freedesktop.NetworkManager"
NM_PATH = "/org/freedesktop/NetworkManager"
MOCK = "org.freedesktop.DBus.Mock"
TEST = "org.mate.Caminito.Test"
ACTIVATED = 2
DEVICE_UNAVAILABLE, DEVICE_ACTIVATED = 20, 100

bus = dbus.SystemBus()
mock = dbus.Interface(bus.get_object(NM, NM_PATH), MOCK)
test = dbus.Interface(bus.get_object(NM, NM_PATH), TEST)
settings = dbus.Interface(bus.get_object(NM, NM_PATH + "/Settings"), NM + ".Settings")


def set_property(path, interface, name, value):
    mock.SetProperty(path, interface, name, value)


def add_connection(name, kind, timestamp=0):
    return settings.AddConnection({"connection": {"id": name, "type": kind,
                                                  "timestamp": dbus.UInt64(timestamp)}})


def address_data(addresses):
    return dbus.Array([dbus.Dictionary({"address": address, "prefix": dbus.UInt32(prefix)},
                                       signature="sv")
                       for address, prefix in addresses], signature="a{sv}")


def set_ip_config(active, family, n, addresses, gateway="", dns=()):
    """An IP4Config or IP6Config object on the active connection."""
    # The mock's AddObject over D-Bus doesn't tell the ObjectManager, which libnm follows.
    mock.AddMethod(TEST, "AddIPConfig", "ssa{sv}", "",
                   "self.AddObject(args[0], args[1], args[2], [])\n"
                   "self.object_manager_emit_added(args[0])")
    path = f"{NM_PATH}/IP{family}Config/{n}"
    properties = dbus.Dictionary({"AddressData": address_data(addresses), "Gateway": gateway},
                                 signature="sv")
    if dns:
        properties["NameserverData"] = dbus.Array(
            [dbus.Dictionary({"address": server}, signature="sv") for server in dns],
            signature="a{sv}")
    test.AddIPConfig(path, f"{NM}.IP{family}Config", properties)
    dbus.Interface(bus.get_object(NM, active), MOCK).AddProperty(
        NM + ".Connection.Active", f"Ip{family}Config", dbus.ObjectPath("/"))
    set_property(active, NM + ".Connection.Active", f"Ip{family}Config", dbus.ObjectPath(path))


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
    set_ip_config(active, 4, 1, [("192.168.1.20", 24)], "192.168.1.1", ["192.168.1.1"])
    set_ip_config(active, 6, 1, [("fe80::7add:8ff:fed2:3d43", 64), ("2001:db8::20", 64)])
    return active


def wifi():
    device = mock.AddWiFiDevice("wlan0", "wlan0", 100)
    dbus.Interface(bus.get_object(NM, device), MOCK).AddProperty(
        NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath("/"))
    # WPA2's PSK on channel 36.
    ap = mock.AddAccessPoint(device, "home", "Home", "00:11:22:33:44:55", 2, 5180, 54000,
                             dbus.Byte(70), 0x100)
    set_property(device, NM + ".Device.Wireless", "Bitrate", dbus.UInt32(866000))
    connection = mock.AddWiFiConnection(device, "home", "Home", "wpa-psk")
    active = mock.AddActiveConnection([device], connection, ap, "wifi", ACTIVATED)
    set_property(device, NM + ".Device.Wireless", "ActiveAccessPoint", dbus.ObjectPath(ap))
    set_property(NM_PATH, NM, "PrimaryConnection", dbus.ObjectPath(active))
    set_ip_config(active, 4, 1, [("192.168.1.34", 24)], "192.168.1.1",
                  ["192.168.1.1", "1.1.1.1"])


state = sys.argv[1]
if state == "move-address":
    set_property(NM_PATH + "/IP4Config/1", NM + ".IP4Config", "AddressData",
                 address_data([("192.168.1.77", 24)]))
    sys.exit()

mock.AddProperty(NM, "PrimaryConnection", dbus.ObjectPath("/"))
set_property(NM_PATH, NM, "WwanEnabled", True)
if state == "wifi":
    ethernet(DEVICE_UNAVAILABLE)
    wifi()
elif state == "wired-vpn":
    base = wired("Office")
    vpn = mock.AddActiveConnection(dbus.Array([], signature="s"),
                                   add_connection("Work VPN", "vpn", 2000), base, "vpn",
                                   ACTIVATED)
    set_ip_config(vpn, 4, 2, [("10.8.0.6", 24)], dns=["10.8.0.1"])
elif state == "nothing":
    ethernet(DEVICE_UNAVAILABLE)
elif state == "wired":
    wired()
else:
    sys.exit(f"no state {state}")
