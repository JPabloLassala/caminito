"""A mock of mate-session's inhibitor calls, for python-dbusmock (265).

    python3 -m dbusmock --template session-mock.py

It has what the Power applet uses of org.gnome.SessionManager: Inhibit, Uninhibit and
GetInhibitors, an object for each inhibitor with GetAppId, GetClientId, GetReason, GetFlags
and GetToplevelXid, and the InhibitorAdded and InhibitorRemoved signals. As in mate-session,
an inhibitor goes when the bus name that made it leaves the bus.

org.caminito.SessionMock is for a test or a check script:

* AddInhibitor(app_id, reason, flags) -> path: an application's inhibitor, which has no
  owner to leave.
* RemoveInhibitor(path).
* GetLog() -> the calls the mock took, in order, as lines:
    Inhibit "app id" <xid> "reason" <flags> -> <cookie>
    Uninhibit <cookie>
    Vanished <cookie>        (its owner left the bus)
"""

import dbus
import dbus.service

BUS_NAME = "org.gnome.SessionManager"
MAIN_OBJ = "/org/gnome/SessionManager"
MAIN_IFACE = "org.gnome.SessionManager"
SYSTEM_BUS = False
INHIBITOR_IFACE = "org.gnome.SessionManager.Inhibitor"
MOCK_IFACE = "org.caminito.SessionMock"


def load(mock, _parameters):
    mock.log_lines = []
    # Inhibitor paths in the order they were made, and the cookie each one has, if any.
    mock.inhibitor_paths = []
    mock.cookies = {}
    mock.next_inhibitor = 1
    mock.next_cookie = 1


def _add(mock, app_id, reason, flags, xid=0):
    path = f"{MAIN_OBJ}/Inhibitor{mock.next_inhibitor}"
    mock.next_inhibitor += 1
    mock.AddObject(
        path,
        INHIBITOR_IFACE,
        {},
        [
            ("GetAppId", "", "s", f"ret = {app_id!r}"),
            ("GetClientId", "", "o", 'ret = "/"'),
            ("GetReason", "", "s", f"ret = {reason!r}"),
            ("GetFlags", "", "u", f"ret = {flags}"),
            ("GetToplevelXid", "", "u", f"ret = {xid}"),
        ],
    )
    mock.inhibitor_paths.append(path)
    mock.EmitSignal(MAIN_IFACE, "InhibitorAdded", "o", [dbus.ObjectPath(path)])
    return path


def _remove(mock, path):
    mock.inhibitor_paths.remove(path)
    mock.RemoveObject(path)
    mock.EmitSignal(MAIN_IFACE, "InhibitorRemoved", "o", [dbus.ObjectPath(path)])


def _drop_when_owner_leaves(mock, sender, cookie):
    def changed(owner):
        if owner == "" and cookie in mock.cookies:
            mock.log_lines.append(f"Vanished {cookie}")
            _remove(mock, mock.cookies.pop(cookie))

    mock.connection.watch_name_owner(sender, changed)


@dbus.service.method(MAIN_IFACE, in_signature="susu", out_signature="u", sender_keyword="sender")
def Inhibit(self, app_id, xid, reason, flags, sender):
    cookie = self.next_cookie
    self.next_cookie += 1
    self.log_lines.append(f'Inhibit "{app_id}" {int(xid)} "{reason}" {int(flags)} -> {cookie}')
    self.cookies[cookie] = _add(self, str(app_id), str(reason), int(flags), int(xid))
    _drop_when_owner_leaves(self, sender, cookie)
    return dbus.UInt32(cookie)


@dbus.service.method(MAIN_IFACE, in_signature="u", out_signature="")
def Uninhibit(self, cookie):
    self.log_lines.append(f"Uninhibit {int(cookie)}")
    if int(cookie) not in self.cookies:
        raise dbus.exceptions.DBusException(
            f"No inhibitor with the cookie {int(cookie)}", name="org.freedesktop.DBus.Error.Failed"
        )
    _remove(self, self.cookies.pop(int(cookie)))


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="ao")
def GetInhibitors(self):
    return dbus.Array([dbus.ObjectPath(p) for p in self.inhibitor_paths], signature="o")


@dbus.service.method(MOCK_IFACE, in_signature="ssu", out_signature="o")
def AddInhibitor(self, app_id, reason, flags):
    return dbus.ObjectPath(_add(self, str(app_id), str(reason), int(flags)))


@dbus.service.method(MOCK_IFACE, in_signature="o", out_signature="")
def RemoveInhibitor(self, path):
    _remove(self, str(path))


@dbus.service.method(MOCK_IFACE, in_signature="", out_signature="as")
def GetLog(self):
    return dbus.Array(self.log_lines, signature="s")
