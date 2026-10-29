#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own: a panel with the Notifications applet
# alone on it, and the daemon's mock with one entry.
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
api() {
  gdbus call --session --dest org.mate.Caminito.Notifications \
    --object-path /org/mate/Caminito/Notifications "$@" >/dev/null
}

"$CAMINITO_PYTHON" -m dbusmock --template "$template" >"$work/mock.log" 2>&1 &
mock=$!
for _ in $(seq 100); do
  gdbus introspect --session --dest org.mate.Caminito.Notifications \
    --object-path /org/mate/Caminito/Notifications >/dev/null 2>&1 && break
  sleep 0.1
done
api --method org.caminito.NotificationsMock.AddEntry 1 org.gnome.Evolution Mail mail-unread \
  "Invoice 2026-114" "Due on Friday, 31 October." "" "[]" false false 600

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
shot 01-off; alive "off"
xt click 12 12 1; shot 02-center-off 420 240; alive "the center, off"
# "1 hour", the first of the durations, on the row under the switch, and the center closed.
xt click 46 80 1; sleep 0.5
xt click 12 12 1; sleep 0.5
shot 03-on-for-an-hour; alive "on for an hour"
xt click 12 12 1; shot 04-center-on 420 240; alive "the center, on"
xt click 12 12 1; sleep 0.5
xt click 12 12 2; shot 05-middle-off; alive "middle-click, off"
xt click 12 12 2; shot 06-middle-on; alive "middle-click, on"
xt click 12 12 1; shot 07-center-until 420 240; alive "the center, until turned off"
xt click 12 12 1; sleep 0.5
api --method org.mate.Caminito.Notifications.OpenCenter
shot 08-open-center 420 240; alive "OpenCenter"

echo "the mock's calls:"
gdbus call --session --dest org.mate.Caminito.Notifications \
  --object-path /org/mate/Caminito/Notifications --method org.caminito.NotificationsMock.GetLog
echo "the panel's warnings:"
grep -iE "warning|critical" "$work/panel.log" | head -10
kill "$panel" "$mock" 2>/dev/null
