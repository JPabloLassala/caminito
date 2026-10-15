#!/usr/bin/env bash
# CMN-109's check on real hardware, for the developer: caminito-bluetooth's pairing agent
# pairs a keyboard that the computer finds, and a phone that starts the pairing itself. Run
# it in a Session or on X11 MATE, on a computer with a Bluetooth adapter, with the keyboard
# and the phone at hand and not yet paired with it.
#
#   real-devices.sh [<caminito-bluetooth binary>]
#
# With a binary, it starts that one with --background unless an instance runs already.
# It needs bluez-utils (bluetoothctl) and busctl.
#
# The keyboard: bluetoothctl scans, and busctl asks BlueZ to pair, so that no agent but
# caminito-bluetooth's answers (busctl has none, and bluetoothctl's goes when it exits).
# The phone: the computer is made discoverable, and the phone pairs from its own settings.
# Each ends with the device paired and trusted, and the dialogs' look is the developer's to
# judge.
set -euo pipefail
binary=${1:-}
adapter=/org/bluez/hci0
fail=0

ask() {
  local answer
  read -r -p "$1 [y/n] " answer
  [ "$answer" = y ]
}

property() {
  busctl --system get-property org.bluez "$1" org.bluez.Device1 "$2" | awk '{print $2}'
}

device_path() {
  local address=${1^^}
  echo "$adapter/dev_${address//:/_}"
}

# Checks that the device at $2 ended paired and trusted.
verdict() {
  local name=$1 path=$2
  local paired trusted
  paired=$(property "$path" Paired)
  trusted=$(property "$path" Trusted)
  echo "$name: Paired=$paired Trusted=$trusted"
  if [ "$paired" = true ] && [ "$trusted" = true ]; then
    echo "PASS: $name"
  else
    echo "FAIL: $name"
    fail=1
  fi
}

if ! busctl --user status org.mate.Caminito.Bluetooth >/dev/null 2>&1; then
  [ -n "$binary" ] || { echo "caminito-bluetooth isn't running: give its binary" >&2; exit 1; }
  "$binary" --background &
  sleep 2
fi
busctl --system get-property org.bluez "$adapter" org.bluez.Adapter1 Powered | grep -q true ||
  { echo "$adapter isn't powered: turn Bluetooth on first" >&2; exit 1; }

echo "== The keyboard"
read -r -p "Put the keyboard in pairing mode, then press Enter. "
bluetoothctl --timeout 20 scan on >/dev/null || true
bluetoothctl devices
read -r -p "The keyboard's address, from the list: " keyboard
keyboard_path=$(device_path "$keyboard")
echo "Pairing. Type the code the Bluetooth Pairing dialog shows on the keyboard, then Enter on it."
if ! busctl --system --timeout=90 call org.bluez "$keyboard_path" org.bluez.Device1 Pair; then
  echo "BlueZ's Pair failed"
fi
ask "Did a Bluetooth Pairing dialog show the code, and close once the keyboard had paired?" ||
  { echo "FAIL: the keyboard's dialog"; fail=1; }
verdict keyboard "$keyboard_path"
ask "Does the keyboard type in a terminal?" || { echo "FAIL: the keyboard types"; fail=1; }

echo "== The phone"
busctl --system set-property org.bluez "$adapter" org.bluez.Adapter1 Discoverable b true
echo "This computer is discoverable as \"$(busctl --system get-property org.bluez "$adapter" \
  org.bluez.Adapter1 Alias | cut -d'"' -f2)\"."
echo "On the phone, pair with it from the Bluetooth settings, and answer the dialogs on both."
read -r -p "The phone's address, from its settings (About), then Enter once it's paired: " phone
busctl --system set-property org.bluez "$adapter" org.bluez.Adapter1 Discoverable b false
ask "Did a Bluetooth Pairing dialog ask to confirm the number the phone showed?" ||
  { echo "FAIL: the phone's dialog"; fail=1; }
verdict phone "$(device_path "$phone")"

echo
if [ "$fail" = 0 ]; then echo "PASS"; else echo "FAIL"; fi
echo "To pair them again, forget them first: bluetoothctl remove <address>."
exit "$fail"
