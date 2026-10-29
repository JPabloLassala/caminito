#!/usr/bin/env bash
# The session that center.sh starts on a bus of its own: a panel with the Notifications applet
# alone on it, the daemon's mock filled with the history, and the center opened.
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
    "$out/center-$1.png"
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
set_ungrouped() {
  gdbus call --session --dest org.mate.Caminito.Notifications \
    --object-path /org/mate/Caminito/Notifications \
    --method org.caminito.NotificationsMock.SetUngrouped "$1" >/dev/null
}
# add <id> <app> <app name> <app icon> <summary> <body> <image> <actions> <has default> <age>
add() {
  gdbus call --session --dest org.mate.Caminito.Notifications \
    --object-path /org/mate/Caminito/Notifications \
    --method org.caminito.NotificationsMock.AddEntry \
    "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" false "${10}" >/dev/null
}

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
start_daemon

# Oldest first, as they arrive.
add 1 org.gnome.Software Software system-software-install "Updates are ready" \
  "12 updates, 340 MB." "" "[]" true $((26 * 60 * 60))
add 2 org.gnome.Evolution Mail mail-unread "Invoice 2026-114" "Due on Friday." \
  "" "[]" true $((3 * 60 * 60))
add 3 org.gnome.Evolution Mail mail-unread "Build passed" "main, 4 min." "" "[]" true \
  $((2 * 60 * 60))
add 4 chat Chat user-available "Bruno" "I'll be ten minutes late." "" "[]" true $((90 * 60))
add 5 org.gnome.Evolution Mail mail-unread "Your order shipped" "Arrives on Monday." "" \
  "[]" true $((60 * 60))
add 6 org.gnome.Evolution Mail mail-unread "Standup notes" "Three items." "" "[]" true \
  $((30 * 60))
add 7 chat Chat user-available "Carla" "Count me in!" "" "[]" true $((10 * 60))
add 8 org.gnome.Evolution Mail mail-unread "Ana López" "Lunch at <b>one</b>?" "" \
  "[('reply', 'Reply')]" true $((5 * 60))

# Off the bell, whose tooltip would cover the center.
xt click 12 12 1; xt move 200 600; shot 01-grouped 420 640; alive "the center, grouped"
set_ungrouped "['chat']"
shot 02-ungrouped 420 640; alive "the center, with Chat ungrouped"
xt burst 200 300 5 15; shot 03-ungrouped-scrolled 420 640; alive "the center, scrolled"
stop_daemon

echo "the panel's warnings:"
grep -iE "warning|critical" "$work/panel.log" | head -10
kill "$panel" 2>/dev/null
