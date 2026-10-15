#!/usr/bin/env bash
# The session run.sh starts, on a session bus of its own: a private bus for UPower's mock, a
# panel with the applet alone on it, then each step.
set -u
root=$1; work=$2; out=$3
upower=/org/freedesktop/UPower
devices=$upower/devices
device=org.freedesktop.UPower.Device
mock=org.freedesktop.DBus.Mock

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
start_mock() {
  "$CAMINITO_PYTHON" -m dbusmock --system --template upower >"$work/mock.log" 2>&1 &
  mockpid=$!
  for _ in $(seq 100); do
    gdbus introspect --system --dest org.freedesktop.UPower --object-path $upower \
      >/dev/null 2>&1 && break
    sleep 0.1
  done
}
call() { gdbus call --system --dest org.freedesktop.UPower --object-path "$1" --method "$2" "${@:3}"; }
# display <state> <percentage> <present>: the panel's own glyph
display() { call $upower $mock.SetupDisplayDevice 2 "$1" "$2" "$2" 100.0 10.0 3600 0 "$3" '' 1 >/dev/null; }
# properties <name> <a{sv}>: properties the template doesn't make, which UPower has. The
# mock tells nobody of a property it adds, but of one it sets: so it's added and set.
properties() {
  call $devices/$1 $mock.AddProperties $device "$2" >/dev/null
  call $upower $mock.SetDeviceProperties "$devices/$1" "$2" >/dev/null
}
# battery <name> <percentage> <limit supported>: a battery of the laptop, with the health
# and, where supported, a charge limit that is UPower's own: it sets the properties it reads
battery() {
  call $upower $mock.AddDischargingBattery "$1" "Model $1" "$2" 7200 >/dev/null
  properties "$1" "{'Capacity': <58.0>}"
  if [ "$3" = yes ]; then
    properties "$1" \
      "{'ChargeThresholdSupported': <true>, 'ChargeThresholdEnabled': <false>, 'ChargeEndThreshold': <uint32 100>}"
    allow "$1"
  fi
}
# What UPower's EnableChargeThreshold does when polkit allows it
allow() {
  call $devices/$1 $mock.AddMethod $device EnableChargeThreshold b '' \
    "self.Set('$device', 'ChargeThresholdEnabled', args[0]); self.Set('$device', 'ChargeEndThreshold', dbus.UInt32(80 if args[0] else 100))" >/dev/null
}
# What it does when polkit refuses
refuse() {
  call $devices/$1 $mock.AddMethod $device EnableChargeThreshold b '' \
    "raise dbus.exceptions.DBusException('Not authorized', name='org.freedesktop.DBus.Error.AccessDenied')" >/dev/null
}
# peripheral <name> <type> <model> <percentage>
peripheral() {
  call $upower $mock.AddDischargingBattery "$1" "$3" "$4" 7200 >/dev/null
  call $upower $mock.SetDeviceProperties "$devices/$1" "{'Type': <uint32 $2>, 'PowerSupply': <false>}" >/dev/null
}
remove() { call $upower $mock.RemoveDevice "$devices/$1" >/dev/null; }
calls_of() { call $devices/$1 $mock.GetMethodCalls EnableChargeThreshold 2>&1 | grep -o "<\(true\|false\)>" | tr -d '<>' | tr '\n' ' '; }
alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }
away() { xdotool mousemove 470 410; sleep 0.8; }
# The pointer is parked at the popup's top right corner, off every control, so that no tooltip
# is over one; the ⋮'s own tooltip would otherwise cover the next row's.
park() { [ -n "$px" ] && xdotool mousemove $((px + pw - 3)) $((py + 3)); }
shot() { park; sleep "${2:-1.5}"; import -window root "$out/$1.png"; }
# The popup's height says how many rows the table has: 44 pixels to a row.
height() { find_popup | sed "s/^popup/$1: popup/"; }

# The popup's window: its geometry in $px $py $pw $ph, from the one window that is as wide as
# the popup and below the panel.
find_popup() {
  px= py= pw= ph=
  for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 300 ] && [ "$WIDTH" -le 340 ] && [ "$Y" -ge 20 ]; then
      px=$X py=$Y pw=$WIDTH ph=$HEIGHT
    fi
  done
  echo "popup (x y w h): ${px:-none} ${py:-} ${pw:-} ${ph:-}"
}
# click_menu <row>: the ⋮ of a laptop battery's row
click_menu() { park; sleep 1.2; xdotool mousemove $((px + pw - 22)) $((py + 8 + 22 + 40 * ($1 - 1))) click 1; sleep 0.8; }

start_mock

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 28
dconf write /org/mate/panel/general/object-id-list "['power']"
dconf write /org/mate/panel/objects/power/object-type "'applet'"
dconf write /org/mate/panel/objects/power/applet-iid \
  "'CaminitoPowerAppletFactory::CaminitoPowerApplet'"
dconf write /org/mate/panel/objects/power/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/power/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4

# One battery, with a charge limit.
battery BAT0 73 yes
display 2 73.0 true
sleep 1.5
xdotool mousemove 14 14 click 1; sleep 1.2
find_popup
shot 1-one-battery; height "one battery"

click_menu 1
shot 2-menu
echo "before any switch, EnableChargeThreshold was called with: [$(calls_of BAT0)]"

# The switch, in the menu that is open: its middle is at the popup's left plus 226 and the
# menu's second line.
click_switch() { xdotool mousemove $((px + 226)) $((py + 65)) click 1; sleep 1.2; }
click_switch
shot 3-limit-on
echo "the switch turned on: EnableChargeThreshold called with: $(calls_of BAT0)"
click_switch
shot 4-limit-off
echo "and off: EnableChargeThreshold called with: $(calls_of BAT0)"

refuse BAT0
click_switch
shot 5-refused
echo "polkit refuses: EnableChargeThreshold called with: $(calls_of BAT0); ChargeThresholdEnabled is $(call $devices/BAT0 org.freedesktop.DBus.Properties.Get $device ChargeThresholdEnabled)"
alive "after the limit"

# A second battery that has no limit; the first's menu is closed by its button.
click_menu 1
battery BAT1 80 no
display 2 76.0 true
shot 6-two-batteries 2; height "two batteries"
click_menu 2
shot 6b-second-menu
click_menu 2

# No battery at all: a mouse and headphones.
remove BAT0; remove BAT1
display 0 0.0 false
peripheral mouse0 5 "MX Master 3S" 55
peripheral heads0 19 "WH-1000XM4" 80
shot 7-peripherals 2; height "peripherals only"

# A battery and a mouse that is nearly flat.
battery BAT0 62 no
display 2 62.0 true
call $upower $mock.SetDeviceProperties "$devices/mouse0" "{'Percentage': <12.0>}" >/dev/null
shot 8-low-peripheral 2; height "a battery and two peripherals"

# The mouse is turned off: UPower removes it.
remove mouse0
shot 9-mouse-off 2; height "the mouse turned off"
echo "the shots with and without the mouse differ in $(compare -metric AE "$out/8-low-peripheral.png" "$out/9-mouse-off.png" null: 2>&1 | cut -d" " -f1 || true) pixels"
alive "the table"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
kill "$mockpid" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
