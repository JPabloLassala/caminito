#!/usr/bin/env bash
# The session run.sh starts once per icon theme, on a session bus of its own: a private
# bus for BlueZ's mock, a panel with the applet alone on it, then each state.
set -u
root=$1; work=$2; out=$3; theme=$4
template=$root/helpers/caminito-bluetooth-applet/bluez-mock.py
hci0=/org/bluez/hci0
headphones=$hci0/dev_11_22_33_44_55_66
mouse=$hci0/dev_AA_BB_CC_DD_EE_01

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
mock=
start_mock() {
  "$CAMINITO_PYTHON" -m dbusmock --system --template "$template" >"$work/mock-$theme.log" 2>&1 &
  mock=$!
  for _ in $(seq 100); do
    gdbus introspect --system --dest org.bluez --object-path / >/dev/null 2>&1 && return
    sleep 0.1
  done
  echo "the mock never started"
}
bluez() { gdbus call --system --dest org.bluez "$@" >/dev/null; }
alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }
shot() { sleep "${2:-1.5}"; import -window root -crop 360x140+0+0 "$out/$theme-$1.png"; }

# The applet alone on a top panel, at its left end.
dconf write /org/mate/panel/general/toplevel-id-list "['top']"
dconf write /org/mate/panel/toplevels/top/orientation "'top'"
dconf write /org/mate/panel/toplevels/top/size 28
dconf write /org/mate/panel/general/object-id-list "['bluetooth']"
dconf write /org/mate/panel/objects/bluetooth/object-type "'applet'"
dconf write /org/mate/panel/objects/bluetooth/applet-iid \
  "'CaminitoBluetoothAppletFactory::CaminitoBluetoothApplet'"
dconf write /org/mate/panel/objects/bluetooth/toplevel-id "'top'"
dconf write /org/mate/panel/objects/bluetooth/position 0

start_mock
mate-panel >"$work/panel-$theme.log" 2>&1 &
panel=$!
sleep 4
xdotool mousemove 600 400
shot 1-no-adapter; alive "no adapter"

bluez --object-path / --method org.bluez.Mock.AddAdapter hci0 my-computer
shot 2-on; alive "on"

bluez --object-path $hci0 --method org.freedesktop.DBus.Mock.UpdateProperties \
  org.bluez.Adapter1 "{'Powered': <false>}"
shot 3-off; alive "off"

bluez --object-path $hci0 --method org.freedesktop.DBus.Mock.UpdateProperties \
  org.bluez.Adapter1 "{'Powered': <true>}"
bluez --object-path / --method org.bluez.Mock.AddDevice hci0 11:22:33:44:55:66 "WH-1000XM4"
bluez --object-path / --method org.bluez.Mock.PairDevice hci0 11:22:33:44:55:66
bluez --object-path $headphones --method org.bluez.Device1.Connect
bluez --object-path / --method org.bluez.Mock.AddBattery "objectpath '$headphones'" "byte 80"
bluez --object-path / --method org.bluez.Mock.AddDevice hci0 AA:BB:CC:DD:EE:01 "MX Master 3"
bluez --object-path / --method org.bluez.Mock.PairDevice hci0 AA:BB:CC:DD:EE:01
bluez --object-path $mouse --method org.bluez.Device1.Connect
sleep 1
xdotool mousemove 12 14
shot 4-connected 2.5; alive "connected"

xdotool mousemove 12 14 click 3
shot 5-menu 1; alive "menu"
# The menu's own action, where it can't reach a real radio: without a writable
# /dev/rfkill it powers the mock's adapters off and on instead. On a Session, where the
# seat's user may write it, it would block the computer's Bluetooth, so it's skipped.
if [ -w /dev/rfkill ]; then
  xdotool key Escape
  echo "menu's off and on: skipped, /dev/rfkill is writable here"
else
  xdotool key Down Return
  xdotool mousemove 600 400
  shot 5-off-from-menu; alive "turned off from the menu"
  xdotool mousemove 12 14 click 3; sleep 1; xdotool key Down Return
  xdotool mousemove 600 400
  shot 5-on-from-menu; alive "turned on from the menu"
fi
xdotool mousemove 600 400

kill "$mock"; wait "$mock" 2>/dev/null
sleep 2; alive "BlueZ stopped"
start_mock
bluez --object-path / --method org.bluez.Mock.AddAdapter hci0 my-computer
shot 6-restarted; alive "BlueZ started again"

grep -i "critical\|warning\|bluetooth" "$work/panel-$theme.log" | grep -v "^$" | head -20 | sed 's/^/panel log: /'
kill "$panel" "$mock" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
