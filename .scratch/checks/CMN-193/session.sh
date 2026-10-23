#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own, which stands for the system bus too: a
# panel with the Network applet alone on it, then each state on a fresh NetworkManager mock.
set -u
here=$1; work=$2; out=$3; python=$4
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS

alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
start_mock() {
  "$python" -m dbusmock --system --template networkmanager >/dev/null 2>&1 &
  mock=$!
  for _ in $(seq 50); do
    gdbus introspect --system -d org.freedesktop.NetworkManager \
      -o /org/freedesktop/NetworkManager >/dev/null 2>&1 && return
    sleep 0.1
  done
  echo "the mock never started"
}
stop_mock() { kill "$mock" 2>/dev/null; wait "$mock" 2>/dev/null; }
# shot <name>: the panel's corner, with the tooltip under it where xdotool can hover
shot() {
  sleep 1.5
  if command -v xdotool >/dev/null; then
    xdotool mousemove 0 0; xdotool mousemove 12 12
    sleep 1.5
  fi
  import -window root -crop 320x90+0+0 +repage -filter point -resize 200% "$out/$1.png"
}

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 24
dconf write /org/mate/panel/general/object-id-list "['network']"
dconf write /org/mate/panel/objects/network/object-type "'applet'"
dconf write /org/mate/panel/objects/network/applet-iid \
  "'CaminitoNetworkAppletFactory::CaminitoNetworkApplet'"
dconf write /org/mate/panel/objects/network/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/network/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4
shot 01-no-networkmanager; alive "no NetworkManager"

n=2
for state in wired wifi-20 wifi-45 wifi-70 wifi-95 connecting limited vpn airplane nothing; do
  start_mock
  "$python" "$here/bed.py" "$state" || echo "$state: the mock refused it"
  shot "$(printf %02d $n)-$state"; alive "$state"
  stop_mock
  n=$((n + 1))
done
# NetworkManager gone again: the applet follows it.
sleep 1
shot "$(printf %02d $n)-stopped-again"; alive "NetworkManager stopped again"

grep -iE 'critical|warning' "$work/panel.log" | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
