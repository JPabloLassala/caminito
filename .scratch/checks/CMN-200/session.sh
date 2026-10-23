#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own, which stands for the system bus too: a
# panel with the Network applet alone on it, then each state on a fresh NetworkManager mock,
# with the popup opened by a click on the applet and its rows clicked where they are with
# the default theme at the panel's size.
set -u
here=$1; work=$2; out=$3; python=$4
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS

# Where the popup's widgets are, from the screen's top left, as Adwaita draws them: the
# search, the list's rows, the bar's button, and "Connect to hidden network…" with the bar
# shown.
SEARCH="200 296"
row() { echo "200 $((338 + $1 * 36))"; }
BAR_BUTTON="300 594"
HIDDEN="130 640"

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
click() { python3 "$here/click.py" $1; sleep 0.5; }
type_text() { python3 "$here/key.py" type "$1"; sleep 0.5; }
key() { python3 "$here/key.py" key "$1"; sleep 0.3; }
# Whether the popup shows: anything but the server's black under the panel.
popup_shows() {
  [ "$(import -window root -crop 300x200+20+40 +repage -format '%[fx:mean>0]' info:)" = 1 ]
}
# The popup, opened by a click on the applet. Under Xwayland the real pointer passing over
# the window can eat a click: then once more.
open_popup() {
  sleep 1.5
  click "12 12"
  sleep 1
  popup_shows || { click "12 12"; sleep 1; }
}
close_popup() { click "12 12"; sleep 0.5; }
shot() { import -window root -crop 420x780+0+0 +repage "$out/$1.png"; }

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

start_mock
"$python" "$here/bed.py" list || echo "list: the mock refused it"
open_popup; shot 01-order
click "$SEARCH"; type_text lib; shot 02-search
for _ in 1 2 3; do key BackSpace; done
# Old, the last row.
click "$(row 6)"; type_text hunter22; shot 03-new-secured
# Library, the third.
click "$(row 2)"; shot 04-saved
# eduroam, the fifth.
click "$(row 4)"; shot 05-enterprise
click "$BAR_BUTTON"
open_popup; click "$HIDDEN"
alive "list"
stop_mock

start_mock
"$python" "$here/bed.py" connecting || echo "connecting: the mock refused it"
open_popup
# Library, coming up, is first now. The rows came as the mock was filled, without it at
# first: it's selected again.
click "$(row 0)"; shot 06-connecting; close_popup; alive "connecting"
stop_mock

start_mock
"$python" "$here/bed.py" wifi-off || echo "wifi-off: the mock refused it"
open_popup; shot 07-wifi-off; close_popup; alive "wifi-off"
stop_mock

if [ -e "$work/editor.log" ]; then sed "s/^/ran: /" "$work/editor.log"
else echo "nm-connection-editor never ran"; fi
grep -iE 'critical|warning' "$work/panel.log" | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
