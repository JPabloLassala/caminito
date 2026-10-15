#!/usr/bin/env bash
# One case of run.sh, on a session bus of its own: BlueZ's mock on a private bus, then a
# fresh user's panel.
set -u
template=$ROOT/helpers/caminito-bluetooth-applet/bluez-mock.py
DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$WORK/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
"$CAMINITO_PYTHON" -m dbusmock --system --template "$template" >"$WORK/mock-$CASE.log" 2>&1 &
for _ in $(seq 100); do
  gdbus introspect --system --dest org.bluez --object-path / >/dev/null 2>&1 && break
  sleep 0.1
done
if [ "$ADAPTER" = yes ]; then
  gdbus call --system --dest org.bluez --object-path / --method org.bluez.Mock.AddAdapter \
    hci0 my-computer >/dev/null
fi

echo "default-layout: $(gsettings get org.mate.panel default-layout)"
echo "object-id-list before: $(gsettings get org.mate.panel object-id-list)"
"$WORK/pkg/usr/bin/mate-panel" >"$WORK/panel-$CASE.log" 2>&1 &
panel=$!
sleep 6
if kill -0 "$panel" 2>/dev/null; then echo "panel running"; else echo "PANEL GONE"; fi
echo "toplevel-id-list: $(gsettings get org.mate.panel toplevel-id-list)"
ids=$(gsettings get org.mate.panel object-id-list | tr -d "[]',")
echo "object-id-list: $ids"
for id in $ids; do
  path=/org/mate/panel/objects/$id/
  schema=org.mate.panel.object:$path
  echo "  $id: $(gsettings get "$schema" object-type) $(gsettings get "$schema" toplevel-id) $(gsettings get "$schema" applet-iid)"
done
xdotool mousemove 450 150
sleep 1
import -window root "$OUT/$CASE.png"
grep -i "warning\|critical\|error" "$WORK/panel-$CASE.log" | head -5
kill "$panel" 2>/dev/null
