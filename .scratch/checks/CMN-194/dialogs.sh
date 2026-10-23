#!/usr/bin/env bash
# CMN-194's check of caminito-network's dialogs: the program on X11 under Caminito's name,
# against python-dbusmock's NetworkManager (with the AgentManager its tests add) on a private
# bus in the system bus's place. The script asks it for secrets as NetworkManager would, and
# screenshots each dialog before it cancels the request with CancelGetSecrets:
#
#   1-psk            the Wi-Fi network "Home" (WPA2 Personal) asks for its password
#   2-psk-again      the same, after a wrong one (REQUEST_NEW)
#   3-wep            "Old Router", a WEP network, asks for its key
#   4-8021x-wifi     "eduroam" (PEAP) asks for a username and password, the username filled in
#   5-8021x-wired    the wired connection "Office" (PEAP), with no username yet
#   6-private-key    "Office" with EAP-TLS asks for the private key's passphrase
#
# and writes report.txt with each request's answer once it was cancelled.
#
#   dialogs.sh <caminito-network binary> [<output directory>]
#
# It needs python-dbusmock, Xvfb and ImageMagick. CAMINITO_PYTHON names a Python other than
# python3 that has dbusmock. CAMINITO_X=Xwayland runs a rootful Xwayland, in a window of the
# desktop, where Xvfb isn't installed.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
binary=$(realpath "$1")
out=$(realpath -m "${2:-$here/out}")
python=${CAMINITO_PYTHON:-python3}
work=$(mktemp -d -t caminito-cmn-194.XXXXXX)
pids=()
cleanup() {
  kill "${pids[@]}" 2>/dev/null || true
  [ -f "$work/bus.pid" ] && kill "$(cat "$work/bus.pid")" 2>/dev/null
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -p "$out"
rm -f "$out"/*.png "$out/report.txt"
unset DISPLAY

for n in $(seq 150 159); do [ -e /tmp/.X11-unix/X$n ] || break; done
if [ "${CAMINITO_X:-Xvfb}" = Xwayland ]; then
  Xwayland ":$n" -geometry 560x300 >/dev/null 2>&1 &
else
  Xvfb ":$n" -screen 0 560x300x24 >/dev/null 2>&1 &
fi
pids+=($!)
export DISPLAY=:$n
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

# One private bus, as both the session and the system bus.
bus=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/bus.pid")
export DBUS_SESSION_BUS_ADDRESS=$bus DBUS_SYSTEM_BUS_ADDRESS=$bus
"$python" -m dbusmock --system \
  --template "$root/caminito-network/tests/mock/networkmanager.py" >"$work/nm.log" 2>&1 &
pids+=($!)
for _ in $(seq 100); do
  gdbus introspect --system --dest org.freedesktop.NetworkManager \
    --object-path /org/freedesktop/NetworkManager/AgentManager >/dev/null 2>&1 && break
  sleep 0.1
done

MATE_WM_NAME=Caminito GDK_BACKEND=x11 GTK_A11Y=none GDK_DEBUG=no-portals \
  "$binary" >"$work/caminito-network.log" 2>&1 &
pids+=($!)
# The agent is on the program's connection to the system bus, not the one that owns the
# application's name: the name that has its object.
agent=
for _ in $(seq 100); do
  for name in $(gdbus call --system --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.ListNames | grep -oE "':[0-9.]+'" | tr -d "'"); do
    gdbus introspect --system --dest "$name" \
      --object-path /org/freedesktop/NetworkManager/SecretAgent 2>/dev/null |
      grep -q GetSecrets && agent=$name && break
  done
  [ -n "$agent" ] && break
  sleep 0.1
done
[ -n "$agent" ] || { echo "caminito-network didn't start" >&2; cat "$work/caminito-network.log" >&2; exit 1; }

# Asks for <setting> of <connection>, with <flags>, screenshots the dialog as <name>.png and
# cancels the request.
ask() {
  local name=$1 connection=$2 setting=$3 flags=$4 path=/org/freedesktop/NetworkManager/Settings/1
  gdbus call --system --dest "$agent" --object-path /org/freedesktop/NetworkManager/SecretAgent \
    --timeout 30 --method org.freedesktop.NetworkManager.SecretAgent.GetSecrets \
    "$connection" "$path" "$setting" "[]" "$flags" >"$work/$name.answer" 2>&1 &
  local call=$!
  sleep 1.5
  import -window root -trim "$out/$name.png"
  gdbus call --system --dest "$agent" --object-path /org/freedesktop/NetworkManager/SecretAgent \
    --method org.freedesktop.NetworkManager.SecretAgent.CancelGetSecrets "$path" "$setting" \
    >/dev/null
  wait "$call" || true
  echo "$name: $(tr -d '\n' <"$work/$name.answer")" >>"$out/report.txt"
}

# An SSID as GVariant's bytes. gdbus garbles a b'' literal with a space in it, and that one
# would end in a NUL that NetworkManager's don't have.
bytes() {
  printf '%s' "$1" | od -An -tx1 | xargs printf '0x%s, ' | sed 's/, $//; s/^/[byte /; s/$/]/'
}

wifi() { # <ssid> <key-mgmt> [more of 802-11-wireless-security]
  echo "{'connection': {'id': <'$1'>, 'type': <'802-11-wireless'>}," \
    "'802-11-wireless': {'ssid': <$(bytes "$1")>}," \
    "'802-11-wireless-security': {'key-mgmt': <'$2'>${3-}}}"
}
ieee8021x() { # <eap> <identity>
  echo "'802-1x': {'eap': <['$1']>, 'identity': <'$2'>}"
}
wired() { # <id> <802-1x>
  echo "{'connection': {'id': <'$1'>, 'type': <'802-3-ethernet'>}, $2}"
}

ask 1-psk "$(wifi Home wpa-psk)" 802-11-wireless-security 1
ask 2-psk-again "$(wifi Home wpa-psk)" 802-11-wireless-security 3
ask 3-wep "$(wifi 'Old Router' none ", 'wep-tx-keyidx': <uint32 0>")" 802-11-wireless-security 1
eduroam=$(wifi eduroam wpa-eap)
ask 4-8021x-wifi "${eduroam%\}}, $(ieee8021x peap ana@example.edu)}" 802-1x 1
ask 5-8021x-wired "$(wired Office "$(ieee8021x peap '')")" 802-1x 1
ask 6-private-key "$(wired Office "$(ieee8021x tls ana)")" 802-1x 1
cat "$out/report.txt"
