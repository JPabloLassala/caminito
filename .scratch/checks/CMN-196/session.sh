#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own, which stands for the system bus too: a
# panel with the Network applet alone on it, then each state on a fresh NetworkManager mock,
# with the popup opened by a click on the applet.
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
click() { python3 "$here/click.py" "$1" "$2"; }
# Whether the popup shows: anything but the server's black under the panel.
popup_shows() {
  [ "$(import -window root -crop 300x200+20+40 +repage -format '%[fx:mean>0]' info:)" = 1 ]
}
# shot <name>: the popup, opened by a click on the applet and closed by a click on it again.
# Under Xwayland the real pointer passing over the window can eat a click: then once more.
shot() {
  sleep 1.5
  click 12 12
  sleep 1.5
  popup_shows || { click 12 12; sleep 1.5; }
  import -window root -crop 420x620+0+0 +repage "$out/$1.png"
  click 12 12
  sleep 0.5
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
for state in wifi-on wifi-off cable-in cable-out vpn airplane limited connecting nothing; do
  start_mock
  "$python" "$here/bed.py" "$state" || echo "$state: the mock refused it"
  shot "$(printf %02d $n)-$state"; alive "$state"
  stop_mock
  n=$((n + 1))
done

grep -iE 'critical|warning' "$work/panel.log" | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
