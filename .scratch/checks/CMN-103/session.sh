#!/usr/bin/env bash
# The session run.sh starts once per panel edge, on a session bus of its own: a private
# bus for BlueZ's mock, a panel with the applet alone on it, then each step.
set -u
root=$1; work=$2; out=$3; edge=$4
template=$root/helpers/caminito-bluetooth-applet/bluez-mock.py
hci0=/org/bluez/hci0
headphones=$hci0/dev_38_18_4C_12_9A_01
mouse=$hci0/dev_D4_1B_70_88_05_13

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
"$CAMINITO_PYTHON" -m dbusmock --system --template "$template" >"$work/mock-$edge.log" 2>&1 &
mock=$!
for _ in $(seq 100); do
  gdbus introspect --system --dest org.bluez --object-path / >/dev/null 2>&1 && break
  sleep 0.1
done
bluez() { gdbus call --system --dest org.bluez "$@" >/dev/null; }
get() { gdbus call --system --dest org.bluez --object-path "$1" --method org.freedesktop.DBus.Properties.Get org.bluez.Device1 "$2"; }
shot() { sleep "${2:-1}"; import -window root "$out/$edge-$1.png"; }
alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }

# The applet alone on a panel at the edge, at its left end.
dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'$edge'"
dconf write /org/mate/panel/toplevels/panel/size 28
dconf write /org/mate/panel/general/object-id-list "['bluetooth']"
dconf write /org/mate/panel/objects/bluetooth/object-type "'applet'"
dconf write /org/mate/panel/objects/bluetooth/applet-iid \
  "'CaminitoBluetoothAppletFactory::CaminitoBluetoothApplet'"
dconf write /org/mate/panel/objects/bluetooth/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/bluetooth/position 0

bluez --object-path / --method org.bluez.Mock.AddAdapter hci0 my-computer
# device <address> <name> <icon> [<battery>]: a paired device.
device() {
  local path=$hci0/dev_${1//:/_}
  bluez --object-path / --method org.bluez.Mock.AddDevice hci0 "$1" "$2"
  bluez --object-path / --method org.bluez.Mock.PairDevice hci0 "$1"
  bluez --object-path "$path" --method org.freedesktop.DBus.Mock.UpdateProperties \
    org.bluez.Device1 "{'Icon': <'$3'>}"
  [ -z "${4:-}" ] || bluez --object-path / --method org.bluez.Mock.AddBattery \
    "objectpath '$path'" "byte $4"
}
device 38:18:4C:12:9A:01 WH-1000XM4 audio-headphones 80
device D4:1B:70:88:05:13 'MX Master 3' input-mouse 12
device 9C:5C:F9:41:22:7B 'Pixel 7' phone
# Nearby, not paired: Add New Device's, not the popup's.
bluez --object-path / --method org.bluez.Mock.AddDevice hci0 64:03:7F:2E:11:90 'Galaxy Buds2'

mate-panel >"$work/panel-$edge.log" 2>&1 &
panel=$!
sleep 4
# Connected after the panel is up, so that the mock announces it as a change.
bluez --object-path $headphones --method org.bluez.Device1.Connect
xdotool mousemove 400 300
sleep 1.5
shot 1-closed; alive "closed"

# Where the applet is, on the panel's edge, and where the popup is: the popup's top moves
# up on a bottom panel when a row opens, so it's asked for before each click.
if [ "$edge" = top ]; then applet_y=14; else applet_y=$((520 - 14)); fi
popup_geometry() {
  local w
  px= py= pw= ph=
  for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 318 ] && [ "$WIDTH" -le 330 ]; then px=$X; py=$Y; pw=$WIDTH; ph=$HEIGHT; fi
  done
}
shown() { popup_geometry; [ -n "$px" ]; }
state() { shown && echo "open (${pw}x${ph} at $px,$py)" || echo "closed"; }
# click_popup <x> <y>: a point in the popup, <y> from the popup's top.
click_popup() { popup_geometry; xdotool mousemove $((px + $1)) $((py + $2)) click 1; sleep 0.8; }
# click_footer <x>: the footer's row, 25 px above the popup's bottom.
click_footer() { popup_geometry; xdotool mousemove $((px + $1)) $((py + ph - 25)) click 1; sleep 0.8; }
open_popup() { xdotool mousemove 14 $applet_y click 1; sleep 1; }
connected() { get "$1" Connected | tr -d '(),<>' ; }
# The pixels that differ from the panel with no popup: 0 when it's gone.
differs_from_closed() {
  xdotool mousemove 400 300
  sleep 1
  import -window root "$work/now.png"
  compare -metric AE "$out/$edge-1-closed.png" "$work/now.png" null: 2>&1 || true
}

open_popup
shot 2-connected
echo "left-click on the applet: popup $(state)"

# Rows, from the popup's top: the connected WH-1000XM4 at 93, Available's MX Master 3 at
# 160 and Pixel 7 at 198, as the first screenshots measured.
click_popup 100 160
shot 3-expanded
echo "MX Master 3's row clicked: popup $(state)"

# Its Connect button, which opens below the row.
click_popup 93 222
shot 4-connected
echo "MX Master 3's Connect pressed: Connected is $(connected $mouse)"

# Now Connected holds MX Master 3 (open) and WH-1000XM4 at 197, whose Disconnect opens 62
# below its row.
click_popup 100 197
click_popup 93 259
shot 5-disconnected
echo "WH-1000XM4's Disconnect pressed: Connected is $(connected $headphones)"

# A row closed again: the window gives the height back.
popup_geometry; before=$ph
click_popup 100 93
sleep 1
popup_geometry
echo "MX Master 3's row closed: the popup went from ${before} to ${ph} high"

# Off with the popup's own switch, which powers the mock's adapter off where /dev/rfkill
# can't be written. Where it can, as on a Session, the switch would block the computer's own
# radio, so the adapter is turned off from outside, as the airplane-mode key would.
set_adapter() {
  if [ -w /dev/rfkill ]; then
    bluez --object-path $hci0 --method org.freedesktop.DBus.Mock.UpdateProperties \
      org.bluez.Adapter1 "{'Powered': <$1>}"
  else
    click_popup 287 20
  fi
  sleep 1.5
}
set_adapter false
shot 6-off
echo "the switch pressed: adapter's Powered is $(gdbus call --system --dest org.bluez --object-path $hci0 --method org.freedesktop.DBus.Properties.Get org.bluez.Adapter1 Powered | tr -d '(),<>'); popup $(state)"
set_adapter true
echo "the switch pressed again: adapter's Powered is $(gdbus call --system --dest org.bluez --object-path $hci0 --method org.freedesktop.DBus.Properties.Get org.bluez.Adapter1 Powered | tr -d '(),<>'); popup $(state)"
sleep 1

click_footer 90
shot 7-after-add
echo "Add New Device… pressed: popup $(state); spawned: $(tr '\n' ';' <"$work/spawned")"

: >"$work/spawned"
open_popup
click_footer 293
echo "the gear pressed: popup $(state); spawned: $(tr '\n' ';' <"$work/spawned")"

open_popup
echo "reopened: popup $(state)"
xdotool mousemove 400 300 key Escape
sleep 1
echo "Escape: popup $(state), $(differs_from_closed) pixels differ from the closed panel"

open_popup
xdotool mousemove 400 300 click 1
sleep 1
echo "a click on the desktop: popup $(state), $(differs_from_closed) pixels differ"

open_popup
xdotool mousemove 14 $applet_y click 1
sleep 1
echo "a second click on the applet: popup $(state), $(differs_from_closed) pixels differ"
alive "the end"

kill "$panel" "$mock" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
