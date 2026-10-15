#!/usr/bin/env bash
# The session run.sh starts, on a session bus of its own: a private bus for UPower's mock, a
# panel with the applet alone on it, then each step.
set -u
root=$1; work=$2; out=$3
upower=/org/freedesktop/UPower

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS
start_mock() {
  "$CAMINITO_PYTHON" -m dbusmock --system --template upower >"$work/mock.log" 2>&1 &
  mock=$!
  for _ in $(seq 100); do
    gdbus introspect --system --dest org.freedesktop.UPower --object-path $upower \
      >/dev/null 2>&1 && break
    sleep 0.1
  done
}
# display <state> <percentage> <seconds to empty> <seconds to full> <present>
display() {
  gdbus call --system --dest org.freedesktop.UPower --object-path $upower \
    --method org.freedesktop.DBus.Mock.SetupDisplayDevice \
    2 "$1" "$2" "$2" 100.0 10.0 "$3" "$4" "$5" '' 1 >/dev/null
}
# The pointer over the applet so that its tooltip shows, which takes a moment.
shot() { xdotool mousemove 14 14; sleep 0.3; xdotool mousemove 16 15; sleep "${2:-2}"; import -window root "$out/$1.png"; }
alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }
away() { xdotool mousemove 300 200; sleep 0.8; }

start_mock

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 28
dconf write /org/mate/panel/general/object-id-list "['power']"
dconf write /org/mate/panel/objects/power/object-type "'applet'"
dconf write /org/mate/panel/objects/power/applet-iid \
  "'CaminitoPowerAppletFactory::CaminitoPowerApplet'"
dconf write /org/mate/panel/objects/power/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/power/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4
shot 1-no-battery; alive "no battery"

display 2 78.0 13200 0 true; sleep 1.5
shot 2-battery
display 1 42.0 0 3000 true; sleep 1.5
shot 3-charging
display 4 100.0 0 0 true; sleep 1.5
shot 4-full
display 2 8.0 1500 0 true; sleep 1.5
shot 5-low

display 1 42.0 0 3000 true; sleep 1
dconf write /org/mate/panel/objects/power/prefs/show-percentage true
sleep 1.5
shot 6-percentage 1
dconf write /org/mate/panel/objects/power/prefs/show-percentage false
sleep 1.5
shot 7-percentage-off 1
echo "'Show percentage' on and off: $(compare -metric AE "$out/6-percentage.png" "$out/7-percentage-off.png" null: 2>&1 | cut -d" " -f1 || true) pixels differ"

away
xdotool mousemove 14 14 click 1; sleep 1.2
import -window root "$out/8-popup.png"
# The popup's "Configure Power Management…": found by the window that appeared, as the
# footer is the bottom row of it.
popup=
for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
  eval "$(xdotool getwindowgeometry --shell "$w")"
  if [ "$WIDTH" -ge 300 ] && [ "$WIDTH" -le 340 ] && [ "$Y" -ge 20 ]; then
    popup="$X $Y $WIDTH $HEIGHT"
  fi
done
echo "popup (x y w h): ${popup:-none}"
if [ -n "$popup" ]; then
  set -- $popup
  : >"$work/spawned"
  xdotool mousemove $(($1 + 90)) $(($2 + $4 - 20)) click 1; sleep 1
  echo "the popup's Configure Power Management… pressed: spawned: $(tr '\n' ';' <"$work/spawned")"
fi
away
xdotool mousemove 14 14 click 1; sleep 0.8
xdotool mousemove 300 200 click 1; sleep 0.8
alive "popup"

# The menu's rows, from the panel's edge: Preferences at 45, Configure at 73.
xdotool mousemove 14 14 click 3; sleep 1.2
import -window root "$out/9-menu.png"
: >"$work/spawned"
xdotool mousemove 60 73 click 1; sleep 1
echo "the menu's Configure Power Management… pressed: spawned: $(tr '\n' ';' <"$work/spawned")"

xdotool mousemove 14 14 click 3; sleep 1.2
xdotool mousemove 60 45 click 1; sleep 1.5
import -window root "$out/10-preferences.png"
dialog=$(xdotool search --onlyvisible --name 'Power Preferences' | head -1)
echo "Preferences: dialog $([ -n "$dialog" ] && echo open || echo 'NOT OPEN')"
if [ -n "$dialog" ]; then
  eval "$(xdotool getwindowgeometry --shell "$dialog")"
  # The check button is the dialog's first row.
  xdotool mousemove $((X + 40)) $((Y + 28)) click 1; sleep 1.5
  echo "the check button pressed: show-percentage is $(dconf read /org/mate/panel/objects/power/prefs/show-percentage)"
  xdotool mousemove 300 200; sleep 1
  import -window root "$out/10b-preferences-on.png"
  xdotool mousemove $((X + 40)) $((Y + 28)) click 1; sleep 1.5
  echo "pressed again: show-percentage is $(dconf read /org/mate/panel/objects/power/prefs/show-percentage)"
  # Its Close button, at the bottom right.
  xdotool mousemove $((X + WIDTH - 50)) $((Y + HEIGHT - 20)) click 1; sleep 1
  echo "Close pressed: dialog $(xdotool search --onlyvisible --name 'Power Preferences' >/dev/null && echo 'STILL OPEN' || echo closed)"
fi
alive "menu and preferences"

# upowerd stopped and started again, as an upgrade of the package does.
display 2 78.0 13200 0 true; sleep 1.5
kill "$mock"; wait "$mock" 2>/dev/null; sleep 1.5
shot 11-upower-gone
alive "upowerd stopped"
start_mock
display 1 61.0 0 1800 true; sleep 2
shot 12-upower-back
alive "upowerd started again"

kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
kill "$mock" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
