#!/usr/bin/env bash
# CMN-107's check of Add New Device: `caminito-bluetooth --scan` on X11, against python-dbusmock's
# BlueZ on a private bus in the system bus's place, with one paired device and four in range: a
# phone, a keyboard, a mouse whose pairing fails, and a device that gives no name. It runs the
# program twice, in GTK's light theme (Default) and its dark one (Default:dark), which is what
# a Menta and a BlackMATE session give a GTK4 program (180), and takes:
#
#   <theme>-1-scan    the page --scan opens, with the three named devices and not the fourth
#   <theme>-2-failed  the mouse clicked: its row says why it didn't pair
#   <theme>-3-paired  the phone clicked: it's in the list on the left, Connected, and off the page
#
# and writes report.txt: hci0's Discovering before the program, with the page shown, after
# This computer is selected, with the page shown again, and after Ctrl+W closes the window;
# and whether the phone ended up Paired and Trusted. Connected shows in the screenshot's list.
#
#   window.sh <caminito-bluetooth binary> [<output directory>]
#
# It needs python-dbusmock, Xvfb, xdotool and ImageMagick.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
binary=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
work=$(mktemp -d -t caminito-cmn-107.XXXXXX)
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

mock() { gdbus call --session --dest org.bluez "$@" >/dev/null; }
property() {
  gdbus call --session --dest org.bluez --object-path "$1" \
    --method org.freedesktop.DBus.Properties.Get "$2" "$3"
}
discovering() { property /org/bluez/hci0 org.bluez.Adapter1 Discovering; }

# The mock BlueZ, started again for each run so that each begins with nothing paired.
mock_pid=
start_mock() {
  [ -z "$mock_pid" ] || { kill "$mock_pid" 2>/dev/null || true; wait "$mock_pid" 2>/dev/null || true; }
  python3 -m dbusmock --template bluez5 >/dev/null 2>&1 &
  mock_pid=$!
  pids+=($mock_pid)
  for _ in $(seq 50); do
    gdbus introspect --session --dest org.bluez --object-path / >/dev/null 2>&1 && break
    sleep 0.1
  done
  mock --object-path / --method org.bluez.Mock.AddAdapter hci0 laptop
  # The template's StartDiscovery reads a filter that only SetDiscoveryFilter stores.
  mock --object-path /org/bluez/hci0 --method org.bluez.Adapter1.SetDiscoveryFilter "{}"
  # device <address> <name> <icon>: a device in range, not paired.
  device() {
    mock --object-path / --method org.bluez.Mock.AddDevice hci0 "$1" "$2"
    mock --object-path "/org/bluez/hci0/dev_${1//:/_}" \
      --method org.freedesktop.DBus.Mock.UpdateProperties org.bluez.Device1 "{'Icon': <'$3'>}"
  }
  device 38:18:4C:12:9A:01 WH-1000XM4 audio-headphones
  mock --object-path / --method org.bluez.Mock.PairDevice hci0 38:18:4C:12:9A:01
  device 9C:5C:F9:41:22:7B 'Pixel 7' phone
  device F2:7A:11:0C:3E:42 'Keychron K2' input-keyboard
  device D4:1B:70:88:05:13 'Travel Mouse' input-mouse
  # Its pairing fails, as when a wrong PIN is typed.
  mock --object-path /org/bluez/hci0/dev_D4_1B_70_88_05_13 \
    --method org.freedesktop.DBus.Mock.AddMethod org.bluez.Device1 Pair '' '' \
    "raise dbus.exceptions.DBusException('Authentication Failed', name='org.bluez.Error.AuthenticationFailed')"
  # A device in range that doesn't give a name, which BlueZ announces with its address for an
  # Alias and no Name.
  mock --object-path / --method org.freedesktop.DBus.Mock.EmitSignal \
    org.freedesktop.DBus.ObjectManager InterfacesAdded 'oa{sa{sv}}' \
    "[<objectpath '/org/bluez/hci0/dev_66_55_44_33_22_11'>, <{'org.bluez.Device1': {'Address': <'66:55:44:33:22:11'>, 'Adapter': <objectpath '/org/bluez/hci0'>, 'Alias': <'66-55-44-33-22-11'>, 'Paired': <false>, 'Trusted': <false>, 'Connected': <false>}}>]"
}

shot() {
  sleep 1
  import -window root "$out/$1.png"
}
# click <x> <y>: inside the window, which has no window manager to place it off 0,0.
click() { xdotool mousemove "$1" "$2" click 1; }
# wait_for <what> <command>...: until the command succeeds, for as long as ten seconds.
wait_for() {
  local what=$1
  shift
  for _ in $(seq 100); do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; done
  echo "gave up waiting for $what" >>"$out/report.txt"
}
is_discovering() { discovering | grep -q true; }

for theme in light dark; do
  gtk_theme=Default
  [ "$theme" = dark ] && gtk_theme=Default:dark
  start_mock
  echo "$theme: before the program, Discovering is $(discovering)" >>"$out/report.txt"
  # No instance is running, so --scan starts one.
  GTK_THEME=$gtk_theme GTK_A11Y=none GDK_BACKEND=x11 "$binary" --scan &
  program=$!
  pids+=($program)
  wait_for "the scan" is_discovering
  echo "$theme: with Add New Device shown, Discovering is $(discovering)" >>"$out/report.txt"
  shot "$theme-1-scan"

  # The rows, by name: Keychron K2, Pixel 7, Travel Mouse.
  click 420 237
  shot "$theme-2-failed"
  click 420 182
  sleep 1.5
  shot "$theme-3-paired"
  phone=/org/bluez/hci0/dev_9C_5C_F9_41_22_7B
  # The mock's Connect sends the change but doesn't store it, so Connected is in the list
  # of the screenshot, not here.
  echo "$theme: the phone is Paired $(property $phone org.bluez.Device1 Paired)," \
    "Trusted $(property $phone org.bluez.Device1 Trusted)" >>"$out/report.txt"

  # This computer is the first row, and the + button is under the list.
  click 100 100
  sleep 1
  echo "$theme: with This computer shown, Discovering is $(discovering)" >>"$out/report.txt"
  click 20 458
  wait_for "the scan again" is_discovering
  echo "$theme: with Add New Device shown again, Discovering is $(discovering)" >>"$out/report.txt"

  # With no window manager, the keyboard goes to the window under the pointer.
  xdotool mousemove 400 100 key ctrl+w
  for _ in $(seq 50); do
    xdotool search --onlyvisible --name '^Bluetooth$' >/dev/null 2>&1 || break
    sleep 0.1
  done
  sleep 0.5
  echo "$theme: after Ctrl+W, Discovering is $(discovering)" >>"$out/report.txt"
  kill -0 "$program" && echo "$theme: caminito-bluetooth still running" >>"$out/report.txt"
  kill "$program"
  wait "$program" 2>/dev/null || true
done

cat "$out/report.txt"
echo "screenshots in $out"
