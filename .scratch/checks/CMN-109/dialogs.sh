#!/usr/bin/env bash
# CMN-109's check of the pairing agent's dialogs: caminito-bluetooth --background on X11,
# against python-dbusmock's BlueZ on a private bus in the system bus's place. The script
# calls the agent as BlueZ would, screenshots each dialog, then answers or cancels it:
#
#   1-pin-code       RequestPinCode, then "1234" typed and Enter
#   2-passkey        RequestPasskey, then cancelled by BlueZ
#   3-confirmation   RequestConfirmation of 012345, then Pair clicked
#   4-authorization  RequestAuthorization, then cancelled
#   5-service        AuthorizeService for an untrusted device, then cancelled
#   6-pin-shown      DisplayPinCode 0000
#   7-passkey-shown  DisplayPasskey 123456 with two digits typed
#   8-trusted        the device paired: the code closes and the device is trusted
#
# and writes report.txt: each call's answer, and the device's Trusted at the end.
#
#   dialogs.sh <caminito-bluetooth binary> [<output directory>]
#
# It needs python-dbusmock, Xvfb, xdotool and ImageMagick.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
binary=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
work=$(mktemp -d -t caminito-cmn-109.XXXXXX)
pids=()
cleanup() {
  kill "${pids[@]}" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -p "$out"
rm -f "$out"/*.png "$out/report.txt"
unset DISPLAY

for n in $(seq 140 149); do [ -e /tmp/.X11-unix/X$n ] || break; done
Xvfb ":$n" -screen 0 800x500x24 >/dev/null 2>&1 &
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
mock --object-path / --method org.bluez.Mock.AddDevice hci0 11:22:33:44:55:66 'Keyboard K380'
device=/org/bluez/hci0/dev_11_22_33_44_55_66

GTK_A11Y=none GDK_BACKEND=x11 "$binary" --background &
program=$!
pids+=($program)
for _ in $(seq 100); do
  gdbus call --session --dest org.bluez --object-path /org/bluez \
    --method org.freedesktop.DBus.Mock.GetMethodCalls RequestDefaultAgent 2>/dev/null |
    grep -q Agent && break
  sleep 0.1
done

# The agent's connection: the program's that has the agent's object.
agent=
for name in $(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
  --method org.freedesktop.DBus.ListNames | grep -o "':[0-9.]*'" | tr -d "'"); do
  if gdbus introspect --session --dest "$name" --object-path /org/mate/Caminito/Bluetooth/Agent \
    2>/dev/null | grep -q org.bluez.Agent1; then
    agent=$name
  fi
done
[ -n "$agent" ] || { echo "the agent isn't on the bus" >&2; exit 1; }

# Calls the agent in the background, with its answer to $work/<step>.
call() {
  local step=$1 method=$2
  shift 2
  gdbus call --session --dest "$agent" --object-path /org/mate/Caminito/Bluetooth/Agent \
    --method "org.bluez.Agent1.$method" "$@" >"$work/$step" 2>&1 &
  last=$!
}
shot() {
  sleep 1
  import -window root "$out/$1.png"
}
report() {
  wait "$last" || true
  echo "$1: $(cat "$work/$1")" >>"$out/report.txt"
}
dialog() { xdotool search --onlyvisible --name 'Bluetooth Pairing' | tail -1; }
cancel() {
  gdbus call --session --dest "$agent" --object-path /org/mate/Caminito/Bluetooth/Agent \
    --method org.bluez.Agent1.Cancel >/dev/null
}

call 1-pin-code RequestPinCode "$device"
shot 1-pin-code
# With no window manager, the keyboard goes to the window under the pointer.
eval "$(xdotool getwindowgeometry --shell "$(dialog)")"
xdotool mousemove $((X + 20)) $((Y + 20)) type 1234
xdotool key Return
report 1-pin-code

call 2-passkey RequestPasskey "$device"
shot 2-passkey
cancel
report 2-passkey

call 3-confirmation RequestConfirmation "$device" 12345
shot 3-confirmation
# Pair is the last button, at the dialog's bottom right.
eval "$(xdotool getwindowgeometry --shell "$(dialog)")"
xdotool mousemove $((X + WIDTH - 40)) $((Y + HEIGHT - 30)) click 1
report 3-confirmation

call 4-authorization RequestAuthorization "$device"
shot 4-authorization
cancel
report 4-authorization

call 5-service AuthorizeService "$device" 0000110b-0000-1000-8000-00805f9b34fb
shot 5-service
cancel
report 5-service

call 6-pin-shown DisplayPinCode "$device" 0000
shot 6-pin-shown
report 6-pin-shown

call 7-passkey-shown DisplayPasskey "$device" 123456 2
shot 7-passkey-shown
report 7-passkey-shown

mock --object-path / --method org.bluez.Mock.PairDevice hci0 11:22:33:44:55:66
shot 8-trusted
echo "dialogs open after pairing: $(xdotool search --onlyvisible --name 'Bluetooth Pairing' | wc -l)" \
  >>"$out/report.txt"
echo "Trusted: $(gdbus call --session --dest org.bluez --object-path "$device" \
  --method org.freedesktop.DBus.Properties.Get org.bluez.Device1 Trusted)" >>"$out/report.txt"
kill -0 "$program" && echo "caminito-bluetooth still running" >>"$out/report.txt"

cat "$out/report.txt"
echo "screenshots in $out"
