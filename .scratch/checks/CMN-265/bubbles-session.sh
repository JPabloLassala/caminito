#!/usr/bin/env bash
# The session that bubbles.sh starts on a bus of its own: the daemon, then Signal's
# notifications, sent as Signal sends them.
set -u
root=$1; work=$2; out=$3; binary=$4

alive() {
  if kill -0 "$daemon" 2>/dev/null; then echo "$1: the daemon lives"
  else echo "$1: THE DAEMON IS GONE"; fi
}
xt() { python3 "$root/.scratch/checks/CMN-213/xtest.py" "$@"; }
# shot <name>: the screen's top left-hand corner, where X11 with no window manager puts the
# bubbles' window
shot() {
  sleep 1
  import -window root -crop 420x900+0+0 +repage "$out/bubble-$1.png"
}
# notify <app> <summary> <body> <actions>: with no timeout
notify() {
  gdbus call --session --dest org.freedesktop.Notifications \
    --object-path /org/freedesktop/Notifications \
    --method org.freedesktop.Notifications.Notify \
    "$1" 0 "" "$2" "$3" "$4" "{}" 0 >/dev/null
}

"$binary" >"$work/daemon.log" 2>&1 &
daemon=$!
for _ in $(seq 100); do
  gdbus introspect --session --dest org.freedesktop.Notifications \
    --object-path /org/freedesktop/Notifications >/dev/null 2>&1 && break
  sleep 0.1
done

notify Signal "Ana López" "Are we still on for lunch?" "['reply', 'Reply']"
notify Signal "Bruno" "I'll be ten minutes late." "['reply', 'Reply']"
shot 01-two-bubbles; alive "two notifications"
notify Signal "Ana López" "There's a new place by the station." "['reply', 'Reply']"
shot 02-merged; alive "the third merges them"
notify Signal "Carla" "Count me in!" "['reply', 'Reply']"
notify Signal "Ana López" "Booked for <b>one</b>." "['reply', 'Reply', 'mute', 'Mute']"
shot 03-grouped; alive "two more join"
# "+2 more", where Adwaita draws it under the header.
xt click "${CAMINITO_MORE_X:-47}" "${CAMINITO_MORE_Y:-215}" 1
shot 04-opened; alive "the group opened"
notify Mail "Invoice 2026-114" "Due on Friday, 31 October." "[]"
shot 05-another-app; alive "another app's notification"

echo "the daemon's warnings:"
grep -iE "warning|critical" "$work/daemon.log" | head -10
kill "$daemon" 2>/dev/null
