# A stand-in owner of org.gnome.SessionManager that prints each call it gets.
from gi.repository import Gio, GLib
xml = """<node><interface name='org.gnome.SessionManager'>
<method name='CanShutdown'><arg type='b' direction='out'/></method>
<method name='Shutdown'/>
<method name='Logout'><arg type='u' direction='in'/></method>
</interface></node>"""
def call(conn, sender, path, iface, method, params, inv):
    print(method, flush=True)
    inv.return_value(GLib.Variant('(b)', (True,)) if method == 'CanShutdown' else None)
def bus(conn, name):
    conn.register_object('/org/gnome/SessionManager', Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0], call, None, None)
Gio.bus_own_name(Gio.BusType.SESSION, 'org.gnome.SessionManager', 0, bus, None, None)
GLib.MainLoop().run()
