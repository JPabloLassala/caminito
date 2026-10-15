#!/usr/bin/env bash
# CMN-110's check of the Bluetooth settings window: caminito-bluetooth on X11, against
# python-dbusmock's BlueZ on a private bus in the system bus's place, with three paired
# devices and one that isn't. It runs the window twice, in GTK's light theme (Default)
# and its dark one (Default:dark), which is what a Menta and a BlackMATE session give a
# GTK4 program (180), and takes:
#
#   <theme>-1-computer   This computer; with two adapters in light, one in dark
#   <theme>-2-device     MX Master 3 selected: its battery is 12%, low; in light, it's then
#                        renamed "My mouse" in its field, and left for another row unapplied
#   <theme>-3-connected  WH-1000XM4 selected: connected, 80%
#   dark-4-forget        the Forget confirmation, which is then cancelled
#
# and writes report.txt: hci0's Discoverable before the window, while it's open, and after
# Ctrl+W closes it.
#
#   window.sh <caminito-bluetooth binary> [<output directory>]
#
# It needs python-dbusmock, Xvfb, xdotool and ImageMagick.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
binary=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
work=$(mktemp -d -t caminito-cmn-110.XXXXXX)
pids=()
cleanup() {
  kill "${pids[@]}" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -p "$out"
rm -f "$out"/*.png "$out/report.txt"
unset DISPLAY

for n in $(seq 150 159); do [ -e /tmp/.X11-unix/X$n ] || break; done
Xvfb ":$n" -screen 0 760x520x24 >/dev/null 2>&1 &
pids+=($!)
export DISPLAY=:$n
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

dbus-daemon --session --nofork --print-address=1 >"$work/address" &
pids+=($!)
for _ in $(seq 50); do [ -s "$work/address" ] && break; sleep 0.1; done
address=$(head -1 "$work/address")
export DBUS_SESSION_BUS_ADDRESS=$address DBUS_SYSTEM_BUS_ADDRESS=$address

python3 -m dbusmock --template bluez5 >/dev/null 2>&1 &
pids+=($!)
for _ in $(seq 50); do
  gdbus introspect --session --dest org.bluez --object-path / >/dev/null 2>&1 && break
  sleep 0.1
done
mock() { gdbus call --session --dest org.bluez "$@" >/dev/null; }
mock --object-path / --method org.bluez.Mock.AddAdapter hci0 laptop
mock --object-path / --method org.bluez.Mock.AddAdapter hci1 dongle
# device <address> <name> <icon> [<battery>]: a paired device.
device() {
  local path=/org/bluez/hci0/dev_${1//:/_}
  mock --object-path / --method org.bluez.Mock.AddDevice hci0 "$1" "$2"
  mock --object-path / --method org.bluez.Mock.PairDevice hci0 "$1"
  mock --object-path "$path" --method org.freedesktop.DBus.Mock.UpdateProperties \
    org.bluez.Device1 "{'Icon': <'$3'>}"
  if [ -n "${4:-}" ]; then
    mock --object-path "$path" --method org.freedesktop.DBus.Mock.AddProperties \
      org.bluez.Battery1 "{'Percentage': <byte $4>}"
  fi
}
device 38:18:4C:12:9A:01 WH-1000XM4 audio-headphones 80
device F2:7A:11:0C:3E:42 'MX Keys' input-keyboard 45
device D4:1B:70:88:05:13 'MX Master 3' input-mouse 12
mock --object-path / --method org.bluez.Mock.ConnectDevice hci0 38:18:4C:12:9A:01
# Not paired: Add New Device's (258), not the list's.
mock --object-path / --method org.bluez.Mock.AddDevice hci0 9C:5C:F9:41:22:7B 'Pixel 7'

discoverable() {
  gdbus call --session --dest org.bluez --object-path /org/bluez/hci0 \
    --method org.freedesktop.DBus.Properties.Get org.bluez.Adapter1 Discoverable
}
shot() {
  sleep 1
  import -window root "$out/$1.png"
}
# click <x> <y>: inside the window, which has no window manager to place it off 0,0.
click() { xdotool mousemove "$1" "$2" click 1; }

for theme in light dark; do
  gtk_theme=Default
  [ "$theme" = dark ] && gtk_theme=Default:dark
  echo "$theme: before the window, Discoverable is $(discoverable)" >>"$out/report.txt"
  GTK_THEME=$gtk_theme GTK_A11Y=none GDK_BACKEND=x11 "$binary" &
  program=$!
  pids+=($program)
  for _ in $(seq 100); do
    discoverable | grep -q true && break
    sleep 0.1
  done
  shot "$theme-1-computer"
  echo "$theme: with the window open, Discoverable is $(discoverable)" >>"$out/report.txt"

  # The rows: This computer, then MX Keys, MX Master 3 and WH-1000XM4, by name.
  click 120 155
  shot "$theme-2-device"
  if [ "$theme" = light ]; then
    # A name typed and not applied, then another row clicked: the name is still given.
    xdotool mousemove 520 171 click 1 key ctrl+a type 'My mouse'
    click 120 200
    sleep 1
    echo "light: MX Master 3 renamed and left for another row, its Alias is" \
      "$(gdbus call --session --dest org.bluez --object-path /org/bluez/hci0/dev_D4_1B_70_88_05_13 \
        --method org.freedesktop.DBus.Properties.Get org.bluez.Device1 Alias)" >>"$out/report.txt"
  fi
  click 120 200
  shot "$theme-3-connected"

  if [ "$theme" = dark ]; then
    click 120 155
    sleep 0.5
    # Forget… is the second button, under the name field.
    click 431 222
    shot dark-4-forget
    xdotool key Escape
    sleep 0.5
    echo "dark: after Forget was cancelled, MX Master 3 is still paired:" \
      "$(gdbus call --session --dest org.bluez --object-path /org/bluez/hci0/dev_D4_1B_70_88_05_13 \
        --method org.freedesktop.DBus.Properties.Get org.bluez.Device1 Paired)" >>"$out/report.txt"
  fi

  # With no window manager, the keyboard goes to the window under the pointer.
  xdotool mousemove 400 100 key ctrl+w
  for _ in $(seq 50); do
    xdotool search --onlyvisible --name '^Bluetooth$' >/dev/null 2>&1 || break
    sleep 0.1
  done
  sleep 0.5
  echo "$theme: after Ctrl+W, Discoverable is $(discoverable)" >>"$out/report.txt"
  kill -0 "$program" && echo "$theme: caminito-bluetooth still running" >>"$out/report.txt"
  kill "$program"
  wait "$program" 2>/dev/null || true

  # The dark run has one adapter: no chooser.
  [ "$theme" = light ] && mock --object-path / --method org.bluez.Mock.RemoveAdapter hci1
done

cat "$out/report.txt"
echo "screenshots in $out"
