#!/usr/bin/env bash
# panel.sh's session, on a session bus of its own: BlueZ's and UPower's mocks on a private bus,
# then a fresh user's panel.
set -u
DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$WORK/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
"$CAMINITO_PYTHON" -m dbusmock --system \
  --template "$ROOT/helpers/caminito-bluetooth-applet/bluez-mock.py" >"$WORK/bluez.log" 2>&1 &
"$CAMINITO_PYTHON" -m dbusmock --system --template upower >"$WORK/upower.log" 2>&1 &
for _ in $(seq 100); do
  gdbus introspect --system --dest org.bluez --object-path / >/dev/null 2>&1 &&
    gdbus introspect --system --dest org.freedesktop.UPower --object-path /org/freedesktop/UPower \
      >/dev/null 2>&1 && break
  sleep 0.1
done
gdbus call --system --dest org.bluez --object-path / --method org.bluez.Mock.AddAdapter \
  hci0 my-computer >/dev/null
# A battery at 78%, discharging, 3 h 40 min to empty.
gdbus call --system --dest org.freedesktop.UPower --object-path /org/freedesktop/UPower \
  --method org.freedesktop.DBus.Mock.SetupDisplayDevice \
  2 2 78.0 78.0 100.0 10.0 13200 0 true '' 1 >/dev/null

echo "default-layout: $(gsettings get org.mate.panel default-layout)"
echo "object-id-list before: $(gsettings get org.mate.panel object-id-list)"
mate-panel >"$WORK/panel.log" 2>&1 &
panel=$!
sleep 6
if kill -0 "$panel" 2>/dev/null; then echo "panel running"; else echo "PANEL GONE"; fi
ids=$(gsettings get org.mate.panel object-id-list | tr -d "[]',")
echo "object-id-list: $ids"
for id in $ids; do
  schema=org.mate.panel.object:/org/mate/panel/objects/$id/
  echo "  $id: $(gsettings get "$schema" toplevel-id) $(gsettings get "$schema" pack-type)" \
    "$(gsettings get "$schema" pack-index) $(gsettings get "$schema" applet-iid)"
done
xdotool mousemove 450 150
sleep 1
import -window root "$OUT/fresh-user.png"
grep -i "warning\|critical\|error" "$WORK/panel.log" | head -5
kill "$panel" 2>/dev/null
