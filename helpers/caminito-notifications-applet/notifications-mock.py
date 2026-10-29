"""A mock of caminito-notifications's private API (ADR 0051), for python-dbusmock (CMN-260).

    python3 -m dbusmock --template notifications-mock.py

It has the Notification history's part of org.mate.Caminito.Notifications as the
Notifications applet uses it: List, Dismiss, ClearApp, ClearAll, InvokeAction, Reply, Counts,
Ungrouped and OpenSettings, with the HistoryChanged, CountsChanged and UngroupedChanged
signals. As in the daemon, an
entry leaves the history when it's dismissed or one of its actions is used, unless it's
resident, and List gives the newest first. It refuses what the daemon refuses: an entry that
isn't there, and an action an entry hasn't, "default" without a default action.

org.caminito.NotificationsMock is for a test or a check script:

* AddEntry(id, app, app_name, app_icon, summary, body, image, actions, has_default, resident,
  age): a new entry, which arrived age seconds ago. image is a path or a theme's name.
* SetUngrouped(apps): the apps whose grouping is off (CMN-265), which Ungrouped gives.
* GetLog() -> the calls the mock took, in order, as lines:
    Dismiss <id>
    ClearApp <app>
    ClearAll
    InvokeAction <id> <key> "<token>"
    Reply <id> "<text>"
    OpenSettings "<app>"
"""

import time

import dbus
import dbus.service

BUS_NAME = "org.mate.Caminito.Notifications"
MAIN_OBJ = "/org/mate/Caminito/Notifications"
MAIN_IFACE = "org.mate.Caminito.Notifications"
SYSTEM_BUS = False
MOCK_IFACE = "org.caminito.NotificationsMock"
ENTRY = "(usssssva(ss)bbbt)"


def load(mock, _parameters):
    mock.log_lines = []
    # Newest first, as List gives them: dicts with the entry's fields.
    mock.entries = []
    mock.ungrouped = []


def _counts(mock):
    counts = {}
    for entry in mock.entries:
        counts[entry["app"]] = counts.get(entry["app"], 0) + 1
    return dbus.Dictionary(counts, signature="su")


def _changed(mock):
    mock.EmitSignal(MAIN_IFACE, "HistoryChanged", "", [])
    mock.EmitSignal(MAIN_IFACE, "CountsChanged", "a{su}", [_counts(mock)])


def _find(mock, entry_id):
    for entry in mock.entries:
        if entry["id"] == entry_id:
            return entry
    raise dbus.exceptions.DBusException(
        f"there's no notification {entry_id} in the history",
        name="org.freedesktop.DBus.Error.InvalidArgs",
    )


def _remove(mock, ids):
    mock.entries = [entry for entry in mock.entries if entry["id"] not in ids]
    _changed(mock)


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="a" + ENTRY)
def List(self):
    return dbus.Array(
        [
            dbus.Struct(
                (
                    dbus.UInt32(e["id"]),
                    e["app"],
                    e["app_name"],
                    e["app_icon"],
                    e["summary"],
                    e["body"],
                    dbus.String(e["image"], variant_level=1),
                    dbus.Array(e["actions"], signature="(ss)"),
                    e["has_default"],
                    e["resident"],
                    False,
                    dbus.UInt64(e["time"]),
                )
            )
            for e in self.entries
        ],
        signature=ENTRY,
    )


@dbus.service.method(MAIN_IFACE, in_signature="u", out_signature="")
def Dismiss(self, entry_id):
    self.log_lines.append(f"Dismiss {int(entry_id)}")
    _find(self, int(entry_id))
    _remove(self, {int(entry_id)})


@dbus.service.method(MAIN_IFACE, in_signature="s", out_signature="")
def ClearApp(self, app):
    self.log_lines.append(f"ClearApp {app}")
    _remove(self, {e["id"] for e in self.entries if e["app"] == app})


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="")
def ClearAll(self):
    self.log_lines.append("ClearAll")
    _remove(self, {e["id"] for e in self.entries})


@dbus.service.method(MAIN_IFACE, in_signature="uss", out_signature="")
def InvokeAction(self, entry_id, key, token):
    self.log_lines.append(f'InvokeAction {int(entry_id)} {key} "{token}"')
    entry = _find(self, int(entry_id))
    if not (key == "default" and entry["has_default"]) and key not in dict(entry["actions"]):
        raise dbus.exceptions.DBusException(
            f"notification {int(entry_id)} has no action {key}",
            name="org.freedesktop.DBus.Error.InvalidArgs",
        )
    if not entry["resident"]:
        _remove(self, {int(entry_id)})


@dbus.service.method(MAIN_IFACE, in_signature="us", out_signature="")
def Reply(self, entry_id, text):
    self.log_lines.append(f'Reply {int(entry_id)} "{text}"')
    _find(self, int(entry_id))
    # Its entries have no reply field, as List says (CMN-266).
    raise dbus.exceptions.DBusException(
        f"notification {int(entry_id)} has no reply field",
        name="org.freedesktop.DBus.Error.InvalidArgs",
    )


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="a{su}")
def Counts(self):
    return _counts(self)


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="as")
def Ungrouped(self):
    return dbus.Array(self.ungrouped, signature="s")


@dbus.service.method(MAIN_IFACE, in_signature="s", out_signature="")
def OpenSettings(self, app):
    self.log_lines.append(f'OpenSettings "{app}"')


@dbus.service.method(MOCK_IFACE, in_signature="ussssssa(ss)bbt", out_signature="")
def AddEntry(
    self, entry_id, app, app_name, app_icon, summary, body, image, actions, has_default,
    resident, age,
):
    self.entries.insert(
        0,
        {
            "id": int(entry_id),
            "app": str(app),
            "app_name": str(app_name),
            "app_icon": str(app_icon),
            "summary": str(summary),
            "body": str(body),
            "image": str(image),
            "actions": [(str(k), str(v)) for k, v in actions],
            "has_default": bool(has_default),
            "resident": bool(resident),
            "time": int(time.time()) - int(age),
        },
    )
    _changed(self)


@dbus.service.method(MOCK_IFACE, in_signature="as", out_signature="")
def SetUngrouped(self, apps):
    self.ungrouped = sorted(str(app) for app in apps)
    self.EmitSignal(MAIN_IFACE, "UngroupedChanged", "as", [dbus.Array(self.ungrouped, signature="s")])


@dbus.service.method(MOCK_IFACE, in_signature="", out_signature="as")
def GetLog(self):
    return dbus.Array(self.log_lines, signature="s")
