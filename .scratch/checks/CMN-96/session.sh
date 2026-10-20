#!/usr/bin/env bash
# The desktop's session, which run.sh starts in CMN-99's fake tree on a session bus of its
# own: a panel with the Thermal applet alone on it, then each step.
set -u
computer=$1; work=$2; out=$3
hwmon=$UMOCKDEV_DIR/sys/class/hwmon
prefs=/org/mate/panel/objects/thermal/prefs

# remove <chip>: the chip goes, as when its module is unloaded
remove() {
  for h in "$hwmon"/*; do
    if [ "$(cat "$h/name")" = "$1" ]; then rm -r "$(realpath "$h")" "$h"; fi
  done
}
alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
keys() {
  echo "$1: panel-sensors $(dconf read $prefs/panel-sensors), units $(dconf read $prefs/units)," \
    "show-all $(dconf read $prefs/show-all)"
}
# The popup's window: the one 340 to 420 wide below the panel. Sets X Y WIDTH HEIGHT.
find_popup() {
  found=; for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 340 ] && [ "$WIDTH" -le 420 ] && [ "$Y" -ge 20 ]; then
      found="$X $Y $WIDTH $HEIGHT"
    fi
  done
  set -- ${found:-}
  X=${1:-}; Y=${2:-}; WIDTH=${3:-}; HEIGHT=${4:-}
}
open_popup() { xdotool mousemove 14 14 click 1; sleep 1.2; find_popup; }
close_popup() { xdotool mousemove 470 890 click 1; sleep 0.8; }
toggle_show_all() { xdotool mousemove $((X + 40)) $((Y + HEIGHT - 12 - 9)) click 1; sleep 1.2; find_popup; }
# pin <y>: the pin of the row at y in the popup, as CMN-97's d2-expanded has the rows
pin() { xdotool mousemove $((X + WIDTH - 23)) $((Y + $1)) click 1; sleep 0.6; }
# panel <name>: the panel's start, where the applet is
panel_shot() { import -window root -crop "220x32+0+0" +repage "$out/$1.png"; }
# shot <name>: the popup alone, with the pointer away from its rows
shot() {
  xdotool mousemove 470 20
  sleep 0.5
  if [ -z "$X" ]; then echo "$1: NO POPUP"; return; fi
  import -window root -crop "${WIDTH}x$HEIGHT+$X+$Y" +repage "$out/$1.png"
}
start_panel() {
  mate-panel >>"$work/panel-$computer.log" 2>&1 &
  panel=$!
  sleep 3
}

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 32
dconf write /org/mate/panel/general/object-id-list "['thermal']"
dconf write /org/mate/panel/objects/thermal/object-type "'applet'"
dconf write /org/mate/panel/objects/thermal/applet-iid \
  "'CaminitoThermalAppletFactory::CaminitoThermalApplet'"
dconf write /org/mate/panel/objects/thermal/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/thermal/position 0

start_panel
panel_shot p1-defaults; alive "defaults"
open_popup
toggle_show_all
# The rows of WD_BLACK and SYSTIN.
pin 436
panel_shot p2-pin-drive; shot q2-pin-drive; alive "drive pinned"
pin 531
panel_shot p3-pin-systin; shot q3-pin-systin; alive "SYSTIN pinned"
keys "after the pins"
dconf write $prefs/units "'fahrenheit'"
sleep 0.8
panel_shot p4-fahrenheit; shot q4-fahrenheit; alive "fahrenheit"
dconf write $prefs/warm-threshold 40.0
sleep 0.8
panel_shot p5-warm-40; shot q5-warm-40; alive "warm from 40"
close_popup

kill "$panel"; wait "$panel" 2>/dev/null
start_panel
panel_shot p6-restarted; alive "restarted"
keys "after the restart"
open_popup
shot q6-restarted
close_popup

remove nct6798
sleep 2.5
panel_shot p7-systin-gone; alive "nct6798 removed"
keys "after the chips went"
grep -iE 'critical|warning' "$work/panel-$computer.log" | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
