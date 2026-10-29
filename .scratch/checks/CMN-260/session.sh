#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own: a panel with the Notifications applet
# alone on it, started before the daemon's mock, then the history filled through the mock.
set -u
root=$1; work=$2; out=$3
template=$root/helpers/caminito-notifications-applet/notifications-mock.py

alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
xt() { python3 "$root/.scratch/checks/CMN-213/xtest.py" "$@"; }
# shot <name> [<width> <height>]: the panel's corner, and the popup under it
shot() {
  sleep 1
  import -window root -crop "${2:-200}x${3:-40}+0+0" +repage -filter point -resize 200% \
    "$out/$1.png"
}
start_daemon() {
  "$CAMINITO_PYTHON" -m dbusmock --template "$template" >"$work/mock.log" 2>&1 &
  mock=$!
  for _ in $(seq 100); do
    gdbus introspect --session --dest org.mate.Caminito.Notifications \
      --object-path /org/mate/Caminito/Notifications >/dev/null 2>&1 && break
    sleep 0.1
  done
}
stop_daemon() { kill "$mock"; wait "$mock" 2>/dev/null; }
# add <id> <app> <app name> <app icon> <summary> <body> <image> <actions> <has default> <age>
add() {
  gdbus call --session --dest org.mate.Caminito.Notifications \
    --object-path /org/mate/Caminito/Notifications \
    --method org.caminito.NotificationsMock.AddEntry \
    "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" false "${10}" >/dev/null
}

magick -size 96x96 gradient:skyblue-navy "$work/photo.png" 2>/dev/null ||
  convert -size 96x96 gradient:skyblue-navy "$work/photo.png"

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 24
dconf write /org/mate/panel/general/object-id-list "['notifications']"
dconf write /org/mate/panel/objects/notifications/object-type "'applet'"
dconf write /org/mate/panel/objects/notifications/applet-iid \
  "'CaminitoNotificationsAppletFactory::CaminitoNotificationsApplet'"
dconf write /org/mate/panel/objects/notifications/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/notifications/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4
shot 01-daemon-absent; alive "no daemon"

start_daemon
shot 02-empty; alive "an empty history"
xt click 12 12 1; shot 03-center-empty 420 200; alive "the center, empty"
xt click 12 12 1; sleep 0.5

add 1 org.gnome.Evolution Mail mail-unread "Invoice 2026-114" "Due on Friday, 31 October." \
  "" "[]" false $((3 * 60 * 60))
add 2 org.gnome.Software Software system-software-install "Updates are ready" \
  "12 updates, 340 MB. Restart to install them." "" \
  "[('restart', 'Restart'), ('later', 'Later')]" true $((26 * 60 * 60))
# The newest last, as it arrives.
add 3 org.gnome.Evolution Mail mail-unread "Ana López" \
  "Lunch at <b>one</b>? There's a new place by the station." "$work/photo.png" \
  "[('reply', 'Reply'), ('archive', 'Archive')]" true $((5 * 60))
shot 04-entries; alive "three entries"
xt click 12 12 1; shot 05-center-entries 420 480; alive "the center, with entries"
xt click 12 12 1; shot 06-center-closed 420 420; alive "the center, closed again"

stop_daemon
shot 07-daemon-gone; alive "the daemon gone"
start_daemon
add 4 org.gnome.Evolution Mail mail-unread "Back again" "" "" "[]" true 0
shot 08-daemon-back; alive "the daemon back"
stop_daemon

echo "the panel's warnings:"
grep -iE "warning|critical" "$work/panel.log" | head -10
kill "$panel" 2>/dev/null
